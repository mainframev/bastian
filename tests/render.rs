use bastian::doc::parse;
use bastian::render::theme::Theme;
use bastian::render::{render, to_plain};
use unicode_width::UnicodeWidthChar;

fn snapshot(name: &str, width: u16) {
    let src = std::fs::read_to_string(format!("tests/fixtures/{name}.md")).unwrap();
    let rendered = render(&parse(&src), width, &Theme::dark());
    insta::assert_snapshot!(format!("{name}_{width}"), to_plain(&rendered.lines));
}

#[test]
fn sample_60() {
    snapshot("sample", 60);
}

#[test]
fn sample_100() {
    snapshot("sample", 100);
}

#[test]
fn callouts_40() {
    snapshot("callouts", 40);
}

#[test]
fn footnotes_40() {
    snapshot("footnotes", 40);
}

#[test]
fn tasks_40() {
    snapshot("tasks", 40);
}

#[test]
fn cjk_30() {
    snapshot("cjk", 30);
}

// Printed output can't open a details block, so every one shows open.
#[test]
fn details_60() {
    snapshot("details", 60);
}

// Frames take the full width, wide characters wrap inside them, long
// labels shorten, and blocks too narrow for a frame drop it.
#[test]
fn code_40() {
    snapshot("code", 40);
}

#[test]
fn code_14() {
    snapshot("code", 14);
}

#[test]
fn code_frames_fit_every_width() {
    let src = std::fs::read_to_string("tests/fixtures/code.md").unwrap();
    let doc = parse(&src);
    for width in 0u16..=40 {
        let r = render(&doc, width, &Theme::dark());
        for line in &r.lines {
            assert!(
                line.width() <= usize::from(width.max(10)),
                "width {width}: {line:?}"
            );
        }
        for c in &r.code_blocks {
            let lines = &r.lines[c.lines.clone()];
            let Some(cols) = &c.frame else { continue };
            // The corners sit at the frame's first and last columns.
            for (line, corners) in [(&lines[0], "┌┐"), (&lines[lines.len() - 1], "└┘")] {
                let text = bastian::render::plain(line);
                let cells: Vec<char> = text
                    .chars()
                    .flat_map(|c| std::iter::repeat_n(c, c.width().unwrap_or(0)))
                    .collect();
                let found = [cells[cols.start], cells[cols.end - 1]];
                assert_eq!(found.iter().collect::<String>(), corners, "{text}");
                assert_eq!(cells.len(), cols.end, "width {width}: {text}");
            }
        }
    }
}

#[test]
fn code_ranges_cover_code_lines() {
    let src = "text\n\n```sh\na\nb\n```\n\nmore\n\n```\nc\n```\n";
    let r = render(&parse(src), 40, &Theme::dark());
    let ids: Vec<usize> = r.code_blocks.iter().map(|c| c.id).collect();
    assert_eq!(ids, [0, 1]);
    // top border + 2 source lines + bottom border
    assert_eq!(r.code_blocks[0].lines, 2..6);
    let plain = to_plain(&r.lines[r.code_blocks[0].lines.clone()]);
    assert!(plain.contains(" a") && plain.contains(" b"), "{plain}");
}

#[test]
fn lines_never_exceed_width() {
    let src = std::fs::read_to_string("tests/fixtures/sample.md").unwrap();
    for width in [20u16, 37, 60, 100] {
        let r = render(&parse(&src), width, &Theme::dark());
        for line in &r.lines {
            assert!(line.width() <= width as usize, "width {width}: {line:?}");
        }
    }
}

#[test]
fn source_files_render_as_bare_code_with_source_lines() {
    use bastian::doc::{Block, parse_for};
    use std::path::Path;

    let src =
        "fn main() {\n    println!(\"a very long line that has to wrap at a narrow width\");\n}\n";
    let doc = parse_for(src, Some(Path::new("main.rs")));
    assert!(doc.code_file);
    assert!(matches!(&doc.blocks[..], [Block::Code(c)] if c.lang.as_deref() == Some("rs")));

    let r = render(&doc, 30, &Theme::dark());
    // No code box: the first rendered line is the first source line.
    assert_eq!(to_plain(&r.lines[..1]), "fn main() {");
    // The long line wraps; only its first segment carries a line number.
    assert_eq!(r.src_line[0], Some(0));
    assert_eq!(r.src_line[1], Some(1));
    assert_eq!(r.src_line[2], None);
    assert_eq!(*r.src_line.last().unwrap(), Some(2));
    assert_eq!(r.src_line.len(), r.lines.len());
    for line in &r.lines {
        assert!(line.width() <= 30);
    }
}

#[test]
fn markdown_and_unknown_extensions_stay_markdown() {
    use bastian::doc::parse_for;
    use std::path::Path;

    for name in ["a.md", "a.mkd", "README", "notes.unknownext"] {
        let doc = parse_for("# Title\n", Some(Path::new(name)));
        assert!(!doc.code_file, "{name}");
        assert_eq!(doc.title().as_deref(), Some("Title"), "{name}");
    }
    let src_line = render(&parse_for("# T\n", None), 40, &Theme::dark()).src_line;
    assert!(src_line.is_empty(), "{src_line:?}");
}

#[test]
fn tables_40() {
    snapshot("tables", 40);
}

#[test]
fn tables_80() {
    snapshot("tables", 80);
}

#[test]
fn links_40() {
    snapshot("links", 40);
}

#[test]
fn outline_80() {
    snapshot("outline", 80);
}

/// Colors as printed with `--color always`, under a few themes, so palette
/// and highlighting changes show up in review.
#[test]
fn themed_ansi() {
    let src = "# Title\n\nSome *text* with `code` and a [link](https://x.dev).\n\n```rust\nfn main() { let s = \"hi\"; }\n```\n\n> quote\n";
    for name in ["tokyonight-night", "catppuccin-latte", "gruvbox-dark"] {
        let theme = bastian::themes::load(name, None).unwrap();
        let rendered = render(&parse(src), 50, &theme);
        insta::assert_snapshot!(
            format!("ansi_{name}"),
            bastian::render::to_ansi(&rendered.lines)
        );
    }
}
