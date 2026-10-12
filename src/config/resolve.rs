use std::collections::HashMap;

use anyhow::{Result, anyhow};

use super::value::Value;
use super::{ColorMode, Config, Confirm, DATE_FORMAT, EXTENSIONS, ListLayout, Selector, SortBy};
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
    pub width: Setting<u16>,
    pub confirm: Setting<Confirm>,
    pub line_numbers: Setting<bool>,
    pub color: Setting<ColorMode>,
    pub pager: Setting<bool>,
    pub all: Setting<bool>,
    pub mouse: Setting<bool>,
    pub logo: Setting<bool>,
    pub images: Setting<bool>,
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
    Ok(Resolved {
        theme: setting("theme", "auto".into(), file.theme, cli.theme, env)?,
        dark_theme: setting(
            "dark_theme",
            themes::DEFAULT_DARK.into(),
            file.dark_theme,
            cli.dark_theme,
            env,
        )?,
        light_theme: setting(
            "light_theme",
            themes::DEFAULT_LIGHT.into(),
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
        selector: setting(
            "selector",
            Selector::default(),
            file.selector,
            cli.selector,
            env,
        )?,
        list_layout: setting(
            "list_layout",
            ListLayout::default(),
            file.list_layout,
            cli.list_layout,
            env,
        )?,
        sort: setting("sort", SortBy::default(), file.sort, cli.sort, env)?,
        dates: setting("dates", true, file.dates, cli.dates, env)?,
        date_format: setting(
            "date_format",
            DATE_FORMAT.into(),
            file.date_format,
            cli.date_format,
            env,
        )?,
        extensions: setting(
            "extensions",
            EXTENSIONS.iter().map(ToString::to_string).collect(),
            file.extensions,
            cli.extensions,
            env,
        )?,
        ignore: setting("ignore", Vec::new(), file.ignore, cli.ignore, env)?,
        image_height: setting("image_height", 24, file.image_height, cli.image_height, env)?,
        tab_width: setting("tab_width", 4, file.tab_width, cli.tab_width, env)?,
        hyperlinks: setting("hyperlinks", true, file.hyperlinks, cli.hyperlinks, env)?,
        scrollbar: setting("scrollbar", true, file.scrollbar, cli.scrollbar, env)?,
        remember_position: setting(
            "remember_position",
            true,
            file.remember_position,
            cli.remember_position,
            env,
        )?,
        editor: setting("editor", String::new(), file.editor, cli.editor, env)?,
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
