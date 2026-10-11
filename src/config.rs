use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use clap::ValueEnum;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, clap::ValueEnum)]
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

/// Where a resolved setting's value came from, lowest precedence first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    Default,
    File,
    Env,
    Flag,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Setting<T> {
    pub value: T,
    pub source: Source,
}

/// Environment variables, passed in so resolution is testable.
pub type EnvMap = HashMap<String, String>;

/// The `BASTIAN_*` variables of the current process.
pub fn env_vars() -> EnvMap {
    std::env::vars()
        .filter(|(k, _)| k.starts_with("BASTIAN_"))
        .collect()
}

/// Settings given on the command line; `None` means "not given".
#[derive(Debug, Default, Clone)]
pub struct CliOverrides {
    pub theme: Option<ThemeName>,
    pub width: Option<u16>,
    pub confirm: Option<Confirm>,
    pub line_numbers: Option<bool>,
}

/// Every effective setting, resolved once at startup. All modes (TUI,
/// print) read from this, so they can't disagree about precedence.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    pub theme: Setting<ThemeName>,
    pub width: Setting<u16>,
    pub confirm: Setting<Confirm>,
    pub line_numbers: Setting<bool>,
    pub interpreters: HashMap<String, Vec<String>>,
}

/// Merge defaults < config file < `BASTIAN_*` environment < flags.
/// Invalid environment values are errors naming the variable.
pub fn resolve(file: Config, env: &EnvMap, cli: CliOverrides) -> Result<Resolved> {
    Ok(Resolved {
        theme: pick(
            ThemeName::Dark,
            file.theme,
            env_value(env, "BASTIAN_THEME", parse_enum, "dark or light")?,
            cli.theme,
        ),
        width: pick(
            100,
            file.width,
            env_value(
                env,
                "BASTIAN_WIDTH",
                |v| v.parse().ok(),
                "a number of columns",
            )?,
            cli.width,
        ),
        confirm: pick(
            Confirm::Always,
            file.confirm,
            env_value(env, "BASTIAN_CONFIRM", parse_enum, "always or never")?,
            cli.confirm,
        ),
        line_numbers: pick(
            false,
            file.line_numbers,
            env_value(env, "BASTIAN_LINE_NUMBERS", parse_bool, "true or false")?,
            cli.line_numbers,
        ),
        interpreters: file.interpreters,
    })
}

fn pick<T>(default: T, file: Option<T>, env: Option<T>, cli: Option<T>) -> Setting<T> {
    let (value, source) = match (cli, env, file) {
        (Some(v), _, _) => (v, Source::Flag),
        (None, Some(v), _) => (v, Source::Env),
        (None, None, Some(v)) => (v, Source::File),
        (None, None, None) => (default, Source::Default),
    };
    Setting { value, source }
}

fn env_value<T>(
    env: &EnvMap,
    key: &str,
    parse: impl Fn(&str) -> Option<T>,
    expected: &str,
) -> Result<Option<T>> {
    match env.get(key).map(|v| v.trim()) {
        None | Some("") => Ok(None),
        Some(v) => parse(v)
            .map(Some)
            .ok_or_else(|| anyhow!("{key}={v:?} is not valid; expected {expected}")),
    }
}

fn parse_enum<T: ValueEnum>(v: &str) -> Option<T> {
    T::from_str(v, true).ok()
}

fn parse_bool(v: &str) -> Option<bool> {
    match v.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> EnvMap {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn precedence_is_default_file_env_flag() {
        let file = || Config {
            theme: Some(ThemeName::Light),
            width: Some(80),
            ..Config::default()
        };
        let none = CliOverrides::default();

        let r = resolve(Config::default(), &env(&[]), none.clone()).unwrap();
        assert_eq!(
            r.theme,
            Setting {
                value: ThemeName::Dark,
                source: Source::Default
            }
        );
        assert_eq!(r.width.value, 100);

        let r = resolve(file(), &env(&[]), none.clone()).unwrap();
        assert_eq!(
            r.theme,
            Setting {
                value: ThemeName::Light,
                source: Source::File
            }
        );

        let e = env(&[("BASTIAN_THEME", "dark"), ("BASTIAN_WIDTH", "120")]);
        let r = resolve(file(), &e, none).unwrap();
        assert_eq!(
            r.theme,
            Setting {
                value: ThemeName::Dark,
                source: Source::Env
            }
        );
        assert_eq!(
            r.width,
            Setting {
                value: 120,
                source: Source::Env
            }
        );

        let cli = CliOverrides {
            theme: Some(ThemeName::Light),
            width: Some(60),
            confirm: Some(Confirm::Never),
            line_numbers: Some(true),
        };
        let r = resolve(file(), &e, cli).unwrap();
        assert_eq!(
            r.theme,
            Setting {
                value: ThemeName::Light,
                source: Source::Flag
            }
        );
        assert_eq!(
            r.width,
            Setting {
                value: 60,
                source: Source::Flag
            }
        );
        assert_eq!(r.confirm.value, Confirm::Never);
        assert!(r.line_numbers.value);
    }

    #[test]
    fn env_values_are_parsed_and_validated() {
        let e = env(&[
            ("BASTIAN_CONFIRM", "NEVER"),
            ("BASTIAN_LINE_NUMBERS", "yes"),
            ("BASTIAN_THEME", ""),
        ]);
        let r = resolve(Config::default(), &e, CliOverrides::default()).unwrap();
        assert_eq!(
            r.confirm,
            Setting {
                value: Confirm::Never,
                source: Source::Env
            }
        );
        assert_eq!(
            r.line_numbers,
            Setting {
                value: true,
                source: Source::Env
            }
        );
        // Empty means unset.
        assert_eq!(r.theme.source, Source::Default);

        let err = resolve(
            Config::default(),
            &env(&[("BASTIAN_WIDTH", "wide")]),
            CliOverrides::default(),
        )
        .unwrap_err()
        .to_string();
        assert!(
            err.contains("BASTIAN_WIDTH") && err.contains("wide"),
            "{err}"
        );
    }

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
