use std::io::{IsTerminal, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};

use bastian::render::theme::Theme;
use bastian::{app, browser, doc, reader, render};

/// Read Markdown in the terminal, and run its code blocks.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Markdown file to read, `-` for stdin, or a directory to browse.
    /// Defaults to browsing the current directory.
    path: Option<PathBuf>,

    /// Maximum width of the rendered document.
    #[arg(short, long, default_value_t = 100)]
    width: u16,

    /// Color theme.
    #[arg(short, long, value_enum, default_value_t = ThemeName::Dark)]
    theme: ThemeName,

    /// Run code blocks without asking first. Blocks marked `confirm` still ask.
    #[arg(short, long)]
    yes: bool,

    /// Print the rendered document instead of opening the TUI.
    #[arg(short, long)]
    print: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum ThemeName {
    Dark,
    Light,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let theme = match cli.theme {
        ThemeName::Dark => Theme::dark(),
        ThemeName::Light => Theme::light(),
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
        app::App::browse(path, theme, cli.width, cli.yes)
    } else {
        let (name, src) = read_source(&path)?;
        let doc = doc::parse(&src);
        if !tui {
            let rendered = render::render(&doc, cli.width, &theme);
            println!("{}", render::to_plain(&rendered.lines));
            return Ok(());
        }
        let mut reader = reader::Reader::new(name, doc, theme, cli.width);
        reader.assume_yes = cli.yes;
        if path.as_os_str() != "-" {
            reader.base_dir = app::base_dir(&path);
        }
        app::App::read(reader)
    };

    let mut terminal = ratatui::init();
    let result = app::run(&mut terminal, app);
    ratatui::restore();
    result
}

fn read_source(path: &Path) -> Result<(String, String)> {
    if path.as_os_str() == "-" {
        let mut src = String::new();
        std::io::stdin()
            .read_to_string(&mut src)
            .context("reading stdin")?;
        return Ok(("stdin".to_string(), src));
    }
    let src =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    Ok((path.display().to_string(), src))
}
