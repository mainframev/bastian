use std::io::{IsTerminal, Read};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

use bastian::app::{self, Settings};
use bastian::config::{self, CliOverrides, ColorMode, Confirm, ThemeName};
use bastian::exec::Interpreters;
use bastian::render::theme::Theme;
use bastian::term::{self, Background};
use bastian::{browser, doc, output, remote, render, text};

/// Read Markdown in the terminal, and run its code blocks.
#[derive(Parser)]
#[command(version, after_help = config_help(), args_conflicts_with_subcommands = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Markdown file to read, `-` for stdin, a directory to browse, an
    /// http(s) URL, or github.com/owner/repo or gitlab.com/group/project for
    /// a repository's README. Defaults to browsing the current directory.
    path: Option<PathBuf>,

    /// Maximum width of the rendered document [default: 100].
    #[arg(short, long)]
    width: Option<u16>,

    /// Color theme [default: auto, matching the terminal background].
    #[arg(short, long, value_enum)]
    theme: Option<ThemeName>,

    /// Run code blocks without asking first. Blocks marked `confirm` still ask.
    #[arg(short, long)]
    yes: bool,

    /// Also list hidden and gitignored files in the browser.
    #[arg(short, long)]
    all: bool,

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

    /// Config file to use instead of the default (also BASTIAN_CONFIG).
    /// `~` and `$VAR` are expanded.
    #[arg(long, value_name = "PATH")]
    config: Option<String>,

    /// Print the effective settings and where each came from, then exit.
    #[arg(long)]
    print_config: bool,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Open the config file in your editor, creating it from a commented
    /// template if it doesn't exist. (To read a file named `config`, use
    /// `./config`.)
    Config,
}

fn config_help() -> String {
    match config::path() {
        Some(p) => format!("Config file: {}", p.display()),
        None => "Config file: none (HOME is not set)".to_string(),
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let lookup = |k: &str| std::env::var(k).ok();
    let located = config::locate(
        cli.config.as_deref(),
        std::env::var("BASTIAN_CONFIG").ok().as_deref(),
        &lookup,
    );
    if let Some(Command::Config) = cli.command {
        let (path, _) = located.context("no config location: HOME is not set")?;
        return edit_config(&path);
    }
    let file = match &located {
        Some((path, explicit)) => config::load_at(path, *explicit)?,
        None => config::Config::default(),
    };
    let resolved = config::resolve(
        file,
        &config::env_vars(),
        CliOverrides {
            theme: cli.theme,
            width: cli.width,
            confirm: cli.yes.then_some(Confirm::Never),
            line_numbers: cli.line_numbers.then_some(true),
            color: cli.color,
            pager: cli.pager.then_some(true),
            all: cli.all.then_some(true),
        },
    )?;
    if cli.print_config {
        print!(
            "{}",
            config::describe(&resolved, located.as_ref().map(|(p, _)| p.as_path()))
        );
        return Ok(());
    }
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

    // A URL or github.com/gitlab.com repository, unless a local file of
    // that name exists.
    let remote = (!path.exists())
        .then(|| path.to_str().and_then(remote::classify))
        .flatten();
    let app = if let Some(source) = remote {
        let fetched = remote::fetch(&source)?;
        if !tui {
            let fake = PathBuf::from(fetched.base.path());
            return print(&doc::parse_for(&fetched.text, Some(&fake)), &settings, &out);
        }
        let reader = settings.remote(fetched);
        app::App::read(reader, settings)
    } else if path.is_dir() {
        if !tui {
            for e in browser::discover(&path, settings.all) {
                println!("{}", e.rel);
            }
            return Ok(());
        }
        app::App::browse(path, settings)
    } else if path.as_os_str() == "-" {
        let mut bytes = Vec::new();
        std::io::stdin()
            .read_to_end(&mut bytes)
            .context("reading stdin")?;
        let src = text::decode(&bytes).context("reading stdin")?;
        let doc = doc::parse(&src);
        if !tui {
            return print(&doc, &settings, &out);
        }
        let mut reader = settings.reader("stdin".into(), doc, PathBuf::from("."));
        reader.text = src;
        app::App::read(reader, settings)
    } else {
        if !tui {
            let src = text::read_text(&path)?;
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

/// Open the config file in the editor, writing the template first if it's
/// missing.
fn edit_config(path: &std::path::Path) -> Result<()> {
    if !path.exists() {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        std::fs::write(path, config::TEMPLATE)
            .with_context(|| format!("writing {}", path.display()))?;
        eprintln!("created {}", path.display());
    }
    let cmd = bastian::editor::command();
    let status =
        bastian::editor::open_with(&cmd, path, None).with_context(|| format!("running {cmd}"))?;
    anyhow::ensure!(status.success(), "{cmd} exited with {status}");
    Ok(())
}

/// Turn resolved configuration into what the app runs with.
fn settings(r: config::Resolved) -> Settings {
    Settings {
        theme: match r.theme.value {
            ThemeName::Dark => Theme::dark(),
            ThemeName::Light => Theme::light(),
            // One detection decides both Markdown and code-block colors.
            ThemeName::Auto => match term::background() {
                Some(Background::Light) => Theme::light(),
                _ => Theme::dark(),
            },
        },
        max_width: r.width.value,
        assume_yes: r.confirm.value == Confirm::Never,
        line_numbers: r.line_numbers.value,
        all: r.all.value,
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
