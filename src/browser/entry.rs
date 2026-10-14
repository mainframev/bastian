use std::path::PathBuf;
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
    /// `name()` in lowercase, for sorting by name.
    pub(super) sort_name: String,
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
        let mut entry = Self {
            path,
            rel,
            title,
            modified,
            created,
            key,
            sort_name: String::new(),
        };
        entry.sort_name = entry.name().to_lowercase();
        entry
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
