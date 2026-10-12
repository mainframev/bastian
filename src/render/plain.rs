use ratatui::text::Line;

/// `s` without control characters. Document text goes straight to the
/// terminal in print and pager modes, and an ESC in it would run as an
/// escape sequence there; ratatui drops them the same way in the TUI.
pub fn strip_controls(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect()
}

/// Text of one rendered line, without styling.
pub fn plain(line: &Line) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

/// Plain text of rendered lines, for printing and snapshot tests.
pub fn to_plain(lines: &[Line]) -> String {
    lines
        .iter()
        .map(|l| strip_controls(plain(l).trim_end()))
        .collect::<Vec<_>>()
        .join("\n")
}
