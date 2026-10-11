use bastian::doc::parse;
use bastian::render::theme::Theme;
use bastian::render::{render, to_plain};

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
fn code_ranges_cover_code_lines() {
    let src = "text\n\n```sh\na\nb\n```\n\nmore\n\n```\nc\n```\n";
    let r = render(&parse(src), 40, &Theme::dark());
    let ids: Vec<usize> = r.code_blocks.iter().map(|c| c.id).collect();
    assert_eq!(ids, [0, 1]);
    // label line + 2 source lines + bottom padding
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
