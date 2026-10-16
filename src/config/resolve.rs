use std::collections::HashMap;

use anyhow::{Result, anyhow};

use super::value::Value;
use super::{
    ColorMode, Config, Confirm, DATE_FORMAT, EXTENSIONS, FrontMatter, ListLayout, PageBackground,
    Selector, SortBy,
};
use crate::themes;

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

/// The environment variable for setting `key`: `BASTIAN_<KEY>`.
pub fn env_var(key: &str) -> String {
    format!("BASTIAN_{}", key.to_ascii_uppercase())
}

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
    pub background: Setting<PageBackground>,
    pub width: Setting<u16>,
    pub confirm: Setting<Confirm>,
    pub line_numbers: Setting<bool>,
    pub color: Setting<ColorMode>,
    pub pager: Setting<bool>,
    pub all: Setting<bool>,
    pub mouse: Setting<bool>,
    pub front_matter: Setting<FrontMatter>,
    pub logo: Setting<bool>,
    pub images: Setting<bool>,
    pub history: Setting<bool>,
    pub selector: Setting<Selector>,
    pub list_layout: Setting<ListLayout>,
    pub sort: Setting<SortBy>,
    pub dates: Setting<bool>,
    pub date_format: Setting<String>,
    pub extensions: Setting<Vec<String>>,
    pub ignore: Setting<Vec<String>>,
    pub image_height: Setting<u16>,
    pub tab_width: Setting<u16>,
    pub hyperlinks: Setting<bool>,
    pub scrollbar: Setting<bool>,
    pub remember_position: Setting<bool>,
    /// Empty when not set.
    pub editor: Setting<String>,
    pub interpreters: HashMap<String, Vec<String>>,
}

/// Merge defaults < config file < `BASTIAN_*` environment < flags.
/// `cli` holds the settings given as flags (its `interpreters` are unused).
/// Invalid environment values are errors naming the variable.
pub fn resolve(file: Config, env: &EnvMap, cli: Config) -> Result<Resolved> {
    // Every setting's key is its field's name.
    macro_rules! set {
        ($key:ident, $default:expr) => {
            setting(stringify!($key), $default, file.$key, cli.$key, env)?
        };
    }
    Ok(Resolved {
        theme: set!(theme, "auto".into()),
        dark_theme: set!(dark_theme, themes::DEFAULT_DARK.into()),
        light_theme: set!(light_theme, themes::DEFAULT_LIGHT.into()),
        background: set!(background, PageBackground::default()),
        width: set!(width, 100),
        confirm: set!(confirm, Confirm::Always),
        line_numbers: set!(line_numbers, false),
        color: set!(color, ColorMode::Auto),
        pager: set!(pager, false),
        all: set!(all, false),
        mouse: set!(mouse, false),
        front_matter: set!(front_matter, FrontMatter::default()),
        logo: set!(logo, true),
        images: set!(images, true),
        history: set!(history, true),
        selector: set!(selector, Selector::default()),
        list_layout: set!(list_layout, ListLayout::default()),
        sort: set!(sort, SortBy::default()),
        dates: set!(dates, true),
        date_format: set!(date_format, DATE_FORMAT.into()),
        extensions: set!(
            extensions,
            EXTENSIONS.iter().map(ToString::to_string).collect()
        ),
        ignore: set!(ignore, Vec::new()),
        image_height: set!(image_height, 24),
        tab_width: set!(tab_width, 4),
        hyperlinks: set!(hyperlinks, true),
        scrollbar: set!(scrollbar, true),
        remember_position: set!(remember_position, true),
        editor: set!(editor, String::new()),
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
    let var = env_var(key);
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
