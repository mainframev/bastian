use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use nucleo_matcher::{Utf32Str, Utf32String};

use crate::config::SortBy;

/// A Markdown file found under the browsed directory.
#[derive(Debug, Clone)]
pub struct Entry {
    pub path: PathBuf,
    /// Path relative to the browsed directory, for display and matching.
    pub rel: String,
    pub title: Option<String>,
    pub modified: Option<SystemTime>,
    /// Creation time, or the modification time where the filesystem
    /// doesn't record one.
    pub created: Option<SystemTime>,
    /// What the filter matches against: the path, then the title.
    pub(super) key: Utf32String,
}

impl Entry {
    pub fn new(
        path: PathBuf,
        rel: String,
        title: Option<String>,
        modified: Option<SystemTime>,
        created: Option<SystemTime>,
    ) -> Self {
        let hay = match &title {
            Some(t) => format!("{rel} {t}"),
            None => rel.clone(),
        };
        // Same representation `Utf32Str::new` gives, kept for every keystroke.
        let mut buf = Vec::new();
        let key = match Utf32Str::new(&hay, &mut buf) {
            Utf32Str::Ascii(_) => Utf32String::Ascii(hay.into()),
            Utf32Str::Unicode(chars) => Utf32String::Unicode(chars.into()),
        };
        Self {
            path,
            rel,
            title,
            modified,
            created,
            key,
        }
    }

    /// The name it's listed under: its title, else its file name.
    pub fn name(&self) -> String {
        self.title.clone().unwrap_or_else(|| {
            self.path
                .file_name()
                .map_or_else(|| self.rel.clone(), |n| n.to_string_lossy().into_owned())
        })
    }

    /// The time shown for it: creation time when sorting by it, else the
    /// modification time the list is ordered by.
    pub fn date(&self, sort: SortBy) -> Option<SystemTime> {
        match sort {
            SortBy::Created => self.created,
            SortBy::Modified | SortBy::Name => self.modified,
        }
    }
}

/// Bytes read from the start of each file to find its title.
pub(super) const TITLE_PREFIX: u64 = 8 * 1024;

pub(super) fn read_title(path: &Path) -> Option<String> {
    let mut buf = Vec::new();
    std::fs::File::open(path)
        .and_then(|f| f.take(TITLE_PREFIX).read_to_end(&mut buf))
        .ok()?;
    title_of(&crate::text::decode_lossy(&buf))
}

/// `title:` from YAML front matter, else the first `# ` heading.
pub(super) fn title_of(src: &str) -> Option<String> {
    let mut body = src;
    if let Some(rest) = src
        .strip_prefix("---\n")
        .or_else(|| src.strip_prefix("---\r\n"))
    {
        let end = rest.find("\n---").unwrap_or(rest.len());
        let front = &rest[..end];
        let title = front.lines().find_map(|l| {
            let v = l
                .strip_prefix("title:")?
                .trim()
                .trim_matches(['"', '\''])
                .trim();
            (!v.is_empty()).then(|| v.to_string())
        });
        if title.is_some() {
            return title;
        }
        body = &rest[end..];
    }
    first_heading(body)
}

/// First ATX `# ` heading outside fenced code, without a full parse.
pub(super) fn first_heading(src: &str) -> Option<String> {
    let mut fenced = false;
    for line in src.lines() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fenced = !fenced;
        } else if !fenced && let Some(h) = t.strip_prefix("# ") {
            let h = h.trim().trim_end_matches('#').trim();
            if !h.is_empty() {
                return Some(h.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_skips_fences() {
        assert_eq!(
            first_heading("```sh\n# not a heading\n```\n# Real #\n").as_deref(),
            Some("Real")
        );
        assert_eq!(first_heading("## only h2\n"), None);
    }

    #[test]
    fn title_from_front_matter_or_heading() {
        assert_eq!(
            title_of("---\ntitle: \"Guide\"\nlayout: x\n---\n# Heading\n").as_deref(),
            Some("Guide")
        );
        assert_eq!(
            title_of("---\nlayout: x\n---\n\n# Heading\n").as_deref(),
            Some("Heading")
        );
        assert_eq!(title_of("no title here\n"), None);
    }

    #[test]
    fn title_only_reads_file_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let late = dir.path().join("late.md");
        let filler = "x\n".repeat(TITLE_PREFIX as usize);
        std::fs::write(&late, format!("{filler}# Too late\n")).unwrap();
        assert_eq!(read_title(&late), None);
    }
}
