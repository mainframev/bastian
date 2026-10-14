//! Opening documents from disk: the one place that knows what a document
//! file is. It reads and decodes the file, parses it as its extension
//! says, and knows where its links and code blocks run from.

use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::doc::{self, Document};
use crate::{paths, text};

/// A document read from a file.
pub struct Opened {
    pub path: PathBuf,
    pub doc: Document,
    /// The decoded source, for copying and for comparing on reload.
    pub text: String,
    /// Directory block `cwd`s and relative links resolve from.
    pub base_dir: PathBuf,
}

/// Read, decode and parse the document at `path`.
pub fn open(path: &Path) -> Result<Opened> {
    let text = text::read_text(path)?;
    let doc = doc::parse_for(&text, Some(path));
    Ok(Opened {
        path: path.to_path_buf(),
        doc,
        text,
        base_dir: paths::base_dir(path),
    })
}

/// Bytes read from the start of a file to find its title.
const TITLE_PREFIX: u64 = 8 * 1024;

/// Title of the document at `path`, from its first 8 KiB: `title:` in YAML
/// front matter, else its first level-1 heading. Best effort: `None` for
/// anything unreadable or untitled.
pub fn peek_title(path: &Path) -> Option<String> {
    let mut head = Vec::new();
    std::fs::File::open(path)
        .and_then(|f| f.take(TITLE_PREFIX).read_to_end(&mut head))
        .ok()?;
    let src = text::decode_lossy(&head);
    // The same parser opening it would use (MDX for .mdx); a cut-off MDX
    // head falls back to plain Markdown.
    front_matter_title(&src).or_else(|| doc::parse_for(&src, Some(path)).title())
}

/// `title:` from YAML front matter at the start of `src`.
fn front_matter_title(src: &str) -> Option<String> {
    let rest = src
        .strip_prefix("---\n")
        .or_else(|| src.strip_prefix("---\r\n"))?;
    let end = rest.find("\n---").unwrap_or(rest.len());
    rest[..end].lines().find_map(|l| {
        let v = l
            .strip_prefix("title:")?
            .trim()
            .trim_matches(['"', '\''])
            .trim();
        (!v.is_empty()).then(|| v.to_string())
    })
}

/// Whether `path` is an existing file bastian opens as Markdown.
pub fn is_markdown_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(doc::is_markdown_ext)
        && path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir_with(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (name, src) in files {
            std::fs::write(dir.path().join(name), src).unwrap();
        }
        dir
    }

    #[test]
    fn peek_title_prefers_front_matter_then_the_first_heading() {
        let dir = dir_with(&[
            (
                "front.md",
                "---\ntitle: \"Guide\"\nlayout: x\n---\n# Heading\n",
            ),
            ("plain.md", "---\nlayout: x\n---\n\n# Heading\n"),
            ("setext.md", "Underlined\n==========\n\ntext\n"),
            ("fenced.md", "```sh\n# not a heading\n```\n# Real #\n"),
            (
                "page.mdx",
                "import X from './x'\n\n<Page>\n\n# Inside JSX\n\n</Page>\n",
            ),
            ("none.md", "no title here\n\n## only h2\n"),
        ]);
        let title = |name: &str| peek_title(&dir.path().join(name));
        assert_eq!(title("front.md").as_deref(), Some("Guide"));
        assert_eq!(title("plain.md").as_deref(), Some("Heading"));
        assert_eq!(title("setext.md").as_deref(), Some("Underlined"));
        assert_eq!(title("fenced.md").as_deref(), Some("Real"));
        assert_eq!(title("page.mdx").as_deref(), Some("Inside JSX"));
        assert_eq!(title("none.md"), None);
        assert_eq!(title("missing.md"), None);
    }

    #[test]
    fn peek_title_reads_only_the_first_8_kib() {
        let filler = "x\n".repeat(TITLE_PREFIX as usize);
        let dir = dir_with(&[("late.md", &format!("{filler}# Too late\n"))]);
        assert_eq!(peek_title(&dir.path().join("late.md")), None);
    }

    #[test]
    fn open_reads_code_files_as_code_with_their_dir_as_base() {
        let dir = dir_with(&[("main.rs", "fn main() {}\n"), ("a.md", "# A\n")]);
        let code = open(&dir.path().join("main.rs")).unwrap();
        assert!(code.doc.code_file);
        assert_eq!(code.base_dir, dir.path());
        assert_eq!(code.text, "fn main() {}\n");
        let md = open(&dir.path().join("a.md")).unwrap();
        assert_eq!(md.doc.title().as_deref(), Some("A"));
        assert!(open(&dir.path().join("missing.md")).is_err());
        // A bare file name resolves against the current directory.
        assert_eq!(paths::base_dir(Path::new("README.md")), Path::new("."));
    }

    #[test]
    fn is_markdown_file_needs_an_existing_file_with_a_markdown_extension() {
        let dir = dir_with(&[
            ("guide.md", "# G\n"),
            ("upper.MD", "# U\n"),
            ("notes.txt", ""),
        ]);
        std::fs::create_dir(dir.path().join("dir.md")).unwrap();
        let is = |name: &str| is_markdown_file(&dir.path().join(name));
        assert!(is("guide.md") && is("upper.MD"));
        assert!(!is("notes.txt") && !is("dir.md") && !is("missing.md"));
    }
}
