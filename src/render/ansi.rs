//! Styled rendered lines as ANSI escape sequences, for print and pager modes.

use std::fmt::Write;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;

/// Lines joined with newlines, each span wrapped in SGR codes for its style.
/// Every styled run ends with a reset so nothing bleeds into the next line.
pub fn to_ansi(lines: &[Line]) -> String {
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        // Merge neighboring spans with the same style into one escape run.
        let mut runs: Vec<(Style, String)> = Vec::new();
        for span in &line.spans {
            let style = line.style.patch(span.style);
            match runs.last_mut() {
                Some((last, text)) if *last == style => text.push_str(&span.content),
                _ => runs.push((style, span.content.to_string())),
            }
        }
        for (style, text) in runs {
            let sgr = sgr(style);
            if sgr.is_empty() {
                out.push_str(&text);
            } else {
                let _ = write!(out, "\x1b[{sgr}m{text}\x1b[0m");
            }
        }
    }
    out
}

/// SGR parameters for a style, e.g. `1;38;2;235;188;186`.
fn sgr(style: Style) -> String {
    let mut codes: Vec<String> = Vec::new();
    let m = style.add_modifier;
    for (flag, code) in [
        (Modifier::BOLD, "1"),
        (Modifier::DIM, "2"),
        (Modifier::ITALIC, "3"),
        (Modifier::UNDERLINED, "4"),
        (Modifier::REVERSED, "7"),
        (Modifier::CROSSED_OUT, "9"),
    ] {
        if m.contains(flag) {
            codes.push(code.into());
        }
    }
    if let Some(fg) = style.fg.and_then(|c| color(c, false)) {
        codes.push(fg);
    }
    if let Some(bg) = style.bg.and_then(|c| color(c, true)) {
        codes.push(bg);
    }
    codes.join(";")
}

fn color(c: Color, bg: bool) -> Option<String> {
    let base = if bg { 40 } else { 30 };
    let ext = if bg { 48 } else { 38 };
    let basic = |n: u8| Some((base + u16::from(n)).to_string());
    let bright = |n: u8| Some((base + 60 + u16::from(n)).to_string());
    match c {
        Color::Reset => None,
        Color::Black => basic(0),
        Color::Red => basic(1),
        Color::Green => basic(2),
        Color::Yellow => basic(3),
        Color::Blue => basic(4),
        Color::Magenta => basic(5),
        Color::Cyan => basic(6),
        Color::Gray => basic(7),
        Color::DarkGray => bright(0),
        Color::LightRed => bright(1),
        Color::LightGreen => bright(2),
        Color::LightYellow => bright(3),
        Color::LightBlue => bright(4),
        Color::LightMagenta => bright(5),
        Color::LightCyan => bright(6),
        Color::White => bright(7),
        Color::Indexed(i) => Some(format!("{ext};5;{i}")),
        Color::Rgb(r, g, b) => Some(format!("{ext};2;{r};{g};{b}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::text::Span;

    #[test]
    fn plain_spans_have_no_escapes() {
        let lines = [Line::from("hello"), Line::from("world")];
        assert_eq!(to_ansi(&lines), "hello\nworld");
    }

    #[test]
    fn styles_become_sgr_and_reset() {
        let style = Style::default()
            .fg(Color::Rgb(1, 2, 3))
            .bg(Color::Indexed(236))
            .bold()
            .italic();
        let lines = [Line::from(vec![Span::styled("x", style), Span::raw("y")])];
        assert_eq!(to_ansi(&lines), "\x1b[1;3;38;2;1;2;3;48;5;236mx\x1b[0my");
    }

    #[test]
    fn same_style_neighbors_share_one_run() {
        let s = Style::default().fg(Color::Red);
        let line = Line::from(vec![Span::styled("a", s), Span::styled(" b", s)]);
        assert_eq!(to_ansi(&[line]), "\x1b[31ma b\x1b[0m");
    }

    #[test]
    fn line_style_applies_under_span_style() {
        let line = Line::from(vec![Span::styled("a", Style::default().fg(Color::Red))])
            .style(Style::default().bg(Color::Blue));
        assert_eq!(to_ansi(&[line]), "\x1b[31;44ma\x1b[0m");
    }
}
