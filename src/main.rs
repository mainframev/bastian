use std::io::{IsTerminal, Read};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser};

use bastian::app::{self, Settings};
use bastian::cli::{Cli, Command};
use bastian::config;
use bastian::reader::{Reader, Source};
use bastian::{browser, doc, open, output, paths, remote, text, themes};

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
        Some(Command::Themes) | None => {}
    }
    let file = match &located {
        Some((path, explicit)) => config::load_at(path, *explicit)?,
        None => config::Config::default(),
    };
    let mut resolved = config::resolve(file, &config::env_vars(), cli.overrides())?;
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
    // The pager only runs on a terminal, so a tty stdout covers paging too;
    // piped output stays plain even with the pager switched on.
    let color = output::use_color(
        resolved.color.value,
        stdout_tty,
        std::env::var("NO_COLOR").ok().as_deref(),
        std::env::var("CLICOLOR_FORCE").ok().as_deref(),
    );
    if list_themes && !pick_theme {
        let roles = [
            ("theme", resolved.theme.value.as_str()),
            ("dark_theme", resolved.dark_theme.value.as_str()),
            ("light_theme", resolved.light_theme.value.as_str()),
        ];
        print!(
            "{}",
            themes::format_list(&roles, themes_dir.as_deref(), color)
        );
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
        resolved.dark_theme.value = themes::DEFAULT_DARK.into();
        resolved.light_theme.value = themes::DEFAULT_LIGHT.into();
    }
    let theme_source = resolved.theme.source;
    let overridden_by = matches!(theme_source, config::Source::Flag | config::Source::Env)
        .then(|| config::theme_origin("theme", theme_source));
    let out = output::Output {
        width: output::print_width(
            &resolved.width,
            crossterm::terminal::size().ok().map(|(w, _)| w),
        ),
        color,
        pager: (resolved.pager.value && stdout_tty)
            .then(|| output::pager_command(std::env::var("PAGER").ok().as_deref())),
    };
    let tui = pick_theme || (!cli.print && out.pager.is_none() && stdout_tty);
    // Plain output has no colors, so it skips the background query.
    let (settings, current) = Settings::from_resolved(
        resolved,
        themes_dir.as_deref(),
        tui || out.color,
        tui,
        stdout_tty,
    )?;
    let theme_files = app::ThemeFiles {
        dir: themes_dir,
        config: located.map(|(p, _)| p),
        current,
        overridden_by,
    };

    if pick_theme {
        let reader = Reader::new(
            Source::in_memory("theme preview", doc::parse(bastian::picker::SAMPLE)),
            &settings.reader,
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
    let print = |d: &doc::Document| out.print(d, &settings.reader.theme, settings.reader.render);
    let mut app = if let Some(source) = remote {
        let fetched = remote::fetch(&source)?;
        if !tui {
            return print(&fetched.parse());
        }
        let reader = Reader::new(Source::remote(fetched), &settings.reader);
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
            return print(&doc);
        }
        let source = Source {
            text: src,
            ..Source::in_memory("stdin", doc)
        };
        let reader = Reader::new(source, &settings.reader);
        app::App::read(reader, settings)
    } else {
        if !tui {
            let opened = open::open(&path)?;
            return print(&opened.doc);
        }
        let reader = settings.open(&path, path.display().to_string())?;
        app::App::read(reader, settings)
    };

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
        paths::write_creating_parent(path, config::TEMPLATE)?;
        eprintln!("created {}", path.display());
    }
    // The `editor` setting, when the file still parses (it may be open to
    // fix it).
    let configured = std::env::var("BASTIAN_EDITOR")
        .ok()
        .filter(|e| !e.trim().is_empty())
        .or_else(|| config::load_from(path).ok().and_then(|c| c.editor));
    let cmd = bastian::editor::command(configured.as_deref());
    let status =
        bastian::editor::open_with(&cmd, path, None).with_context(|| format!("running {cmd}"))?;
    anyhow::ensure!(status.success(), "{cmd} exited with {status}");
    Ok(())
}
