use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use ratatui::DefaultTerminal;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{EnterAlternateScreen, enable_raw_mode};

/// Whether mouse capture is on, for the panic hook to undo.
static MOUSE: AtomicBool = AtomicBool::new(false);

/// Take over the terminal: raw mode and the alternate screen, as
/// `ratatui::init` does, plus mouse capture when the mouse option is on.
/// Undo with `leave_terminal`. An error (no controlling terminal, say)
/// leaves the terminal as it was.
pub fn enter_terminal(mouse: bool) -> Result<DefaultTerminal> {
    install_panic_hook();
    let setup = || -> std::io::Result<DefaultTerminal> {
        enable_raw_mode()?;
        execute!(std::io::stdout(), EnterAlternateScreen)?;
        if mouse {
            execute!(std::io::stdout(), EnableMouseCapture)?;
            MOUSE.store(true, Ordering::Relaxed);
        }
        Terminal::new(CrosstermBackend::new(std::io::stdout()))
    };
    setup()
        .inspect_err(|_| leave_terminal(mouse))
        .context("taking over the terminal")
}

/// Restore the terminal before a panic message prints. Installed once:
/// `ratatui::init` adds a hook on every call, and the terminal is taken
/// over again after each editor round trip and ctrl-z.
fn install_panic_hook() {
    static HOOK: std::sync::Once = std::sync::Once::new();
    HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            // ratatui's restore doesn't undo mouse capture, which would
            // leave the shell printing escapes on every click.
            if MOUSE.load(Ordering::Relaxed) {
                let _ = execute!(std::io::stdout(), DisableMouseCapture);
            }
            ratatui::restore();
            previous(info);
        }));
    });
}

pub fn leave_terminal(mouse: bool) {
    if mouse {
        let _ = execute!(std::io::stdout(), DisableMouseCapture);
        MOUSE.store(false, Ordering::Relaxed);
    }
    ratatui::restore();
}
/// Give the terminal back to the shell while `f` runs, then take it over
/// again with a clean screen.
pub(super) fn with_terminal_released<T>(
    terminal: &mut DefaultTerminal,
    mouse: bool,
    f: impl FnOnce() -> T,
) -> Result<T> {
    leave_terminal(mouse);
    let result = f();
    *terminal = enter_terminal(mouse)?;
    terminal.clear()?;
    Ok(result)
}

/// Stop like a shell job (ctrl-z), restoring the terminal first and taking
/// it back when resumed with `fg`.
pub(super) fn suspend(terminal: &mut DefaultTerminal, mouse: bool) -> Result<()> {
    #[cfg(unix)]
    with_terminal_released(terminal, mouse, || {
        // SAFETY: raise(3) with a standard signal; SIGTSTP's default action
        // stops the process until SIGCONT.
        unsafe { libc::raise(libc::SIGTSTP) };
    })?;
    #[cfg(not(unix))]
    let _ = (terminal, mouse);
    Ok(())
}
