mod run;
mod screen;
mod settings;
mod terminal;

pub use run::run;
pub use settings::Settings;
pub use terminal::{Tui, enter_terminal, leave_terminal};

use std::path::{Path, PathBuf};

use anyhow::Result;
use ratatui::crossterm::clipboard::CopyToClipboard;
use ratatui::crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, MouseButton, MouseEventKind,
};
use ratatui::crossterm::execute;

use crate::browser::{self, Browser};
use crate::editor;
use crate::links;
use crate::nav;
use crate::paths;
use crate::picker::{self, ThemePicker};
use crate::reader::{self, Reader};
use crate::render::theme::Theme;
use terminal::with_terminal_released;

/// Top-level TUI state: an optional file browser with a reader on top.
/// Followed links push the previous reader onto `history`.
pub struct App {
    pub settings: Settings,
    browser: Option<Browser>,
    reader: Option<Reader>,
    history: Vec<Reader>,
    /// Files read this session, most recent first and as they were
    /// opened, for the `H` list.
    opened: Vec<PathBuf>,
    /// File to open in the editor once the current event is handled.
    edit: Option<(PathBuf, Option<usize>)>,
    /// Set by ctrl-z; the run loop suspends bastian like a shell job.
    suspend: bool,
    /// The mouse moved onto, off or between links: the run loop redraws,
    /// though pointer motion is otherwise ignored.
    hover_changed: bool,
    /// Where themes come from and where a picked one is saved.
    pub themes: ThemeFiles,
    /// The open theme picker, and the theme to restore if it's cancelled.
    picker: Option<(ThemePicker, Theme)>,
    /// Columns the theme picker opens with: the last width set with `<`
    /// and `>`, for the rest of the session.
    picker_width: u16,
    /// Quit once the picker closes (`bastian themes`).
    pub pick_and_exit: bool,
    /// Shown after the TUI exits, e.g. what `bastian themes` saved.
    exit_note: Option<String>,
}

/// Theme locations and the current choice, for the theme picker.
#[derive(Clone, Default)]
pub struct ThemeFiles {
    /// The user's themes directory.
    pub dir: Option<PathBuf>,
    /// Config file a picked theme is written to.
    pub config: Option<PathBuf>,
    /// The theme in use, by name.
    pub current: String,
    /// What overrides the config file's `theme` this session, if anything
    /// (`--theme` or `BASTIAN_THEME`).
    pub overridden_by: Option<String>,
}

impl App {
    pub fn browse(root: PathBuf, settings: Settings) -> Self {
        let browser = Browser::new(root, settings.scan.clone());
        Self::new(settings, Some(browser), None)
    }

    pub fn read(reader: Reader, settings: Settings) -> Self {
        Self::new(settings, None, Some(reader))
    }

    fn new(settings: Settings, browser: Option<Browser>, reader: Option<Reader>) -> Self {
        let mut app = Self {
            settings,
            browser,
            reader,
            history: Vec::new(),
            opened: Vec::new(),
            edit: None,
            suspend: false,
            hover_changed: false,
            themes: ThemeFiles::default(),
            picker: None,
            picker_width: picker::DEFAULT_WIDTH,
            pick_and_exit: false,
            exit_note: None,
        };
        app.note_opened();
        app
    }

    /// Put the open reader's file first in `opened`. A file reached by
    /// another path replaces its entry, but keeps the path it was opened
    /// by: through a symlink, links and images resolve from the link's
    /// directory, not the target's.
    fn note_opened(&mut self) {
        if !self.settings.history {
            return;
        }
        let Some(path) = self.reader.as_ref().and_then(Reader::file) else {
            return;
        };
        let (path, id) = (path.to_path_buf(), reader::canonical(path));
        self.opened.retain(|p| reader::canonical(p) != id);
        self.opened.insert(0, path);
    }

    pub fn open_picker(&mut self) {
        let picker = ThemePicker::new(
            self.themes.dir.as_deref(),
            &self.themes.current,
            self.picker_width,
        );
        self.picker = Some((picker, self.settings.reader.theme.clone()));
    }

    /// Switch pictures on or off in every reader, asking the terminal how
    /// to draw them the first time they're turned on.
    fn toggle_images(&mut self) {
        let s = &mut self.settings;
        s.show_images = !s.show_images;
        if s.show_images && s.images.is_none() {
            s.images = Some(crate::images::Support::detect());
        }
        s.reader.images = s.image_support();
        let state = if s.show_images { "on" } else { "off" };
        let support = s.reader.images.clone();
        for r in self.reader.iter_mut().chain(self.history.iter_mut()) {
            r.set_images(support.clone());
        }
        self.set_message(format!("images {state}"));
    }

    /// Use `theme` everywhere: the browser, the open reader and history.
    fn apply_theme(&mut self, theme: Theme) {
        let theme = self.settings.on_page(theme);
        for r in self.reader.iter_mut().chain(self.history.iter_mut()) {
            r.set_theme(theme.clone());
        }
        self.settings.reader.theme = theme;
    }

    /// Returns false when the app should exit.
    fn on_picker_key(&mut self, key: KeyEvent) -> bool {
        let Some((picker, _)) = &mut self.picker else {
            return true;
        };
        let event = picker.on_key(key);
        self.picker_width = picker.width;
        match event {
            picker::Event::None => {}
            picker::Event::Preview(theme) => self.apply_theme(theme),
            picker::Event::Cancel => {
                if let Some((_, before)) = self.picker.take() {
                    self.apply_theme(before);
                }
                return !self.pick_and_exit;
            }
            picker::Event::Pick { name, theme } => {
                self.picker = None;
                self.apply_theme(theme);
                let note = self.save_theme(&name);
                self.themes.current = name;
                if self.pick_and_exit {
                    self.exit_note = Some(note);
                    return false;
                }
                self.set_message(note);
            }
        }
        true
    }

    /// Write `name` to the config file; returns what to tell the user.
    fn save_theme(&self, name: &str) -> String {
        let Some(path) = &self.themes.config else {
            return format!("theme {name} (no config file to save it to)");
        };
        match crate::config::save_theme(path, name) {
            Ok(()) => {
                let mut note = format!("theme {name} saved to {}", path.display());
                if let Some(by) = &self.themes.overridden_by {
                    note.push_str(&format!("; {by} still overrides it"));
                }
                note
            }
            Err(e) => format!("theme {name} not saved: {e:#}"),
        }
    }

    /// Show `next` on top, keeping the current reader for Back.
    fn push_reader(&mut self, next: Result<Reader>) {
        match next {
            Ok(next) => {
                if let Some(prev) = self.reader.replace(next) {
                    prev.remember_position();
                    self.history.push(prev);
                }
                self.note_opened();
            }
            Err(err) => self.set_message(format!("{err:#}")),
        }
    }

    /// Whether `event` only moves the open reader's view: the wheel, or a
    /// scroll key with no Overlay or theme picker open. It changes no
    /// layout, so the next event can be handled before a draw.
    fn only_moves(&self, event: &Event) -> bool {
        let Some(r) = &self.reader else {
            return false;
        };
        match event {
            Event::Mouse(m) => matches!(
                m.kind,
                MouseEventKind::ScrollDown | MouseEventKind::ScrollUp
            ),
            Event::Key(k) => {
                k.kind == KeyEventKind::Press && self.picker.is_none() && r.only_scrolls(k)
            }
            _ => false,
        }
    }

    /// Returns false when the app should exit.
    fn on_event(&mut self, event: &Event) -> bool {
        if let Event::Mouse(m) = event {
            let step = match m.kind {
                MouseEventKind::ScrollDown => 1,
                MouseEventKind::ScrollUp => -1,
                MouseEventKind::Down(MouseButton::Left) => {
                    if let Some((p, _)) = &mut self.picker {
                        p.on_click(m.column, m.row);
                        self.picker_width = p.width;
                    } else if let Some(r) = &mut self.reader {
                        r.on_click(m.column, m.row);
                        self.settings.reader.toc_width = r.toc_width();
                    }
                    return true;
                }
                MouseEventKind::Moved => {
                    if self.picker.is_none()
                        && let Some(r) = &mut self.reader
                    {
                        self.hover_changed |= r.on_hover(m.column, m.row);
                    }
                    return true;
                }
                _ => return true,
            };
            if let Some(r) = &mut self.reader {
                let lines = isize::try_from(self.settings.wheel_lines).unwrap_or(isize::MAX);
                r.scroll_by(step * lines);
            } else if let Some(b) = &mut self.browser {
                b.move_by(step);
            }
            return true;
        }
        let Event::Key(key) = *event else { return true };
        if key.kind != KeyEventKind::Press {
            return true;
        }
        if key.code == KeyCode::Char('c') && nav::ctrl(&key) {
            return false;
        }
        if key.code == KeyCode::Char('z') && nav::ctrl(&key) {
            self.suspend = true;
            return true;
        }
        if self.picker.is_some() {
            return self.on_picker_key(key);
        }

        if let Some(r) = &mut self.reader {
            let action = r.on_key(key);
            // Readers opened later start with the table of contents as wide.
            self.settings.reader.toc_width = r.toc_width();
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
                // No reader is open here, so nothing goes to history.
                self.push_reader(self.settings.open(&path, name));
                true
            }
            browser::Action::Edit(path) => {
                self.edit = Some((path, None));
                true
            }
            browser::Action::PickTheme => {
                self.open_picker();
                true
            }
        }
    }

    fn on_reader_action(&mut self, action: reader::Action) -> bool {
        match action {
            reader::Action::None => {}
            reader::Action::Quit => return false,
            reader::Action::Back => {
                if let Some(r) = &self.reader {
                    r.remember_position();
                }
                self.reader = self.history.pop();
                if let Some(r) = &mut self.reader {
                    r.set_toc_width(self.settings.reader.toc_width);
                }
                self.note_opened();
                return self.reader.is_some() || self.browser.is_some();
            }
            reader::Action::OpenDocument { path, anchor } => {
                let next = self.settings.open(&path, paths::display_name(&path));
                let next = next.map(|mut r| {
                    if let Some(anchor) = anchor {
                        r.go_to_anchor(&anchor);
                    }
                    r
                });
                self.push_reader(next);
            }
            reader::Action::OpenRemote(url) => {
                let next = crate::remote::fetch(&crate::remote::Remote::Url(url))
                    .map(|doc| Reader::new(reader::Source::remote(doc), &self.settings.reader));
                self.push_reader(next);
            }
            reader::Action::OpenExternal(target) => {
                if let Err(err) = links::open_external(&target) {
                    self.set_message(format!("could not open {target}: {err}"));
                }
            }
            reader::Action::Edit(path, line) => self.edit = Some((path, line)),
            reader::Action::PickTheme => self.open_picker(),
            reader::Action::ToggleImages => self.toggle_images(),
            reader::Action::ShowHistory => {
                if !self.settings.history {
                    self.set_message("history is off (history = false)".into());
                } else if let Some(r) = &mut self.reader {
                    r.show_history(self.opened.clone());
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
            r.set_message(msg);
        } else if let Some(b) = &mut self.browser {
            b.message = Some(msg);
        }
    }

    /// Hand the terminal to the user's editor, then take it back. An open
    /// reader picks up the changes through its file watch.
    fn run_editor(&mut self, terminal: &mut Tui, path: &Path, line: Option<usize>) -> Result<()> {
        let cmd = editor::command(self.settings.editor.as_deref());
        let result = with_terminal_released(terminal, self.settings.mouse, || {
            editor::open_with(&cmd, path, line)
        })?;
        match result {
            Ok(status) if status.success() => {}
            // The shell's "command not found" status.
            Ok(status) if status.code() == Some(127) => {
                self.set_message(format!(
                    "editor not found: {cmd} (set `editor` in the config, $VISUAL or $EDITOR)"
                ));
            }
            Ok(status) => self.set_message(format!("{cmd} exited with {status}")),
            Err(err) => self.set_message(format!("could not run {cmd}: {err}")),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
