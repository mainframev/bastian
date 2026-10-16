// A test crate: helpers outside #[test] fns may unwrap too.
#![allow(clippy::unwrap_used)]

use std::thread;
use std::time::Duration;

use bastian::app::Settings;
use bastian::doc::parse;
use bastian::picker::{self, ThemePicker};
use bastian::reader::{Action, Location, Options, Overlay, Reader, Source};
use bastian::remote::RemoteDoc;
use bastian::render::theme::Theme;
use bastian::ui::reader::{GUTTER_GAP, MARGIN, draw, draw_with, gutter_width};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::text::Line;
use ratatui::{Terminal, backend::TestBackend};

mod common;
use common::{confirm, key};

fn ctrl(r: &mut Reader, c: char) -> Action {
    r.on_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL))
}

/// The target an open-external confirmation is waiting on.
fn confirm_open(r: &Reader) -> Option<&str> {
    match &r.overlay {
        Some(Overlay::OpenConfirm(t)) => Some(t),
        _ => None,
    }
}

/// The location popup's contents.
fn location(r: &Reader) -> Option<&Location> {
    match &r.overlay {
        Some(Overlay::Location(l)) => Some(l),
        _ => None,
    }
}

/// A reader over `source` with default options, laid out at 80×10.
fn open(source: Source) -> Reader {
    let mut r = Reader::new(source, &Options::default());
    r.layout(80, 10);
    r
}

fn reader(src: &str) -> Reader {
    open(Source::in_memory("t.md", parse(src)))
}

/// A reader over `src` whose relative links resolve from `base_dir`.
fn reader_in(src: &str, base_dir: &std::path::Path) -> Reader {
    open(Source {
        base_dir: base_dir.to_path_buf(),
        ..Source::in_memory("t.md", parse(src))
    })
}

/// A reader over `src` with default options, not yet laid out.
fn in_memory(src: &str) -> Reader {
    Reader::new(Source::in_memory("t.md", parse(src)), &Options::default())
}

/// `text` as fetched from `https://example.com/repo/README.md`.
fn remote(text: &str) -> Source {
    Source::remote(RemoteDoc {
        name: "README.md".into(),
        base: url::Url::parse("https://example.com/repo/README.md").unwrap(),
        text: text.into(),
    })
}

/// `n` short paragraphs, "para 0" onwards.
fn filler(n: usize) -> String {
    (0..n).map(|i| format!("para {i}\n\n")).collect()
}

/// A `w`×`h` test terminal.
fn term(w: u16, h: u16) -> Terminal<TestBackend> {
    Terminal::new(TestBackend::new(w, h)).unwrap()
}

/// Columns `from..to` of each row of the screen.
fn columns(t: &Terminal<TestBackend>, from: u16, to: u16) -> Vec<String> {
    let buf = t.backend().buffer();
    (0..buf.area.height)
        .map(|y| (from..to).map(|x| buf[(x, y)].symbol()).collect())
        .collect()
}

/// The screen after drawing `r` on a `w`×`h` terminal, row by row.
fn screen_of(r: &mut Reader, w: u16, h: u16) -> String {
    let mut terminal = term(w, h);
    terminal.draw(|f| draw(f, r)).unwrap();
    columns(&terminal, 0, w).join("\n")
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
    let filler = filler(40);
    let src = format!("[jump](#getting-started)\n\n{filler}## Getting Started\n\n{filler}");
    let mut r = reader(&src);
    key(&mut r, KeyCode::Char('L'));
    assert!(matches!(r.overlay, Some(Overlay::Links(_))));
    key(&mut r, KeyCode::Enter);
    let line: String = r.rendered().lines[r.scroll()]
        .spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect();
    assert_eq!(line, "Getting Started");
}

#[test]
fn anchor_link_skips_lines_that_only_look_like_the_heading() {
    let filler = filler(40);
    let src = format!("[Usage](#usage)\n\n{filler}## Usage\n\n{filler}");
    let mut r = reader(&src);
    key(&mut r, KeyCode::Char('L'));
    key(&mut r, KeyCode::Enter);
    let line = bastian::render::plain(&r.rendered().lines[r.scroll()]);
    assert_eq!(line, "Usage");
}

#[test]
fn footnote_reference_scrolls_to_its_note() {
    let filler = filler(40);
    let src = format!("Claim[^src].\n\n{filler}[^src]: The source.\n");
    let mut r = reader(&src);
    key(&mut r, KeyCode::Char('L'));
    key(&mut r, KeyCode::Enter);
    assert!(r.scroll() > 0);
    let shown: Vec<String> = r.rendered().lines[r.scroll()..]
        .iter()
        .take(10)
        .map(bastian::render::plain)
        .collect();
    assert!(shown.iter().any(|l| l == "[1] The source."), "{shown:?}");
}

#[test]
fn footnote_reference_in_a_title_is_listed_and_followed() {
    let filler = filler(40);
    let src = format!("# Title[^t]\n\n{filler}[^t]: Where the title comes from.\n");
    let mut r = reader(&src);
    key(&mut r, KeyCode::Char('L'));
    let Some(Overlay::Links(picker)) = &r.overlay else {
        panic!("the title's reference is listed");
    };
    assert_eq!(picker.links[0].url, "#fn:t");
    key(&mut r, KeyCode::Enter);
    let shown: Vec<String> = r.rendered().lines[r.scroll()..]
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
    let filler = filler(40);
    let src = format!("x[^a]\n\n# fn-a\n\n# fn:a\n\n{filler}[^a]: The note.\n");
    let mut r = reader(&src);
    key(&mut r, KeyCode::Char('L'));
    key(&mut r, KeyCode::Enter);
    let shown: Vec<String> = r.rendered().lines[r.scroll()..]
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
    let filler = filler(30);
    let src = format!(
        "[a very long link text that has to wrap across two lines](#usage) then \
         [next](next.md), [web](https://example.com), [gone](gone.md) and a note[^n].\n\n\
         {filler}## Usage\n\n{filler}[^n]: The footnote text.\n"
    );
    let source = Source {
        base_dir: dir.path().to_path_buf(),
        ..Source::in_memory("t.md", parse(&src))
    };
    let mut r = Reader::new(
        source,
        &Options {
            max_width: 40,
            ..Options::default()
        },
    );
    r.layout(40, 10);

    // The wrapped link is one stop.
    let stops = r.link_stops();
    assert_eq!(stops.len(), 5, "{:?}", r.rendered().links);
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
        bastian::render::plain(&r.rendered().lines[r.scroll()]),
        "Usage"
    );
    key(&mut r, KeyCode::Char('g'));
    key(&mut r, KeyCode::Char('s'));
    key(&mut r, KeyCode::Char('s'));
    assert!(matches!(
        key(&mut r, KeyCode::Enter),
        Action::OpenDocument { path, .. } if path.ends_with("next.md")
    ));
}

#[test]
fn link_focus_starts_in_view_and_scrolls_to_far_links() {
    let filler = filler(40);
    let src = format!("[top](#a)\n\n{filler}[far](#b)\n\n{filler}");
    let mut r = reader(&src);
    key(&mut r, KeyCode::Char('S'));
    assert_eq!(r.link_focus, Some(0));
    key(&mut r, KeyCode::Char('s'));
    assert_eq!(r.link_focus, Some(1));
    let line = r.rendered().links[r.link_stops()[1][0]].line;
    assert!((r.scroll()..r.scroll() + 10).contains(&line));

    let mut none = reader("no links here\n");
    key(&mut none, KeyCode::Char('s'));
    assert_eq!(none.link_focus, None);
    assert_eq!(none.message(), Some("no links in this document"));
}

#[test]
fn link_stops_are_links_not_urls() {
    // Separate links to the same place on consecutive lines stay separate.
    let r = reader("- [Install](a.md)\n- [Setup](a.md)\n");
    assert_eq!(r.link_stops(), [vec![0], vec![1]]);
    // A link with styled parts is one stop; another link to the same place
    // is another.
    let r = reader("[the **docs**](a.md) [again](a.md) and [b](b.md)\n");
    assert_eq!(r.link_stops().len(), 3, "{:?}", r.rendered().links);
}

#[test]
fn link_stops_follow_link_occurrences() {
    let urls = |r: &Reader| -> Vec<String> {
        r.link_stops()
            .iter()
            .map(|s| r.rendered().links[s[0]].url.clone())
            .collect()
    };
    // Two links to one place with nothing between them stay two.
    let r = reader("[a](x)[b](x)\n");
    assert_eq!(urls(&r), ["x", "x"]);

    // In a table, a link wrapping in one cell and a link in the next cell
    // keep their own targets, and each is one stop.
    let mut r =
        in_memory("| a | b |\n|---|---|\n| [a long link that wraps](one.md) | [two](two.md) |\n");
    r.layout(22, 10);
    assert_eq!(urls(&r), ["one.md", "two.md"]);
    assert!(r.link_stops()[0].len() > 1, "{:?}", r.rendered().links);
    for &i in &r.link_stops()[0] {
        assert_eq!(r.rendered().links[i].url, "one.md");
    }

    // An MDX tag inside a link isn't drawn as a link, and doesn't tie the
    // link to the one before it.
    let doc = bastian::doc::parse_for(
        "[old](y) [<Badge/>new](x)\n",
        Some(std::path::Path::new("t.mdx")),
    );
    let r = open(Source::in_memory("t.mdx", doc));
    assert_eq!(urls(&r), ["y", "x"]);
}

#[test]
fn link_focus_starts_with_a_link_wrapping_into_view() {
    let words = "word ".repeat(30);
    let src = format!("[{words}](#x)\n\nafter [b](#b)\n");
    let mut r = Reader::new(
        Source::in_memory("t.md", parse(&src)),
        &Options {
            max_width: 20,
            ..Options::default()
        },
    );
    r.layout(20, 3);
    // Scroll so the link's first line is above the view but its rest shows.
    r.scroll_to(2);
    key(&mut r, KeyCode::Char('s'));
    assert_eq!(r.link_focus, Some(0));
}

#[test]
fn previews_read_mdx_titles_and_whole_notes() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("page.mdx"),
        "import X from './x'\n\n<Page>\n\n# Inside JSX\n\n</Page>\n",
    )
    .unwrap();
    let src =
        "[page](page.mdx) and a note[^t].\n\n[^t]: | k | v |\n    |---|---|\n    | key | value |\n";
    let mut r = reader_in(src, dir.path());
    key(&mut r, KeyCode::Char('s'));
    assert_eq!(r.link_preview, "→ page.mdx · Inside JSX");
    key(&mut r, KeyCode::Char('s'));
    assert_eq!(r.link_preview, "[1] k v key value");
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
    let mut r = reader_in(
        "[next](next.md) and [web](https://example.com) and [gone](gone.md)\n",
        dir.path(),
    );
    key(&mut r, KeyCode::Char('L'));
    match key(&mut r, KeyCode::Enter) {
        Action::OpenDocument { path, anchor } => {
            assert_eq!(path, dir.path().join("next.md"));
            assert_eq!(anchor, None);
        }
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
    assert!(r.message().unwrap_or("").starts_with("not found"));
}

#[test]
fn reloads_when_file_changes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("doc.md");
    std::fs::write(&path, "# One\n").unwrap();
    let source = Source {
        file: Some(path.clone()),
        ..Source::in_memory("t.md", parse("# One\n"))
    };
    thread_local!(static NOW: std::cell::Cell<Option<std::time::Instant>> = const { std::cell::Cell::new(None) });
    fn now() -> std::time::Instant {
        NOW.with(|n| *n.get().get_or_insert_with(std::time::Instant::now))
    }
    let opts = Options {
        now,
        ..Options::default()
    };
    let mut r = Reader::new(source, &opts);
    r.layout(80, 10);
    std::fs::write(&path, "# Two\n\n```sh\necho x\n```\n").unwrap();
    // A later mtime than the first write's, on any filesystem's granularity.
    let later = std::time::SystemTime::now() + Duration::from_secs(10);
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(later)
        .unwrap();
    // Inside the throttle nothing is checked; past it, the change shows.
    assert!(!r.tick());
    NOW.with(|n| n.set(Some(now() + Duration::from_secs(1))));
    assert!(r.tick());
    r.layout(80, 10);
    assert_eq!(r.doc().title().as_deref(), Some("Two"));
    assert_eq!(r.rendered().code_blocks.len(), 1);
    assert_eq!(r.message(), Some("reloaded"));
}

#[test]
fn esc_returns_to_plain_reading_in_one_step() {
    let mut r = reader("[a](#a) and [b](#b)\n\nmore text here\n");
    key(&mut r, KeyCode::Char('/'));
    for c in "text".chars() {
        key(&mut r, KeyCode::Char(c));
    }
    key(&mut r, KeyCode::Enter);
    key(&mut r, KeyCode::Char('s'));
    assert!(r.search.is_some() && r.link_focus.is_some());
    // One Esc drops the search and the focus together...
    assert!(matches!(key(&mut r, KeyCode::Esc), Action::None));
    assert!(r.search.is_none() && r.link_focus.is_none());
    // ...and the next one goes back.
    assert!(matches!(key(&mut r, KeyCode::Esc), Action::Back));
}

#[test]
fn opening_an_overlay_replaces_the_open_one() {
    let mut r = reader("[a](https://a.dev)\n");
    key(&mut r, KeyCode::Char('L'));
    assert!(matches!(r.overlay, Some(Overlay::Links(_))));
    // The picker owns the keyboard: `?` moves nothing and opens nothing.
    key(&mut r, KeyCode::Char('?'));
    assert!(matches!(r.overlay, Some(Overlay::Links(_))));
    key(&mut r, KeyCode::Esc);
    assert!(r.overlay.is_none());
    key(&mut r, KeyCode::Char('?'));
    assert!(matches!(r.overlay, Some(Overlay::Help(_))));
}

#[test]
fn help_scrolls_and_closes_on_any_other_key() {
    let mut r = reader("text\n");
    key(&mut r, KeyCode::Char('?'));
    assert!(matches!(r.overlay, Some(Overlay::Help(_))));
    key(&mut r, KeyCode::Char('j'));
    assert!(
        matches!(&r.overlay, Some(Overlay::Help(h)) if h.scroll == 1),
        "j scrolls the help"
    );
    key(&mut r, KeyCode::Char('q'));
    assert!(r.overlay.is_none(), "first other key only closes help");
}

#[test]
fn edit_needs_a_file() {
    let mut r = reader("text\n");
    assert!(matches!(ctrl(&mut r, 'e'), Action::None));
    assert_eq!(r.message(), Some("no local file to edit"));

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("doc.md");
    std::fs::write(&path, "text\n").unwrap();
    let source = Source {
        file: Some(path.clone()),
        ..Source::in_memory("t.md", parse("text\n"))
    };
    let mut r = open(source);
    assert!(
        matches!(key(&mut r, KeyCode::Char('e')), Action::None),
        "only ctrl-e edits"
    );
    assert!(matches!(ctrl(&mut r, 'e'), Action::Edit(p, _) if p == path));
}

#[test]
fn p_shows_and_copies_the_file_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("doc.md");
    std::fs::write(&path, "text\n").unwrap();
    let opened = bastian::open::open(&path).unwrap();
    let mut r = open(Source::file("doc.md".into(), opened));
    let absolute = std::fs::canonicalize(&path).unwrap().display().to_string();

    key(&mut r, KeyCode::Char('p'));
    assert!(matches!(location(&r), Some(Location::File { .. })));
    assert!(matches!(key(&mut r, KeyCode::Char('A')), Action::Copy(p) if p == absolute));
    assert_eq!(r.message(), Some("copied absolute path"));
    // Outside the current directory, the relative path is the one opened.
    let relative = path.display().to_string();
    assert!(matches!(key(&mut r, KeyCode::Char('R')), Action::Copy(p) if p == relative));
    assert_eq!(r.message(), Some("copied relative path"));
    // Other keys are ignored while it's open; esc, q and p close it.
    assert!(matches!(key(&mut r, KeyCode::Char('j')), Action::None));
    for close in [KeyCode::Esc, KeyCode::Char('q'), KeyCode::Char('p')] {
        assert!(location(&r).is_some());
        assert!(matches!(key(&mut r, close), Action::None));
        assert!(location(&r).is_none());
        key(&mut r, KeyCode::Char('p'));
    }
}

#[test]
fn p_without_a_file() {
    let mut r = reader("text\n");
    key(&mut r, KeyCode::Char('p'));
    assert_eq!(location(&r), Some(&Location::Unnamed("t.md".into())));
    assert!(matches!(key(&mut r, KeyCode::Char('R')), Action::None));
    assert_eq!(r.message(), Some("no file path to copy"));

    let mut r = open(remote("text\n"));
    key(&mut r, KeyCode::Char('p'));
    let url = "https://example.com/repo/README.md";
    assert!(matches!(key(&mut r, KeyCode::Char('A')), Action::Copy(u) if u == url));
    assert_eq!(r.message(), Some("copied URL"));
}

#[test]
fn path_popup_wraps_long_paths() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(format!("{}.md", "x".repeat(60)));
    std::fs::write(&path, "text\n").unwrap();
    let opened = bastian::open::open(&path).unwrap();
    let mut r = Reader::new(Source::file("x.md".into(), opened), &Options::default());
    key(&mut r, KeyCode::Char('p'));
    let screen = screen_of(&mut r, 40, 20);
    assert!(screen.contains(" File Path "), "{screen}");
    assert!(
        screen.contains("Relative:") && screen.contains("Absolute:"),
        "{screen}"
    );
    let shown: String = screen.chars().filter(|c| !"│ \n".contains(*c)).collect();
    let absolute = std::fs::canonicalize(&path).unwrap().display().to_string();
    assert!(shown.contains(&absolute), "{screen}");
    // Too narrow for the keys on one row: one per row.
    assert!(screen.contains("R copy relative "), "{screen}");
    assert!(screen.contains("esc close"), "{screen}");
}

#[test]
fn path_popup_elides_paths_too_long_for_the_screen() {
    let dir = tempfile::tempdir().unwrap();
    let deep = dir.path().join("a".repeat(70)).join("b".repeat(70));
    std::fs::create_dir_all(&deep).unwrap();
    let path = deep.join("end.md");
    std::fs::write(&path, "text\n").unwrap();
    let opened = bastian::open::open(&path).unwrap();
    let mut r = Reader::new(Source::file("end.md".into(), opened), &Options::default());
    key(&mut r, KeyCode::Char('p'));

    let screen = screen_of(&mut r, 40, 12);
    assert!(
        screen.contains("Relative:") && screen.contains("Absolute:"),
        "{screen}"
    );
    assert!(screen.contains('…'), "{screen}");
    // The start and the file name of the absolute path stay in view.
    let absolute = std::fs::canonicalize(&path).unwrap().display().to_string();
    let shown: String = screen.chars().filter(|c| !"│ \n".contains(*c)).collect();
    assert!(shown.contains(&absolute[..10]), "{screen}");
    assert!(shown.contains("bbbb/end.md"), "{screen}");
    // So do the keys, and the box keeps its bottom border.
    assert!(screen.contains("esc close"), "{screen}");
    assert!(screen.contains('┘'), "{screen}");
}

#[test]
fn path_popup_names_stdin() {
    let source = Source::in_memory("stdin", parse("text\n"));
    let mut r = Reader::new(source, &Options::default());
    key(&mut r, KeyCode::Char('p'));
    assert_eq!(location(&r), Some(&Location::Unnamed("stdin".into())));
    let screen = screen_of(&mut r, 40, 12);
    assert!(screen.contains("(stdin)"), "{screen}");
    assert!(screen.contains("no local file"), "{screen}");
    assert!(screen.contains("esc close"), "{screen}");
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
    let src = std::fs::read_to_string("tests/fixtures/sample.md").unwrap();
    let opts = Options {
        max_width: 200,
        line_numbers: true,
        ..Options::default()
    };
    let mut r = Reader::new(Source::in_memory("t.md", parse(&src)), &opts);
    let (w, h) = (60u16, 20u16);
    let mut terminal = term(w, h);
    terminal.draw(|f| draw(f, &mut r)).unwrap();

    let gutter = gutter_width(&r, r.rendered().lines.len());
    assert!(gutter > GUTTER_GAP);
    let content = (w - MARGIN * 2 - gutter) as usize;
    for line in &r.rendered().lines {
        assert!(line.width() <= content, "{line:?} wider than {content}");
    }

    // Numbers sit right after the margin, right-aligned, starting at 1, on
    // the first row below the top padding, with a gap before the text.
    let row = &columns(&terminal, 0, w)[1];
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
    let mut r = in_memory("See [docs](https://x.dev) here.\n");
    let (w, h) = (60u16, 8u16);
    let mut terminal = term(w, h);
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
    let status = &columns(&terminal, 0, w)[usize::from(h - 1)];
    assert!(status.contains("→ https://x.dev"), "{status}");
    assert!(status.contains("link 1/1"), "{status}");
}

#[test]
fn focused_details_summary_is_highlighted() {
    let mut r = in_memory("intro\n\n<details>\n<summary>More</summary>\n\nhidden\n\n</details>\n");
    let (w, h) = (40u16, 8u16);
    let mut terminal = term(w, h);
    terminal.draw(|f| draw(f, &mut r)).unwrap();
    key(&mut r, KeyCode::Char('d'));
    terminal.draw(|f| draw(f, &mut r)).unwrap();

    let buf = terminal.backend().buffer();
    let theme = Theme::dark();
    let y = r.rendered().details[0].line as u16 + 1; // below the top padding
    let focused: Vec<_> = (0..w)
        .map(|x| &buf[(x, y)])
        .filter(|c| c.bg == theme.current_bg)
        .collect();
    let text: String = focused.iter().map(|c| c.symbol().to_string()).collect();
    assert!(text.contains("▶ More"), "{text:?}");
    // The row's text takes the contrasting foreground, as link focus does.
    assert!(focused.iter().all(|c| c.fg == theme.h1_fg), "{focused:?}");
}

#[test]
fn focused_code_block_frame_takes_the_accent() {
    let mut r = in_memory("- item\n\n  ```sh\n  echo hi\n  ```\n");
    let (w, h) = (40u16, 10u16);
    let mut terminal = term(w, h);
    terminal.draw(|f| draw(f, &mut r)).unwrap();
    let before = r.rendered().lines.clone();
    // All but the status bar, which names the focused block.
    let rows = |t: &Terminal<TestBackend>| columns(t, 0, w)[..usize::from(h - 1)].to_vec();
    let text = rows(&terminal);
    let theme = Theme::dark();
    let corner = |t: &Terminal<TestBackend>| t.backend().buffer()[(MARGIN + 2, 2)].clone();
    assert_eq!(corner(&terminal).symbol(), "┌");
    assert_eq!(corner(&terminal).fg, theme.muted);
    key(&mut r, KeyCode::Tab);
    terminal.draw(|f| draw(f, &mut r)).unwrap();

    // Focus only recolors: the lines and what's on screen stay put.
    assert_eq!(r.rendered().lines, before);
    assert_eq!(rows(&terminal), text);

    let buf = terminal.backend().buffer();
    let block = &r.rendered().code_blocks[0];
    let cols = block.frame.clone().unwrap();
    let x = |col: usize| MARGIN + col as u16;
    // Below the blank top row.
    let (top, bottom) = (block.lines.start as u16 + 1, block.lines.end as u16);
    for y in top..=bottom {
        for col in cols.clone() {
            let cell = &buf[(x(col), y)];
            let edge = y == top || y == bottom || col == cols.start || col == cols.end - 1;
            if edge {
                assert_eq!(cell.fg, theme.accent, "{col},{y} {:?}", cell.symbol());
            } else {
                assert_ne!(cell.fg, theme.accent, "{col},{y} {:?}", cell.symbol());
            }
        }
    }
    // Nothing in the margin or the list marker's column.
    assert_ne!(buf[(0, top)].fg, theme.accent);
    assert_ne!(buf[(x(0), top)].fg, theme.accent);
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
    let source = Source {
        file: Some("doc.md".into()),
        ..Source::in_memory("t.md", parse(&src))
    };
    let mut r = open(source);
    match ctrl(&mut r, 'e') {
        Action::Edit(_, line) => assert_eq!(line, Some(1)),
        _ => panic!("expected edit"),
    }
    // Scroll to paragraph 20 (rendered line 40, source line 41).
    for _ in 0..40 {
        key(&mut r, KeyCode::Char('j'));
    }
    match ctrl(&mut r, 'e') {
        Action::Edit(_, line) => assert_eq!(line, Some(41)),
        _ => panic!("expected edit"),
    }
}

#[test]
fn c_copies_the_whole_document() {
    let mut r = reader("# A\n\ntext\n");
    assert!(matches!(key(&mut r, KeyCode::Char('c')), Action::None));
    assert_eq!(r.message(), Some("nothing to copy"));
    let source = Source {
        text: "# A\n\ntext\n".into(),
        ..Source::in_memory("t.md", parse("# A\n\ntext\n"))
    };
    let mut r = open(source);
    match key(&mut r, KeyCode::Char('c')) {
        Action::Copy(t) => assert_eq!(t, "# A\n\ntext\n"),
        _ => panic!("expected copy"),
    }
    assert_eq!(r.message(), Some("copied document (3 lines)"));
}

fn settings(assume_yes: bool, positions: Option<std::path::PathBuf>) -> Settings {
    Settings {
        reader: Options {
            assume_yes,
            positions,
            ..Options::default()
        },
        scan: bastian::browser::Scan::default(),
        date_format: Some(bastian::config::DATE_FORMAT.into()),
        editor: None,
        mouse: false,
        logo: true,
        images: None,
        show_images: false,
        selector: bastian::config::Selector::Bar,
        list_layout: bastian::config::ListLayout::Cards,
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
    assert_eq!(r.scroll(), 0);
    for _ in 0..40 {
        key(&mut r, KeyCode::Char('j'));
    }
    let left_at = r.source_line();
    assert!(left_at > Some(1));
    r.remember_position();

    // A narrower window still opens at the same block.
    let mut again = s.open(&file, "long.md".into()).unwrap();
    again.layout(60, 10);
    assert!(again.scroll() > 0);
    assert_eq!(again.source_line(), left_at);

    // Keys that don't move the view keep the restore going; moving ends it.
    key(&mut again, KeyCode::Char('#'));
    again.layout(60, 10);
    assert_eq!(again.source_line(), left_at);
    let before = again.scroll();
    key(&mut again, KeyCode::Char('j'));
    again.layout(60, 10);
    // A restore still in force would snap the view back to the block top.
    assert_eq!(again.scroll(), before + 1);
    again.layout(60, 10);
    assert_eq!(again.scroll(), before + 1);

    // Back at the top, the position is forgotten.
    key(&mut again, KeyCode::Char('g'));
    again.remember_position();
    let mut third = s.open(&file, "long.md".into()).unwrap();
    third.layout(80, 10);
    assert_eq!(third.scroll(), 0);

    // Turned off: nothing is restored or saved.
    let off = settings(false, None);
    let mut r = off.open(&file, "long.md".into()).unwrap();
    r.layout(80, 10);
    key(&mut r, KeyCode::Char('G'));
    r.remember_position();
    assert_eq!(bastian::positions::load(&store, &file), None);
}

/// A reader over one link to `url`, its picker open.
fn picking(url: &str, base_dir: &std::path::Path) -> Reader {
    let mut r = reader_in(&format!("[x]({url})\n"), base_dir);
    key(&mut r, KeyCode::Char('L'));
    r
}

#[test]
fn web_links_open_without_asking() {
    for url in ["https://x.dev/a", "mailto:a@b.c"] {
        let mut r = picking(url, std::path::Path::new("."));
        assert!(
            matches!(key(&mut r, KeyCode::Enter), Action::OpenExternal(t) if t == url),
            "{url}"
        );
        assert_eq!(confirm_open(&r), None);
    }
}

#[test]
fn other_links_ask_before_opening() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("setup.command"), "").unwrap();
    let local = dir.path().join("setup.command").display().to_string();
    for (url, target) in [
        ("file:///Applications/X.app", "file:///Applications/X.app"),
        ("vscode://file/etc/hosts", "vscode://file/etc/hosts"),
        ("tel:+15551234", "tel:+15551234"),
        ("javascript:alert(1)", "javascript:alert(1)"),
        ("setup.command", local.as_str()),
    ] {
        let mut r = picking(url, dir.path());
        assert!(matches!(key(&mut r, KeyCode::Enter), Action::None), "{url}");
        assert_eq!(confirm_open(&r), Some(target));
        assert!(
            matches!(key(&mut r, KeyCode::Char('y')), Action::OpenExternal(t) if t == target),
            "{url}"
        );
        assert_eq!(confirm_open(&r), None);

        let mut r = picking(url, dir.path());
        key(&mut r, KeyCode::Enter);
        assert!(
            matches!(key(&mut r, KeyCode::Char('n')), Action::None),
            "{url}"
        );
        assert_eq!(confirm_open(&r), None);
    }
}

#[test]
fn open_prompt_shows_the_whole_target() {
    let target = format!("vscode://file/{}/end", "x".repeat(60));
    let mut r = picking(&target, std::path::Path::new("."));
    key(&mut r, KeyCode::Enter);
    let screen = screen_of(&mut r, 40, 12);
    // Wrapped across rows, with no cell lost to the box edge.
    let shown: String = screen.chars().filter(|c| !"│ \n".contains(*c)).collect();
    assert!(shown.contains(&target), "{screen}");
}

#[test]
fn open_prompt_elides_the_middle_of_a_long_target() {
    // About 15 rows at 36 columns, where 7 fit.
    let target = format!("vscode://host/{}/tail/setup.command", "x".repeat(500));
    let mut r = picking(&target, std::path::Path::new("."));
    key(&mut r, KeyCode::Enter);
    assert_eq!(confirm_open(&r), Some(target.as_str()));
    let screen = screen_of(&mut r, 40, 12);
    let rows: Vec<&str> = screen.lines().collect();
    let shown: String = screen.chars().filter(|c| !"│ \n".contains(*c)).collect();
    assert!(shown.contains("vscode://host/xxx"), "{screen}");
    assert!(shown.contains("xxx/tail/setup.command"), "{screen}");
    assert!(
        rows.iter().any(|row| row.trim_matches(['│', ' ']) == "…"),
        "{screen}"
    );
    assert!(
        rows.iter()
            .any(|row| row.contains("y open") && row.contains("n cancel")),
        "{screen}"
    );

    // Even a screen too short for the question keeps the keys.
    for height in 3..8 {
        let screen = screen_of(&mut r, 40, height);
        let has_keys = screen.lines().any(|row| row.contains("y open"));
        assert!(has_keys, "height {height}");
    }
}

#[test]
fn remote_documents_resolve_links_and_always_confirm() {
    // Built the way the app builds fetched documents, with --yes on.
    let settings = settings(true, None);
    let src = "See [guide](docs/guide.md) and [logo](img/logo.png).\n\n```sh\necho hi\n```\n";
    let mut r = Reader::new(remote(src), &settings.reader);
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
    assert_eq!(confirm(&r), Some(&[0][..]));
}

#[test]
fn remote_markdown_links_with_other_schemes_ask() {
    let settings = settings(false, None);
    for target in [
        "file:///tmp/a.md",
        "vscode://file/a.md",
        "mailto:user@example.md",
    ] {
        let mut r = Reader::new(remote(&format!("[x]({target})\n")), &settings.reader);
        r.layout(80, 10);
        key(&mut r, KeyCode::Char('L'));
        let action = key(&mut r, KeyCode::Enter);
        if target.starts_with("mailto:") {
            assert!(matches!(action, Action::OpenExternal(t) if t == target));
        } else {
            assert!(matches!(action, Action::None), "{target}");
            assert_eq!(confirm_open(&r), Some(target));
        }
    }
}

#[test]
fn reload_with_changed_code_drops_pending_confirmation() {
    let mut r = reader("```sh\necho one\n```\n");
    key(&mut r, KeyCode::Tab);
    key(&mut r, KeyCode::Char('r'));
    assert_eq!(confirm(&r), Some(&[0][..]));

    // Prose edits keep the prompt: block 0 is still the same code.
    r.replace_doc(parse("intro\n\n```sh\necho one\n```\n"));
    assert_eq!(confirm(&r), Some(&[0][..]));

    r.replace_doc(parse("```sh\nrm -rf ~\n```\n\n```sh\necho one\n```\n"));
    assert_eq!(confirm(&r), None);
}

#[test]
fn reload_drops_pending_open_prompt() {
    let mut r = picking("vscode://file/etc/hosts", std::path::Path::new("."));
    key(&mut r, KeyCode::Enter);
    assert_eq!(confirm_open(&r), Some("vscode://file/etc/hosts"));

    // Even an unchanged link may now sit elsewhere; ask again after a reload.
    r.replace_doc(parse("[x](vscode://file/etc/hosts)\n"));
    assert_eq!(confirm_open(&r), None);
    assert!(matches!(key(&mut r, KeyCode::Char('y')), Action::None));
}

#[test]
fn replaced_document_is_highlighted_afresh() {
    let mut r = reader("```sh\necho old\n```\n");
    let text = |r: &Reader| bastian::render::to_plain(&r.rendered().lines);
    assert!(text(&r).contains("echo old"));
    r.replace_doc(parse("```sh\necho new\n```\n"));
    r.layout(80, 10);
    assert!(text(&r).contains("echo new"), "{}", text(&r));
}

#[test]
fn scrollbar_shows_only_when_the_document_overflows() {
    let column = |src: &str, scroll: usize| {
        let mut r = in_memory(src);
        let mut terminal = term(40, 12);
        terminal.draw(|f| draw(f, &mut r)).unwrap();
        r.scroll_to(scroll);
        terminal.draw(|f| draw(f, &mut r)).unwrap();
        // Document rows, between the top and bottom padding, in the last column.
        columns(&terminal, 39, 40)[1..10].concat()
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
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("guide.md"), "# Guide\n").unwrap();
    let source = Source {
        base_dir: dir.path().to_path_buf(),
        ..Source::in_memory(
            "t.md",
            parse("See [docs](https://x.dev/docs) and [guide](guide.md#setup), [top](#top).\n"),
        )
    };
    let mut r = Reader::new(source, &Options::default());
    let mut guide = url::Url::from_file_path(dir.path().join("guide.md")).unwrap();
    guide.set_fragment(Some("setup"));
    let mut terminal = term(80, 6);
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
        symbols.iter().any(|s| s.starts_with(&osc(guide.as_str()))),
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
    assert_eq!(r.hyperlink_targets()[2], None);
}

#[test]
fn popups_hide_hyperlinks_beneath_them() {
    let doc = "See [docs](https://x.dev/docs) and [more](https://x.dev/more) here.\n";
    let mut r = in_memory(doc);
    key(&mut r, KeyCode::Char('?'));
    let mut terminal = term(100, 30);
    let frame = terminal.draw(|f| draw(f, &mut r)).unwrap();
    let buf = frame.buffer;
    let cells: String = (0..30)
        .flat_map(|y| (0..100).map(move |x| (x, y)))
        .map(|(x, y)| buf[(x, y)].symbol().to_string())
        .collect();
    // Their labels would print over the popup.
    assert!(!cells.contains("\u{1b}]8;;"), "no OSC 8 links under help");
}

#[test]
fn hyperlink_targets_follow_the_rendered_links() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    std::fs::write(d.join("guide.md"), "# Guide\n").unwrap();
    std::fs::write(d.join("setup.command"), "").unwrap();
    std::fs::create_dir(d.join("X.app")).unwrap();
    std::fs::create_dir(d.join("dir.md")).unwrap();
    let file = |name: &str| url::Url::from_file_path(d.join(name)).unwrap().to_string();
    let urls = [
        "https://x.dev/a".to_string(),
        "mailto:a@b.c".into(),
        "guide.md".into(),
        file("guide.md"),
        "setup.command".into(),
        file("setup.command"),
        "X.app".into(),
        file("X.app"),
        "file:///Applications/X.app".into(),
        "vscode://file/etc/hosts".into(),
        "vscode://file/tmp/a.md".into(),
        "javascript:alert(1)".into(),
        "tel:+15551234".into(),
        "dir.md".into(),
        "missing.md".into(),
    ];
    let src: String = urls.iter().map(|u| format!("[x]({u})\n\n")).collect();
    let source = Source {
        base_dir: d.to_path_buf(),
        ..Source::in_memory("t.md", parse(&src))
    };
    let mut r = Reader::new(source, &Options::default());
    r.layout(80, 40);
    assert_eq!(r.rendered().links.len(), urls.len());
    // Web and mail links, and existing Markdown files, open on a click;
    // anything that could launch an app or run a script does not.
    let want = [
        Some("https://x.dev/a".to_string()),
        Some("mailto:a@b.c".into()),
        Some(file("guide.md")),
        Some(file("guide.md")),
    ];
    assert_eq!(&r.hyperlink_targets()[..4], &want);
    for (url, target) in urls.iter().zip(r.hyperlink_targets()).skip(4) {
        assert_eq!(*target, None, "{url}");
    }

    // In a remote document, only web targets.
    let src = format!(
        "[a](docs/guide.md)\n\n[b](vscode://file/etc/hosts)\n\n[c]({})\n",
        file("guide.md")
    );
    let source = Source {
        remote_base: Some(url::Url::parse("https://example.com/repo/README.md").unwrap()),
        ..Source::in_memory("t.md", parse(&src))
    };
    let mut r = Reader::new(source, &Options::default());
    r.layout(80, 40);
    assert_eq!(
        r.hyperlink_targets(),
        [
            Some("https://example.com/repo/docs/guide.md".to_string()),
            None,
            None
        ]
    );

    // With hyperlinks off, nothing is worked out.
    let opts = Options {
        hyperlinks: false,
        ..Options::default()
    };
    let mut r = Reader::new(
        Source::in_memory("t.md", parse("[x](https://x.dev)\n")),
        &opts,
    );
    r.layout(80, 40);
    assert_eq!(r.hyperlink_targets(), [] as [Option<String>; 0]);
}

#[test]
fn code_files_always_show_line_numbers() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();
    let opened = bastian::open::open(&dir.path().join("main.rs")).unwrap();
    let r = Reader::new(Source::file("main.rs".into(), opened), &Options::default());
    assert!(r.line_numbers);
    assert_eq!(r.base_dir(), dir.path());

    // A code file fetched over HTTP keeps the configured setting.
    let fetched = RemoteDoc {
        name: "main.rs".into(),
        base: url::Url::parse("https://example.com/x/main.rs").unwrap(),
        text: "fn main() {}\n".into(),
    };
    let r = Reader::new(Source::remote(fetched), &Options::default());
    assert!(r.doc().code_file && !r.line_numbers);
}

#[test]
fn image_sources_resolve_like_links() {
    use bastian::images::ImageOrigin;
    let source = Source {
        base_dir: std::path::PathBuf::from("/docs"),
        ..Source::in_memory("t.md", parse("x\n"))
    };
    let r = Reader::new(source, &Options::default());
    assert_eq!(
        r.image_source("img/a.png?raw=1"),
        Some(ImageOrigin::File("/docs/img/a.png".into()))
    );
    assert_eq!(
        r.image_source("https://github.com/o/r/blob/main/shot.png"),
        Some(ImageOrigin::Url(
            "https://raw.githubusercontent.com/o/r/main/shot.png".into()
        ))
    );
    assert_eq!(r.image_source("data:image/png;base64,AAAA"), None);
    let source = Source {
        remote_base: Some(url::Url::parse("https://github.com/o/r/blob/main/README.md").unwrap()),
        ..Source::in_memory("t.md", parse("x\n"))
    };
    let r = Reader::new(source, &Options::default());
    assert_eq!(
        r.image_source("docs/b.png"),
        Some(ImageOrigin::Url(
            "https://raw.githubusercontent.com/o/r/main/docs/b.png".into()
        ))
    );
}

#[test]
fn pictures_partly_scrolled_off_show_their_visible_rows() {
    use bastian::images::Support;
    use ratatui_image::picker::Picker;

    let dir = tempfile::tempdir().unwrap();
    image::RgbImage::from_pixel(200, 200, image::Rgb([220, 40, 40]))
        .save(dir.path().join("p.png"))
        .unwrap();
    let tail: String = (0..40).map(|i| format!("line {i}\n\n")).collect();
    let src = format!("intro\n\n![pic](p.png)\n\n{tail}");
    let source = Source {
        base_dir: dir.path().to_path_buf(),
        ..Source::in_memory("t.md", parse(&src))
    };
    let opts = Options {
        images: Some(Support::fixed(Picker::halfblocks())),
        ..Options::default()
    };
    let mut r = Reader::new(source, &opts);

    let mut terminal = term(60, 20);
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while r.rendered().images.is_empty() {
        terminal.draw(|f| draw(f, &mut r)).unwrap();
        r.tick();
        assert!(std::time::Instant::now() < deadline, "picture did not load");
        thread::sleep(Duration::from_millis(10));
    }
    terminal.draw(|f| draw(f, &mut r)).unwrap();
    let slot = r.rendered().images[0].clone();
    assert!(slot.rows >= 4, "{slot:?}");

    // Scroll the top half of the picture off screen.
    r.scroll_to(slot.line + usize::from(slot.rows) / 2);
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
fn toc_pane_sits_beside_the_document_and_follows_the_scroll() {
    use ratatui::style::Modifier;

    let long = "word ".repeat(60);
    let src = std::fs::read_to_string("tests/fixtures/outline.md").unwrap() + "\n" + &long;
    let mut r = in_memory(&src);
    let widest = |r: &Reader| r.rendered().lines.iter().map(Line::width).max().unwrap();
    let mut terminal = term(70, 12);
    terminal.draw(|f| draw(f, &mut r)).unwrap();
    // The document fills the screen less its margins.
    assert!(widest(&r) > 40, "{}", widest(&r));

    assert!(matches!(key(&mut r, KeyCode::Char('t')), Action::None));
    terminal.draw(|f| draw(f, &mut r)).unwrap();
    // Re-wrapped beside the 30-column pane, within its own margins.
    assert!(widest(&r) <= 70 - 30 - 4, "{}", widest(&r));
    let pane = columns(&terminal, 0, 30);
    assert_eq!(pane[0], " Table of Contents       < > │");
    assert!(pane[1].contains("────"), "{pane:#?}");
    // The lone title isn't numbered; its sections are, and deeper
    // headings indent under them, numbered within their section.
    let entries: Vec<&str> = pane[2..9]
        .iter()
        .map(|r| r.trim_end_matches([' ', '│']))
        .collect();
    assert_eq!(
        entries,
        [
            " Guide",
            " 01 Install",
            "    1 From source",
            "    2 With a package manager",
            " 02 Usage",
            "    1 Skipped a level",
            " 03 Usage",
        ]
    );
    let doc = columns(&terminal, 30, 70);
    assert!(doc[1].starts_with("   Guide"), "{doc:#?}");

    // `2` scrolls to the second section, which the pane then highlights.
    key(&mut r, KeyCode::Char('2'));
    let top = bastian::render::plain(&r.rendered().lines[r.scroll()]);
    assert_eq!(top.trim(), "Usage");
    terminal.draw(|f| draw(f, &mut r)).unwrap();
    let pane = columns(&terminal, 0, 30);
    let row = pane
        .iter()
        .position(|r| r.starts_with(" 02 Usage"))
        .unwrap();
    let cell = &terminal.backend().buffer()[(4, row as u16)];
    assert_eq!(cell.fg, Theme::dark().accent);
    assert!(cell.modifier.contains(Modifier::BOLD));
    let other = pane
        .iter()
        .position(|r| r.starts_with(" 01 Install"))
        .unwrap();
    let cell = &terminal.backend().buffer()[(4, other as u16)];
    assert!(!cell.modifier.contains(Modifier::BOLD));

    // Closed again, the document takes the full width back.
    key(&mut r, KeyCode::Char('t'));
    terminal.draw(|f| draw(f, &mut r)).unwrap();
    assert!(widest(&r) > 40);
}

#[test]
fn toc_pane_keeps_the_same_source_at_the_top() {
    // Long paragraphs, so the rendered rows above one change with the width.
    let src: String = std::iter::once("# Title\n\n".to_string())
        .chain((0..30).map(|i| format!("p{i} {}\n\n", "word ".repeat(40))))
        .collect();
    let mut r = in_memory(&src);
    let mut terminal = term(100, 12);
    terminal.draw(|f| draw(f, &mut r)).unwrap();
    let top = |r: &Reader| bastian::render::plain(&r.rendered().lines[r.scroll()]);
    let p20 = r
        .rendered()
        .lines
        .iter()
        .position(|l| bastian::render::plain(l).trim_start().starts_with("p20 "))
        .unwrap();
    r.scroll_by(p20 as isize);
    assert!(top(&r).trim_start().starts_with("p20 "), "{}", top(&r));

    for _ in 0..2 {
        key(&mut r, KeyCode::Char('t'));
        terminal.draw(|f| draw(f, &mut r)).unwrap();
        assert!(top(&r).trim_start().starts_with("p20 "), "{}", top(&r));
    }
    // Resizing the open pane keeps it too.
    key(&mut r, KeyCode::Char('t'));
    key(&mut r, KeyCode::Char('>'));
    terminal.draw(|f| draw(f, &mut r)).unwrap();
    assert!(top(&r).trim_start().starts_with("p20 "), "{}", top(&r));
}

#[test]
fn focused_code_block_frame_follows_the_toc_pane() {
    let mut r = in_memory("# A\n\n```sh\necho hi\n```\n");
    let mut terminal = term(80, 12);
    terminal.draw(|f| draw(f, &mut r)).unwrap();
    key(&mut r, KeyCode::Char('t'));
    key(&mut r, KeyCode::Tab);
    terminal.draw(|f| draw(f, &mut r)).unwrap();
    let pane = r.fit_toc(80);
    assert!(pane > 0);

    let buf = terminal.backend().buffer();
    let block = &r.rendered().code_blocks[0];
    let cols = block.frame.clone().unwrap();
    let theme = Theme::dark();
    // Below the blank top row, right of the pane.
    let y = block.lines.start as u16 + 1;
    let x = |col: usize| pane + MARGIN + col as u16;
    assert_eq!(buf[(x(cols.start), y)].symbol(), "┌");
    for col in cols.clone() {
        assert_eq!(buf[(x(col), y)].fg, theme.accent, "{col}");
    }
    assert_eq!(buf[(x(cols.end - 1), y)].symbol(), "┐");
    // Nothing left of the frame is recolored.
    assert_ne!(buf[(x(cols.start) - 1, y)].fg, theme.accent);
}

#[test]
fn theme_picker_pane_sits_right_of_the_document() {
    let src = "# Title\n\n".to_string() + &"word ".repeat(80);
    let mut r = in_memory(&src);
    let widest = |r: &Reader| r.rendered().lines.iter().map(Line::width).max().unwrap();
    let mut terminal = term(100, 12);
    terminal.draw(|f| draw_with(f, &mut r, None)).unwrap();
    assert!(widest(&r) > 70, "{}", widest(&r));

    let mut p = ThemePicker::new(None, "nord", picker::DEFAULT_WIDTH);
    terminal
        .draw(|f| draw_with(f, &mut r, Some(&mut p)))
        .unwrap();
    // Re-wrapped left of the 34-column pane, within its own margins.
    assert!(widest(&r) <= 100 - 34 - 4, "{}", widest(&r));
    let pane = columns(&terminal, 66, 100);
    assert!(pane[0].starts_with("│ Themes"), "{pane:#?}");
    assert!(pane.iter().any(|r| r.starts_with("│ ✓ nord")), "{pane:#?}");
    let doc = columns(&terminal, 0, 66);
    assert!(doc[1].starts_with("   Title"), "{doc:#?}");
    // Above the status bar, which keeps the whole width and the keys.
    assert!(!pane[11].contains('│'), "{pane:#?}");
    assert!(columns(&terminal, 0, 100)[11].contains("enter use · esc cancel"));
}

#[test]
fn theme_picker_and_toc_share_the_screen_when_it_fits() {
    let src = "# A\n\ntext\n\n# B\n\nmore\n";
    let mut r = in_memory(src);
    let mut p = ThemePicker::new(None, "nord", picker::DEFAULT_WIDTH);
    // Wide: the table of contents left, the picker right, the document
    // between them.
    let mut terminal = term(120, 12);
    terminal.draw(|f| draw_with(f, &mut r, None)).unwrap();
    key(&mut r, KeyCode::Char('t'));
    terminal
        .draw(|f| draw_with(f, &mut r, Some(&mut p)))
        .unwrap();
    let screen = columns(&terminal, 0, 120);
    assert!(screen[0].starts_with(" Table of Contents"), "{screen:#?}");
    assert_eq!(screen[0].chars().nth(120 - 34), Some('│'), "{screen:#?}");
    assert!(screen[1].contains("   A"), "{screen:#?}");

    // At 80 columns there's no room for both: the table of contents makes
    // way for the picker while it's open.
    let mut terminal = term(80, 12);
    terminal
        .draw(|f| draw_with(f, &mut r, Some(&mut p)))
        .unwrap();
    let screen = columns(&terminal, 0, 80);
    assert!(
        !screen.join("").contains("Table of Contents"),
        "{screen:#?}"
    );
    assert!(screen[1].starts_with("   A"), "{screen:#?}");
    assert!(screen[0].contains("│ Themes"), "{screen:#?}");
    // And it's back once the picker closes.
    terminal.draw(|f| draw_with(f, &mut r, None)).unwrap();
    let screen = columns(&terminal, 0, 80);
    assert!(screen[0].starts_with(" Table of Contents"), "{screen:#?}");
}

#[test]
fn theme_picker_covers_a_screen_too_narrow_to_share() {
    let mut r = in_memory("# A\n\ntext\n");
    let mut terminal = term(30, 12);
    terminal.draw(|f| draw_with(f, &mut r, None)).unwrap();
    let lines = r.rendered().lines.len();
    let mut p = ThemePicker::new(None, "nord", picker::DEFAULT_WIDTH);
    terminal
        .draw(|f| draw_with(f, &mut r, Some(&mut p)))
        .unwrap();
    let screen = columns(&terminal, 0, 30);
    assert!(screen[0].starts_with("│ Themes"), "{screen:#?}");
    assert!(
        screen.iter().any(|r| r.starts_with("│ ✓ nord")),
        "{screen:#?}"
    );
    // The document underneath isn't laid out again.
    assert_eq!(r.rendered().lines.len(), lines);
}

#[test]
fn theme_picker_pane_keeps_the_same_source_at_the_top() {
    let src: String = std::iter::once("# Title\n\n".to_string())
        .chain((0..30).map(|i| format!("p{i} {}\n\n", "word ".repeat(40))))
        .collect();
    let mut r = in_memory(&src);
    let mut terminal = term(100, 12);
    terminal.draw(|f| draw_with(f, &mut r, None)).unwrap();
    let top = |r: &Reader| bastian::render::plain(&r.rendered().lines[r.scroll()]);
    let p20 = r
        .rendered()
        .lines
        .iter()
        .position(|l| bastian::render::plain(l).trim_start().starts_with("p20 "))
        .unwrap();
    r.scroll_by(p20 as isize);

    let mut p = ThemePicker::new(None, "nord", picker::DEFAULT_WIDTH);
    terminal
        .draw(|f| draw_with(f, &mut r, Some(&mut p)))
        .unwrap();
    assert!(top(&r).trim_start().starts_with("p20 "), "{}", top(&r));
    // Widening the pane, and closing it, keep it too.
    p.on_key(KeyEvent::new(KeyCode::Char('>'), KeyModifiers::NONE));
    terminal
        .draw(|f| draw_with(f, &mut r, Some(&mut p)))
        .unwrap();
    assert!(top(&r).trim_start().starts_with("p20 "), "{}", top(&r));
    terminal.draw(|f| draw_with(f, &mut r, None)).unwrap();
    assert!(top(&r).trim_start().starts_with("p20 "), "{}", top(&r));
}

#[test]
fn hyperlinks_and_scrollbar_can_be_turned_off() {
    let long: String = (0..60)
        .map(|i| format!("[link {i}](https://x.dev/{i})\n\n"))
        .collect();
    let opts = Options {
        hyperlinks: false,
        scrollbar: false,
        ..Options::default()
    };
    let mut r = Reader::new(Source::in_memory("t.md", parse(&long)), &opts);
    let mut terminal = term(40, 12);
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

#[test]
fn the_line_gone_to_is_highlighted() {
    let mut r = in_memory(&filler(50));
    let mut terminal = term(60, 14);
    terminal.draw(|f| draw(f, &mut r)).unwrap();
    key(&mut r, KeyCode::Char(':'));
    // Rendered line 97 is "para 48", near the end.
    for c in "97".chars() {
        key(&mut r, KeyCode::Char(c));
    }
    key(&mut r, KeyCode::Enter);
    let current_bg = r.theme().current_bg;
    let row_of = |terminal: &Terminal<TestBackend>, text: &str| {
        let rows = columns(terminal, 0, 60);
        let y = rows.iter().position(|row| row.contains(text));
        y.unwrap_or_else(|| panic!("no {text}")) as u16
    };
    terminal.draw(|f| draw(f, &mut r)).unwrap();
    let y = row_of(&terminal, "para 48");
    let buf = terminal.backend().buffer();
    let x = (0..60).find(|&x| buf[(x, y)].symbol() == "p").unwrap();
    assert_eq!(buf[(x, y)].bg, current_bg, "the target line");
    let other = row_of(&terminal, "para 49");
    assert_ne!(buf[(x, other)].bg, current_bg, "only the target line");
    // The next key clears it.
    key(&mut r, KeyCode::Char('j'));
    terminal.draw(|f| draw(f, &mut r)).unwrap();
    let y = row_of(&terminal, "para 48");
    assert_ne!(terminal.backend().buffer()[(x, y)].bg, current_bg);
}
