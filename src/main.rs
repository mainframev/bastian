use std::io::{IsTerminal, Read};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

use bastian::app::{self, Settings};
use bastian::config::{self, Confirm, ThemeName};
use bastian::exec::Interpreters;
use bastian::render::theme::Theme;
use bastian::{browser, doc, render};

/// Read Markdown in the terminal, and run its code blocks.
#[derive(Parser)]
#[command(version, after_help = config_help())]
struct Cli {
    /// Markdown file to read, `-` for stdin, or a directory to browse.
    /// Defaults to browsing the current directory.
    path: Option<PathBuf>,

    /// Maximum width of the rendered document [default: 100].
    #[arg(short, long)]
    width: Option<u16>,

    /// Color theme [default: dark].
    #[arg(short, long, value_enum)]
    theme: Option<ThemeName>,

    /// Run code blocks without asking first. Blocks marked `confirm` still ask.
    #[arg(short, long)]
    yes: bool,

    /// Show line numbers in the reader.
    #[arg(short = 'n', long)]
    line_numbers: bool,

    /// Print the rendered document instead of opening the TUI.
    #[arg(short, long)]
    print: bool,
}

fn config_help() -> String {
    match config::path() {
        Some(p) => format!("Config file: {}", p.display()),
        None => "Config file: none (HOME is not set)".to_string(),
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = config::load()?;

    // Command-line flags win over the config file.
    let theme = match cli.theme.or(cfg.theme).unwrap_or(ThemeName::Dark) {
        ThemeName::Dark => Theme::dark(),
        ThemeName::Light => Theme::light(),
    };
    let settings = Settings {
        theme,
        max_width: cli.width.or(cfg.width).unwrap_or(100),
        assume_yes: cli.yes || cfg.confirm == Some(Confirm::Never),
        line_numbers: cli.line_numbers || cfg.line_numbers == Some(true),
        interpreters: Interpreters::new(cfg.interpreters),
    };

    let tui = !cli.print && std::io::stdout().is_terminal();
    let path = cli.path.unwrap_or_else(|| PathBuf::from("."));

    let app = if path.is_dir() {
        if !tui {
            for e in browser::discover(&path) {
                println!("{}", e.rel);
            }
            return Ok(());
        }
        app::App::browse(path, settings)
    } else if path.as_os_str() == "-" {
        let mut src = String::new();
        std::io::stdin()
            .read_to_string(&mut src)
            .context("reading stdin")?;
        let doc = doc::parse(&src);
        if !tui {
            return print(&doc, &settings);
        }
        let reader = settings.reader("stdin".into(), doc, PathBuf::from("."));
        app::App::read(reader, settings)
    } else {
        if !tui {
            let src = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            return print(&doc::parse_for(&src, Some(&path)), &settings);
        }
        let reader = settings.open(&path, path.display().to_string())?;
        app::App::read(reader, settings)
    };

    let mut terminal = ratatui::init();
    let result = app::run(&mut terminal, app);
    ratatui::restore();
    result
}

fn print(doc: &doc::Document, settings: &Settings) -> Result<()> {
    let rendered = render::render(doc, settings.max_width, &settings.theme);
    println!("{}", render::to_plain(&rendered.lines));
    Ok(())
}
