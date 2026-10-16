use std::io::{self, Write};
use std::time::Duration;

use anyhow::Result;
use ratatui::Terminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::layout::Size;

use super::App;
use super::screen::{Screen, draw_frame};
use super::terminal::{Tui, suspend};
use crate::reader::Reader;
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

/// Most events handled in one batch, before the screen is drawn again:
/// enough to keep up with a held key, few enough that it still shows the
/// view moving.
pub(super) const BATCH: usize = 64;

/// Where events come from.
pub(super) trait Input {
    /// Whether an event is waiting already.
    fn ready(&mut self) -> io::Result<bool>;
    fn read(&mut self) -> io::Result<Event>;
}

/// The terminal's keyboard, mouse and resize events.
struct Crossterm;

/// How long `Crossterm::ready` waits. crossterm's poll with no timeout at
/// all returns before looking at input it has read from the tty but not
/// yet handed out (the tty source loops only while time is left), so it
/// would never see the rest of a burst; this is the least it needs, too
/// short to delay a frame.
const READY_WAIT: Duration = Duration::from_micros(10);

impl Input for Crossterm {
    fn ready(&mut self) -> io::Result<bool> {
        event::poll(READY_WAIT)
    }

    fn read(&mut self) -> io::Result<Event> {
        event::read()
    }
}

/// What a batch of events leaves for the run loop to do.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Handled {
    /// An event asked to quit; the events after it weren't read.
    pub quit: bool,
    /// Something on screen changed. A scroll that left the view as it was
    /// changes nothing.
    pub redraw: bool,
    /// The terminal's new size.
    pub resize: Option<Size>,
    /// An event read but not handled: the first of the next batch.
    pub next: Option<Event>,
}

/// Handle `first`, then the events already waiting behind it, in order,
/// for as long as each only moves the view: a held key's repeats then
/// cost one frame between them rather than one each. Any other event
/// waiting is left in `Handled::next`, to be handled after the frame, so
/// it acts on what the screen shows (a link picked from the view, say) and
/// whatever it changes (a layout, a new reader, a resize, a hand-off to
/// the editor) is drawn before the event after it, as one event at a time
/// did. At most `BATCH` events.
pub(super) fn handle_batch(
    app: &mut App,
    input: &mut impl Input,
    first: Event,
) -> io::Result<Handled> {
    let mut handled = Handled::default();
    let mut event = first;
    for n in 1.. {
        let ignored = is_ignored(&event);
        let moves = app.only_moves(&event);
        let view = |app: &App| app.reader.as_ref().map(Reader::scroll_view);
        let before = moves.then(|| view(app));
        if !app.on_event(&event) {
            handled.quit = true;
            break;
        }
        if let Event::Resize(width, height) = event {
            handled.resize = Some(Size::new(width, height));
        }
        // A scroll that moved nothing (at the end, say) needs no frame.
        let changed = match before {
            Some(before) => before != view(app),
            None => !ignored,
        };
        handled.redraw |= changed || std::mem::take(&mut app.hover_changed);
        if !(ignored || moves) || n >= BATCH || !input.ready()? {
            break;
        }
        event = input.read()?;
        if !(is_ignored(&event) || app.only_moves(&event)) {
            handled.next = Some(event);
            break;
        }
    }
    Ok(handled)
}

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
    let mut next = None;
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
        let first = match next.take() {
            Some(event) => Some(event),
            None if event::poll(TICK)? => Some(event::read()?),
            None => None,
        };
        if let Some(first) = first {
            let handled = handle_batch(&mut app, &mut Crossterm, first)?;
            next = handled.next;
            if let Some(size) = handled.resize {
                terminal.backend_mut().resize(size);
            }
            if handled.quit {
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
            redraw |= handled.redraw;
        }
    }
}
