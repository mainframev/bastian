use std::time::Duration;

use super::*;
use crate::{doc, reader, render};
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
        reader: reader::Options::default(),
        scan: browser::Scan::default(),
        date_format: Some(crate::config::DATE_FORMAT.into()),
        editor: None,
        mouse: true,
        logo: true,
        images: None,
        show_images: false,
        history: true,
        selector: crate::config::Selector::Bar,
        list_layout: crate::config::ListLayout::Cards,
        background: crate::config::PageBackground::Theme,
    }
}

/// A reader over `doc` opened with `s`, with code running in `base_dir`.
fn reader_in(s: &Settings, doc: doc::Document, base_dir: PathBuf) -> Reader {
    let source = reader::Source {
        base_dir,
        ..reader::Source::in_memory("t", doc)
    };
    Reader::new(source, &s.reader)
}

fn press(app: &mut App, code: KeyCode) -> bool {
    app.on_event(&Event::Key(KeyEvent::new(code, KeyModifiers::NONE)))
}

/// An app reading a short document, with theme files and a config file
/// in `dir`.
fn reading(dir: &Path, current: &str) -> App {
    let s = settings();
    let reader = reader_in(&s, doc::parse("# A\n\ntext\n"), PathBuf::from("."));
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
fn keeping_the_terminal_background_holds_through_theme_changes() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = reading(dir.path(), "nord");
    app.settings.background = crate::config::PageBackground::Terminal;
    app.apply_theme(crate::themes::load("nord", None).unwrap());
    let page = |app: &App| app.reader.as_ref().unwrap().theme().background;
    assert_eq!(page(&app), ratatui::style::Color::Reset);
    // Previewing another theme in the picker keeps it too.
    press(&mut app, KeyCode::Char('T'));
    press(&mut app, KeyCode::Char('j'));
    assert_ne!(
        app.settings.reader.theme.accent,
        crate::themes::load("nord", None).unwrap().accent
    );
    assert_eq!(page(&app), ratatui::style::Color::Reset);
    assert_eq!(
        app.settings.reader.theme.background,
        ratatui::style::Color::Reset
    );

    // By default the theme's own is painted.
    let mut app = reading(dir.path(), "nord");
    app.apply_theme(crate::themes::load("nord", None).unwrap());
    assert_eq!(page(&app), ratatui::style::Color::Rgb(0x2e, 0x34, 0x40));
}

#[test]
fn theme_picker_previews_and_cancels() {
    let dir = tempfile::tempdir().unwrap();
    // Next in the list is dracula, which looks nothing like nord.
    let mut app = reading(dir.path(), "nord");
    app.apply_theme(crate::themes::load("nord", None).unwrap());
    let before = app.settings.reader.theme.accent;
    assert!(press(&mut app, KeyCode::Char('T')));
    assert!(app.picker.is_some());
    press(&mut app, KeyCode::Char('j'));
    // The whole app previews the highlighted theme...
    let preview = app.settings.reader.theme.accent;
    assert_ne!(preview, before);
    assert_eq!(app.reader.as_ref().unwrap().theme().accent, preview);
    // ...and Esc puts the old one back without saving anything.
    press(&mut app, KeyCode::Esc);
    assert!(app.picker.is_none());
    assert_eq!(app.settings.reader.theme.accent, before);
    assert_eq!(app.reader.as_ref().unwrap().theme().accent, before);
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
    let msg = app.reader.as_ref().unwrap().message().unwrap().to_string();
    assert!(msg.contains("saved to"), "{msg}");

    // With --theme in effect, saving says the flag still wins.
    let mut app = reading(dir.path(), "rose-pine");
    app.themes.overridden_by = Some("--theme".into());
    press(&mut app, KeyCode::Char('T'));
    press(&mut app, KeyCode::Enter);
    let msg = app.reader.as_ref().unwrap().message().unwrap().to_string();
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
fn theme_picker_width_carries_to_the_next_time_it_opens() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = reading(dir.path(), "nord");
    app.reader.as_mut().unwrap().layout(80, 10);
    press(&mut app, KeyCode::Char('t'));
    assert!(app.reader.as_ref().unwrap().toc);
    press(&mut app, KeyCode::Char('T'));
    // Sized against the screen at the last draw.
    app.picker.as_mut().unwrap().0.fit(100);
    press(&mut app, KeyCode::Char('>'));
    assert!(app.picker.is_some(), "resizing keeps it open");
    // The picker has the keys: the table of contents stays as it was.
    let toc = app.reader.as_ref().unwrap().toc_width();
    assert_eq!(toc, crate::toc::DEFAULT_WIDTH);
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Char('T'));
    let wider = picker::DEFAULT_WIDTH + crate::pane::STEP;
    assert_eq!(app.picker.as_ref().unwrap().0.width, wider);
}

#[test]
fn mouse_wheel_scrolls_the_reader() {
    let settings = settings();
    let src: String = (0..50).map(|i| format!("line {i}\n\n")).collect();
    let mut reader = reader_in(&settings, doc::parse(&src), PathBuf::from("."));
    reader.layout(80, 10);
    let mut app = App::read(reader, settings);
    assert!(app.on_event(&wheel(MouseEventKind::ScrollDown)));
    assert!(app.on_event(&wheel(MouseEventKind::ScrollDown)));
    assert_eq!(app.reader.as_ref().unwrap().scroll(), 6);
    app.on_event(&wheel(MouseEventKind::ScrollUp));
    assert_eq!(app.reader.as_ref().unwrap().scroll(), 3);
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
    s.reader.images = s.image_support();
    let reader = reader_in(
        &s,
        doc::parse("![a red square](p.png)\n"),
        dir.path().to_path_buf(),
    );
    let mut app = App::read(reader, s);
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let r = app.reader.as_mut().unwrap();
        r.layout(60, 20);
        if !r.rendered().images.is_empty() {
            break;
        }
        r.tick();
        assert!(std::time::Instant::now() < deadline, "picture did not load");
        std::thread::sleep(Duration::from_millis(10));
    }
    let text = |app: &mut App| {
        let r = app.reader.as_mut().unwrap();
        r.layout(60, 20);
        r.rendered()
            .lines
            .iter()
            .map(render::plain)
            .collect::<Vec<_>>()
            .join("\n")
    };

    press(&mut app, KeyCode::Char('i'));
    let off = text(&mut app);
    let r = app.reader.as_ref().unwrap();
    assert!(r.rendered().images.is_empty(), "{:?}", r.rendered().images);
    assert!(off.contains("a red square"), "{off}");
    assert_eq!(r.message(), Some("images off"));
    // Readers opened afterwards start with pictures off too.
    assert!(
        !reader_in(&app.settings, doc::parse(""), dir.path().into())
            .images
            .enabled()
    );

    press(&mut app, KeyCode::Char('i'));
    text(&mut app);
    let r = app.reader.as_ref().unwrap();
    assert_eq!(r.rendered().images.len(), 1);
    assert_eq!(r.message(), Some("images on"));
}

#[test]
fn toc_width_carries_to_readers_opened_later() {
    let mut app = reading(Path::new("."), "nord");
    let r = app.reader.as_mut().unwrap();
    r.layout(80, 10);
    r.fit_toc(100);
    press(&mut app, KeyCode::Char('t'));
    press(&mut app, KeyCode::Char('>'));
    let wider = crate::toc::DEFAULT_WIDTH + crate::pane::STEP;
    assert_eq!(app.settings.reader.toc_width, wider);
    let next = reader_in(&app.settings, doc::parse("# B\n"), PathBuf::from("."));
    assert_eq!(next.toc_width(), wider);

    // Narrowed in the next one, the previous one comes back as narrow.
    app.push_reader(Ok(next));
    let r = app.reader.as_mut().unwrap();
    r.layout(80, 10);
    r.fit_toc(100);
    press(&mut app, KeyCode::Char('t'));
    press(&mut app, KeyCode::Char('<'));
    press(&mut app, KeyCode::Char('<'));
    press(&mut app, KeyCode::Esc);
    let narrower = crate::toc::DEFAULT_WIDTH - crate::pane::STEP;
    assert_eq!(app.reader.as_ref().unwrap().toc_width(), narrower);
}

#[test]
fn clicking_the_toc_markers_resizes_it() {
    let mut app = reading(Path::new("."), "nord");
    let r = app.reader.as_mut().unwrap();
    r.layout(80, 10);
    press(&mut app, KeyCode::Char('t'));
    let width = app.reader.as_mut().unwrap().fit_toc(100);
    let (_, wider) = crate::pane::markers(width);
    let click = Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: wider,
        row: 0,
        modifiers: KeyModifiers::NONE,
    });
    assert!(!run::is_ignored(&click));
    assert!(app.on_event(&click));
    assert_eq!(app.settings.reader.toc_width, width + crate::pane::STEP);
}

#[test]
fn clicking_the_theme_picker_markers_resizes_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = reading(dir.path(), "nord");
    press(&mut app, KeyCode::Char('T'));
    let width = app.picker.as_mut().unwrap().0.fit(100);
    let (_, wider) = crate::pane::markers(width);
    let click = Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 100 - width + wider,
        row: 0,
        modifiers: KeyModifiers::NONE,
    });
    assert!(app.on_event(&click));
    assert_eq!(app.picker_width, width + crate::pane::STEP);
    assert!(app.picker.is_some());
}

#[test]
fn history_lists_opened_files_newest_first_and_reopens_one() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (dir.path().join("a.md"), dir.path().join("b.md"));
    std::fs::write(&a, "# A\n").unwrap();
    std::fs::write(&b, "# B\n").unwrap();
    let s = settings();
    let reader = s.open(&a, "a.md".into()).unwrap();
    let mut app = App::read(reader, s);
    app.on_reader_action(reader::Action::OpenDocument {
        path: b.clone(),
        anchor: None,
    });
    press(&mut app, KeyCode::Char('H'));
    let Some(reader::Overlay::History(picker)) = &app.reader.as_ref().unwrap().overlay else {
        panic!("no history");
    };
    assert_eq!(picker.paths, [b.clone(), a.clone()]);
    // This document is marked; the one before it is selected.
    assert_eq!((picker.current, picker.selected.get()), (Some(0), 1));
    press(&mut app, KeyCode::Enter);
    let open = app.reader.as_ref().unwrap().file().map(reader::canonical);
    assert_eq!(open, Some(reader::canonical(&a)));
    assert_eq!(app.opened, [a, b]);
}

#[test]
fn history_off_keeps_no_list_but_back_still_works() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (dir.path().join("a.md"), dir.path().join("b.md"));
    std::fs::write(&a, "# A\n").unwrap();
    std::fs::write(&b, "# B\n").unwrap();
    let s = Settings {
        history: false,
        ..settings()
    };
    let reader = s.open(&a, "a.md".into()).unwrap();
    let mut app = App::read(reader, s);
    app.on_reader_action(reader::Action::OpenDocument {
        path: b,
        anchor: None,
    });
    assert!(app.opened.is_empty());
    press(&mut app, KeyCode::Char('H'));
    let r = app.reader.as_ref().unwrap();
    assert!(r.overlay.is_none());
    assert_eq!(r.message(), Some("history is off (history = false)"));
    // Going back from the opened document doesn't depend on the list.
    press(&mut app, KeyCode::Esc);
    let open = app.reader.as_ref().unwrap().file().map(reader::canonical);
    assert_eq!(open, Some(reader::canonical(&a)));
}

#[cfg(unix)]
#[test]
fn history_reopens_a_symlink_by_its_own_path() {
    let dir = tempfile::tempdir().unwrap();
    let (docs, shared) = (dir.path().join("docs"), dir.path().join("shared"));
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::create_dir_all(&shared).unwrap();
    std::fs::write(shared.join("guide.md"), "# Guide\n").unwrap();
    let link = docs.join("guide.md");
    std::os::unix::fs::symlink(shared.join("guide.md"), &link).unwrap();
    let other = dir.path().join("other.md");
    std::fs::write(&other, "# Other\n").unwrap();
    let s = settings();
    let reader = s.open(&link, "guide.md".into()).unwrap();
    let mut app = App::read(reader, s);
    app.on_reader_action(reader::Action::OpenDocument {
        path: other.clone(),
        anchor: None,
    });
    // The target by its own path is the same file: one entry, newest path.
    app.on_reader_action(reader::Action::OpenDocument {
        path: shared.join("guide.md"),
        anchor: None,
    });
    assert_eq!(app.opened, [shared.join("guide.md"), other.clone()]);
    app.on_reader_action(reader::Action::OpenDocument {
        path: link.clone(),
        anchor: None,
    });
    assert_eq!(app.opened, [link.clone(), other.clone()]);
    // From the other file, H selects the guide; Enter reopens it by the
    // symlink's path, so it keeps the symlink's directory.
    app.on_reader_action(reader::Action::OpenDocument {
        path: other,
        anchor: None,
    });
    press(&mut app, KeyCode::Char('H'));
    press(&mut app, KeyCode::Enter);
    let r = app.reader.as_ref().unwrap();
    assert_eq!(r.file(), Some(link.as_path()));
    assert_eq!(r.base_dir(), docs.as_path());
}

#[test]
fn pointer_motion_redraws_only_when_the_hovered_link_changes() {
    let s = settings();
    let reader = reader_in(
        &s,
        doc::parse("[docs](https://x.dev)\n"),
        PathBuf::from("."),
    );
    let mut app = App::read(reader, s);
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(60, 8)).unwrap();
    terminal
        .draw(|f| crate::ui::reader::draw(f, app.reader.as_mut().unwrap()))
        .unwrap();
    let r = app.reader.as_ref().unwrap();
    let link = &r.rendered().links[0];
    let x = crate::ui::reader::MARGIN
        + crate::ui::reader::gutter_width(r, r.rendered().lines.len())
        + link.col as u16;
    let y = 1 + link.line as u16;
    let moved = |column, row| {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Moved,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        })
    };
    // Motion alone never asks for a redraw; the hover change does.
    assert!(run::is_ignored(&moved(x, y)));
    assert!(app.on_event(&moved(x, y)));
    assert!(std::mem::take(&mut app.hover_changed));
    assert!(app.on_event(&moved(x + 1, y)));
    assert!(!app.hover_changed, "still on the same link");
    assert!(app.on_event(&moved(0, 0)));
    assert!(std::mem::take(&mut app.hover_changed));

    // The theme picker takes the mouse.
    press(&mut app, KeyCode::Char('T'));
    assert!(app.on_event(&moved(x, y)));
    assert!(!app.hover_changed);
}

/// A writer that keeps each write it's given, for checking what reaches
/// the terminal and in how many pieces. With `fail_first`, the first write
/// fails, as a closed terminal would.
#[derive(Clone, Default)]
struct Writes {
    chunks: std::rc::Rc<std::cell::RefCell<Vec<Vec<u8>>>>,
    fail_first: std::rc::Rc<std::cell::Cell<bool>>,
}

impl std::io::Write for Writes {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.fail_first.replace(false) {
            return Err(std::io::Error::other("terminal gone"));
        }
        self.chunks.borrow_mut().push(buf.to_vec());
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Writes {
    fn chunks(&self) -> Vec<Vec<u8>> {
        self.chunks.borrow().clone()
    }
}

/// A terminal of `width` by `height` drawing into `out`.
fn screen(out: &Writes, width: u16, height: u16) -> ratatui::Terminal<screen::Screen<Writes>> {
    let size = ratatui::layout::Size::new(width, height);
    ratatui::Terminal::new(screen::Screen::new(out.clone(), size)).unwrap()
}

const BEGIN_SYNC: &[u8] = b"\x1b[?2026h";
const END_SYNC: &[u8] = b"\x1b[?2026l";

#[test]
fn a_frame_goes_out_in_one_write_as_one_synchronized_update() {
    let out = Writes::default();
    let mut terminal = screen(&out, 20, 3);
    let hello = |f: &mut ratatui::Frame| {
        f.render_widget(ratatui::widgets::Paragraph::new("hello"), f.area());
    };
    screen::draw_frame(&mut terminal, hello).unwrap();
    let chunks = out.chunks();
    assert_eq!(chunks.len(), 1, "{chunks:?}");
    let frame = &chunks[0];
    assert!(frame.starts_with(BEGIN_SYNC), "{frame:?}");
    assert!(frame.ends_with(END_SYNC), "{frame:?}");
    assert!(frame.windows(5).any(|w| w == b"hello"));

    // Nothing changed: the next frame is still one bracketed write.
    screen::draw_frame(&mut terminal, hello).unwrap();
    let chunks = out.chunks();
    assert_eq!(chunks.len(), 2);
    assert!(chunks[1].starts_with(BEGIN_SYNC) && chunks[1].ends_with(END_SYNC));
}

#[test]
fn a_render_that_panics_opens_no_synchronized_update() {
    let out = Writes::default();
    let mut terminal = screen(&out, 20, 3);
    screen::draw_frame(&mut terminal, |_| {}).unwrap();
    let drawn = out.chunks().len();
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = screen::draw_frame(&mut terminal, |_| panic!("render failed"));
    }));
    assert!(panicked.is_err());
    // Dropping the terminal shows the cursor again, outside any update.
    drop(terminal);
    let written = out.chunks()[drawn..].concat();
    assert!(!written.is_empty());
    assert!(!written.windows(BEGIN_SYNC.len()).any(|w| w == BEGIN_SYNC));
}

#[test]
fn a_failed_draw_still_ends_the_synchronized_update() {
    let out = Writes::default();
    out.fail_first.set(true);
    let mut terminal = screen(&out, 300, 300);
    // Every cell a different colour: far more than the frame buffer holds,
    // so the failing write comes part way through the frame.
    let result = screen::draw_frame(&mut terminal, |f| {
        let area = f.area();
        for (i, pos) in area.positions().enumerate() {
            let [r, g, b, _] = (i as u32).to_le_bytes();
            f.buffer_mut()[pos]
                .set_char('x')
                .set_fg(ratatui::style::Color::Rgb(r, g, b));
        }
    });
    assert!(result.is_err());
    let chunks = out.chunks();
    let last = chunks.last().expect("the rest of the frame went out");
    assert!(last.ends_with(END_SYNC), "{:?}", &last[last.len() - 16..]);
}

#[test]
fn a_resize_lays_the_next_frame_out_at_the_new_size() {
    let s = settings();
    let words = "word ".repeat(200);
    let reader = reader_in(&s, doc::parse(&words), PathBuf::from("."));
    let mut app = App::read(reader, s);
    let out = Writes::default();
    let mut terminal = screen(&out, 80, 24);
    run::draw(&mut terminal, &mut app).unwrap();
    let wide = app.reader.as_ref().unwrap().rendered().lines.len();

    // The size comes from the resize event, not from asking the terminal.
    terminal
        .backend_mut()
        .resize(ratatui::layout::Size::new(30, 10));
    run::draw(&mut terminal, &mut app).unwrap();
    assert_eq!(
        terminal.get_frame().area(),
        ratatui::layout::Rect::new(0, 0, 30, 10)
    );
    let narrow = app.reader.as_ref().unwrap().rendered().lines.len();
    assert!(
        narrow > wide,
        "rewrapped: {wide} lines at 80, {narrow} at 30"
    );
    // The old frame is cleared away before the new one is drawn.
    let frame = out.chunks().pop().unwrap();
    assert!(frame.windows(4).any(|w| w == b"\x1b[2J"), "{frame:?}");
}

impl run::Input for std::collections::VecDeque<Event> {
    fn ready(&mut self) -> std::io::Result<bool> {
        Ok(!self.is_empty())
    }

    fn read(&mut self) -> std::io::Result<Event> {
        self.pop_front()
            .ok_or_else(|| std::io::Error::other("no event waiting"))
    }
}

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

/// An app reading 50 short paragraphs, laid out at 80x10.
fn long_read() -> App {
    let s = settings();
    let src: String = (0..50).map(|i| format!("line {i}\n\n")).collect();
    let mut reader = reader_in(&s, doc::parse(&src), PathBuf::from("."));
    reader.layout(80, 10);
    App::read(reader, s)
}

/// Handle `events` as the run loop does: the first, then a batch of those
/// waiting behind it. Returns what was handled and the events left.
fn batch(app: &mut App, events: &[Event]) -> (run::Handled, Vec<Event>) {
    let mut waiting: std::collections::VecDeque<Event> = events.iter().cloned().collect();
    let first = waiting.pop_front().unwrap();
    let handled = run::handle_batch(app, &mut waiting, first).unwrap();
    (handled, waiting.into())
}

#[test]
fn waiting_scroll_keys_are_handled_together_in_order() {
    let mut app = long_read();
    let end = app.reader.as_ref().unwrap().max_scroll();
    let (handled, left) = batch(
        &mut app,
        &[
            key(KeyCode::Char('G')),
            key(KeyCode::Char('k')),
            key(KeyCode::Char('k')),
        ],
    );
    assert!(left.is_empty());
    assert!(handled.redraw && !handled.quit);
    // G first, then up twice: in any other order the view would end at G.
    assert_eq!(app.reader.as_ref().unwrap().scroll(), end - 2);
}

#[test]
fn a_batch_stops_at_the_cap() {
    let mut app = long_read();
    let held = vec![wheel(MouseEventKind::ScrollDown); run::BATCH + 10];
    let (_, left) = batch(&mut app, &held);
    assert_eq!(left.len(), 10);
}

#[test]
fn anything_but_a_scroll_waits_for_the_frame_before_it() {
    let j = || key(KeyCode::Char('j'));
    // Line numbers narrow the text, and s picks a link from the view: they
    // wait for the frame that shows the j before them.
    for code in [KeyCode::Char('#'), KeyCode::Char('s')] {
        let mut app = long_read();
        let (handled, left) = batch(&mut app, &[j(), key(code), j()]);
        assert!(handled.redraw);
        assert_eq!(handled.next, Some(key(code)));
        assert_eq!(left, [j()]);
        let r = app.reader.as_ref().unwrap();
        assert!(!r.line_numbers && r.link_focus.is_none());
    }

    // So does a resize.
    let mut app = long_read();
    let (handled, left) = batch(&mut app, &[j(), Event::Resize(40, 12), j()]);
    assert_eq!(handled.resize, None);
    assert_eq!(handled.next, Some(Event::Resize(40, 12)));
    assert_eq!(left, [j()]);
    // Handled first in the next batch, it's passed on, and drawn before
    // the keys behind it.
    let (handled, left) = batch(&mut app, &[Event::Resize(40, 12), j()]);
    assert_eq!(handled.resize, Some(ratatui::layout::Size::new(40, 12)));
    assert_eq!(left, [j()]);

    // Quitting reads nothing after it.
    let mut app = long_read();
    let (handled, left) = batch(&mut app, &[key(KeyCode::Char('q')), j()]);
    assert!(handled.quit);
    assert_eq!(left, [j()]);

    // An Overlay takes the keys after it: they wait for it to be drawn.
    let mut app = long_read();
    let slash = key(KeyCode::Char('/'));
    let (handled, left) = batch(&mut app, &[slash, j(), j()]);
    assert_eq!(handled.next, None);
    assert_eq!(left, [j(), j()]);
}

#[test]
fn keys_counted_as_scrolling_only_scroll() {
    let codes = (' '..='~').map(KeyCode::Char).chain([
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::PageUp,
        KeyCode::PageDown,
        KeyCode::Home,
        KeyCode::End,
        KeyCode::Enter,
        KeyCode::Esc,
        KeyCode::Tab,
    ]);
    let mut scrolling = 0;
    for code in codes {
        for modifiers in [KeyModifiers::NONE, KeyModifiers::CONTROL] {
            let mut app = long_read();
            let k = KeyEvent::new(code, modifiers);
            if !app.only_moves(&Event::Key(k)) {
                continue;
            }
            scrolling += 1;
            assert!(app.on_event(&Event::Key(k)), "{k:?}");
            let r = app.reader.as_ref().unwrap();
            let still = r.overlay.is_none()
                && r.focus.is_none()
                && r.link_focus.is_none()
                && r.details_focus.is_none()
                && r.search.is_none()
                && !r.toc
                && !r.line_numbers
                && r.message().is_none()
                && app.edit.is_none()
                && !app.suspend
                && app.picker.is_none();
            assert!(still, "{k:?} did more than scroll");
        }
    }
    // j k space f b g G, ctrl-d ctrl-u, and the arrows and paging keys.
    assert!(scrolling >= 15, "{scrolling}");
}
