use anyhow::Result;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::doc::Document;
use crate::render::theme::Theme;
use crate::render::{self, Rendered};
use crate::ui;

/// State of the document reader view.
pub struct Reader {
    pub name: String,
    pub doc: Document,
    pub theme: Theme,
    pub max_width: u16,
    pub rendered: Rendered,
    pub scroll: usize,
    width: u16,
    height: usize,
}

impl Reader {
    pub fn new(name: String, doc: Document, theme: Theme, max_width: u16) -> Self {
        Self {
            name,
            doc,
            theme,
            max_width,
            rendered: Rendered::default(),
            scroll: 0,
            width: 0,
            height: 0,
        }
    }

    /// Re-render when the content width changes and keep scroll in range.
    pub fn layout(&mut self, width: u16, height: u16) {
        if width != self.width {
            self.rendered = render::render(&self.doc, width, &self.theme);
            self.width = width;
        }
        self.height = usize::from(height);
        self.scroll = self.scroll.min(self.max_scroll());
    }

    pub fn max_scroll(&self) -> usize {
        self.rendered.lines.len().saturating_sub(self.height)
    }

    fn scroll_by(&mut self, delta: isize) {
        self.scroll = self
            .scroll
            .saturating_add_signed(delta)
            .min(self.max_scroll());
    }

    /// Returns false when the reader should close.
    fn on_key(&mut self, key: KeyEvent) -> bool {
        let page = self.height.max(1) as isize;
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return false,
            KeyCode::Char('c') if ctrl => return false,
            KeyCode::Char('d') if ctrl => self.scroll_by(page / 2),
            KeyCode::Char('u') if ctrl => self.scroll_by(-page / 2),
            KeyCode::Char('j') | KeyCode::Down => self.scroll_by(1),
            KeyCode::Char('k') | KeyCode::Up => self.scroll_by(-1),
            KeyCode::Char(' ' | 'f') | KeyCode::PageDown => self.scroll_by(page),
            KeyCode::Char('b') | KeyCode::PageUp => self.scroll_by(-page),
            KeyCode::Char('g') | KeyCode::Home => self.scroll = 0,
            KeyCode::Char('G') | KeyCode::End => self.scroll = self.max_scroll(),
            _ => {}
        }
        true
    }
}

pub fn run(terminal: &mut DefaultTerminal, mut reader: Reader) -> Result<()> {
    loop {
        terminal.draw(|f| ui::reader::draw(f, &mut reader))?;
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press && !reader.on_key(key) => {
                return Ok(());
            }
            _ => {}
        }
    }
}
