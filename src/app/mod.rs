mod run;
mod settings;
mod terminal;

pub use run::run;
pub use settings::Settings;
pub use terminal::{enter_terminal, leave_terminal};

use std::path::{Path, PathBuf};

use anyhow::Result;
use ratatui::DefaultTerminal;
use ratatui::crossterm::clipboard::CopyToClipboard;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, MouseEventKind};
use ratatui::crossterm::execute;

use crate::browser::{self, Browser};
use crate::editor;
use crate::links;
use crate::paths;
use crate::picker::{self, ThemePicker};
use crate::prompt;
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
    /// File to open in the editor once the current event is handled.
    edit: Option<(PathBuf, Option<usize>)>,
    /// Set by ctrl-z; the run loop suspends bastian like a shell job.
    suspend: bool,
    /// Where themes come from and where a picked one is saved.
    pub themes: ThemeFiles,
    /// The open theme picker, and the theme to restore if it's cancelled.
    picker: Option<(ThemePicker, Theme)>,
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
        Self {
            settings,
            browser,
            reader,
            history: Vec::new(),
            edit: None,
            suspend: false,
            themes: ThemeFiles::default(),
            picker: None,
            pick_and_exit: false,
            exit_note: None,
        }
    }

    pub fn open_picker(&mut self) {
        let picker = ThemePicker::new(self.themes.dir.as_deref(), &self.themes.current);
        self.picker = Some((picker, self.settings.theme.clone()));
    }

    /// Switch pictures on or off in every reader, asking the terminal how
    /// to draw them the first time they're turned on.
    fn toggle_images(&mut self) {
        let s = &mut self.settings;
        s.show_images = !s.show_images;
        if s.show_images && s.images.is_none() {
            s.images = Some(crate::images::Support::detect());
        }
        let support = s.image_support();
        for r in self.reader.iter_mut().chain(self.history.iter_mut()) {
            r.set_images(support.clone());
        }
        let state = if self.settings.show_images {
            "on"
        } else {
            "off"
        };
        self.set_message(format!("images {state}"));
    }

    /// Use `theme` everywhere: the browser, the open reader and history.
    fn apply_theme(&mut self, theme: Theme) {
        for r in self.reader.iter_mut().chain(self.history.iter_mut()) {
            r.set_theme(theme.clone());
        }
        self.settings.theme = theme;
    }

    /// Returns false when the app should exit.
    fn on_picker_key(&mut self, key: KeyEvent) -> bool {
        let Some((picker, _)) = &mut self.picker else {
            return true;
        };
        match picker.on_key(key) {
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
            }
            Err(err) => self.set_message(format!("{err:#}")),
        }
    }

    /// Returns false when the app should exit.
    fn on_event(&mut self, event: &Event) -> bool {
        if let Event::Mouse(m) = event {
            let step = match m.kind {
                MouseEventKind::ScrollDown => 1,
                MouseEventKind::ScrollUp => -1,
                _ => return true,
            };
            if let Some(r) = &mut self.reader {
                r.scroll_by(step * WHEEL_LINES);
            } else if let Some(b) = &mut self.browser {
                b.move_by(step);
            }
            return true;
        }
        let Event::Key(key) = *event else { return true };
        if key.kind != KeyEventKind::Press {
            return true;
        }
        if key.code == KeyCode::Char('c') && prompt::ctrl(&key) {
            return false;
        }
        if key.code == KeyCode::Char('z') && prompt::ctrl(&key) {
            self.suspend = true;
            return true;
        }
        if self.picker.is_some() {
            return self.on_picker_key(key);
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
                    Err(err) => self.set_message(format!("{err:#}")),
                }
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
                let next = crate::remote::fetch(&crate::remote::Source::Url(url))
                    .map(|doc| self.settings.remote(doc));
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

/// Lines the reader scrolls per mouse-wheel step.
const WHEEL_LINES: isize = 3;

#[cfg(test)]
mod tests;
