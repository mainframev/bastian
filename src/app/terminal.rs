use anyhow::Result;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use ratatui::crossterm::execute;

/// Take over the terminal: ratatui's setup, plus mouse capture when the
/// mouse option is on. Undo with `leave_terminal`.
pub fn enter_terminal(mouse: bool) -> Result<DefaultTerminal> {
    let terminal = ratatui::init();
    if mouse {
        // ratatui's panic hook restores the screen but not mouse capture,
        // which would leave the shell printing escapes on every click.
        static HOOK: std::sync::Once = std::sync::Once::new();
        HOOK.call_once(|| {
            let restore = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                let _ = execute!(std::io::stdout(), DisableMouseCapture);
                restore(info);
            }));
        });
        if let Err(e) = execute!(std::io::stdout(), EnableMouseCapture) {
            leave_terminal(mouse);
            return Err(e.into());
        }
    }
    Ok(terminal)
}

pub fn leave_terminal(mouse: bool) {
    if mouse {
        let _ = execute!(std::io::stdout(), DisableMouseCapture);
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
