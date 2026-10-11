use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

/// `~/.config/bastian/config.toml`, or under `$XDG_CONFIG_HOME` when set.
#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub theme: Option<ThemeName>,
    pub width: Option<u16>,
    pub confirm: Option<Confirm>,
    /// Show rendered line numbers in the reader.
    pub line_numbers: Option<bool>,
    /// Language → command line; the block source is appended as the last
    /// argument. An empty list stops that language from running.
    pub interpreters: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ThemeName {
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confirm {
    /// Ask before every run (the default).
    Always,
    /// Only ask for blocks marked `confirm`, like `--yes`.
    Never,
}

pub fn path() -> Option<PathBuf> {
    let base = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => PathBuf::from(std::env::var_os("HOME")?).join(".config"),
    };
    Some(base.join("bastian").join("config.toml"))
}

/// Load the config file. A missing file is an empty config; a broken one is an error.
pub fn load() -> Result<Config> {
    match path() {
        Some(p) if p.exists() => load_from(&p),
        _ => Ok(Config::default()),
    }
}

pub fn load_from(path: &Path) -> Result<Config> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_fields() {
        let c: Config = toml::from_str(
            r#"
            theme = "light"
            width = 80
            confirm = "never"
            line_numbers = true
            [interpreters]
            ruby = ["ruby", "-e"]
            fish = []
            "#,
        )
        .unwrap();
        assert_eq!(c.theme, Some(ThemeName::Light));
        assert_eq!(c.width, Some(80));
        assert_eq!(c.confirm, Some(Confirm::Never));
        assert_eq!(c.line_numbers, Some(true));
        assert_eq!(c.interpreters["ruby"], ["ruby", "-e"]);
        assert!(c.interpreters["fish"].is_empty());
    }

    #[test]
    fn empty_is_default() {
        assert_eq!(toml::from_str::<Config>("").unwrap(), Config::default());
    }

    #[test]
    fn rejects_unknown_keys() {
        let err = toml::from_str::<Config>("colour = \"red\"").unwrap_err();
        assert!(err.to_string().contains("colour"), "{err}");
    }

    #[test]
    fn load_reports_path_on_error() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.toml");
        std::fs::write(&p, "width = \"wide\"").unwrap();
        let err = format!("{:#}", load_from(&p).unwrap_err());
        assert!(err.contains("config.toml"), "{err}");
    }
}
