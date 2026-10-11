use std::io::{IsTerminal, Read};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser};

use bastian::app::{self, Settings};
use bastian::config::{self, ColorMode, Confirm};
use bastian::exec::Interpreters;

use bastian::term::{self, Background};
use bastian::{browser, doc, images, output, remote, render, text, themes};

/// Read Markdown in the terminal: browse, search and follow your docs.
#[derive(Parser)]
#[command(version, after_help = config_help(), args_conflicts_with_subcommands = true)]
#[allow(
    clippy::doc_markdown,
    reason = "clap prints these doc comments as --help text, where backticks would show"
)]
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

    /// Color theme: auto (match the terminal background), dark, light, or a
    /// theme name; `bastian themes` lists them [default: auto].
    #[arg(short, long, value_name = "NAME", value_parser = ThemeArg, hide_possible_values = true)]
    theme: Option<String>,

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

    /// Color printed output: auto colors on a terminal, honoring NO_COLOR and
    /// CLICOLOR_FORCE [default: auto].
    #[arg(long, value_enum)]
    color: Option<ColorMode>,

    /// Config file to use instead of the default (also BASTIAN_CONFIG).
    /// `~` and `$VAR` are expanded.
    #[arg(long, value_name = "PATH")]
    config: Option<String>,

    /// Scroll with the mouse wheel. Captures the mouse, so selecting text in
    /// the terminal needs a modifier key (often shift or option).
    #[arg(long)]
    mouse: bool,

    /// Hide the logo at the top of the file browser.
    #[arg(long)]
    no_logo: bool,

    /// Draw pictures for images in documents, where the terminal can
    /// (overrides `images = false`; `i` toggles them in the reader).
    #[arg(long, overrides_with = "no_images")]
    images: bool,

    /// Show images in documents as their alt text instead of pictures.
    #[arg(long, overrides_with = "images")]
    no_images: bool,

    /// How the file browser marks the selected file [default: bar].
    #[arg(long, value_enum, value_name = "STYLE")]
    selector: Option<config::Selector>,

    /// How the file browser lays out each file [default: cards].
    #[arg(long, value_enum, value_name = "LAYOUT")]
    browser_layout: Option<config::BrowserLayout>,

    /// Order of the files in the browser [default: modified].
    #[arg(long, value_enum, value_name = "ORDER")]
    sort: Option<config::SortBy>,

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
    /// Print a shell completion script, e.g. `bastian completions zsh`.
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    /// Print the man page (roff), e.g. `bastian man | man -l -`.
    Man,
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

fn main() -> Result<()> {
    // Exit quietly when a reader such as `head` closes our output, like
    // other command-line tools, instead of reporting a broken pipe.
    #[cfg(unix)]
    // SAFETY: restoring the default disposition of a standard signal at
    // startup, before any threads are spawned.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
    let cli = Cli::parse();
    let lookup = |k: &str| std::env::var(k).ok();
    let located = config::locate(
        cli.config.as_deref(),
        std::env::var("BASTIAN_CONFIG").ok().as_deref(),
        &lookup,
    );
    // User themes live in `themes/` beside the config file.
    let themes_dir = located
        .as_ref()
        .and_then(|(p, _)| p.parent())
        .map(|d| d.join("themes"));
    let list_themes = matches!(cli.command, Some(Command::Themes));
    match cli.command {
        Some(Command::Config) => {
            let (path, _) = located.context("no config location: HOME is not set")?;
            return edit_config(&path);
        }
        Some(Command::Completions { shell }) => {
            clap_complete::generate(
                shell,
                &mut Cli::command(),
                "bastian",
                &mut std::io::stdout(),
            );
            return Ok(());
        }
        Some(Command::Man) => {
            clap_mangen::Man::new(Cli::command()).render(&mut std::io::stdout())?;
            return Ok(());
        }
        Some(Command::Themes) | None => {}
    }
    let file = match &located {
        Some((path, explicit)) => config::load_at(path, *explicit)?,
        None => config::Config::default(),
    };
    let resolved = config::resolve(
        file,
        &config::env_vars(),
        config::Config {
            theme: cli.theme,
            width: cli.width,
            confirm: cli.yes.then_some(Confirm::Never),
            line_numbers: cli.line_numbers.then_some(true),
            color: cli.color,
            pager: cli.pager.then_some(true),
            all: cli.all.then_some(true),
            mouse: cli.mouse.then_some(true),
            logo: cli.no_logo.then_some(false),
            images: match (cli.images, cli.no_images) {
                (true, _) => Some(true),
                (_, true) => Some(false),
                _ => None,
            },
            selector: cli.selector,
            browser_layout: cli.browser_layout,
            sort: cli.sort,
            ..config::Config::default()
        },
    )?;
    let mut resolved = resolved;
    if cli.print_config {
        print!(
            "{}",
            config::describe(&resolved, located.as_ref().map(|(p, _)| p.as_path()))
        );
        return Ok(());
    }
    let stdout_tty = std::io::stdout().is_terminal();
    // In a terminal, `bastian themes` opens the picker; otherwise it lists.
    let pick_theme = list_themes && stdout_tty;
    if list_themes && !pick_theme {
        let color = output::use_color(
            resolved.color.value,
            stdout_tty,
            std::env::var("NO_COLOR").ok().as_deref(),
            std::env::var("CLICOLOR_FORCE").ok().as_deref(),
        );
        print!("{}", theme_list(&resolved, themes_dir.as_deref(), color));
        return Ok(());
    }
    let checked = config::check_themes(&resolved, |name| {
        themes::exists(name, themes_dir.as_deref())
    });
    if let Err(e) = checked {
        if !pick_theme {
            return Err(e);
        }
        // The picker is how a bad theme setting gets fixed: start from the
        // defaults instead of refusing to open.
        resolved.theme.value = "auto".into();
        resolved.dark_theme.value = "rose-pine".into();
        resolved.light_theme.value = "rose-pine-dawn".into();
    }
    let theme_files = app::ThemeFiles {
        dir: themes_dir.clone(),
        config: located.as_ref().map(|(p, _)| p.clone()),
        current: String::new(),
        overridden_by: match resolved.theme.source {
            config::Source::Flag => Some("--theme".into()),
            config::Source::Env => Some("BASTIAN_THEME".into()),
            _ => None,
        },
    };
    let out = Output {
        width: output::print_width(
            &resolved.width,
            crossterm::terminal::size().ok().map(|(w, _)| w),
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
    let tui = pick_theme || (!cli.print && out.pager.is_none() && stdout_tty);
    // Plain output has no colors, so it skips the background query.
    let (settings, current) = settings(resolved, themes_dir.as_deref(), tui || out.color, tui)?;
    let theme_files = app::ThemeFiles {
        current,
        ..theme_files
    };

    if pick_theme {
        let reader = settings.reader(
            "theme preview".into(),
            doc::parse(bastian::picker::SAMPLE),
            PathBuf::from("."),
        );
        let mut app = app::App::read(reader, settings);
        app.themes = theme_files;
        app.pick_and_exit = true;
        app.open_picker();
        return run_tui(app);
    }

    let path = cli.path.unwrap_or_else(|| PathBuf::from("."));

    // A URL or github.com/gitlab.com repository, unless a local file of
    // that name exists.
    let remote = (!path.exists())
        .then(|| path.to_str().and_then(remote::classify))
        .flatten();
    let app = if let Some(source) = remote {
        let fetched = remote::fetch(&source)?;
        if !tui {
            return print(&fetched.parse(), &settings, &out);
        }
        let reader = settings.remote(fetched);
        app::App::read(reader, settings)
    } else if path.is_dir() {
        if !tui {
            for e in browser::discover(&path, &settings.scan) {
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

    let mut app = app;
    app.themes = theme_files;
    run_tui(app)
}

fn run_tui(app: app::App) -> Result<()> {
    let mouse = app.settings.mouse;
    let mut terminal = app::enter_terminal(mouse)?;
    let result = app::run(&mut terminal, app);
    app::leave_terminal(mouse);
    if let Some(note) = result? {
        println!("{note}");
    }
    Ok(())
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

/// Turn resolved configuration into what the app runs with. `colored` says
/// whether the theme's colors will show; if not, `auto` doesn't ask the
/// terminal for its background.
fn settings(
    r: config::Resolved,
    themes_dir: Option<&std::path::Path>,
    colored: bool,
    tui: bool,
) -> Result<(Settings, String)> {
    let light = colored && r.theme.value == "auto" && term::background() == Some(Background::Light);
    let name = config::theme_name(&r, light).to_string();
    let scan = browser::Scan {
        all: r.all.value,
        sort: r.sort.value,
        extensions: r.extensions.value,
        ignore: r.ignore.value,
    };
    scan.overrides(std::path::Path::new("."))?;
    let date_format = r.dates.value.then_some(r.date_format.value);
    if let Some(f) = &date_format {
        bastian::ui::browser::check_date_format(f)?;
    }
    let settings = Settings {
        theme: themes::load(&name, themes_dir)?,
        max_width: r.width.value,
        assume_yes: r.confirm.value == Confirm::Never,
        line_numbers: r.line_numbers.value,
        scan,
        date_format,
        mouse: r.mouse.value,
        logo: r.logo.value,
        // Only the TUI draws pictures.
        images: (tui && r.images.value).then(images::Support::detect),
        show_images: r.images.value,
        selector: r.selector.value,
        browser_layout: r.browser_layout.value,
        interpreters: Interpreters::new(r.interpreters),
    };
    Ok((settings, name))
}

/// `bastian themes`: one row per theme with its appearance, where it comes
/// from, a color swatch when `color`, and which settings point at it.
fn theme_list(r: &config::Resolved, dir: Option<&std::path::Path>, color: bool) -> String {
    use ratatui::style::Color;
    let rows = themes::list(dir);
    let w = rows.iter().map(|l| l.name.len()).max().unwrap_or(0);
    let swatch = |c: Color| match c {
        Color::Rgb(r, g, b) => format!("\x1b[38;2;{r};{g};{b}m██\x1b[0m"),
        _ => String::new(),
    };
    let mut out = String::new();
    for l in rows {
        let roles: Vec<&str> = [
            ("theme", &r.theme.value),
            ("dark_theme", &r.dark_theme.value),
            ("light_theme", &r.light_theme.value),
        ]
        .into_iter()
        .filter(|(_, v)| themes::canonical(v) == l.name)
        .map(|(k, _)| k)
        .collect();
        // Paths under the home directory as `~/…`.
        let shown = |p: &std::path::Path| match std::env::var_os("HOME") {
            Some(home) if p.starts_with(&home) => {
                format!("~/{}", p.strip_prefix(&home).unwrap_or(p).display())
            }
            _ => p.display().to_string(),
        };
        let origin = match (&l.path, l.overrides) {
            (Some(p), true) => format!("{} (overrides built-in)", shown(p)),
            (Some(p), false) => shown(p),
            (None, _) => "built-in".to_string(),
        };
        let (appearance, colors) = match &l.theme {
            Ok(t) => {
                let appearance = match t.appearance {
                    themes::Appearance::Dark => "dark",
                    themes::Appearance::Light => "light",
                };
                let colors: String = [t.accent, t.heading, t.link, t.code, t.ok, t.warn, t.err]
                    .into_iter()
                    .map(swatch)
                    .collect();
                (
                    appearance,
                    if color {
                        format!("{colors}  ")
                    } else {
                        String::new()
                    },
                )
            }
            Err(_) => ("?", String::new()),
        };
        out.push_str(&format!(
            "{:<w$}  {appearance:<5}  {colors}{origin}",
            l.name
        ));
        if !roles.is_empty() {
            out.push_str(&format!("  ← {}", roles.join(", ")));
        }
        if let Err(e) = &l.theme {
            out.push_str(&format!("\n{:<w$}  error: {e:#}", ""));
        }
        out.push('\n');
    }
    out
}

/// `--theme` accepts any name (checked later against the registry) but
/// offers the built-ins to shell completion.
#[derive(Clone)]
struct ThemeArg;

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
        let names = ["auto", "dark", "light"]
            .into_iter()
            .chain(themes::builtin_names());
        Some(Box::new(names.map(clap::builder::PossibleValue::new)))
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
    if let Some(pager) = &out.pager {
        output::page(&format!("{text}\n"), pager)
    } else {
        println!("{text}");
        Ok(())
    }
}
