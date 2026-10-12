//! Path helpers: expanding `~` and variables, showing paths short.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Expand a leading `~` (home directory) and `$VAR` / `${VAR}` references.
/// Unknown variables expand to nothing, as in a shell.
pub fn expand(input: &str, lookup: &dyn Fn(&str) -> Option<String>) -> PathBuf {
    let mut s = input.to_string();
    if (s == "~" || s.starts_with("~/"))
        && let Some(home) = lookup("HOME")
    {
        s.replace_range(..1, &home);
    }
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '$' {
            out.push(c);
            continue;
        }
        let braced = chars.peek() == Some(&'{');
        if braced {
            chars.next();
        }
        let mut name = String::new();
        while let Some(&n) = chars.peek() {
            if n.is_ascii_alphanumeric() || n == '_' {
                name.push(n);
                chars.next();
            } else {
                break;
            }
        }
        if braced && chars.peek() == Some(&'}') {
            chars.next();
        }
        if name.is_empty() {
            out.push('$');
        } else {
            out.push_str(&lookup(&name).unwrap_or_default());
        }
    }
    PathBuf::from(out)
}

/// `path` with the home directory shortened to `~`.
pub fn tilde(path: &Path) -> String {
    match std::env::var_os("HOME").and_then(|h| path.strip_prefix(h).ok()) {
        Some(rel) if rel.as_os_str().is_empty() => "~".to_string(),
        Some(rel) => format!("~/{}", rel.display()),
        None => path.display().to_string(),
    }
}

/// Path relative to the current directory when possible, for the status bar.
pub fn display_name(path: &Path) -> String {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| path.strip_prefix(cwd).ok().map(Path::to_path_buf))
        .unwrap_or_else(|| path.to_path_buf())
        .display()
        .to_string()
}

/// Directory a document's blocks run in: the one containing the file.
pub fn base_dir(path: &Path) -> PathBuf {
    match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

/// Write `contents` to `path`, creating its directory first.
pub fn write_creating_parent(path: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    std::fs::write(path, contents).with_context(|| format!("writing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_home_and_variables() {
        let lookup = |k: &str| match k {
            "HOME" => Some("/home/me".to_string()),
            "XDG" => Some("/xdg".to_string()),
            _ => None,
        };
        assert_eq!(
            expand("~/b.toml", &lookup),
            PathBuf::from("/home/me/b.toml")
        );
        assert_eq!(
            expand("$XDG/b/c.toml", &lookup),
            PathBuf::from("/xdg/b/c.toml")
        );
        assert_eq!(expand("${XDG}x", &lookup), PathBuf::from("/xdgx"));
        assert_eq!(expand("$NOPE/a", &lookup), PathBuf::from("/a"));
        assert_eq!(expand("a~b$", &lookup), PathBuf::from("a~b$"));
    }
}
