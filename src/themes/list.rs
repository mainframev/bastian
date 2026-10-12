use std::path::{Path, PathBuf};

use anyhow::Result;

use super::file::user_file;
use super::{builtin_names, load};
use crate::render::theme::Theme;

/// One row of `bastian themes`.
pub struct Listing {
    pub name: String,
    /// The file it's read from, for user themes.
    pub path: Option<PathBuf>,
    /// A user file that replaces a built-in of the same name.
    pub overrides: bool,
    /// The loaded theme, or why it doesn't load.
    pub theme: Result<Theme>,
}

/// Built-in themes, then the user's, each sorted by name. A user theme
/// that shadows a built-in replaces its row.
pub fn list(dir: Option<&Path>) -> Vec<Listing> {
    let mut user: Vec<(String, PathBuf)> = dir
        .and_then(|d| std::fs::read_dir(d).ok())
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let path = e.ok()?.path();
            let name = path.file_stem()?.to_str()?.to_string();
            (path.extension()? == "toml" && user_file(&name, dir).is_some()).then_some((name, path))
        })
        .collect();
    user.sort();
    let mut rows: Vec<Listing> = builtin_names()
        .map(|name| match user.iter().position(|(n, _)| n == name) {
            Some(i) => {
                let (name, path) = user.remove(i);
                Listing {
                    theme: load(&name, dir),
                    name,
                    path: Some(path),
                    overrides: true,
                }
            }
            None => Listing {
                name: name.to_string(),
                path: None,
                overrides: false,
                theme: load(name, None),
            },
        })
        .collect();
    rows.extend(user.into_iter().map(|(name, path)| Listing {
        theme: load(&name, dir),
        name,
        path: Some(path),
        overrides: false,
    }));
    rows
}
