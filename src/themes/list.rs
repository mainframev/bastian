use std::path::{Path, PathBuf};

use anyhow::Result;

use super::Theme;
use super::file::user_file;
use super::{Appearance, builtin_names, canonical, load};
use crate::config::Resolved;
use crate::paths;

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

/// `bastian themes`: one row per theme with its appearance, where it comes
/// from, a color swatch when `color`, and which settings point at it.
pub fn format_list(r: &Resolved, dir: Option<&Path>, color: bool) -> String {
    use ratatui::style::Color;
    let rows = list(dir);
    let w = rows.iter().map(|l| l.name.len()).max().unwrap_or(0);
    let swatch = |c: Color| match c {
        Color::Rgb(r, g, b) => format!("\x1b[38;2;{r};{g};{b}m██\x1b[0m"),
        _ => String::new(),
    };
    let mut out = String::new();
    for l in rows {
        let roles: Vec<&str> = [
            ("theme", &r.theme.value),
            ("dark_theme", &r.dark_theme.value),
            ("light_theme", &r.light_theme.value),
        ]
        .into_iter()
        .filter(|(_, v)| canonical(v, dir) == l.name)
        .map(|(k, _)| k)
        .collect();
        let origin = match (&l.path, l.overrides) {
            (Some(p), true) => format!("{} (overrides built-in)", paths::tilde(p)),
            (Some(p), false) => paths::tilde(p),
            (None, _) => "built-in".to_string(),
        };
        let (appearance, colors) = match &l.theme {
            Ok(t) => {
                let appearance = match t.appearance {
                    Appearance::Dark => "dark",
                    Appearance::Light => "light",
                };
                let colors: String = [t.accent, t.heading, t.link, t.code, t.ok, t.warn, t.err]
                    .into_iter()
                    .map(swatch)
                    .collect();
                (
                    appearance,
                    if color {
                        format!("{colors}  ")
                    } else {
                        String::new()
                    },
                )
            }
            Err(_) => ("?", String::new()),
        };
        out.push_str(&format!(
            "{:<w$}  {appearance:<5}  {colors}{origin}",
            l.name
        ));
        if !roles.is_empty() {
            out.push_str(&format!("  ← {}", roles.join(", ")));
        }
        if let Err(e) = &l.theme {
            out.push_str(&format!("\n{:<w$}  error: {e:#}", ""));
        }
        out.push('\n');
    }
    out
}
