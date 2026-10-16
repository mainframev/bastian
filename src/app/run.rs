use std::io::{self, Write};
use std::time::Duration;

use anyhow::Result;
use ratatui::Terminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::layout::Size;

use super::App;
use super::screen::{Screen, draw_frame};
use super::terminal::{Tui, suspend};
use crate::ui;

/// Events that change nothing on screen: pointer motion (unless it moves
/// onto or off a link, which `App` reports), clicks other than the left
/// button (the wheel scrolls, a left click may resize the table of contents
/// or the theme picker) and key releases.
pub(super) fn is_ignored(event: &Event) -> bool {
    match event {
        Event::Mouse(m) => !matches!(
            m.kind,
            MouseEventKind::ScrollDown
                | MouseEventKind::ScrollUp
                | MouseEventKind::Down(MouseButton::Left)
        ),
        Event::Key(k) => k.kind != KeyEventKind::Press,
        _ => false,
    }
}

/// How often to wake up for process output while waiting for keys.
pub(super) const TICK: Duration = Duration::from_millis(100);

/// Draw the open reader, or the file browser when no reader is open.
pub(super) fn draw<W: Write>(terminal: &mut Terminal<Screen<W>>, app: &mut App) -> io::Result<()> {
    draw_frame(terminal, |f| {
        let picker = app.picker.as_mut().map(|(p, _)| p);
        if let Some(r) = &mut app.reader {
            ui::reader::draw_with(f, r, picker);
        } else if let Some(b) = &mut app.browser {
            let s = &app.settings;
            let look = ui::browser::Look {
                logo: s.logo,
                date_format: s.date_format.as_deref(),
                scrollbar: s.reader.scrollbar,
                selector: s.selector,
                layout: s.list_layout,
            };
            ui::browser::draw(f, b, &s.reader.theme, look, picker);
        }
    })
}

/// Run the TUI until the user quits. Returns a note to print afterwards.
pub fn run(terminal: &mut Tui, mut app: App) -> Result<Option<String>> {
    let mut redraw = true;
    loop {
        if let Some(r) = &mut app.reader {
            redraw |= r.tick();
        }
        for r in &mut app.history {
            r.tick_hidden();
        }
        if let Some(b) = &mut app.browser {
            redraw |= b.tick();
        }
        if std::mem::take(&mut redraw) {
            draw(terminal, &mut app)?;
        }
        if event::poll(TICK)? {
            let event = event::read()?;
            if let Event::Resize(width, height) = event {
                terminal.backend_mut().resize(Size::new(width, height));
            }
            let ignored = is_ignored(&event);
            if !app.on_event(&event) {
                if let Some(r) = &app.reader {
                    r.remember_position();
                }
                return Ok(app.exit_note.take());
            }
            if let Some((path, line)) = app.edit.take() {
                app.run_editor(terminal, &path, line)?;
                redraw = true;
            }
            if std::mem::take(&mut app.suspend) {
                // In case the suspended job is never resumed.
                if let Some(r) = &app.reader {
                    r.remember_position();
                }
                suspend(terminal, app.settings.mouse)?;
                redraw = true;
            }
            redraw |= !ignored || std::mem::take(&mut app.hover_changed);
        }
    }
}
