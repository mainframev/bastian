use std::path::{Path, PathBuf};

use crate::doc::{self, Block, Document, Inline};

#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    pub text: String,
    pub url: String,
}

/// Links in document order. Adjacent runs pointing at the same URL (e.g. a
/// partly bold link) are merged into one.
pub fn collect(doc: &Document) -> Vec<Link> {
    let mut out = Vec::new();
    doc::visit(&doc.blocks, &mut |b| match b {
        Block::Heading { inlines, .. } | Block::Paragraph(inlines) => {
            inline_links(inlines, &mut out);
        }
        Block::Table { header, rows, .. } => {
            for cell in header.iter().chain(rows.iter().flatten()) {
                inline_links(cell, &mut out);
            }
        }
        _ => {}
    });
    out
}

fn inline_links(inlines: &[Inline], out: &mut Vec<Link>) {
    let mut prev: Option<&str> = None;
    for i in inlines {
        match (&i.style.link, prev) {
            (Some(url), Some(p)) if p == url => {
                if let Some(last) = out.last_mut() {
                    last.text.push_str(&i.text);
                }
            }
            (Some(url), _) => out.push(Link {
                text: i.text.clone(),
                url: url.clone(),
            }),
            (None, _) => {}
        }
        prev = i.style.link.as_deref();
    }
}

/// What following a link should do.
#[derive(Debug, PartialEq)]
pub enum Target {
    /// A Markdown file to open in bastian.
    Document(PathBuf),
    /// Anything to hand to the system opener: URLs and other local files.
    External(String),
    /// A `#heading` anchor within the current document.
    Anchor(String),
    Missing(PathBuf),
}

pub fn resolve(url: &str, base_dir: &Path) -> Target {
    if let Some(anchor) = url.strip_prefix('#') {
        return Target::Anchor(anchor.to_string());
    }
    if url.contains("://") || has_scheme(url) {
        return Target::External(url.to_string());
    }
    let path_part = url.split('#').next().unwrap_or(url);
    let path = base_dir.join(path_part);
    if !path.exists() {
        return Target::Missing(path);
    }
    let is_md = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(doc::is_markdown_ext);
    if is_md && path.is_file() {
        Target::Document(path)
    } else {
        Target::External(path.display().to_string())
    }
}

/// Whether `url` starts with a URI scheme (`mailto:`, `tel:`, `javascript:`,
/// …). One-letter schemes are left out: those are Windows drive letters.
fn has_scheme(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|u| u.scheme().len() >= 2)
}

/// Whether `target` is a web or mail link, safe to open without asking.
/// Other schemes and local files can launch apps or run scripts.
pub fn is_web(target: &str) -> bool {
    url::Url::parse(target).is_ok_and(|u| matches!(u.scheme(), "http" | "https" | "mailto"))
}

/// GitHub-style heading anchor: lowercase, spaces to dashes, punctuation dropped.
pub fn slug(text: &str) -> String {
    text.trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            c if c.is_alphanumeric() || c == '-' || c == '_' => Some(c),
            _ => None,
        })
        .collect()
}

/// Open with the platform's default handler, detached from the TUI.
pub fn open_external(target: &str) -> std::io::Result<()> {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(opener)
        .arg(target)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::parse;

    #[test]
    fn collects_and_merges_links() {
        let doc = parse(
            "See [the **docs**](https://x.dev) and [setup](./setup.md).\n\n- [item](#usage)\n\n| [cell](a.md) |\n|---|\n",
        );
        let links = collect(&doc);
        let pairs: Vec<(&str, &str)> = links
            .iter()
            .map(|l| (l.text.as_str(), l.url.as_str()))
            .collect();
        assert_eq!(
            pairs,
            [
                ("the docs", "https://x.dev"),
                ("setup", "./setup.md"),
                ("item", "#usage"),
                ("cell", "a.md"),
            ]
        );
    }

    #[test]
    fn resolves_targets() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("setup.md"), "# Setup\n").unwrap();
        std::fs::write(dir.path().join("logo.png"), "").unwrap();
        let d = dir.path();
        assert_eq!(
            resolve("https://x.dev", d),
            Target::External("https://x.dev".into())
        );
        assert_eq!(resolve("#usage", d), Target::Anchor("usage".into()));
        assert_eq!(
            resolve("setup.md#install", d),
            Target::Document(d.join("setup.md"))
        );
        assert!(matches!(resolve("logo.png", d), Target::External(_)));
        assert!(matches!(resolve("nope.md", d), Target::Missing(_)));
    }

    #[test]
    fn any_uri_scheme_is_external() {
        let d = Path::new("/nowhere");
        for url in [
            "mailto:a@b.c",
            "MAILTO:a@b.c",
            "tel:+15551234",
            "sms:+15551234",
            "javascript:alert(1)",
            "vscode://file/etc/hosts",
            "file:///Applications/X.app",
        ] {
            assert_eq!(resolve(url, d), Target::External(url.into()), "{url}");
        }
        assert!(is_web("MAILTO:a@b.c"));
        // Drive letters and plain paths stay local.
        for url in ["C:/docs/a.md", "c:\\docs\\a.md", "docs/a.md", "a.md#x"] {
            assert!(matches!(resolve(url, d), Target::Missing(_)), "{url}");
        }
    }

    #[test]
    fn every_markdown_extension_opens_in_bastian() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        for name in ["a.mkd", "b.mdown", "c.mkdn", "d.MD"] {
            std::fs::write(d.join(name), "# Doc\n").unwrap();
            assert_eq!(resolve(name, d), Target::Document(d.join(name)), "{name}");
        }
    }

    #[test]
    fn only_web_and_mail_links_are_web() {
        for t in [
            "https://x.dev",
            "http://x.dev/a?b#c",
            "HTTPS://X.DEV",
            "mailto:a@b.c",
        ] {
            assert!(is_web(t), "{t}");
        }
        for t in [
            "file:///Applications/X.app",
            "vscode://file/etc/passwd",
            "javascript:alert(1)",
            "/tmp/setup.command",
            "setup.command",
            "",
        ] {
            assert!(!is_web(t), "{t}");
        }
    }

    #[test]
    fn slugs() {
        assert_eq!(slug("Getting Started!"), "getting-started");
        assert_eq!(slug("API v2 (beta)"), "api-v2-beta");
    }
}
