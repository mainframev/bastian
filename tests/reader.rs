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
