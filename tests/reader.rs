use std::thread;
use std::time::Duration;

use bastian::app::Settings;
use bastian::doc::parse;
use bastian::exec::Interpreters;
use bastian::reader::{Action, Reader};
use bastian::remote::RemoteDoc;
use bastian::render::theme::Theme;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(r: &mut Reader, code: KeyCode) -> Action {
    r.on_key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn reader(src: &str) -> Reader {
    let mut r = Reader::new("t.md".into(), parse(src), Theme::dark(), 80);
    r.layout(80, 10);
    r
}

#[test]
fn copy_focused_block() {
    let mut r = reader("text\n\n```sh\necho hi\n```\n");
    assert!(matches!(key(&mut r, KeyCode::Char('y')), Action::None));
    key(&mut r, KeyCode::Tab);
    match key(&mut r, KeyCode::Char('y')) {
        Action::Copy(text) => assert_eq!(text, "echo hi"),
        _ => panic!("expected a copy action"),
    }
}

#[test]
fn anchor_link_scrolls_to_heading() {
    let filler: String = (0..40).map(|i| format!("para {i}\n\n")).collect();
    let src = format!("[jump](#getting-started)\n\n{filler}## Getting Started\n\n{filler}");
    let mut r = reader(&src);
    key(&mut r, KeyCode::Char('L'));
    assert!(r.link_picker.is_some());
    key(&mut r, KeyCode::Enter);
    let line: String = r.rendered.lines[r.scroll]
        .spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect();
    assert_eq!(line, "## Getting Started");
}

#[test]
fn anchor_link_skips_lines_that_only_look_like_the_heading() {
    let filler: String = (0..40).map(|i| format!("para {i}\n\n")).collect();
    let src = format!("[Usage](#usage)\n\n{filler}## Usage\n\n{filler}");
    let mut r = reader(&src);
    key(&mut r, KeyCode::Char('L'));
    key(&mut r, KeyCode::Enter);
    let line = bastian::render::plain(&r.rendered.lines[r.scroll]);
    assert_eq!(line, "## Usage");
}

#[test]
fn footnote_reference_scrolls_to_its_note() {
    let filler: String = (0..40).map(|i| format!("para {i}\n\n")).collect();
    let src = format!("Claim[^src].\n\n{filler}[^src]: The source.\n");
    let mut r = reader(&src);
    key(&mut r, KeyCode::Char('L'));
    key(&mut r, KeyCode::Enter);
    assert!(r.scroll > 0);
    let shown: Vec<String> = r.rendered.lines[r.scroll..]
        .iter()
        .take(10)
        .map(bastian::render::plain)
        .collect();
    assert!(shown.iter().any(|l| l == "[1] The source."), "{shown:?}");
}

#[test]
fn footnote_reference_in_a_title_is_listed_and_followed() {
    let filler: String = (0..40).map(|i| format!("para {i}\n\n")).collect();
    let src = format!("# Title[^t]\n\n{filler}[^t]: Where the title comes from.\n");
    let mut r = reader(&src);
    key(&mut r, KeyCode::Char('L'));
    let picker = r
        .link_picker
        .as_ref()
        .expect("the title's reference is listed");
    assert_eq!(picker.links[0].url, "#fn:t");
    key(&mut r, KeyCode::Enter);
    let shown: Vec<String> = r.rendered.lines[r.scroll..]
        .iter()
        .take(10)
        .map(bastian::render::plain)
        .collect();
    assert!(
        shown.iter().any(|l| l == "[1] Where the title comes from."),
        "{shown:?}"
    );
}

#[test]
fn footnote_targets_never_hit_a_heading() {
    // A heading whose slug looks like a footnote anchor.
    let filler: String = (0..40).map(|i| format!("para {i}\n\n")).collect();
    let src = format!("x[^a]\n\n# fn-a\n\n# fn:a\n\n{filler}[^a]: The note.\n");
    let mut r = reader(&src);
    key(&mut r, KeyCode::Char('L'));
    key(&mut r, KeyCode::Enter);
    let shown: Vec<String> = r.rendered.lines[r.scroll..]
        .iter()
        .take(10)
        .map(bastian::render::plain)
        .collect();
    assert!(shown.iter().any(|l| l == "[1] The note."), "{shown:?}");
}

#[test]
fn link_focus_steps_previews_and_follows() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("next.md"), "---\nx: 1\n---\n# Next steps\n").unwrap();
    let filler: String = (0..30).map(|i| format!("para {i}\n\n")).collect();
    let src = format!(
        "[a very long link text that has to wrap across two lines](#usage) then \
         [next](next.md), [web](https://example.com), [gone](gone.md) and a note[^n].\n\n\
         {filler}## Usage\n\n{filler}[^n]: The footnote text.\n"
    );
    let mut r = Reader::new("t.md".into(), parse(&src), Theme::dark(), 40);
    r.base_dir = dir.path().to_path_buf();
    r.layout(40, 10);

    // The wrapped link is one stop.
    let stops = r.link_stops();
    assert_eq!(stops.len(), 5, "{:?}", r.rendered.links);
    assert!(stops[0].len() > 1);

    key(&mut r, KeyCode::Char('s'));
    assert_eq!(r.link_focus, Some(0));
    assert_eq!(r.link_preview, "→ § Usage");
    key(&mut r, KeyCode::Char('s'));
    assert_eq!(r.link_preview, "→ next.md · Next steps");
    key(&mut r, KeyCode::Char('s'));
    assert_eq!(r.link_preview, "→ https://example.com");
    key(&mut r, KeyCode::Char('s'));
    assert!(
        r.link_preview.starts_with("not found: "),
        "{}",
        r.link_preview
    );
    key(&mut r, KeyCode::Char('s'));
    assert_eq!(r.link_preview, "[1] The footnote text.");
    // Stays on the last one.
    key(&mut r, KeyCode::Char('s'));
    assert_eq!(r.link_position(), Some((5, 5)));
    key(&mut r, KeyCode::Char('S'));
    assert_eq!(r.link_focus, Some(3));

    // Esc clears the focus instead of going back; Tab moves to code focus.
    assert!(matches!(key(&mut r, KeyCode::Esc), Action::None));
    assert_eq!(r.link_focus, None);
    key(&mut r, KeyCode::Char('s'));
    key(&mut r, KeyCode::Tab);
    assert_eq!(r.link_focus, None);

    // Enter follows the focused link.
    key(&mut r, KeyCode::Char('g'));
    key(&mut r, KeyCode::Char('s'));
    assert!(matches!(key(&mut r, KeyCode::Enter), Action::None));
    assert_eq!(r.link_focus, None);
    assert_eq!(
        bastian::render::plain(&r.rendered.lines[r.scroll]),
        "## Usage"
    );
    key(&mut r, KeyCode::Char('g'));
    key(&mut r, KeyCode::Char('s'));
    key(&mut r, KeyCode::Char('s'));
    assert!(matches!(
        key(&mut r, KeyCode::Enter),
        Action::OpenDocument(p) if p.ends_with("next.md")
    ));
}

#[test]
fn link_focus_starts_in_view_and_scrolls_to_far_links() {
    let filler: String = (0..40).map(|i| format!("para {i}\n\n")).collect();
    let src = format!("[top](#a)\n\n{filler}[far](#b)\n\n{filler}");
    let mut r = reader(&src);
    key(&mut r, KeyCode::Char('S'));
    assert_eq!(r.link_focus, Some(0));
    key(&mut r, KeyCode::Char('s'));
    assert_eq!(r.link_focus, Some(1));
    let line = r.rendered.links[r.link_stops()[1].start].line;
    assert!((r.scroll..r.scroll + 10).contains(&line));

    let mut none = reader("no links here\n");
    key(&mut none, KeyCode::Char('s'));
    assert_eq!(none.link_focus, None);
    assert_eq!(none.message.as_deref(), Some("no links in this document"));
}

#[test]
fn link_stops_are_links_not_urls() {
    // Separate links to the same place on consecutive lines stay separate.
    let r = reader("- [Install](a.md)\n- [Setup](a.md)\n");
    assert_eq!(r.link_stops(), [0..1, 1..2]);
    // A link with styled parts is one stop; another link to the same place
    // is another.
    let r = reader("[the **docs**](a.md) [again](a.md) and [b](b.md)\n");
    assert_eq!(r.link_stops().len(), 3, "{:?}", r.rendered.links);
}

#[test]
fn reload_drops_link_focus() {
    let mut r = reader("[one](#one)\n");
    key(&mut r, KeyCode::Char('s'));
    assert_eq!(r.link_focus, Some(0));
    r.replace_doc(parse("[zero](https://example.com) [one](#one)\n"));
    r.layout(80, 10);
    assert_eq!(r.link_focus, None);
    assert!(matches!(key(&mut r, KeyCode::Enter), Action::None));
}

#[test]
fn relative_markdown_link_opens_document() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("next.md"), "# Next\n").unwrap();
    let mut r = reader("[next](next.md) and [web](https://example.com) and [gone](gone.md)\n");
    r.base_dir = dir.path().to_path_buf();
    key(&mut r, KeyCode::Char('L'));
    match key(&mut r, KeyCode::Enter) {
        Action::OpenDocument(p) => assert_eq!(p, dir.path().join("next.md")),
        _ => panic!("expected OpenDocument"),
    }
    key(&mut r, KeyCode::Char('L'));
    key(&mut r, KeyCode::Char('j'));
    assert!(
        matches!(key(&mut r, KeyCode::Enter), Action::OpenExternal(u) if u == "https://example.com")
    );
    key(&mut r, KeyCode::Char('L'));
    key(&mut r, KeyCode::Char('j'));
    key(&mut r, KeyCode::Char('j'));
    key(&mut r, KeyCode::Enter);
    assert!(r.message.as_deref().unwrap_or("").starts_with("not found"));
}

#[test]
fn reloads_when_file_changes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("doc.md");
    std::fs::write(&path, "# One\n").unwrap();
    let mut r = reader("# One\n");
    r.watch(path.clone());
    // Make sure the new mtime differs even on coarse-grained filesystems.
    thread::sleep(Duration::from_millis(1100));
    std::fs::write(&path, "# Two\n\n```sh\necho x\n```\n").unwrap();
    thread::sleep(Duration::from_millis(600));
    assert!(r.tick());
    r.layout(80, 10);
    assert_eq!(r.doc.title().as_deref(), Some("Two"));
    assert_eq!(r.rendered.code_blocks.len(), 1);
    assert_eq!(r.message.as_deref(), Some("reloaded"));
}

#[test]
fn help_closes_on_any_key() {
    let mut r = reader("text\n");
    key(&mut r, KeyCode::Char('?'));
    assert!(r.help);
    key(&mut r, KeyCode::Char('q'));
    assert!(!r.help, "first key only closes help");
}

#[test]
fn edit_needs_a_file() {
    let mut r = reader("text\n");
    assert!(matches!(key(&mut r, KeyCode::Char('e')), Action::None));
    assert_eq!(r.message.as_deref(), Some("no local file to edit"));

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("doc.md");
    std::fs::write(&path, "text\n").unwrap();
    r.watch(path.clone());
    assert!(matches!(key(&mut r, KeyCode::Char('e')), Action::Edit(p, _) if p == path));
}

#[test]
fn hash_toggles_line_numbers() {
    let mut r = reader("text\n");
    assert!(!r.line_numbers);
    key(&mut r, KeyCode::Char('#'));
    assert!(r.line_numbers);
    key(&mut r, KeyCode::Char('#'));
    assert!(!r.line_numbers);
}

#[test]
fn line_number_gutter_fits_in_terminal() {
    use bastian::ui::reader::{GUTTER_GAP, MARGIN, draw, gutter_width};
    use ratatui::{Terminal, backend::TestBackend};

    let src = std::fs::read_to_string("tests/fixtures/sample.md").unwrap();
    let mut r = Reader::new("t.md".into(), parse(&src), Theme::dark(), 200);
    r.line_numbers = true;
    let (w, h) = (60u16, 20u16);
    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
    terminal.draw(|f| draw(f, &mut r)).unwrap();

    let gutter = gutter_width(&r, r.rendered.lines.len());
    assert!(gutter > GUTTER_GAP);
    let content = (w - MARGIN * 2 - gutter) as usize;
    for line in &r.rendered.lines {
        assert!(line.width() <= content, "{line:?} wider than {content}");
    }

    // Numbers sit right after the margin, right-aligned, starting at 1, on
    // the first row below the top padding, with a gap before the text.
    let buf = terminal.backend().buffer();
    let row: String = (0..w).map(|x| buf[(x, 1)].symbol().to_string()).collect();
    let digits = usize::from(gutter - GUTTER_GAP);
    let start = usize::from(MARGIN);
    let gap = usize::from(GUTTER_GAP);
    assert_eq!(
        &row[start..start + digits + gap],
        format!("{:>digits$}{}", 1, " ".repeat(gap))
    );
}

#[test]
fn focused_link_is_highlighted_and_previewed() {
    use bastian::ui::reader::draw;
    use ratatui::{Terminal, backend::TestBackend};

    let mut r = Reader::new(
        "t.md".into(),
        parse("See [docs](https://x.dev) here.\n"),
        Theme::dark(),
        80,
    );
    let (w, h) = (60u16, 8u16);
    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
    terminal.draw(|f| draw(f, &mut r)).unwrap();
    key(&mut r, KeyCode::Char('s'));
    terminal.draw(|f| draw(f, &mut r)).unwrap();

    let buf = terminal.backend().buffer();
    let theme = Theme::dark();
    let highlighted: String = (0..w)
        .flat_map(|x| (0..h).map(move |y| (x, y)))
        .filter(|&p| buf[p].bg == theme.current_bg)
        .map(|p| buf[p].symbol().to_string())
        .collect();
    assert_eq!(highlighted, "docs");
    let status: String = (0..w)
        .map(|x| buf[(x, h - 1)].symbol().to_string())
        .collect();
    assert!(status.contains("→ https://x.dev"), "{status}");
    assert!(status.contains("link 1/1"), "{status}");
}

#[test]
fn blocks_remember_their_source_lines() {
    let doc = parse("# Title\n\npara one\nstill one\n\n- a\n- b\n\n```sh\nx\n```\n");
    assert_eq!(doc.lines, [0, 2, 5, 8]);
    let mdx = bastian::doc::parse_for(
        "---\ntitle: t\n---\n\n# H\n\n<Note>\ninside\n</Note>\n",
        Some(std::path::Path::new("a.mdx")),
    );
    assert_eq!(mdx.lines.len(), mdx.blocks.len());
    assert_eq!(mdx.lines[0], 4, "heading after front matter");
}

#[test]
fn edit_opens_at_the_top_visible_source_line() {
    let src: String = (0..60).map(|i| format!("paragraph {i}\n\n")).collect();
    let mut r = reader(&src);
    r.source = Some("doc.md".into());
    r.layout(80, 10);
    match key(&mut r, KeyCode::Char('e')) {
        Action::Edit(_, line) => assert_eq!(line, Some(1)),
        _ => panic!("expected edit"),
    }
    // Scroll to paragraph 20 (rendered line 40, source line 41).
    for _ in 0..40 {
        key(&mut r, KeyCode::Char('j'));
    }
    match key(&mut r, KeyCode::Char('e')) {
        Action::Edit(_, line) => assert_eq!(line, Some(41)),
        _ => panic!("expected edit"),
    }
}

#[test]
fn c_copies_the_whole_document() {
    let mut r = reader("# A\n\ntext\n");
    assert!(matches!(key(&mut r, KeyCode::Char('c')), Action::None));
    assert_eq!(r.message.as_deref(), Some("nothing to copy"));
    r.text = "# A\n\ntext\n".into();
    match key(&mut r, KeyCode::Char('c')) {
        Action::Copy(t) => assert_eq!(t, "# A\n\ntext\n"),
        _ => panic!("expected copy"),
    }
    assert_eq!(r.message.as_deref(), Some("copied document (3 lines)"));
}

fn settings(assume_yes: bool, positions: Option<std::path::PathBuf>) -> Settings {
    Settings {
        theme: Theme::dark(),
        max_width: 80,
        assume_yes,
        interpreters: Interpreters::default(),
        line_numbers: false,
        scan: bastian::browser::Scan::default(),
        date_format: Some(bastian::config::DATE_FORMAT.into()),
        render: bastian::render::Options::default(),
        hyperlinks: true,
        scrollbar: true,
        editor: None,
        mouse: false,
        logo: true,
        images: None,
        show_images: false,
        selector: bastian::config::Selector::Bar,
        list_layout: bastian::config::ListLayout::Cards,
        positions,
    }
}

#[test]
fn reopening_a_file_returns_to_where_it_was_left() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("state/positions.json");
    let file = dir.path().join("long.md");
    let src: String = (0..60)
        .map(|i| format!("## Part {i}\n\ntext {i}\n\n"))
        .collect();
    std::fs::write(&file, src).unwrap();
    let s = settings(false, Some(store.clone()));

    let mut r = s.open(&file, "long.md".into()).unwrap();
    r.layout(80, 10);
    assert_eq!(r.scroll, 0);
    for _ in 0..40 {
        key(&mut r, KeyCode::Char('j'));
    }
    let left_at = r.source_line();
    assert!(left_at > Some(1));
    r.remember_position();

    // A narrower window still opens at the same block.
    let mut again = s.open(&file, "long.md".into()).unwrap();
    again.layout(60, 10);
    assert!(again.scroll > 0);
    assert_eq!(again.source_line(), left_at);

    // Keys that don't move the view keep the restore going; moving ends it.
    key(&mut again, KeyCode::Char('#'));
    again.layout(60, 10);
    assert_eq!(again.source_line(), left_at);
    assert!(again.restore.is_some());
    key(&mut again, KeyCode::Char('j'));
    again.layout(60, 10);
    assert_eq!(again.restore, None);

    // Back at the top, the position is forgotten.
    key(&mut again, KeyCode::Char('g'));
    again.remember_position();
    let mut third = s.open(&file, "long.md".into()).unwrap();
    third.layout(80, 10);
    assert_eq!(third.scroll, 0);

    // Turned off: nothing is restored or saved.
    let off = settings(false, None);
    let mut r = off.open(&file, "long.md".into()).unwrap();
    r.layout(80, 10);
    key(&mut r, KeyCode::Char('G'));
    r.remember_position();
    assert_eq!(bastian::positions::load(&store, &file), None);
}

#[test]
fn remote_documents_resolve_links_and_always_confirm() {
    // Built the way the app builds fetched documents, with --yes on.
    let settings = settings(true, None);
    let mut r = settings.remote(RemoteDoc {
        name: "README.md".into(),
        base: url::Url::parse("https://example.com/repo/README.md").unwrap(),
        text: "See [guide](docs/guide.md) and [logo](img/logo.png).\n\n```sh\necho hi\n```\n"
            .into(),
    });
    r.layout(80, 10);

    key(&mut r, KeyCode::Char('L'));
    match key(&mut r, KeyCode::Enter) {
        Action::OpenRemote(u) => assert_eq!(u.as_str(), "https://example.com/repo/docs/guide.md"),
        _ => panic!("expected OpenRemote"),
    }
    key(&mut r, KeyCode::Char('L'));
    key(&mut r, KeyCode::Char('j'));
    assert!(matches!(
        key(&mut r, KeyCode::Enter),
        Action::OpenExternal(u) if u == "https://example.com/repo/img/logo.png"
    ));

    // --yes doesn't skip confirmation for an untrusted document.
    key(&mut r, KeyCode::Tab);
    key(&mut r, KeyCode::Enter);
    assert_eq!(r.confirm, Some(vec![0]));
}

#[test]
fn reload_with_changed_code_drops_pending_confirmation() {
    let mut r = reader("```sh\necho one\n```\n");
    key(&mut r, KeyCode::Tab);
    key(&mut r, KeyCode::Char('r'));
    assert_eq!(r.confirm, Some(vec![0]));

    // Prose edits keep the prompt: block 0 is still the same code.
    r.replace_doc(parse("intro\n\n```sh\necho one\n```\n"));
    assert_eq!(r.confirm, Some(vec![0]));

    r.replace_doc(parse("```sh\nrm -rf ~\n```\n\n```sh\necho one\n```\n"));
    assert_eq!(r.confirm, None);
}

#[test]
fn replaced_document_is_highlighted_afresh() {
    let mut r = reader("```sh\necho old\n```\n");
    let text = |r: &Reader| bastian::render::to_plain(&r.rendered.lines);
    assert!(text(&r).contains("echo old"));
    r.replace_doc(parse("```sh\necho new\n```\n"));
    r.layout(80, 10);
    assert!(text(&r).contains("echo new"), "{}", text(&r));
}

#[test]
fn scrollbar_shows_only_when_the_document_overflows() {
    use bastian::ui::reader::draw;
    use ratatui::{Terminal, backend::TestBackend};

    let column = |src: &str, scroll: usize| {
        let mut r = Reader::new("t.md".into(), parse(src), Theme::dark(), 80);
        let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
        terminal.draw(|f| draw(f, &mut r)).unwrap();
        r.scroll = scroll;
        terminal.draw(|f| draw(f, &mut r)).unwrap();
        let buf = terminal.backend().buffer();
        // Document rows, between the top and bottom padding, in the last column.
        (1..10)
            .map(|y| buf[(39, y)].symbol().to_string())
            .collect::<String>()
    };
    let short = column("# Short\n\ntext\n", 0);
    assert!(short.trim().is_empty(), "{short:?}");

    let long: String = (0..60).map(|i| format!("line {i}\n\n")).collect();
    let top = column(&long, 0);
    assert!(top.starts_with('█'), "{top:?}");
    let bottom = column(&long, 1000);
    assert!(
        bottom.ends_with('█') && !bottom.starts_with('█'),
        "{bottom:?}"
    );
}

#[test]
fn links_are_terminal_hyperlinks() {
    use bastian::ui::reader::draw;
    use ratatui::{Terminal, backend::TestBackend};

    let mut r = Reader::new(
        "t.md".into(),
        parse("See [docs](https://x.dev/docs) and [guide](guide.md#setup), [top](#top).\n"),
        Theme::dark(),
        80,
    );
    r.base_dir = std::path::PathBuf::from("/tmp/proj");
    let mut terminal = Terminal::new(TestBackend::new(80, 6)).unwrap();
    // The frame as drawn; the test backend can't measure OSC 8 cells.
    let frame = terminal.draw(|f| draw(f, &mut r)).unwrap();
    let buf = frame.buffer;
    let symbols: Vec<String> = (0..80).map(|x| buf[(x, 1)].symbol().to_string()).collect();
    let osc = |target: &str| format!("\u{1b}]8;;{target}\u{1b}\\");
    assert!(
        symbols
            .iter()
            .any(|s| s.starts_with(&osc("https://x.dev/docs"))),
        "{symbols:?}"
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.starts_with(&osc("file:///tmp/proj/guide.md#setup"))),
        "{symbols:?}"
    );
    // The link cell declares its real width, so the diff doesn't skip the
    // text after it.
    let x = symbols.iter().position(|s| s.contains("x.dev")).unwrap();
    assert_eq!(
        buf[(x as u16, 1)].diff_option,
        ratatui::buffer::CellDiffOption::ForcedWidth(std::num::NonZeroU16::new(4).unwrap())
    );
    // In-document anchors stay plain text.
    assert_eq!(
        symbols.iter().filter(|s| s.contains("\u{1b}]8;;")).count(),
        2
    );
    assert_eq!(r.hyperlink_target("#top"), None);
}

#[test]
fn image_sources_resolve_like_links() {
    use bastian::images::Source;
    let mut r = Reader::new("t.md".into(), parse("x\n"), Theme::dark(), 80);
    r.base_dir = std::path::PathBuf::from("/docs");
    assert_eq!(
        r.image_source("img/a.png?raw=1"),
        Some(Source::File("/docs/img/a.png".into()))
    );
    assert_eq!(
        r.image_source("https://github.com/o/r/blob/main/shot.png"),
        Some(Source::Url(
            "https://raw.githubusercontent.com/o/r/main/shot.png".into()
        ))
    );
    assert_eq!(r.image_source("data:image/png;base64,AAAA"), None);
    r.remote_base = Some(url::Url::parse("https://github.com/o/r/blob/main/README.md").unwrap());
    assert_eq!(
        r.image_source("docs/b.png"),
        Some(Source::Url(
            "https://raw.githubusercontent.com/o/r/main/docs/b.png".into()
        ))
    );
}

#[test]
fn pictures_partly_scrolled_off_show_their_visible_rows() {
    use bastian::images::{Images, Support};
    use bastian::ui::reader::draw;
    use ratatui::{Terminal, backend::TestBackend};
    use ratatui_image::picker::Picker;

    let dir = tempfile::tempdir().unwrap();
    image::RgbImage::from_pixel(200, 200, image::Rgb([220, 40, 40]))
        .save(dir.path().join("p.png"))
        .unwrap();
    let tail: String = (0..40).map(|i| format!("line {i}\n\n")).collect();
    let src = format!("intro\n\n![pic](p.png)\n\n{tail}");
    let mut r = Reader::new("t.md".into(), parse(&src), Theme::dark(), 80);
    r.base_dir = dir.path().to_path_buf();
    r.images = Images::new(Some(Support::fixed(Picker::halfblocks())));

    let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while r.rendered.images.is_empty() {
        terminal.draw(|f| draw(f, &mut r)).unwrap();
        r.tick();
        assert!(std::time::Instant::now() < deadline, "picture did not load");
        thread::sleep(Duration::from_millis(10));
    }
    terminal.draw(|f| draw(f, &mut r)).unwrap();
    let slot = r.rendered.images[0].clone();
    assert!(slot.rows >= 4, "{slot:?}");

    // Scroll the top half of the picture off screen.
    r.scroll = slot.line + usize::from(slot.rows) / 2;
    let frame = terminal.draw(|f| draw(f, &mut r)).unwrap();
    let visible = usize::from(slot.rows) - usize::from(slot.rows) / 2;
    let x = 2 + slot.col as u16; // after the left margin
    // Half-blocks draw the (single-color) picture as colored cells.
    let red = ratatui::style::Color::Rgb(220, 40, 40);
    for row in 0..visible as u16 {
        let cell = &frame.buffer[(x, 1 + row)]; // below the top padding
        assert!(
            cell.bg == red || cell.fg == red,
            "row {row} of the picture is blank: {cell:?}"
        );
    }
}

#[test]
fn outline_panel_lists_headings_and_jumps() {
    use bastian::ui::reader::draw;
    use ratatui::{Terminal, backend::TestBackend};

    let src = std::fs::read_to_string("tests/fixtures/outline.md").unwrap();
    let mut r = Reader::new("t.md".into(), parse(&src), Theme::dark(), 80);
    let mut terminal = Terminal::new(TestBackend::new(70, 30)).unwrap();
    terminal.draw(|f| draw(f, &mut r)).unwrap();
    assert!(matches!(key(&mut r, KeyCode::Char('O')), Action::None));
    terminal.draw(|f| draw(f, &mut r)).unwrap();
    let screen: String = {
        let buf = terminal.backend().buffer();
        (0..30)
            .map(|y| {
                (0..70)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
                    + "\n"
            })
            .collect()
    };
    assert!(screen.contains("outline · enter jump"), "{screen}");
    for heading in ["Guide", "Install", "From source", "Skipped a level"] {
        assert!(screen.contains(heading), "{heading} missing:\n{screen}");
    }

    // On a short screen, down to "Install" and jump: it lands at the top
    // of the view.
    key(&mut r, KeyCode::Esc);
    let mut short = Terminal::new(TestBackend::new(70, 8)).unwrap();
    short.draw(|f| draw(f, &mut r)).unwrap();
    key(&mut r, KeyCode::Char('O'));
    short.draw(|f| draw(f, &mut r)).unwrap();
    key(&mut r, KeyCode::Char('j'));
    key(&mut r, KeyCode::Enter);
    assert!(r.outline.is_none());
    let top = bastian::render::plain(&r.rendered.lines[r.scroll]);
    assert_eq!(top.trim(), "## Install");

    // A document without headings says so instead.
    let mut plain = reader("just text\n");
    key(&mut plain, KeyCode::Char('O'));
    assert!(plain.outline.is_none());
    assert_eq!(
        plain.message.as_deref(),
        Some("no headings in this document")
    );
}

#[test]
fn hyperlinks_and_scrollbar_can_be_turned_off() {
    use bastian::ui::reader::draw;
    use ratatui::{Terminal, backend::TestBackend};

    let long: String = (0..60)
        .map(|i| format!("[link {i}](https://x.dev/{i})\n\n"))
        .collect();
    let mut r = Reader::new("t.md".into(), parse(&long), Theme::dark(), 80);
    r.hyperlinks = false;
    r.scrollbar = false;
    let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
    let frame = terminal.draw(|f| draw(f, &mut r)).unwrap();
    let buf = frame.buffer;
    let cells: String = (0..12)
        .flat_map(|y| (0..40).map(move |x| (x, y)))
        .map(|(x, y)| buf[(x, y)].symbol().to_string())
        .collect();
    assert!(!cells.contains("\u{1b}]8;;"), "no OSC 8 links");
    assert!(cells.contains("link 0"));
    assert!(!cells.contains('█'), "no scrollbar thumb");
}
