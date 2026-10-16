//! The backend the TUI draws through: crossterm's, with each frame sent in
//! one write as one synchronized update.

use std::cell::Cell;
use std::io::{self, BufWriter, Write};
use std::rc::Rc;

use ratatui::backend::{Backend, ClearType, CrosstermBackend, WindowSize};
use ratatui::buffer::Cell as BufferCell;
use ratatui::crossterm::queue;
use ratatui::crossterm::terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate};
use ratatui::layout::{Position, Size};
use ratatui::{Frame, Terminal};

/// Bytes buffered per frame: a full 200x50 screen of styled text is about
/// 13 KB, so this holds most frames whole.
const FRAME_BUFFER: usize = 64 * 1024;

/// Where a frame stands, shared by `Screen` and its `FrameOut`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FrameState {
    /// Not drawing: flushes go through.
    Idle,
    /// Drawing, nothing written yet: the first write opens the update.
    Begun,
    /// Drawing, with the synchronized update opened.
    Open,
}

/// Where frames are written: a buffer big enough for a whole screen, so a
/// frame goes out in one write rather than in the pieces stdout's line
/// buffer would cut it into. During a frame the flushes crossterm's
/// backend makes part way through (after hiding the cursor, say) wait for
/// the frame's end. Between frames it holds nothing, so writes straight to
/// stdout (OSC 52, leaving the terminal) can't overtake a frame.
struct FrameOut<W: Write> {
    out: BufWriter<W>,
    frame: Rc<Cell<FrameState>>,
}

impl<W: Write> Write for FrameOut<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.frame.get() == FrameState::Begun {
            queue!(self.out, BeginSynchronizedUpdate)?;
            self.frame.set(FrameState::Open);
        }
        self.out.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.frame.get() {
            FrameState::Idle => self.out.flush(),
            FrameState::Begun | FrameState::Open => Ok(()),
        }
    }
}

/// crossterm's backend writing through a `FrameOut`.
pub struct Screen<W: Write> {
    inner: CrosstermBackend<FrameOut<W>>,
    frame: Rc<Cell<FrameState>>,
}

impl<W: Write> Screen<W> {
    pub(super) fn new(out: W) -> Self {
        let frame = Rc::new(Cell::new(FrameState::Idle));
        let out = FrameOut {
            out: BufWriter::with_capacity(FRAME_BUFFER, out),
            frame: Rc::clone(&frame),
        };
        Self {
            inner: CrosstermBackend::new(out),
            frame,
        }
    }

    /// Close the frame's synchronized update, if one was opened, and send
    /// the frame.
    fn end_frame(&mut self) -> io::Result<()> {
        let open = self.frame.replace(FrameState::Idle) == FrameState::Open;
        if open {
            queue!(self.inner, EndSynchronizedUpdate)?;
        }
        Backend::flush(&mut self.inner)
    }
}

/// Draw a frame as one synchronized update (mode 2026): terminals that
/// support it show the frame whole, others ignore the markers. The update
/// is opened by the frame's first byte, so a render that panics leaves
/// nothing behind, and is closed even when drawing fails part way.
pub(super) fn draw_frame<W: Write>(
    terminal: &mut Terminal<Screen<W>>,
    render: impl FnOnce(&mut Frame),
) -> io::Result<()> {
    terminal.backend().frame.set(FrameState::Begun);
    let _unopened = Unopened(Rc::clone(&terminal.backend().frame));
    let drawn = terminal.draw(render).map(drop);
    let ended = terminal.backend_mut().end_frame();
    drawn.and(ended)
}

/// Forgets a frame that wrote nothing, if its render panics: the cursor
/// shown as the terminal is dropped then mustn't open an update that
/// nothing closes.
struct Unopened(Rc<Cell<FrameState>>);

impl Drop for Unopened {
    fn drop(&mut self) {
        if self.0.get() == FrameState::Begun {
            self.0.set(FrameState::Idle);
        }
    }
}

impl<W: Write> Backend for Screen<W> {
    type Error = io::Error;

    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a BufferCell)>,
    {
        self.inner.draw(content)
    }

    fn append_lines(&mut self, n: u16) -> io::Result<()> {
        self.inner.append_lines(n)
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.inner.hide_cursor()
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.inner.show_cursor()
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        self.inner.get_cursor_position()
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        self.inner.set_cursor_position(position)
    }

    fn clear(&mut self) -> io::Result<()> {
        self.inner.clear()
    }

    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        self.inner.clear_region(clear_type)
    }

    fn size(&self) -> io::Result<Size> {
        self.inner.size()
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        self.inner.window_size()
    }

    fn flush(&mut self) -> io::Result<()> {
        Backend::flush(&mut self.inner)
    }
}
