// A test crate: helpers outside #[test] fns may unwrap too.
#![allow(clippy::unwrap_used)]

//! The command line, run as a user would: the built binary in a clean
//! environment with its own config directory.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

/// `bastian` with `args`, an empty config dir and none of the variables
/// that change its behavior.
fn bastian(config: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
    bastian_with_stdin(config, args, env, None)
}

fn bastian_with_stdin(
    config: &Path,
    args: &[&str],
    env: &[(&str, &str)],
    stdin: Option<&str>,
) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_bastian"));
    cmd.args(args)
        .env("XDG_CONFIG_HOME", config)
        .env_remove("NO_COLOR")
        .env_remove("CLICOLOR_FORCE")
        .env_remove("BASTIAN_CONFIG")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, _) in std::env::vars().filter(|(k, _)| k.starts_with("BASTIAN_")) {
        cmd.env_remove(k);
    }
    cmd.envs(env.iter().copied());
    let mut child = cmd.spawn().unwrap();
    if let Some(input) = stdin {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    drop(child.stdin.take());
    child.wait_with_output().unwrap()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[test]
fn short_and_long_help_match() {
    let dir = tempfile::tempdir().unwrap();
    let short = bastian(dir.path(), &["-h"], &[]);
    let long = bastian(dir.path(), &["--help"], &[]);
    assert!(short.status.success());
    assert_eq!(stdout(&short), stdout(&long));
    assert!(stdout(&short).contains("--images <WHEN>"));
}

#[test]
fn print_config_names_every_source() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("bastian")).unwrap();
    std::fs::write(dir.path().join("bastian/config.toml"), "width = 70\n").unwrap();
    let out = bastian(
        dir.path(),
        &["--print-config", "-t", "nord"],
        &[("BASTIAN_LOGO", "false")],
    );
    let text = stdout(&out);
    let line = |key: &str| {
        text.lines()
            .find(|l| l.starts_with(&format!("{key} =")))
            .unwrap_or_else(|| panic!("no {key} in {text}"))
            .to_string()
    };
    assert!(line("theme").ends_with("# flag"), "{text}");
    assert!(line("width").ends_with("# config file"), "{text}");
    assert!(line("logo").ends_with("# environment"), "{text}");
    assert!(line("mouse").ends_with("# default"), "{text}");
}

#[test]
fn images_flag_overrides_the_config_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("bastian")).unwrap();
    std::fs::write(dir.path().join("bastian/config.toml"), "images = false\n").unwrap();
    let images = |args: &[&str]| {
        let mut all = vec!["--print-config"];
        all.extend(args);
        let out = bastian(dir.path(), &all, &[]);
        stdout(&out)
            .lines()
            .find(|l| l.starts_with("images ="))
            .unwrap()
            .to_string()
    };
    assert!(images(&[]).starts_with("images = false"));
    assert!(images(&["--images", "on"]).starts_with("images = true"));
    assert!(images(&["--images=on"]).ends_with("# flag"));
    // The last one wins.
    assert!(images(&["--images", "on", "--images", "off"]).starts_with("images = false"));
    assert!(images(&["--images", "off", "--images", "on"]).starts_with("images = true"));
}

#[test]
fn bad_settings_say_where_they_came_from() {
    let dir = tempfile::tempdir().unwrap();
    let doc = dir.path().join("a.md");
    std::fs::write(&doc, "# A\n").unwrap();
    let doc = doc.to_str().unwrap();

    let out = bastian(dir.path(), &["-p", doc], &[("BASTIAN_WIDTH", "wide")]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("BASTIAN_WIDTH=\"wide\" is not valid"),
        "{}",
        stderr(&out)
    );

    let out = bastian(dir.path(), &["-p", doc], &[("BASTIAN_THEME", "nope")]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("unknown theme \"nope\" (from BASTIAN_THEME)"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn printing_is_plain_unless_color_is_asked_for() {
    let dir = tempfile::tempdir().unwrap();
    let doc = dir.path().join("a.md");
    std::fs::write(&doc, "# Title\n\nSome `code`.\n\n![A picture](pic.png)\n").unwrap();
    let doc = doc.to_str().unwrap();

    let plain = bastian(dir.path(), &["-p", "-w", "40", doc], &[]);
    assert!(plain.status.success());
    assert!(!stdout(&plain).contains('\u{1b}'));
    // Pictures are a TUI thing; printing keeps the alt text.
    assert!(stdout(&plain).contains("[image: A picture]"));

    let colored = bastian(dir.path(), &["-p", "--color", "always", doc], &[]);
    assert!(stdout(&colored).contains("\u{1b}["));
    let forced = bastian(dir.path(), &["-p", doc], &[("CLICOLOR_FORCE", "1")]);
    assert!(stdout(&forced).contains("\u{1b}["));
    let none = bastian(
        dir.path(),
        &["-p", doc],
        &[("CLICOLOR_FORCE", "1"), ("NO_COLOR", "1")],
    );
    assert!(!stdout(&none).contains('\u{1b}'));

    // Display-only flags are accepted when printing.
    let flags = bastian(
        dir.path(),
        &["-p", "--no-logo", "--images", "off", doc],
        &[],
    );
    assert!(flags.status.success(), "{}", stderr(&flags));
}

#[test]
fn reads_stdin() {
    let dir = tempfile::tempdir().unwrap();
    let out = bastian_with_stdin(
        dir.path(),
        &["-p", "-"],
        &[],
        Some("# From stdin\n\ntext\n"),
    );
    assert!(out.status.success());
    assert!(stdout(&out).contains("From stdin"));
}

#[test]
fn themes_are_listed_when_piped() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("bastian/themes")).unwrap();
    std::fs::write(
        dir.path().join("bastian/themes/mine.toml"),
        "extends = \"nord\"\n[ui]\naccent = \"#ff0000\"\n",
    )
    .unwrap();
    let out = bastian(dir.path(), &["themes"], &[]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(!text.contains('\u{1b}'), "no colors when piped");
    for name in [
        "tokyonight-night",
        "rose-pine",
        "kanagawa-wave",
        "catppuccin-latte",
        "nord",
    ] {
        assert!(
            text.lines().any(|l| l.starts_with(name)),
            "{name} missing:\n{text}"
        );
    }
    assert!(
        text.lines()
            .any(|l| l.starts_with("mine ") && l.contains("mine.toml")),
        "{text}"
    );
    assert!(
        text.lines()
            .any(|l| l.starts_with("rose-pine ") && l.contains("← dark_theme")),
        "{text}"
    );
}

#[test]
fn listing_a_directory_skips_hidden_paths() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    std::fs::create_dir_all(root.join(".config/app")).unwrap();
    std::fs::create_dir_all(root.join("docs")).unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    // Like a dotfiles repo: ignore everything, then re-include .config.
    std::fs::write(root.join(".gitignore"), "/*\n!/.config/\n!/docs/\n").unwrap();
    std::fs::write(root.join(".config/app/notes.md"), "# notes\n").unwrap();
    std::fs::write(root.join("docs/guide.md"), "# guide\n").unwrap();
    let root = root.to_str().unwrap();

    let out = bastian(dir.path(), &["-p", root], &[]);
    assert_eq!(stdout(&out).trim(), "docs/guide.md");
    let all = bastian(dir.path(), &["-p", "-a", root], &[]);
    assert!(
        stdout(&all).contains(".config/app/notes.md"),
        "{}",
        stdout(&all)
    );
}

#[test]
fn fetches_a_remote_document() {
    use std::io::{BufRead, BufReader};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        while reader.read_line(&mut line).unwrap() > 2 {
            line.clear();
        }
        let body = "# Remote\n\nFetched over HTTP.\n";
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: text/markdown\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    });
    let dir = tempfile::tempdir().unwrap();
    let url = format!("http://{addr}/README.md");
    let out = bastian(dir.path(), &["-p", &url], &[]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("Fetched over HTTP."));
}

#[test]
fn completions() {
    let dir = tempfile::tempdir().unwrap();
    let zsh = bastian(dir.path(), &["completions", "zsh"], &[]);
    assert!(stdout(&zsh).contains("#compdef bastian"));
    // Theme names are offered for --theme.
    assert!(stdout(&zsh).contains("kanagawa-wave"));
}

#[test]
fn listing_follows_sort_extensions_and_ignore() {
    let dir = tempfile::tempdir().unwrap();
    let docs = dir.path().join("docs");
    std::fs::create_dir_all(docs.join("drafts")).unwrap();
    std::fs::write(docs.join("b.md"), "# Beta\n").unwrap();
    std::fs::write(docs.join("a.txt"), "Alpha\n").unwrap();
    std::fs::write(docs.join("drafts/c.md"), "# Gamma\n").unwrap();
    std::fs::create_dir_all(dir.path().join("bastian")).unwrap();
    std::fs::write(
        dir.path().join("bastian/config.toml"),
        "extensions = [\"md\", \"txt\"]\nignore = [\"drafts/\"]\n",
    )
    .unwrap();
    let docs = docs.to_str().unwrap();
    let out = bastian(dir.path(), &["-p", "--sort", "name", docs], &[]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "a.txt\nb.md\n");

    let bad = bastian(
        dir.path(),
        &["-p", docs],
        &[("BASTIAN_DATE_FORMAT", "%Y %")],
    );
    assert!(!bad.status.success());
    assert!(stderr(&bad).contains("date_format"), "{}", stderr(&bad));
}

#[cfg(unix)]
#[test]
fn a_fifo_is_refused_instead_of_blocking() {
    let dir = tempfile::tempdir().unwrap();
    let fifo = dir.path().join("pipe.md");
    assert!(
        Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_bastian"))
        .args(["-p", fifo.to_str().unwrap()])
        .env("XDG_CONFIG_HOME", dir.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Opening a FIFO for reading waits for a writer; don't hang the suite.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while child.try_wait().unwrap().is_none() {
        if std::time::Instant::now() > deadline {
            child.kill().unwrap();
            panic!("bastian blocked reading a FIFO");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let out = child.wait_with_output().unwrap();
    assert!(!out.status.success());
    assert!(stderr(&out).contains("is not a file"), "{}", stderr(&out));
}
