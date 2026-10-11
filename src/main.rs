use std::io::{IsTerminal, Read};
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Parser, ValueEnum};

use bastian::render::theme::Theme;
use bastian::{app, doc, render};

/// Read Markdown in the terminal, and run its code blocks.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Markdown file to read, or `-` for stdin. Defaults to README.md.
    path: Option<PathBuf>,

    /// Maximum width of the rendered document.
    #[arg(short, long, default_value_t = 100)]
    width: u16,

    /// Color theme.
    #[arg(short, long, value_enum, default_value_t = ThemeName::Dark)]
    theme: ThemeName,

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
    let (name, src) = read_source(cli.path)?;
    let doc = doc::parse(&src);
    let theme = match cli.theme {
        ThemeName::Dark => Theme::dark(),
        ThemeName::Light => Theme::light(),
    };

    if cli.print || !std::io::stdout().is_terminal() {
        let rendered = render::render(&doc, cli.width, &theme);
        println!("{}", render::to_plain(&rendered.lines));
        return Ok(());
    }

    let reader = app::Reader::new(name, doc, theme, cli.width);
    let mut terminal = ratatui::init();
    let result = app::run(&mut terminal, reader);
    ratatui::restore();
    result
}

fn read_source(path: Option<PathBuf>) -> Result<(String, String)> {
    let path = path.unwrap_or_else(|| PathBuf::from("README.md"));
    if path.as_os_str() == "-" {
        let mut src = String::new();
        std::io::stdin()
            .read_to_string(&mut src)
            .context("reading stdin")?;
        return Ok(("stdin".to_string(), src));
    }
    if path.is_dir() {
        bail!(
            "{} is a directory; the file browser is not implemented yet",
            path.display()
        );
    }
    let src =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    Ok((path.display().to_string(), src))
}
