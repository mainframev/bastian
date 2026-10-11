use std::path::{Path, PathBuf};

use std::time::Duration;

use anyhow::{Context, Result};
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

use crate::browser::{self, Browser};
use crate::doc;
use crate::reader::{self, Reader};
use crate::render::theme::Theme;
use crate::ui;

/// Top-level TUI state: an optional file browser with a reader on top.
pub struct App {
    pub theme: Theme,
    pub max_width: u16,
    pub browser: Option<Browser>,
    pub reader: Option<Reader>,
    /// Run blocks without asking, except those marked `confirm`.
    pub assume_yes: bool,
}

impl App {
    pub fn browse(root: PathBuf, theme: Theme, max_width: u16, assume_yes: bool) -> Self {
        Self {
            browser: Some(Browser::new(root)),
            reader: None,
            theme,
            max_width,
            assume_yes,
        }
    }

    pub fn read(reader: Reader) -> Self {
        Self {
            theme: reader.theme.clone(),
            max_width: reader.max_width,
            assume_yes: reader.assume_yes,
            browser: None,
            reader: Some(reader),
        }
    }

    fn open(&mut self, path: &Path, name: String) -> Result<()> {
        let src =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let mut reader = Reader::new(name, doc::parse(&src), self.theme.clone(), self.max_width);
        reader.base_dir = base_dir(path);
        reader.assume_yes = self.assume_yes;
        self.reader = Some(reader);
        Ok(())
    }

    /// Returns false when the app should exit.
    fn on_event(&mut self, event: Event) -> bool {
        let Event::Key(key) = event else { return true };
        if key.kind != KeyEventKind::Press {
            return true;
        }
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return false;
        }

        if let Some(r) = &mut self.reader {
            return match r.on_key(key) {
                reader::Action::None => true,
                reader::Action::Quit => false,
                reader::Action::Back => {
                    self.reader = None;
                    self.browser.is_some()
                }
            };
        }

        let Some(b) = &mut self.browser else {
            return false;
        };
        match b.on_key(key) {
            browser::Action::None => true,
            browser::Action::Quit => false,
            browser::Action::Open(path) => {
                let name = b
                    .selected_entry()
                    .map_or_else(|| path.display().to_string(), |e| e.rel.clone());
                if let Err(err) = self.open(&path, name)
                    && let Some(b) = &mut self.browser
                {
                    b.message = Some(format!("{err:#}"));
                }
                true
            }
        }
    }
}

/// Directory a document's blocks run in: the one containing the file.
pub fn base_dir(path: &Path) -> PathBuf {
    match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

/// How often to wake up for process output while waiting for keys.
const TICK: Duration = Duration::from_millis(100);

pub fn run(terminal: &mut DefaultTerminal, mut app: App) -> Result<()> {
    let mut redraw = true;
    loop {
        if let Some(r) = &mut app.reader {
            redraw |= r.tick();
        }
        if !redraw && !event::poll(TICK)? {
            continue;
        }
        redraw = false;
        terminal.draw(|f| {
            if let Some(r) = &mut app.reader {
                ui::reader::draw(f, r);
            } else if let Some(b) = &mut app.browser {
                ui::browser::draw(f, b, &app.theme);
            }
        })?;
        if event::poll(TICK)? {
            if !app.on_event(event::read()?) {
                return Ok(());
            }
            redraw = true;
        }
    }
}
