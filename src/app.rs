use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use ratatui::DefaultTerminal;
use ratatui::crossterm::clipboard::CopyToClipboard;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::crossterm::execute;

use crate::browser::{self, Browser};
use crate::doc::{self, Document};
use crate::exec::Interpreters;
use crate::links;
use crate::reader::{self, Reader};
use crate::render::theme::Theme;
use crate::ui;

/// Options shared by every document the app opens.
#[derive(Clone)]
pub struct Settings {
    pub theme: Theme,
    pub max_width: u16,
    /// Run blocks without asking, except those marked `confirm`.
    pub assume_yes: bool,
    pub interpreters: Interpreters,
}

impl Settings {
    pub fn reader(&self, name: String, doc: Document, base_dir: PathBuf) -> Reader {
        let mut r = Reader::new(name, doc, self.theme.clone(), self.max_width);
        r.base_dir = base_dir;
        r.assume_yes = self.assume_yes;
        r.interpreters = self.interpreters.clone();
        r
    }

    /// Open a file in a reader that reloads when the file changes.
    pub fn open(&self, path: &Path, name: String) -> Result<Reader> {
        let src =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let mut r = self.reader(name, doc::parse(&src), base_dir(path));
        r.watch(path.to_path_buf());
        Ok(r)
    }
}

/// Top-level TUI state: an optional file browser with a reader on top.
/// Followed links push the previous reader onto `history`.
pub struct App {
    pub settings: Settings,
    pub browser: Option<Browser>,
    pub reader: Option<Reader>,
    pub history: Vec<Reader>,
}

impl App {
    pub fn browse(root: PathBuf, settings: Settings) -> Self {
        Self {
            browser: Some(Browser::new(root)),
            reader: None,
            history: Vec::new(),
            settings,
        }
    }

    pub fn read(reader: Reader, settings: Settings) -> Self {
        Self {
            browser: None,
            reader: Some(reader),
            history: Vec::new(),
            settings,
        }
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
            let action = r.on_key(key);
            return self.on_reader_action(action);
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
                match self.settings.open(&path, name) {
                    Ok(r) => self.reader = Some(r),
                    Err(err) => b.message = Some(format!("{err:#}")),
                }
                true
            }
        }
    }

    fn on_reader_action(&mut self, action: reader::Action) -> bool {
        match action {
            reader::Action::None => {}
            reader::Action::Quit => return false,
            reader::Action::Back => {
                self.reader = self.history.pop();
                return self.reader.is_some() || self.browser.is_some();
            }
            reader::Action::OpenDocument(path) => {
                let name = display_name(&path);
                match self.settings.open(&path, name) {
                    Ok(next) => {
                        if let Some(prev) = self.reader.replace(next) {
                            self.history.push(prev);
                        }
                    }
                    Err(err) => self.set_message(format!("{err:#}")),
                }
            }
            reader::Action::OpenExternal(target) => {
                if let Err(err) = links::open_external(&target) {
                    self.set_message(format!("could not open {target}: {err}"));
                }
            }
            reader::Action::Copy(text) => {
                // OSC 52 asks the terminal to set the clipboard, which also
                // works over ssh. Terminals without support ignore it.
                if let Err(err) =
                    execute!(std::io::stdout(), CopyToClipboard::to_clipboard_from(text))
                {
                    self.set_message(format!("copy failed: {err}"));
                }
            }
        }
        true
    }

    fn set_message(&mut self, msg: String) {
        if let Some(r) = &mut self.reader {
            r.message = Some(msg);
        }
    }
}

/// Path relative to the current directory when possible, for the status bar.
fn display_name(path: &Path) -> String {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| path.strip_prefix(cwd).ok().map(Path::to_path_buf))
        .unwrap_or_else(|| path.to_path_buf())
        .display()
        .to_string()
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
        if let Some(b) = &mut app.browser {
            redraw |= b.tick();
        }
        if !redraw && !event::poll(TICK)? {
            continue;
        }
        redraw = false;
        terminal.draw(|f| {
            if let Some(r) = &mut app.reader {
                ui::reader::draw(f, r);
            } else if let Some(b) = &mut app.browser {
                ui::browser::draw(f, b, &app.settings.theme);
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
