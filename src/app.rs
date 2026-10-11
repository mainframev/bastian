use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;
use ratatui::DefaultTerminal;
use ratatui::crossterm::clipboard::CopyToClipboard;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    MouseEventKind,
};
use ratatui::crossterm::execute;

use crate::browser::{self, Browser};
use crate::doc::{self, Document};
use crate::editor;
use crate::exec::Interpreters;
use crate::links;
use crate::picker::{self, ThemePicker};
use crate::prompt;
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
    /// Scroll with the mouse wheel (captures the mouse, so terminal text
    /// selection needs a modifier key).
    pub mouse: bool,
    /// Show the logo at the top of the file browser.
    pub logo: bool,
    /// How the terminal draws pictures, once asked; `None` until then.
    pub images: Option<crate::images::Support>,
    /// Draw pictures for images; off leaves them as their alt text.
    pub show_images: bool,
    /// How the file browser marks the selected file.
    pub selector: crate::config::Selector,
    /// How the file browser lays out each file.
    pub browser_layout: crate::config::BrowserLayout,
}

impl Settings {
    pub fn reader(&self, name: String, doc: Document, base_dir: PathBuf) -> Reader {
        let mut r = Reader::new(name, doc, self.theme.clone(), self.max_width);
        r.base_dir = base_dir;
        r.assume_yes = self.assume_yes;
        r.interpreters = self.interpreters.clone();
        r.line_numbers = self.line_numbers;
        r.images = crate::images::Images::new(self.image_support());
        r
    }

    /// The support readers draw pictures with: `None` while they're off.
    fn image_support(&self) -> Option<crate::images::Support> {
        self.images.clone().filter(|_| self.show_images)
    }

    /// A reader for a fetched document. Its code blocks run in the current
    /// directory and always ask first; it has no file to watch or edit.
    pub fn remote(&self, doc: crate::remote::RemoteDoc) -> Reader {
        let parsed = doc.parse();
        let mut r = self.reader(doc.name, parsed, PathBuf::from("."));
        r.text = doc.text;
        r.remote_base = Some(doc.base);
        // Untrusted source: every run asks first, even with `--yes`.
        r.assume_yes = false;
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
    /// Where themes come from and where a picked one is saved.
    pub themes: ThemeFiles,
    /// The open theme picker, and the theme to restore if it's cancelled.
    picker: Option<(ThemePicker, Theme)>,
    /// Quit once the picker closes (`bastian themes`).
    pub pick_and_exit: bool,
    /// Shown after the TUI exits, e.g. what `bastian themes` saved.
    pub exit_note: Option<String>,
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
        let browser = Browser::new(root, settings.all);
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
                    Err(err) => b.message = Some(format!("{err:#}")),
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
                self.reader = self.history.pop();
                return self.reader.is_some() || self.browser.is_some();
            }
            reader::Action::OpenDocument(path) => {
                let next = self.settings.open(&path, display_name(&path));
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
        let cmd = editor::command();
        let result = with_terminal_released(terminal, self.settings.mouse, || {
            editor::open_with(&cmd, path, line)
        })?;
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

/// Take over the terminal: ratatui's setup, plus mouse capture when the
/// mouse option is on. Undo with `leave_terminal`.
pub fn enter_terminal(mouse: bool) -> Result<DefaultTerminal> {
    let terminal = ratatui::init();
    if mouse {
        execute!(std::io::stdout(), EnableMouseCapture)?;
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
fn with_terminal_released<T>(
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

/// Lines the reader scrolls per mouse-wheel step.
const WHEEL_LINES: isize = 3;

/// Stop like a shell job (ctrl-z), restoring the terminal first and taking
/// it back when resumed with `fg`.
fn suspend(terminal: &mut DefaultTerminal, mouse: bool) -> Result<()> {
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

/// Events that change nothing on screen: pointer motion and clicks (only
/// the wheel scrolls) and key releases.
fn is_ignored(event: &Event) -> bool {
    match event {
        Event::Mouse(m) => !matches!(
            m.kind,
            MouseEventKind::ScrollDown | MouseEventKind::ScrollUp
        ),
        Event::Key(k) => k.kind != KeyEventKind::Press,
        _ => false,
    }
}

/// How often to wake up for process output while waiting for keys.
const TICK: Duration = Duration::from_millis(100);

/// Run the TUI until the user quits. Returns a note to print afterwards.
pub fn run(terminal: &mut DefaultTerminal, mut app: App) -> Result<Option<String>> {
    let mut redraw = true;
    loop {
        if let Some(r) = &mut app.reader {
            redraw |= r.tick();
        }
        if let Some(b) = &mut app.browser {
            redraw |= b.tick();
        }
        if std::mem::take(&mut redraw) {
            terminal.draw(|f| {
                if let Some(r) = &mut app.reader {
                    ui::reader::draw(f, r);
                } else if let Some(b) = &mut app.browser {
                    let s = &app.settings;
                    let look = ui::browser::Look {
                        logo: s.logo,
                        selector: s.selector,
                        layout: s.browser_layout,
                    };
                    ui::browser::draw(f, b, &s.theme, look);
                }
                if let Some((p, _)) = &app.picker {
                    ui::picker::draw(f, &app.settings.theme, p);
                }
            })?;
        }
        if event::poll(TICK)? {
            let event = event::read()?;
            let ignored = is_ignored(&event);
            if !app.on_event(&event) {
                return Ok(app.exit_note.take());
            }
            if let Some((path, line)) = app.edit.take() {
                app.run_editor(terminal, &path, line)?;
                redraw = true;
            }
            if std::mem::take(&mut app.suspend) {
                suspend(terminal, app.settings.mouse)?;
                redraw = true;
            }
            redraw |= !ignored;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{KeyModifiers, MouseEvent};

    fn wheel(kind: MouseEventKind) -> Event {
        Event::Mouse(MouseEvent {
            kind,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        })
    }

    fn settings() -> Settings {
        Settings {
            theme: Theme::dark(),
            max_width: 80,
            assume_yes: false,
            interpreters: Interpreters::default(),
            line_numbers: false,
            all: false,
            mouse: true,
            logo: true,
            images: None,
            show_images: false,
            selector: crate::config::Selector::Bar,
            browser_layout: crate::config::BrowserLayout::Cards,
        }
    }

    fn press(app: &mut App, code: KeyCode) -> bool {
        app.on_event(&Event::Key(KeyEvent::new(code, KeyModifiers::NONE)))
    }

    /// An app reading a short document, with theme files and a config file
    /// in `dir`.
    fn reading(dir: &Path, current: &str) -> App {
        let s = settings();
        let reader = s.reader("t".into(), doc::parse("# A\n\ntext\n"), PathBuf::from("."));
        let mut app = App::read(reader, s);
        app.themes = ThemeFiles {
            dir: Some(dir.join("themes")),
            config: Some(dir.join("config.toml")),
            current: current.into(),
            overridden_by: None,
        };
        app
    }

    #[test]
    fn theme_picker_previews_and_cancels() {
        let dir = tempfile::tempdir().unwrap();
        // Next in the list is dracula, which looks nothing like nord.
        let mut app = reading(dir.path(), "nord");
        app.apply_theme(crate::themes::load("nord", None).unwrap());
        let before = app.settings.theme.accent;
        assert!(press(&mut app, KeyCode::Char('T')));
        assert!(app.picker.is_some());
        press(&mut app, KeyCode::Char('j'));
        // The whole app previews the highlighted theme...
        let preview = app.settings.theme.accent;
        assert_ne!(preview, before);
        assert_eq!(app.reader.as_ref().unwrap().theme.accent, preview);
        // ...and Esc puts the old one back without saving anything.
        press(&mut app, KeyCode::Esc);
        assert!(app.picker.is_none());
        assert_eq!(app.settings.theme.accent, before);
        assert_eq!(app.reader.as_ref().unwrap().theme.accent, before);
        assert!(!dir.path().join("config.toml").exists());
    }

    #[test]
    fn picking_a_theme_saves_it() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), "# mine\nwidth = 90\n").unwrap();
        let mut app = reading(dir.path(), "rose-pine");
        press(&mut app, KeyCode::Char('T'));
        press(&mut app, KeyCode::Char('g'));
        assert!(press(&mut app, KeyCode::Enter));
        let first = crate::themes::builtin_names().next().unwrap();
        assert_eq!(app.themes.current, first);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("config.toml")).unwrap(),
            format!("# mine\ntheme = \"{first}\"\nwidth = 90\n")
        );
        let msg = app.reader.as_ref().unwrap().message.clone().unwrap();
        assert!(msg.contains("saved to"), "{msg}");

        // With --theme in effect, saving says the flag still wins.
        let mut app = reading(dir.path(), "rose-pine");
        app.themes.overridden_by = Some("--theme".into());
        press(&mut app, KeyCode::Char('T'));
        press(&mut app, KeyCode::Enter);
        let msg = app.reader.as_ref().unwrap().message.clone().unwrap();
        assert!(msg.ends_with("--theme still overrides it"), "{msg}");
    }

    #[test]
    fn bastian_themes_quits_once_a_theme_is_picked() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = reading(dir.path(), "nord");
        app.pick_and_exit = true;
        app.open_picker();
        assert!(!press(&mut app, KeyCode::Enter), "picking ends the app");
        assert!(
            app.exit_note
                .as_deref()
                .unwrap()
                .starts_with("theme nord saved")
        );

        let mut app = reading(dir.path(), "nord");
        app.pick_and_exit = true;
        app.open_picker();
        assert!(!press(&mut app, KeyCode::Esc), "cancelling ends it too");
    }

    #[test]
    fn mouse_wheel_scrolls_the_reader() {
        let settings = Settings {
            theme: Theme::dark(),
            max_width: 80,
            assume_yes: false,
            interpreters: Interpreters::default(),
            line_numbers: false,
            all: false,
            mouse: true,
            logo: true,
            images: None,
            show_images: false,
            selector: crate::config::Selector::Bar,
            browser_layout: crate::config::BrowserLayout::Cards,
        };
        let src: String = (0..50).map(|i| format!("line {i}\n\n")).collect();
        let mut reader = settings.reader("t".into(), doc::parse(&src), PathBuf::from("."));
        reader.layout(80, 10);
        let mut app = App::read(reader, settings);
        assert!(app.on_event(&wheel(MouseEventKind::ScrollDown)));
        assert!(app.on_event(&wheel(MouseEventKind::ScrollDown)));
        assert_eq!(app.reader.as_ref().unwrap().scroll, 6);
        app.on_event(&wheel(MouseEventKind::ScrollUp));
        assert_eq!(app.reader.as_ref().unwrap().scroll, 3);
    }

    #[test]
    fn i_toggles_pictures_in_every_reader() {
        use crate::images::Support;
        use ratatui_image::picker::Picker;

        let dir = tempfile::tempdir().unwrap();
        image::RgbImage::from_pixel(40, 40, image::Rgb([220, 40, 40]))
            .save(dir.path().join("p.png"))
            .unwrap();
        let mut s = settings();
        s.images = Some(Support::fixed(Picker::halfblocks()));
        s.show_images = true;
        let reader = s.reader(
            "t".into(),
            doc::parse("![a red square](p.png)\n"),
            dir.path().to_path_buf(),
        );
        let mut app = App::read(reader, s);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let r = app.reader.as_mut().unwrap();
            r.layout(60, 20);
            if !r.rendered.images.is_empty() {
                break;
            }
            r.tick();
            assert!(std::time::Instant::now() < deadline, "picture did not load");
            std::thread::sleep(Duration::from_millis(10));
        }
        let text = |app: &mut App| {
            let r = app.reader.as_mut().unwrap();
            r.layout(60, 20);
            r.rendered
                .lines
                .iter()
                .map(crate::render::plain)
                .collect::<Vec<_>>()
                .join("\n")
        };

        press(&mut app, KeyCode::Char('i'));
        let off = text(&mut app);
        let r = app.reader.as_ref().unwrap();
        assert!(r.rendered.images.is_empty());
        assert!(off.contains("a red square"), "{off}");
        assert_eq!(r.message.as_deref(), Some("images off"));
        // Readers opened afterwards start with pictures off too.
        assert!(
            !app.settings
                .reader("u".into(), doc::parse(""), dir.path().into())
                .images
                .enabled()
        );

        press(&mut app, KeyCode::Char('i'));
        text(&mut app);
        let r = app.reader.as_ref().unwrap();
        assert_eq!(r.rendered.images.len(), 1);
        assert_eq!(r.message.as_deref(), Some("images on"));
    }
}
