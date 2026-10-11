use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use clap::ValueEnum;
use serde::Deserialize;

/// `~/.config/bastian/config.toml`, or under `$XDG_CONFIG_HOME` when set.
#[derive(Debug, Default, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// `auto` (follow the terminal background), `dark`, `light`, or a
    /// theme name.
    pub theme: Option<String>,
    /// Theme used by `auto` on dark backgrounds, and by `dark`.
    pub dark_theme: Option<String>,
    /// Theme used by `auto` on light backgrounds, and by `light`.
    pub light_theme: Option<String>,
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
    /// Scroll with the mouse wheel.
    pub mouse: Option<bool>,
    /// Show the logo at the top of the file browser.
    pub logo: Option<bool>,
    /// Show pictures for images in documents, where the terminal can.
    pub images: Option<bool>,
    /// Language → command line; the block source is appended as the last
    /// argument. An empty list stops that language from running.
    pub interpreters: HashMap<String, Vec<String>>,
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
    // Color when writing to a terminal (or with `CLICOLOR_FORCE`), never
    // with `NO_COLOR`. A plain comment: a doc comment here would switch
    // `--help` to clap's long layout.
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

# theme = "auto"        # "auto" (match the terminal), "dark", "light" or a
#                       # theme name; `bastian themes` lists them, and your
#                       # own go in themes/<name>.toml next to this file
# dark_theme = "rose-pine"        # used by "auto" and "dark"
# light_theme = "rose-pine-dawn"  # used by "auto" and "light"
# width = 100           # maximum document width
# confirm = "always"    # "never" runs code blocks without asking, like --yes
# line_numbers = false  # show line numbers in the reader
# color = "auto"        # printed output: "auto", "always" or "never"
# pager = false         # show documents in $PAGER instead of the TUI
# all = false           # also list hidden and gitignored files
# mouse = false         # scroll with the mouse wheel
# logo = true           # show the logo at the top of the file browser
# images = true         # show pictures for images in documents

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
    fn row<T: Value>(key: &str, s: &Setting<T>) -> (String, Source) {
        (format!("{key} = {}", s.value.toml()), s.source)
    }
    let mut out = String::new();
    if let Some(f) = file {
        out.push_str(&format!("# config file: {}\n", f.display()));
    }
    let rows = [
        row("theme", &r.theme),
        row("dark_theme", &r.dark_theme),
        row("light_theme", &r.light_theme),
        row("width", &r.width),
        row("confirm", &r.confirm),
        row("line_numbers", &r.line_numbers),
        row("color", &r.color),
        row("pager", &r.pager),
        row("all", &r.all),
        row("mouse", &r.mouse),
        row("logo", &r.logo),
        row("images", &r.images),
    ];
    // Line the source comments up, at least two spaces after the value.
    let w = rows.iter().map(|(text, _)| text.len()).max().unwrap_or(0) + 2;
    for (text, source) in rows {
        out.push_str(&format!("{text:<w$}# {}\n", src(source)));
    }
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

/// Every effective setting, resolved once at startup. All modes (TUI,
/// print) read from this, so they can't disagree about precedence.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    pub theme: Setting<String>,
    pub dark_theme: Setting<String>,
    pub light_theme: Setting<String>,
    pub width: Setting<u16>,
    pub confirm: Setting<Confirm>,
    pub line_numbers: Setting<bool>,
    pub color: Setting<ColorMode>,
    pub pager: Setting<bool>,
    pub all: Setting<bool>,
    pub mouse: Setting<bool>,
    pub logo: Setting<bool>,
    pub images: Setting<bool>,
    pub interpreters: HashMap<String, Vec<String>>,
}

/// Merge defaults < config file < `BASTIAN_*` environment < flags.
/// `cli` holds the settings given as flags (its `interpreters` are unused).
/// Invalid environment values are errors naming the variable.
pub fn resolve(file: Config, env: &EnvMap, cli: Config) -> Result<Resolved> {
    Ok(Resolved {
        theme: setting("theme", "auto".into(), file.theme, cli.theme, env)?,
        dark_theme: setting(
            "dark_theme",
            "rose-pine".into(),
            file.dark_theme,
            cli.dark_theme,
            env,
        )?,
        light_theme: setting(
            "light_theme",
            "rose-pine-dawn".into(),
            file.light_theme,
            cli.light_theme,
            env,
        )?,
        width: setting("width", 100, file.width, cli.width, env)?,
        confirm: setting("confirm", Confirm::Always, file.confirm, cli.confirm, env)?,
        line_numbers: setting(
            "line_numbers",
            false,
            file.line_numbers,
            cli.line_numbers,
            env,
        )?,
        color: setting("color", ColorMode::Auto, file.color, cli.color, env)?,
        pager: setting("pager", false, file.pager, cli.pager, env)?,
        all: setting("all", false, file.all, cli.all, env)?,
        mouse: setting("mouse", false, file.mouse, cli.mouse, env)?,
        logo: setting("logo", true, file.logo, cli.logo, env)?,
        images: setting("images", true, file.images, cli.images, env)?,
        interpreters: file.interpreters,
    })
}

/// One setting from its sources; the environment variable is
/// `BASTIAN_<KEY>`.
fn setting<T: Value>(
    key: &str,
    default: T,
    file: Option<T>,
    cli: Option<T>,
    env: &EnvMap,
) -> Result<Setting<T>> {
    let var = format!("BASTIAN_{}", key.to_ascii_uppercase());
    let env = match env.get(&var).map(|v| v.trim()) {
        None | Some("") => None,
        Some(v) => Some(
            T::parse_env(v)
                .ok_or_else(|| anyhow!("{var}={v:?} is not valid; expected {}", T::expected()))?,
        ),
    };
    Ok(pick(default, file, env, cli))
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

/// Write `theme = "<name>"` into the config file at `path`, replacing a
/// top-level `theme` line if there is one and keeping everything else
/// (comments, other keys, tables) as it is. Creates the file if needed.
pub fn save_theme(path: &Path, name: &str) -> Result<()> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    };
    let updated = with_theme(&text, name);
    // Never write something that no longer parses.
    toml::from_str::<Config>(&updated)
        .with_context(|| format!("updating {} would break it", path.display()))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    std::fs::write(path, updated).with_context(|| format!("writing {}", path.display()))
}

/// `text` with its top-level `theme` set to `name`.
fn with_theme(text: &str, name: &str) -> String {
    let line = format!("theme = {name:?}");
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    // Top-level keys come before the first `[table]`.
    let top = lines
        .iter()
        .position(|l| l.trim_start().starts_with('['))
        .unwrap_or(lines.len());
    let existing = lines[..top].iter().position(|l| {
        let l = l.trim_start();
        l.strip_prefix("theme")
            .is_some_and(|rest| rest.trim_start().starts_with('='))
    });
    if let Some(i) = existing {
        // Keep a trailing comment on the line.
        let comment = lines[i]
            .find('#')
            .map(|h| lines[i][lines[i][..h].trim_end().len()..].to_string());
        lines[i] = format!("{line}{}", comment.unwrap_or_default());
    } else {
        // After any leading comment block, before the first key or table.
        let at = lines[..top]
            .iter()
            .position(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty())
            .unwrap_or(top);
        lines.insert(at, line);
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// Name of the theme to use. `auto` follows the terminal background
/// (`light_background`, unknown counts as dark); `dark` and `light` pick
/// `dark_theme` and `light_theme`.
pub fn theme_name(r: &Resolved, light_background: bool) -> &str {
    match r.theme.value.as_str() {
        "auto" if light_background => &r.light_theme.value,
        "auto" | "dark" => &r.dark_theme.value,
        "light" => &r.light_theme.value,
        name => name,
    }
}

/// Every theme the settings name exists; the error says where a bad name
/// came from.
pub fn check_themes(r: &Resolved, exists: impl Fn(&str) -> bool) -> Result<()> {
    let named = ["auto", "dark", "light"];
    let settings = [
        ("theme", &r.theme),
        ("dark_theme", &r.dark_theme),
        ("light_theme", &r.light_theme),
    ];
    for (key, s) in settings {
        if (key == "theme" && named.contains(&s.value.as_str())) || exists(&s.value) {
            continue;
        }
        let from = match s.source {
            Source::Flag => "--theme".to_string(),
            Source::Env => format!("BASTIAN_{}", key.to_ascii_uppercase()),
            Source::File => format!("`{key}` in the config file"),
            Source::Default => "the default".to_string(),
        };
        return Err(anyhow!(
            "unknown theme {:?} (from {from}); run `bastian themes` to list them",
            s.value
        ));
    }
    Ok(())
}

/// A setting's value type: how it reads from the environment and prints
/// in `--print-config`.
trait Value: Sized {
    fn parse_env(v: &str) -> Option<Self>;
    /// What a valid value looks like, for error messages.
    fn expected() -> String;
    fn toml(&self) -> String;
}

impl Value for bool {
    fn parse_env(v: &str) -> Option<Self> {
        match v.to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        }
    }
    fn expected() -> String {
        "true or false".into()
    }
    fn toml(&self) -> String {
        self.to_string()
    }
}

impl Value for String {
    fn parse_env(v: &str) -> Option<Self> {
        Some(v.to_string())
    }
    fn expected() -> String {
        "a theme name".into()
    }
    fn toml(&self) -> String {
        format!("{self:?}")
    }
}

impl Value for u16 {
    fn parse_env(v: &str) -> Option<Self> {
        v.parse().ok()
    }
    fn expected() -> String {
        "a number of columns".into()
    }
    fn toml(&self) -> String {
        self.to_string()
    }
}

macro_rules! enum_value {
    ($($t:ty),*) => {$(
        impl Value for $t {
            fn parse_env(v: &str) -> Option<Self> {
                <$t as ValueEnum>::from_str(v, true).ok()
            }
            fn expected() -> String {
                let names: Vec<String> = <$t as ValueEnum>::value_variants()
                    .iter()
                    .map(enum_name)
                    .collect();
                match names.split_last() {
                    Some((last, [])) => last.clone(),
                    Some((last, rest)) => format!("{} or {last}", rest.join(", ")),
                    None => String::new(),
                }
            }
            fn toml(&self) -> String {
                format!("{:?}", enum_name(self))
            }
        }
    )*};
}
enum_value!(Confirm, ColorMode);

fn enum_name<T: ValueEnum>(v: &T) -> String {
    v.to_possible_value()
        .map_or_else(String::new, |p| p.get_name().to_string())
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
            theme: Some("light".into()),
            width: Some(80),
            ..Config::default()
        };
        let none = Config::default();

        let r = resolve(Config::default(), &env(&[]), none.clone()).unwrap();
        assert_eq!(
            r.theme,
            Setting {
                value: "auto".to_string(),
                source: Source::Default
            }
        );
        assert_eq!(r.width.value, 100);

        let r = resolve(file(), &env(&[]), none.clone()).unwrap();
        assert_eq!(
            r.theme,
            Setting {
                value: "light".to_string(),
                source: Source::File
            }
        );

        let e = env(&[("BASTIAN_THEME", "dark"), ("BASTIAN_WIDTH", "120")]);
        let r = resolve(file(), &e, none).unwrap();
        assert_eq!(
            r.theme,
            Setting {
                value: "dark".to_string(),
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

        let cli = Config {
            theme: Some("light".into()),
            width: Some(60),
            confirm: Some(Confirm::Never),
            line_numbers: Some(true),
            ..Config::default()
        };
        let r = resolve(file(), &e, cli).unwrap();
        assert_eq!(
            r.theme,
            Setting {
                value: "light".to_string(),
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
        let r = resolve(file, &env(&[("BASTIAN_THEME", "light")]), Config::default()).unwrap();
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
        let r = resolve(Config::default(), &e, Config::default()).unwrap();
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
            Config::default(),
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
            mouse = true
            [interpreters]
            ruby = ["ruby", "-e"]
            fish = []
            "#,
        )
        .unwrap();
        assert_eq!(c.theme, Some("light".into()));
        assert_eq!(c.width, Some(80));
        assert_eq!(c.confirm, Some(Confirm::Never));
        assert_eq!(c.line_numbers, Some(true));
        assert_eq!(c.color, Some(ColorMode::Never));
        assert_eq!(c.pager, Some(true));
        assert_eq!(c.mouse, Some(true));
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

    #[test]
    fn theme_name_follows_auto_dark_and_light() {
        let r = |theme: &str| {
            resolve(
                Config {
                    theme: Some(theme.into()),
                    dark_theme: Some("nord".into()),
                    ..Config::default()
                },
                &env(&[]),
                Config::default(),
            )
            .unwrap()
        };
        assert_eq!(theme_name(&r("auto"), false), "nord");
        assert_eq!(theme_name(&r("auto"), true), "rose-pine-dawn");
        assert_eq!(theme_name(&r("dark"), true), "nord");
        assert_eq!(theme_name(&r("light"), false), "rose-pine-dawn");
        assert_eq!(theme_name(&r("dracula"), true), "dracula");
    }

    #[test]
    fn unknown_themes_name_their_source() {
        let exists = |n: &str| ["rose-pine", "rose-pine-dawn", "nord"].contains(&n);
        let check = |file: Config, e: &[(&str, &str)], cli: Config| {
            check_themes(&resolve(file, &env(e), cli).unwrap(), exists)
        };
        assert!(check(Config::default(), &[], Config::default()).is_ok());
        let flag = Config {
            theme: Some("nope".into()),
            ..Config::default()
        };
        let err = check(Config::default(), &[], flag).unwrap_err().to_string();
        assert!(
            err.contains("unknown theme \"nope\" (from --theme)"),
            "{err}"
        );
        let err = check(
            Config::default(),
            &[("BASTIAN_LIGHT_THEME", "x")],
            Config::default(),
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("(from BASTIAN_LIGHT_THEME)"), "{err}");
        let file = Config {
            dark_theme: Some("y".into()),
            ..Config::default()
        };
        let err = check(file, &[], Config::default()).unwrap_err().to_string();
        assert!(
            err.contains("(from `dark_theme` in the config file)"),
            "{err}"
        );
    }

    #[test]
    fn saving_a_theme_keeps_the_rest_of_the_file() {
        assert_eq!(with_theme("", "nord"), "theme = \"nord\"\n");
        assert_eq!(
            with_theme("# mine\n\nwidth = 80\n[interpreters]\ntheme = []\n", "nord"),
            "# mine\n\ntheme = \"nord\"\nwidth = 80\n[interpreters]\ntheme = []\n"
        );
        assert_eq!(
            with_theme(
                "width = 80\ntheme = \"auto\"  # follow the terminal\n",
                "dracula"
            ),
            "width = 80\ntheme = \"dracula\"  # follow the terminal\n"
        );
        // The commented template line stays a comment.
        let t = with_theme(TEMPLATE, "nord");
        assert!(t.contains("# theme = \"auto\""), "{t}");
        let c: Config = toml::from_str(&t).unwrap();
        assert_eq!(c.theme.as_deref(), Some("nord"));

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub/config.toml");
        save_theme(&path, "kanagawa").unwrap();
        save_theme(&path, "nord").unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "theme = \"nord\"\n"
        );
    }
}
