use std::path::Path;

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::{Action, Reader};

/// Where the document came from, as the `p` popup shows it.
#[derive(Debug, PartialEq, Eq)]
pub enum Location {
    /// A local file: relative to the current directory (or as opened, when
    /// it isn't under it) and in full.
    File { relative: String, absolute: String },
    /// A document fetched over HTTP.
    Url(String),
    /// Read from stdin (named `stdin`) or made in memory, like the theme
    /// preview: only its name is known.
    Unnamed(String),
}

impl Location {
    fn of(r: &Reader) -> Self {
        if let Some(path) = &r.source {
            let cwd = std::env::current_dir().unwrap_or_default();
            return Self::file(path, &cwd);
        }
        match &r.remote_base {
            Some(url) => Self::Url(url.to_string()),
            None => Self::Unnamed(r.name.clone()),
        }
    }

    /// `path` seen from `cwd`. Symlinks are resolved on both sides, so a
    /// file opened through one still shows as under the current directory.
    fn file(path: &Path, cwd: &Path) -> Self {
        let joined = cwd.join(path);
        let absolute = std::fs::canonicalize(&joined).unwrap_or(joined);
        let cwd = std::fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
        let relative = absolute
            .strip_prefix(&cwd)
            .map_or_else(|_| path.display().to_string(), |p| p.display().to_string());
        Self::File {
            relative,
            absolute: absolute.display().to_string(),
        }
    }
}

impl Reader {
    /// Show where the document came from.
    pub(super) fn open_location(&mut self) {
        self.location = Some(Location::of(self));
    }

    /// Keys while the `p` popup is open: `R`/`A` copy, and it stays open
    /// with a message; anything else is ignored.
    pub(super) fn on_location_key(&mut self, key: KeyEvent) -> Action {
        let Some(location) = &self.location else {
            return Action::None;
        };
        let relative = match key.code {
            KeyCode::Char('R') => true,
            KeyCode::Char('A') => false,
            KeyCode::Esc | KeyCode::Char('q' | 'p') => {
                self.location = None;
                return Action::None;
            }
            _ => return Action::None,
        };
        let (what, text) = match location {
            Location::File { relative: rel, .. } if relative => ("relative path", rel.clone()),
            Location::File { absolute, .. } => ("absolute path", absolute.clone()),
            Location::Url(url) => ("URL", url.clone()),
            Location::Unnamed(_) => {
                self.message = Some("no file path to copy".into());
                return Action::None;
            }
        };
        self.message = Some(format!("copied {what}"));
        Action::Copy(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_show_relative_to_the_current_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("docs")).unwrap();
        let file = dir.path().join("docs/a.md");
        std::fs::write(&file, "").unwrap();
        let canonical = std::fs::canonicalize(&file).unwrap();
        let absolute = canonical.display().to_string();

        assert_eq!(
            Location::file(&file, dir.path()),
            Location::File {
                relative: "docs/a.md".into(),
                absolute: absolute.clone(),
            }
        );
        // Opened as `docs/../docs/a.md`: shown cleaned up.
        assert_eq!(
            Location::file(Path::new("docs/../docs/a.md"), dir.path()),
            Location::file(&file, dir.path()),
        );
        // Outside the current directory: as opened.
        let elsewhere = tempfile::tempdir().unwrap();
        assert_eq!(
            Location::file(&file, elsewhere.path()),
            Location::File {
                relative: file.display().to_string(),
                absolute,
            }
        );
    }
}
