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
    /// Colored output when printing: auto, always or never.
    pub color: Option<ColorMode>,
    /// Page printed output through `$PAGER` instead of writing it out.
    pub pager: Option<bool>,
    /// List hidden and gitignored files in the browser.
    pub all: Option<bool>,
    /// Language → command line; the block source is appended as the last
    /// argument. An empty list stops that language from running.
    pub interpreters: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ThemeName {
    /// Light or dark to match the terminal's background.
    Auto,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ColorMode {
    /// Color when writing to a terminal (or with `CLICOLOR_FORCE`), never
    /// with `NO_COLOR`.
    Auto,
    Always,
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

/// Which config file to use: `--config`, else `BASTIAN_CONFIG`, else the
/// default location. Returns the path and whether it was given explicitly
/// (an explicit file must exist). `~` and `$VAR` are expanded in both.
pub fn locate(
    flag: Option<&str>,
    env_path: Option<&str>,
    lookup: &dyn Fn(&str) -> Option<String>,
) -> Option<(PathBuf, bool)> {
    match flag.or(env_path.filter(|p| !p.is_empty())) {
        Some(p) => Some((expand(p, lookup), true)),
        None => path().map(|p| (p, false)),
    }
}

/// Load the located config file. A missing default file is an empty
/// config; a missing explicit one is an error.
pub fn load_at(path: &Path, explicit: bool) -> Result<Config> {
    if path.exists() {
        load_from(path)
    } else if explicit {
        Err(anyhow!("config file {} does not exist", path.display()))
    } else {
        Ok(Config::default())
    }
}

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

/// Commented starting point written by `bastian config`.
pub const TEMPLATE: &str = r#"# bastian configuration. Every key is optional; command-line flags and
# BASTIAN_* environment variables take precedence over this file.

# theme = "auto"        # "auto" (match the terminal), "dark" or "light"
# width = 100           # maximum document width
# confirm = "always"    # "never" runs code blocks without asking, like --yes
# line_numbers = false  # show line numbers in the reader
# color = "auto"        # printed output: "auto", "always" or "never"
# pager = false         # show documents in $PAGER instead of the TUI
# all = false           # also list hidden and gitignored files

# [interpreters]
# The block source is appended as the last argument.
# ruby = ["ruby", "-e"]
# fish = []             # an empty list stops a language from running
"#;

/// The resolved settings as TOML, each value annotated with where it came
/// from, for `--print-config`.
pub fn describe(r: &Resolved, file: Option<&Path>) -> String {
    fn src(s: Source) -> &'static str {
        match s {
            Source::Default => "default",
            Source::File => "config file",
            Source::Env => "environment",
            Source::Flag => "flag",
        }
    }
    fn name<T: ValueEnum>(v: &T) -> String {
        v.to_possible_value()
            .map_or_else(String::new, |p| p.get_name().to_string())
    }
    let mut out = String::new();
    if let Some(f) = file {
        out.push_str(&format!("# config file: {}\n", f.display()));
    }
    let mut row = |key: &str, value: String, source: Source| {
        out.push_str(&format!(
            "{:<22}# {}\n",
            format!("{key} = {value}"),
            src(source)
        ));
    };
    row(
        "theme",
        format!("{:?}", name(&r.theme.value)),
        r.theme.source,
    );
    row("width", r.width.value.to_string(), r.width.source);
    row(
        "confirm",
        format!("{:?}", name(&r.confirm.value)),
        r.confirm.source,
    );
    row(
        "line_numbers",
        r.line_numbers.value.to_string(),
        r.line_numbers.source,
    );
    row(
        "color",
        format!("{:?}", name(&r.color.value)),
        r.color.source,
    );
    row("pager", r.pager.value.to_string(), r.pager.source);
    row("all", r.all.value.to_string(), r.all.source);
    if !r.interpreters.is_empty() {
        out.push_str("\n[interpreters]\n");
        let mut langs: Vec<_> = r.interpreters.iter().collect();
        langs.sort();
        for (lang, cmd) in langs {
            out.push_str(&format!("{lang} = {cmd:?}\n"));
        }
    }
    out
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
    pub color: Option<ColorMode>,
    pub pager: Option<bool>,
    pub all: Option<bool>,
}

/// Every effective setting, resolved once at startup. All modes (TUI,
/// print) read from this, so they can't disagree about precedence.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    pub theme: Setting<ThemeName>,
    pub width: Setting<u16>,
    pub confirm: Setting<Confirm>,
    pub line_numbers: Setting<bool>,
    pub color: Setting<ColorMode>,
    pub pager: Setting<bool>,
    pub all: Setting<bool>,
    pub interpreters: HashMap<String, Vec<String>>,
}

/// Merge defaults < config file < `BASTIAN_*` environment < flags.
/// Invalid environment values are errors naming the variable.
pub fn resolve(file: Config, env: &EnvMap, cli: CliOverrides) -> Result<Resolved> {
    Ok(Resolved {
        theme: pick(
            ThemeName::Auto,
            file.theme,
            env_value(env, "BASTIAN_THEME", parse_enum, "auto, dark or light")?,
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
        color: pick(
            ColorMode::Auto,
            file.color,
            env_value(env, "BASTIAN_COLOR", parse_enum, "auto, always or never")?,
            cli.color,
        ),
        pager: pick(
            false,
            file.pager,
            env_value(env, "BASTIAN_PAGER", parse_bool, "true or false")?,
            cli.pager,
        ),
        all: pick(
            false,
            file.all,
            env_value(env, "BASTIAN_ALL", parse_bool, "true or false")?,
            cli.all,
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
                value: ThemeName::Auto,
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
            ..CliOverrides::default()
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

    #[test]
    fn locate_prefers_flag_then_env() {
        let lookup = |k: &str| (k == "HOME").then(|| "/h".to_string());
        assert_eq!(
            locate(Some("~/f.toml"), Some("/e.toml"), &lookup),
            Some((PathBuf::from("/h/f.toml"), true))
        );
        assert_eq!(
            locate(None, Some("/e.toml"), &lookup),
            Some((PathBuf::from("/e.toml"), true))
        );
        assert!(matches!(
            locate(None, Some(""), &lookup),
            Some((_, false)) | None
        ));
    }

    #[test]
    fn explicit_missing_config_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("none.toml");
        assert!(load_at(&missing, true).is_err());
        assert_eq!(load_at(&missing, false).unwrap(), Config::default());
    }

    #[test]
    fn template_parses_and_describe_shows_sources() {
        assert_eq!(
            toml::from_str::<Config>(TEMPLATE).unwrap(),
            Config::default()
        );
        let file = Config {
            width: Some(80),
            ..Config::default()
        };
        let r = resolve(
            file,
            &env(&[("BASTIAN_THEME", "light")]),
            CliOverrides::default(),
        )
        .unwrap();
        let text = describe(&r, None);
        assert!(text.contains("theme = \"light\""), "{text}");
        assert!(
            text.lines()
                .any(|l| l.starts_with("theme") && l.ends_with("# environment")),
            "{text}"
        );
        assert!(
            text.lines()
                .any(|l| l.starts_with("width = 80") && l.ends_with("# config file")),
            "{text}"
        );
        assert!(
            text.lines()
                .any(|l| l.starts_with("pager = false") && l.ends_with("# default")),
            "{text}"
        );
        // The output is itself valid config.
        let stripped: String = text
            .lines()
            .map(|l| format!("{}\n", l.split('#').next().unwrap()))
            .collect();
        assert!(toml::from_str::<Config>(&stripped).is_ok(), "{stripped}");
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
            color = "never"
            pager = true
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
        assert_eq!(c.color, Some(ColorMode::Never));
        assert_eq!(c.pager, Some(true));
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
