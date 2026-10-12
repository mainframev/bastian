mod date;
mod describe;
mod file;
mod resolve;
mod theme;
mod value;

pub use date::{check_date_format, format_time};
pub use describe::describe;
pub use file::{TEMPLATE, load_at, load_from, locate, path};
pub use resolve::{EnvMap, Resolved, Setting, Source, env_var, env_vars, resolve};
pub use theme::{check_themes, save_theme, theme_name};

use std::collections::HashMap;

use serde::Deserialize;

/// `theme` values that pick `dark_theme` or `light_theme` rather than
/// naming a theme.
pub const THEME_MODES: [&str; 3] = ["auto", "dark", "light"];

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
    /// How the file browser marks the selected file.
    pub selector: Option<Selector>,
    /// How the file list lays out each file.
    pub list_layout: Option<ListLayout>,
    /// Order of the files in the browser.
    pub sort: Option<SortBy>,
    /// Show each file's date in the browser.
    pub dates: Option<bool>,
    /// strftime format of those dates.
    pub date_format: Option<String>,
    /// Extensions of the files the browser lists.
    pub extensions: Option<Vec<String>>,
    /// Gitignore-style globs of paths the browser leaves out.
    pub ignore: Option<Vec<String>>,
    /// Pictures are scaled down to at most this many rows.
    pub image_height: Option<u16>,
    /// Columns a tab in code takes up.
    pub tab_width: Option<u16>,
    /// Make links clickable terminal hyperlinks (OSC 8).
    pub hyperlinks: Option<bool>,
    /// Show scrollbars.
    pub scrollbar: Option<bool>,
    /// Reopen files where they were last read.
    pub remember_position: Option<bool>,
    /// Editor command line, used before `$VISUAL` and `$EDITOR`.
    pub editor: Option<String>,
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Selector {
    // `│` beside the selected file (the default), `┃`, `▌`, `❯` beside its
    // title only, or a highlighted background. Plain comments: doc
    // comments would switch `--help` to clap's long layout.
    #[default]
    Bar,
    Thick,
    Block,
    Arrow,
    Highlight,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ListLayout {
    // Title, path and date on rows of their own (the default), or one row
    // per file.
    #[default]
    Cards,
    Compact,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum SortBy {
    // Last modified first (the default), created first, or by name.
    #[default]
    Modified,
    Created,
    Name,
}

/// The browser's dates when nothing else is set: `06 Aug 2026 09:06 CEST`.
pub const DATE_FORMAT: &str = "%d %b %Y %H:%M %Z";

/// The Markdown extensions the browser lists by default.
pub const EXTENSIONS: &[&str] = &["md", "markdown", "mdx", "mdown", "mkdn", "mkd"];

#[cfg(test)]
mod tests;
