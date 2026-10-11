use std::thread;
use std::time::Duration;

use bastian::doc::parse;
use bastian::reader::{Action, Reader};
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
    assert_eq!(r.message.as_deref(), Some("stdin has no file to edit"));

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
    use bastian::ui::reader::{MARGIN, draw, gutter_width};
    use ratatui::{Terminal, backend::TestBackend};

    let src = std::fs::read_to_string("tests/fixtures/sample.md").unwrap();
    let mut r = Reader::new("t.md".into(), parse(&src), Theme::dark(), 200);
    r.line_numbers = true;
    let (w, h) = (60u16, 20u16);
    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
    terminal.draw(|f| draw(f, &mut r)).unwrap();

    let gutter = gutter_width(&r, r.rendered.lines.len());
    assert!(gutter >= 2);
    let content = (w - MARGIN * 2 - gutter) as usize;
    for line in &r.rendered.lines {
        assert!(line.width() <= content, "{line:?} wider than {content}");
    }

    // Numbers sit right after the margin, right-aligned, starting at 1.
    let buf = terminal.backend().buffer();
    let row: String = (0..w).map(|x| buf[(x, 0)].symbol().to_string()).collect();
    let digits = usize::from(gutter - 1);
    let start = usize::from(MARGIN);
    assert_eq!(&row[start..start + digits + 1], format!("{:>digits$} ", 1));
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
