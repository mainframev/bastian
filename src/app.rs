use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;
use ratatui::DefaultTerminal;
use ratatui::crossterm::clipboard::CopyToClipboard;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::crossterm::execute;

use crate::browser::{self, Browser};
use crate::doc::{self, Document};
use crate::editor;
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
    /// Show line numbers in readers.
    pub line_numbers: bool,
    /// Browse hidden and gitignored files too.
    pub all: bool,
}

impl Settings {
    pub fn reader(&self, name: String, doc: Document, base_dir: PathBuf) -> Reader {
        let mut r = Reader::new(name, doc, self.theme.clone(), self.max_width);
        r.base_dir = base_dir;
        r.assume_yes = self.assume_yes;
        r.interpreters = self.interpreters.clone();
        r.line_numbers = self.line_numbers;
        r
    }

    /// A reader for a fetched document. Its code blocks run in the current
    /// directory and always ask first; it has no file to watch or edit.
    pub fn remote(&self, doc: crate::remote::RemoteDoc) -> Reader {
        let fake_path = std::path::PathBuf::from(doc.base.path());
        let parsed = doc::parse_for(&doc.text, Some(&fake_path));
        let mut r = self.reader(doc.name, parsed, PathBuf::from("."));
        r.text = doc.text;
        r.remote_base = Some(doc.base);
        r.untrusted = true;
        r
    }

    /// Open a file in a reader that reloads when the file changes.
    pub fn open(&self, path: &Path, name: String) -> Result<Reader> {
        let src = crate::text::read_text(path)?;
        let doc = doc::parse_for(&src, Some(path));
        let code_file = doc.code_file;
        let mut r = self.reader(name, doc, base_dir(path));
        // Source files always show their line numbers.
        r.line_numbers |= code_file;
        r.text = src;
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
    /// File to open in the editor once the current event is handled.
    edit: Option<(PathBuf, Option<usize>)>,
    /// Set by ctrl-z; the run loop suspends bastian like a shell job.
    suspend: bool,
}

impl App {
    pub fn browse(root: PathBuf, settings: Settings) -> Self {
        Self {
            browser: Some(Browser::with_options(root, settings.all)),
            reader: None,
            history: Vec::new(),
            edit: None,
            suspend: false,
            settings,
        }
    }

    pub fn read(reader: Reader, settings: Settings) -> Self {
        Self {
            browser: None,
            reader: Some(reader),
            history: Vec::new(),
            edit: None,
            suspend: false,
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
        if key.code == KeyCode::Char('z') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.suspend = true;
            return true;
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
            browser::Action::Edit(path) => {
                self.edit = Some((path, None));
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
            reader::Action::OpenRemote(url) => {
                match crate::remote::fetch(&crate::remote::Source::Url(url)) {
                    Ok(doc) => {
                        let next = self.settings.remote(doc);
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
            reader::Action::Edit(path, line) => self.edit = Some((path, line)),
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
        } else if let Some(b) = &mut self.browser {
            b.message = Some(msg);
        }
    }

    /// Hand the terminal to the user's editor, then take it back. An open
    /// reader picks up the changes through its file watch.
    fn run_editor(
        &mut self,
        terminal: &mut DefaultTerminal,
        path: &Path,
        line: Option<usize>,
    ) -> Result<()> {
        let cmd = editor::command();
        ratatui::restore();
        let result = editor::open_with(&cmd, path, line);
        *terminal = ratatui::init();
        terminal.clear()?;
        match result {
            Ok(status) if status.success() => {}
            // The shell's "command not found" status.
            Ok(status) if status.code() == Some(127) => {
                self.set_message(format!("editor not found: {cmd} (set $VISUAL or $EDITOR)"));
            }
            Ok(status) => self.set_message(format!("{cmd} exited with {status}")),
            Err(err) => self.set_message(format!("could not run {cmd}: {err}")),
        }
        Ok(())
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

/// Stop like a shell job (ctrl-z), restoring the terminal first and taking
/// it back when resumed with `fg`.
fn suspend(terminal: &mut DefaultTerminal) -> Result<()> {
    #[cfg(unix)]
    {
        ratatui::restore();
        // SAFETY: raise(3) with a standard signal; SIGTSTP's default action
        // stops the process until SIGCONT.
        unsafe { libc::raise(libc::SIGTSTP) };
        *terminal = ratatui::init();
        terminal.clear()?;
    }
    #[cfg(not(unix))]
    let _ = terminal;
    Ok(())
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
            if let Some((path, line)) = app.edit.take() {
                app.run_editor(terminal, &path, line)?;
            }
            if std::mem::take(&mut app.suspend) {
                suspend(terminal)?;
            }
            redraw = true;
        }
    }
}
