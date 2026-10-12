//! Command-line arguments.

use std::path::PathBuf;

use clap::Parser;

use crate::config::{self, ColorMode, Config, Confirm};
use crate::themes;

/// Read Markdown in the terminal: browse, search and follow your docs.
#[derive(Parser)]
#[command(version, after_help = config_help(), args_conflicts_with_subcommands = true)]
#[allow(
    clippy::doc_markdown,
    reason = "clap prints these doc comments as --help text, where backticks would show"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Markdown file to read, `-` for stdin, a directory to browse, an
    /// http(s) URL, or github.com/owner/repo or gitlab.com/group/project for
    /// a repository's README. Defaults to browsing the current directory.
    pub path: Option<PathBuf>,

    /// Maximum width of the rendered document [default: 100].
    #[arg(short, long)]
    pub width: Option<u16>,

    /// Color theme: auto (match the terminal background), dark, light, or a
    /// theme name; `bastian themes` lists them [default: auto].
    #[arg(short, long, value_name = "NAME", value_parser = ThemeArg, hide_possible_values = true)]
    pub theme: Option<String>,

    /// Run code blocks without asking first. Blocks marked `confirm` still ask.
    #[arg(short, long)]
    pub yes: bool,

    /// Also list hidden and gitignored files in the browser.
    #[arg(short, long)]
    pub all: bool,

    /// Show line numbers in the reader.
    #[arg(short = 'n', long)]
    pub line_numbers: bool,

    /// Print the rendered document instead of opening the TUI.
    #[arg(short, long)]
    pub print: bool,

    /// Show the rendered document in $PAGER (default `less -R`) instead of
    /// the TUI.
    #[arg(long)]
    pub pager: bool,

    /// Color printed output: auto colors on a terminal, honoring NO_COLOR and
    /// CLICOLOR_FORCE [default: auto].
    #[arg(long, value_enum)]
    pub color: Option<ColorMode>,

    /// Config file to use instead of the default (also BASTIAN_CONFIG).
    /// `~` and `$VAR` are expanded.
    #[arg(long, value_name = "PATH")]
    pub config: Option<String>,

    /// Scroll with the mouse wheel. Captures the mouse, so selecting text in
    /// the terminal needs a modifier key (often shift or option).
    #[arg(long)]
    pub mouse: bool,

    /// Hide the logo at the top of the file browser.
    #[arg(long)]
    pub no_logo: bool,

    /// Draw pictures for images in documents, where the terminal can
    /// (overrides `images = false`; `i` toggles them in the reader).
    #[arg(long, overrides_with = "no_images")]
    pub images: bool,

    /// Show images in documents as their alt text instead of pictures.
    #[arg(long, overrides_with = "images")]
    pub no_images: bool,

    /// How the file browser marks the selected file [default: bar].
    #[arg(long, value_enum, value_name = "STYLE")]
    pub selector: Option<config::Selector>,

    /// How the file list lays out each file [default: cards].
    #[arg(long, value_enum, value_name = "LAYOUT")]
    pub list_layout: Option<config::ListLayout>,

    /// Order of the files in the browser [default: modified].
    #[arg(long, value_enum, value_name = "ORDER")]
    pub sort: Option<config::SortBy>,

    /// Print the effective settings and where each came from, then exit.
    #[arg(long)]
    pub print_config: bool,
}

#[derive(clap::Subcommand)]
pub enum Command {
    /// Open the config file in your editor, creating it from a commented
    /// template if it doesn't exist. (To read a file named `config`, use
    /// `./config`.)
    Config,
    /// Print a shell completion script, e.g. `bastian completions zsh`.
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    /// Pick a theme with a live preview and save it to the config file
    /// (lists the themes instead when output isn't a terminal).
    Themes,
}

fn config_help() -> String {
    match config::path() {
        Some(p) => format!("Config file: {}", p.display()),
        None => "Config file: none (HOME is not set)".to_string(),
    }
}

/// `--theme` accepts any name (checked later against the registry) but
/// offers the built-ins to shell completion.
#[derive(Clone)]
pub struct ThemeArg;

impl clap::builder::TypedValueParser for ThemeArg {
    type Value = String;

    fn parse_ref(
        &self,
        _cmd: &clap::Command,
        _arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<String, clap::Error> {
        value
            .to_str()
            .map(str::to_string)
            .ok_or_else(|| clap::Error::new(clap::error::ErrorKind::InvalidUtf8))
    }

    fn possible_values(
        &self,
    ) -> Option<Box<dyn Iterator<Item = clap::builder::PossibleValue> + '_>> {
        let names = config::THEME_MODES
            .into_iter()
            .chain(themes::builtin_names());
        Some(Box::new(names.map(clap::builder::PossibleValue::new)))
    }
}

impl Cli {
    /// The settings given as flags, as a config layer above the file and
    /// the environment.
    pub fn overrides(&self) -> Config {
        Config {
            theme: self.theme.clone(),
            width: self.width,
            confirm: self.yes.then_some(Confirm::Never),
            line_numbers: self.line_numbers.then_some(true),
            color: self.color,
            pager: self.pager.then_some(true),
            all: self.all.then_some(true),
            mouse: self.mouse.then_some(true),
            logo: self.no_logo.then_some(false),
            images: match (self.images, self.no_images) {
                (true, _) => Some(true),
                (_, true) => Some(false),
                _ => None,
            },
            selector: self.selector,
            list_layout: self.list_layout,
            sort: self.sort,
            ..Config::default()
        }
    }
}
