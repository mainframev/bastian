use std::io::{IsTerminal, Read};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

use bastian::app::{self, Settings};
use bastian::config::{self, CliOverrides, ColorMode, Confirm, ThemeName};
use bastian::exec::Interpreters;
use bastian::render::theme::Theme;
use bastian::{browser, doc, output, render};

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

    /// Show the rendered document in $PAGER (default `less -R`) instead of
    /// the TUI.
    #[arg(long)]
    pager: bool,

    /// Color printed output [default: auto; honors NO_COLOR and CLICOLOR_FORCE].
    #[arg(long, value_enum)]
    color: Option<ColorMode>,
}

fn config_help() -> String {
    match config::path() {
        Some(p) => format!("Config file: {}", p.display()),
        None => "Config file: none (HOME is not set)".to_string(),
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let resolved = config::resolve(
        config::load()?,
        &config::env_vars(),
        CliOverrides {
            theme: cli.theme,
            width: cli.width,
            confirm: cli.yes.then_some(Confirm::Never),
            line_numbers: cli.line_numbers.then_some(true),
            color: cli.color,
            pager: cli.pager.then_some(true),
        },
    )?;
    let stdout_tty = std::io::stdout().is_terminal();
    let out = Output {
        width: output::print_width(
            &resolved.width,
            ratatui::crossterm::terminal::size().ok().map(|(w, _)| w),
        ),
        // A pager shows colors itself, so `auto` colors when paging too.
        color: output::use_color(
            resolved.color.value,
            stdout_tty || resolved.pager.value,
            std::env::var("NO_COLOR").ok().as_deref(),
            std::env::var("CLICOLOR_FORCE").ok().as_deref(),
        ),
        pager: (resolved.pager.value && stdout_tty)
            .then(|| output::pager_command(std::env::var("PAGER").ok().as_deref())),
    };
    let settings = settings(resolved);

    let tui = !cli.print && out.pager.is_none() && stdout_tty;
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
            return print(&doc, &settings, &out);
        }
        let reader = settings.reader("stdin".into(), doc, PathBuf::from("."));
        app::App::read(reader, settings)
    } else {
        if !tui {
            let src = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            return print(&doc::parse_for(&src, Some(&path)), &settings, &out);
        }
        let reader = settings.open(&path, path.display().to_string())?;
        app::App::read(reader, settings)
    };

    let mut terminal = ratatui::init();
    let result = app::run(&mut terminal, app);
    ratatui::restore();
    result
}

/// Turn resolved configuration into what the app runs with.
fn settings(r: config::Resolved) -> Settings {
    Settings {
        theme: match r.theme.value {
            ThemeName::Dark => Theme::dark(),
            ThemeName::Light => Theme::light(),
        },
        max_width: r.width.value,
        assume_yes: r.confirm.value == Confirm::Never,
        line_numbers: r.line_numbers.value,
        interpreters: Interpreters::new(r.interpreters),
    }
}

/// How non-interactive output is written.
struct Output {
    width: u16,
    color: bool,
    /// Pager command when paging, `None` to write to stdout.
    pager: Option<String>,
}

fn print(doc: &doc::Document, settings: &Settings, out: &Output) -> Result<()> {
    let rendered = render::render(doc, out.width, &settings.theme);
    let text = if out.color {
        render::to_ansi(&rendered.lines)
    } else {
        render::to_plain(&rendered.lines)
    };
    match &out.pager {
        Some(pager) => output::page(&format!("{text}\n"), pager),
        None => {
            println!("{text}");
            Ok(())
        }
    }
}
