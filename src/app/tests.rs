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
