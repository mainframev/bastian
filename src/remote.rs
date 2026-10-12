//! Documents fetched over HTTP: plain URLs and GitHub/GitLab repository
//! READMEs. Relative links in a remote document resolve against `base`.

use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;
use url::Url;

use crate::doc::{self, Document};

/// Upper bound on a fetched document.
const MAX_BYTES: u64 = 5 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(10);

/// A command-line or link target that names something remote.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    Url(Url),
    GitHub { owner: String, repo: String },
    GitLab { project: String },
}

#[derive(Debug, Clone)]
pub struct RemoteDoc {
    /// Short display name, e.g. `owner/repo README.md`.
    pub name: String,
    /// Where relative links resolve from.
    pub base: Url,
    pub text: String,
}

impl RemoteDoc {
    /// Parse the text, judging Markdown, MDX or a code file by the URL's
    /// path as if it were a local file name.
    pub fn parse(&self) -> Document {
        doc::parse_for(&self.text, Some(Path::new(self.base.path())))
    }
}

/// Recognize `http(s)://…`, `github.com/owner/repo` and
/// `gitlab.com/group/project` (with or without a scheme). Anything else is
/// not remote.
pub fn classify(input: &str) -> Option<Source> {
    let with_scheme = if input.starts_with("http://") || input.starts_with("https://") {
        input.to_string()
    } else if input.starts_with("github.com/") || input.starts_with("gitlab.com/") {
        format!("https://{input}")
    } else {
        return None;
    };
    let url = Url::parse(&with_scheme).ok()?;
    let segments: Vec<&str> = url
        .path_segments()
        .map(|s| s.filter(|p| !p.is_empty()).collect())
        .unwrap_or_default();
    match (url.host_str(), segments.as_slice()) {
        (Some("github.com"), [owner, repo]) => Some(Source::GitHub {
            owner: owner.to_string(),
            repo: repo.trim_end_matches(".git").to_string(),
        }),
        (Some("gitlab.com"), project) if project.len() >= 2 && !project.contains(&"-") => {
            Some(Source::GitLab {
                project: project.join("/").trim_end_matches(".git").to_string(),
            })
        }
        _ => Some(Source::Url(url)),
    }
}

/// Fetch a remote document.
pub fn fetch(source: &Source) -> Result<RemoteDoc> {
    let agent = agent();
    match source {
        Source::Url(url) => {
            // A file page on GitHub/GitLab is HTML; read the raw file behind
            // it. Links still resolve against the page, so they open as pages
            // in a browser and Markdown ones come back here as raw files.
            let raw = raw_url(url).unwrap_or_else(|| url.clone());
            let text = get(agent, raw.as_str(), "text/markdown, text/plain, */*")?;
            Ok(RemoteDoc {
                name: url.to_string(),
                base: url.clone(),
                text,
            })
        }
        Source::GitHub { owner, repo } => {
            let api = format!("https://api.github.com/repos/{owner}/{repo}/readme");
            let meta: Value =
                serde_json::from_str(&get(agent, &api, "application/vnd.github+json")?)
                    .context("reading GitHub's README metadata")?;
            let raw = meta["download_url"]
                .as_str()
                .ok_or_else(|| anyhow!("GitHub returned no README download URL"))?;
            let base = Url::parse(raw).context("GitHub README URL")?;
            let file = meta["name"].as_str().unwrap_or("README");
            Ok(RemoteDoc {
                name: format!("{owner}/{repo} {file}"),
                text: get(agent, raw, "text/plain")?,
                base,
            })
        }
        Source::GitLab { project } => {
            let api = format!(
                "https://gitlab.com/api/v4/projects/{}",
                url::form_urlencoded::byte_serialize(project.as_bytes()).collect::<String>()
            );
            let meta: Value = serde_json::from_str(&get(agent, &api, "application/json")?)
                .context("reading GitLab's project metadata")?;
            let readme = meta["readme_url"]
                .as_str()
                .ok_or_else(|| anyhow!("{project} has no README on GitLab"))?;
            let raw = gitlab_raw(readme);
            let base = Url::parse(&raw).context("GitLab README URL")?;
            let file = base
                .path_segments()
                .and_then(|mut s| s.next_back())
                .unwrap_or("README");
            Ok(RemoteDoc {
                name: format!("{project} {file}"),
                text: get(agent, &raw, "text/plain")?,
                base,
            })
        }
    }
}

/// The raw-file URL behind a file page, for documents and images:
/// `github.com/o/r/blob/<ref>/<path>` → `raw.githubusercontent.com/o/r/<ref>/<path>`,
/// and GitLab's `…/-/blob/…` → `…/-/raw/…` on any host.
pub(crate) fn raw_url(url: &Url) -> Option<Url> {
    let path = url.path();
    let raw = if url.host_str() == Some("github.com") {
        let mut parts = path.trim_start_matches('/').splitn(4, '/');
        let (owner, repo, blob, rest) =
            (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
        if blob != "blob" || rest.is_empty() {
            return None;
        }
        format!("https://raw.githubusercontent.com/{owner}/{repo}/{rest}")
    } else if path.contains("/-/blob/") {
        let mut raw = url.clone();
        raw.set_path(&gitlab_raw(path));
        raw.set_query(None);
        raw.set_fragment(None);
        return Some(raw);
    } else {
        return None;
    };
    Url::parse(&raw).ok()
}

/// `…/-/blob/main/README.md` → `…/-/raw/main/README.md`.
fn gitlab_raw(readme_url: &str) -> String {
    readme_url.replacen("/-/blob/", "/-/raw/", 1)
}

/// The HTTP client, shared so a document's images reuse its connections.
fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::Agent::new_with_config(
            ureq::Agent::config_builder()
                .timeout_global(Some(TIMEOUT))
                .http_status_as_error(false)
                .build(),
        )
    })
}

/// GET `url` as text, with clear errors for HTTP failures and GitHub's rate
/// limit. `GITHUB_TOKEN` is sent to api.github.com when set.
fn get(agent: &ureq::Agent, url: &str, accept: &str) -> Result<String> {
    respond(agent, url, accept)?
        .body_mut()
        .with_config()
        .limit(MAX_BYTES)
        .read_to_string()
        .with_context(|| format!("reading {url}"))
}

/// GET `url` as raw bytes (e.g. an image), up to `limit` bytes.
pub(crate) fn fetch_bytes(url: &str, limit: u64) -> Result<Vec<u8>> {
    respond(agent(), url, "*/*")?
        .body_mut()
        .with_config()
        .limit(limit)
        .read_to_vec()
        .with_context(|| format!("reading {url}"))
}

/// Send the GET and turn HTTP failures into errors.
fn respond(
    agent: &ureq::Agent,
    url: &str,
    accept: &str,
) -> Result<ureq::http::Response<ureq::Body>> {
    let mut req = agent
        .get(url)
        .header("Accept", accept)
        .header("User-Agent", concat!("bastian/", env!("CARGO_PKG_VERSION")));
    if url.starts_with("https://api.github.com/")
        && let Ok(token) = std::env::var("GITHUB_TOKEN")
        && !token.is_empty()
    {
        req = req.header("Authorization", format!("Bearer {token}"));
    }
    let resp = req.call().with_context(|| format!("fetching {url}"))?;
    let status = resp.status().as_u16();
    let rate_limited = resp
        .headers()
        .get("x-ratelimit-remaining")
        .is_some_and(|v| v == "0");
    match status {
        200..=299 => Ok(resp),
        403 | 429 if rate_limited => {
            bail!("GitHub rate limit reached fetching {url}; set GITHUB_TOKEN to raise it")
        }
        404 => bail!("not found: {url}"),
        _ => bail!("{url} returned HTTP {status}"),
    }
}

/// Resolve a link inside a remote document against its base URL.
pub fn resolve_link(base: &Url, href: &str) -> Option<Url> {
    base.join(href).ok()
}

/// Whether a resolved link points at a document bastian should open itself.
pub(crate) fn is_document(url: &Url) -> bool {
    let path = url.path().to_ascii_lowercase();
    doc::is_markdown_ext(path.rsplit('.').next().unwrap_or("")) && path.contains('.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_share_one_agent() {
        assert!(std::ptr::eq(agent(), agent()));
    }
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    #[test]
    fn classifies_inputs() {
        assert_eq!(
            classify("github.com/charmbracelet/vhs"),
            Some(Source::GitHub {
                owner: "charmbracelet".into(),
                repo: "vhs".into()
            })
        );
        assert_eq!(
            classify("https://github.com/o/r.git/"),
            Some(Source::GitHub {
                owner: "o".into(),
                repo: "r".into()
            })
        );
        assert_eq!(
            classify("gitlab.com/group/sub/project"),
            Some(Source::GitLab {
                project: "group/sub/project".into()
            })
        );
        // Deeper GitHub/GitLab paths are plain URLs.
        assert!(matches!(
            classify("https://github.com/o/r/blob/main/docs/a.md"),
            Some(Source::Url(_))
        ));
        assert!(matches!(
            classify("https://gitlab.com/g/p/-/raw/main/README.md"),
            Some(Source::Url(_))
        ));
        assert!(matches!(
            classify("https://example.com/a.md"),
            Some(Source::Url(_))
        ));
        assert_eq!(classify("README.md"), None);
        assert_eq!(classify("docs/github.com/x"), None);
    }

    #[test]
    fn gitlab_raw_urls() {
        assert_eq!(
            gitlab_raw("https://gitlab.com/g/p/-/blob/main/README.md"),
            "https://gitlab.com/g/p/-/raw/main/README.md"
        );
    }

    #[test]
    fn file_pages_map_to_raw_files() {
        let raw = |u: &str| raw_url(&Url::parse(u).unwrap()).map(|u| u.to_string());
        assert_eq!(
            raw("https://github.com/microsoft/fluentui/blob/master/README.md").as_deref(),
            Some("https://raw.githubusercontent.com/microsoft/fluentui/master/README.md")
        );
        assert_eq!(
            raw("https://github.com/o/r/blob/v1.2/docs/a%20b.md?plain=1#usage").as_deref(),
            Some("https://raw.githubusercontent.com/o/r/v1.2/docs/a%20b.md")
        );
        assert_eq!(
            raw("https://gitlab.example.com/g/sub/p/-/blob/main/README.md#top").as_deref(),
            Some("https://gitlab.example.com/g/sub/p/-/raw/main/README.md")
        );
        // Already raw, a repository page, or another site: fetched as given.
        assert_eq!(
            raw("https://raw.githubusercontent.com/o/r/main/README.md"),
            None
        );
        assert_eq!(raw("https://github.com/o/r/tree/main/docs"), None);
        assert_eq!(raw("https://github.com/o/r/blob/"), None);
        assert_eq!(raw("https://example.com/blob/x.md"), None);
    }

    #[test]
    fn resolves_relative_links() {
        let base = Url::parse("https://raw.githubusercontent.com/o/r/main/README.md").unwrap();
        let docs = resolve_link(&base, "docs/guide.md").unwrap();
        assert_eq!(
            docs.as_str(),
            "https://raw.githubusercontent.com/o/r/main/docs/guide.md"
        );
        assert!(is_document(&docs));
        let up = resolve_link(&base, "../other/x.png").unwrap();
        assert!(!is_document(&up));
        let abs = resolve_link(&base, "https://example.com/").unwrap();
        assert_eq!(abs.as_str(), "https://example.com/");
    }

    /// Serve one HTTP response on a local port, returning its URL.
    fn serve_once(status: &str, body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let status = status.to_string();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            // Drain the request headers.
            while reader.read_line(&mut line).unwrap() > 2 {
                line.clear();
            }
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Type: text/markdown\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        format!("http://{addr}/docs/README.md")
    }

    #[test]
    fn fetches_a_url() {
        let url = serve_once("200 OK", "# Remote\n\nSee [guide](guide.md).\n");
        let Some(source @ Source::Url(_)) = classify(&url) else {
            panic!("expected a URL source");
        };
        let doc = fetch(&source).unwrap();
        assert_eq!(doc.text, "# Remote\n\nSee [guide](guide.md).\n");
        let guide = resolve_link(&doc.base, "guide.md").unwrap();
        assert!(guide.as_str().ends_with("/docs/guide.md"));
    }

    #[test]
    fn http_errors_are_reported() {
        let url = serve_once("404 Not Found", "");
        let err = fetch(&classify(&url).unwrap()).unwrap_err().to_string();
        assert!(err.contains("not found"), "{err}");
    }
}
