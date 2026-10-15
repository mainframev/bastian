//! Non-interactive output: printing rendered documents with or without
//! color, and handing them to a pager.

use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

use crate::config::{ColorMode, Setting, Source};
use crate::doc::Document;
use crate::render::{self, theme::Theme};

/// Whether printed output should carry ANSI colors.
///
/// `NO_COLOR` (any non-empty value) turns color off unless it was asked for
/// explicitly with `always`; `CLICOLOR_FORCE` (non-empty, not `0`) turns it
/// on for `auto` even when stdout isn't a terminal.
pub fn use_color(
    mode: ColorMode,
    stdout_is_tty: bool,
    no_color: Option<&str>,
    clicolor_force: Option<&str>,
) -> bool {
    let set = |v: Option<&str>| v.is_some_and(|v| !v.is_empty());
    match mode {
        ColorMode::Always => true,
        ColorMode::Never => false,
        ColorMode::Auto if set(no_color) => false,
        ColorMode::Auto => stdout_is_tty || (set(clicolor_force) && clicolor_force != Some("0")),
    }
}

/// Width for printed output: an explicitly configured width wins; otherwise
/// the terminal's width capped at 120, or 80 without a terminal.
pub fn print_width(width: &Setting<u16>, terminal_cols: Option<u16>) -> u16 {
    if width.source != Source::Default {
        return width.value;
    }
    terminal_cols.map_or(80, |c| c.min(120))
}

/// The pager command: `$PAGER`, else `less -R`. Run through the shell so
/// values with arguments or quoted paths work.
pub fn pager_command(pager_env: Option<&str>) -> String {
    match pager_env.map(str::trim) {
        Some(p) if !p.is_empty() => p.to_string(),
        _ => "less -R".to_string(),
    }
}

/// Write `text` into the pager's stdin and wait for it to exit. Like git,
/// `LESS=FRX` is set when unset so `less` shows colors and quits on short
/// output.
pub fn page(text: &str, pager: &str) -> Result<()> {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(pager).stdin(Stdio::piped());
    if std::env::var_os("LESS").is_none() {
        cmd.env("LESS", "FRX");
    }
    let mut child = cmd
        .spawn()
        .with_context(|| format!("starting pager `{pager}`"))?;
    if let Some(mut stdin) = child.stdin.take() {
        // A pager quit early (e.g. `q` in less) closes the pipe; that's fine.
        match stdin.write_all(text.as_bytes()) {
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {}
            other => other.context("writing to pager")?,
        }
    }
    child.wait().context("waiting for pager")?;
    Ok(())
}

/// How non-interactive output is written.
pub struct Output {
    pub width: u16,
    pub color: bool,
    /// Pager command when paging, `None` to write to stdout.
    pub pager: Option<String>,
}

impl Output {
    /// Render `doc` and write it out in `theme`.
    pub fn print(&self, doc: &Document, theme: &Theme, opts: render::Options) -> Result<()> {
        // Plain text has no background to show a chip on.
        let opts = render::Options {
            chips: self.color,
            ..opts
        };
        let rendered = render::render_with(doc, self.width, theme, opts);
        let text = if self.color {
            render::to_ansi(&rendered.lines)
        } else {
            render::to_plain(&rendered.lines)
        };
        if let Some(pager) = &self.pager {
            page(&format!("{text}\n"), pager)
        } else {
            println!("{text}");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_policy() {
        use ColorMode::*;
        assert!(use_color(Auto, true, None, None));
        assert!(!use_color(Auto, false, None, None));
        assert!(!use_color(Auto, true, Some("1"), None));
        assert!(
            use_color(Auto, true, Some(""), None),
            "empty NO_COLOR is unset"
        );
        assert!(use_color(Auto, false, None, Some("1")));
        assert!(!use_color(Auto, false, None, Some("0")));
        assert!(
            !use_color(Auto, false, Some("1"), Some("1")),
            "NO_COLOR wins over force"
        );
        assert!(use_color(Always, false, Some("1"), None));
        assert!(!use_color(Never, true, None, Some("1")));
    }

    #[test]
    fn width_policy() {
        let default = Setting {
            value: 100,
            source: Source::Default,
        };
        let flag = Setting {
            value: 60,
            source: Source::Flag,
        };
        assert_eq!(print_width(&default, Some(200)), 120);
        assert_eq!(print_width(&default, Some(90)), 90);
        assert_eq!(print_width(&default, None), 80);
        assert_eq!(print_width(&flag, Some(200)), 60);
    }

    #[test]
    fn pager_default_and_override() {
        assert_eq!(pager_command(None), "less -R");
        assert_eq!(pager_command(Some("  ")), "less -R");
        assert_eq!(
            pager_command(Some("bat --paging=always")),
            "bat --paging=always"
        );
    }

    #[test]
    fn pager_receives_text() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out.txt");
        page("hello\n", &format!("cat > '{}'", out.display())).unwrap();
        assert_eq!(std::fs::read_to_string(out).unwrap(), "hello\n");
    }
}
