use std::thread;
use std::time::{Duration, Instant};

use bastian::doc::parse;
use bastian::exec::Status;
use bastian::reader::Reader;
use bastian::render::theme::Theme;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn reader(src: &str) -> Reader {
    let mut r = Reader::new("t.md".into(), parse(src), Theme::dark(), 80);
    r.base_dir = std::env::temp_dir();
    r.layout(80, 40);
    r
}

fn key(r: &mut Reader, code: KeyCode) {
    r.on_key(KeyEvent::new(code, KeyModifiers::NONE));
}

/// Tick until nothing is running, then return each block's final status.
fn settle(r: &mut Reader) -> Vec<Option<Status>> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        r.tick();
        r.layout(80, 40);
        if !r.is_running() {
            break;
        }
        assert!(Instant::now() < deadline, "blocks did not finish");
        thread::sleep(Duration::from_millis(20));
    }
    let n = r.rendered.code_blocks.len();
    (0..n)
        .map(|id| r.runs.get(&id).map(|b| b.status.clone()))
        .collect()
}

fn output(r: &Reader, id: usize) -> Vec<String> {
    r.runs[&id].output.iter().map(|l| l.text.clone()).collect()
}

#[test]
fn confirm_then_run_shows_output() {
    let mut r = reader("```sh\necho hello\n```\n");
    key(&mut r, KeyCode::Enter);
    assert_eq!(r.confirm, Some(vec![0]));
    key(&mut r, KeyCode::Char('y'));
    settle(&mut r);
    assert!(matches!(r.runs[&0].status, Status::Ok(_)));
    assert_eq!(output(&r, 0), ["hello"]);
    let text = bastian::render::to_plain(&r.rendered.lines);
    assert!(text.contains("✓ done in"), "{text}");
    assert!(text.contains("│ hello"), "{text}");
}

#[test]
fn declining_does_not_run() {
    let mut r = reader("```sh\necho hello\n```\n");
    key(&mut r, KeyCode::Enter);
    key(&mut r, KeyCode::Char('n'));
    assert!(r.confirm.is_none());
    assert!(r.runs.is_empty());
}

#[test]
fn assume_yes_skips_confirm_except_marked_blocks() {
    let mut r = reader("```sh\necho a\n```\n\n```sh {confirm}\necho b\n```\n");
    r.assume_yes = true;
    key(&mut r, KeyCode::Enter);
    assert!(r.confirm.is_none());
    settle(&mut r);
    key(&mut r, KeyCode::Tab);
    key(&mut r, KeyCode::Enter);
    assert_eq!(r.confirm, Some(vec![1]));
}

#[test]
fn run_all_stops_at_first_failure_and_skips_unrunnable() {
    let src = "```sh\necho one\n```\n\n```text\nnot code\n```\n\n```sh\nexit 2\n```\n\n```sh\necho never\n```\n";
    let mut r = reader(src);
    r.assume_yes = true;
    key(&mut r, KeyCode::Char('R'));
    let statuses = settle(&mut r);
    assert!(matches!(statuses[0], Some(Status::Ok(_))));
    assert_eq!(statuses[1], None);
    assert!(matches!(
        statuses[2],
        Some(Status::Failed { code: Some(2), .. })
    ));
    assert_eq!(statuses[3], None);
}

#[test]
fn timeout_kills_block() {
    let mut r = reader("```sh {timeout=200ms}\nsleep 5\n```\n");
    r.assume_yes = true;
    key(&mut r, KeyCode::Enter);
    let start = Instant::now();
    let statuses = settle(&mut r);
    assert!(
        matches!(statuses[0], Some(Status::TimedOut(_))),
        "{statuses:?}"
    );
    assert!(start.elapsed() < Duration::from_secs(3));
}

#[test]
fn cancel_with_x() {
    let mut r = reader("```sh\nsleep 5\n```\n");
    r.assume_yes = true;
    key(&mut r, KeyCode::Enter);
    thread::sleep(Duration::from_millis(100));
    key(&mut r, KeyCode::Char('x'));
    let statuses = settle(&mut r);
    assert!(
        matches!(statuses[0], Some(Status::Cancelled(_))),
        "{statuses:?}"
    );
}

#[test]
fn cwd_is_relative_to_document() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    let mut r = reader("```sh {cwd=sub}\npwd\n```\n");
    r.base_dir = dir.path().to_path_buf();
    r.assume_yes = true;
    key(&mut r, KeyCode::Enter);
    settle(&mut r);
    let got = std::fs::canonicalize(&output(&r, 0)[0]).unwrap();
    assert_eq!(got, std::fs::canonicalize(dir.path().join("sub")).unwrap());
}

#[test]
fn unrunnable_block_reports_why() {
    let mut r = reader("```rust\nfn main() {}\n```\n");
    key(&mut r, KeyCode::Enter);
    assert_eq!(r.message.as_deref(), Some("no interpreter for `rust`"));
    assert!(r.confirm.is_none());
}

#[test]
fn missing_interpreter_is_an_error_status() {
    // A real language whose program is unlikely to exist under this name
    // can't be tested portably, so check a missing cwd instead.
    let mut r = reader("```sh {cwd=does-not-exist}\necho x\n```\n");
    r.assume_yes = true;
    key(&mut r, KeyCode::Enter);
    let statuses = settle(&mut r);
    assert!(
        matches!(&statuses[0], Some(Status::Error(_))),
        "{statuses:?}"
    );
}

#[test]
fn backtab_from_bottom_focuses_last_visible_block() {
    let src = std::fs::read_to_string("tests/fixtures/runbook.md").unwrap();
    let mut r = Reader::new("t.md".into(), parse(&src), Theme::dark(), 76);
    r.layout(76, 19);
    key(&mut r, KeyCode::Char('G'));
    r.layout(76, 19);
    key(&mut r, KeyCode::BackTab);
    assert_eq!(
        r.focus,
        Some(4),
        "scroll {} lines {}",
        r.scroll,
        r.rendered.lines.len()
    );
}

#[test]
fn view_follows_output_of_focused_run() {
    let src = std::fs::read_to_string("tests/fixtures/runbook.md").unwrap();
    let mut r = Reader::new("t.md".into(), parse(&src), Theme::dark(), 76);
    r.base_dir = std::env::temp_dir();
    r.assume_yes = true;
    r.layout(76, 19);
    key(&mut r, KeyCode::Char('G'));
    r.layout(76, 19);
    key(&mut r, KeyCode::BackTab);
    key(&mut r, KeyCode::Enter);
    let deadline = Instant::now() + Duration::from_secs(10);
    while r.is_running() || r.runs.is_empty() {
        assert!(Instant::now() < deadline);
        r.tick();
        r.layout(76, 19);
        thread::sleep(Duration::from_millis(20));
    }
    r.tick();
    r.layout(76, 19);
    let range = r.rendered.code_blocks[4].clone();
    assert!(
        range.output_end > range.lines.end,
        "python block has no output panel"
    );
    assert!(
        r.scroll + 19 >= range.output_end,
        "output not visible: scroll {} output_end {}",
        r.scroll,
        range.output_end
    );
    assert_eq!(
        r.runs[&4]
            .output
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>(),
        ["45"]
    );
}
