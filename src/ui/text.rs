//! Width-aware helpers for laying out styled text in terminal cells.

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Keep the last `max` columns of `s`, marking the cut with `…`.
pub fn shorten_left(s: &str, max: usize) -> String {
    if s.width() <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut keep = Vec::new();
    let mut w = 0;
    for c in s.chars().rev() {
        let cw = c.width().unwrap_or(0);
        if w + cw + 1 > max {
            break;
        }
        keep.push(c);
        w += cw;
    }
    format!("…{}", keep.iter().rev().collect::<String>())
}

/// Keep the first `max` columns of `s`, marking the cut with `…`.
pub fn shorten_right(s: &str, max: usize) -> String {
    if s.width() <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    format!("{}…", take_width(s, max - 1))
}

/// The longest start of `s` that fits in `max` columns.
pub fn take_width(s: &str, max: usize) -> &str {
    let mut w = 0;
    for (i, c) in s.char_indices() {
        w += c.width().unwrap_or(0);
        if w > max {
            return &s[..i];
        }
    }
    s
}

/// Restyle char ranges of `line`, splitting spans where needed.
pub fn highlight(line: &Line<'static>, ranges: &[(usize, usize, Style)]) -> Line<'static> {
    if ranges.is_empty() {
        return line.clone();
    }
    let style_at = |pos: usize| {
        ranges
            .iter()
            .find(|(s, e, _)| (*s..*e).contains(&pos))
            .map(|r| r.2)
    };
    let mut spans = Vec::new();
    let mut pos = 0;
    for span in &line.spans {
        let mut buf = String::new();
        let mut cur: Option<Style> = None;
        for c in span.content.chars() {
            let hl = style_at(pos);
            if hl != cur && !buf.is_empty() {
                let style = cur.map_or(span.style, |h| span.style.patch(h));
                spans.push(Span::styled(std::mem::take(&mut buf), style));
            }
            cur = hl;
            buf.push(c);
            pos += 1;
        }
        if !buf.is_empty() {
            let style = cur.map_or(span.style, |h| span.style.patch(h));
            spans.push(Span::styled(buf, style));
        }
    }
    Line::from(spans).style(line.style)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortens_paths_from_the_left() {
        assert_eq!(shorten_left("~/code/app", 20), "~/code/app");
        assert_eq!(shorten_left("~/code/app", 6), "…e/app");
        assert_eq!(shorten_left("~/code/app", 0), "");
        // Wide characters count by the columns they take.
        assert_eq!(shorten_left("日本語/説明書.md", 10), "…説明書.md");
        assert!(shorten_left("日本語/説明書.md", 10).width() <= 10);
    }

    #[test]
    fn shortens_titles_from_the_right() {
        assert_eq!(shorten_right("Guide", 10), "Guide");
        assert_eq!(shorten_right("A long title", 6), "A lon…");
        assert_eq!(shorten_right("Guide", 0), "");
    }

    #[test]
    fn takes_whole_characters_within_the_width() {
        assert_eq!(take_width("abc", 2), "ab");
        assert_eq!(take_width("日本語", 5), "日本");
        assert_eq!(take_width("日本語", 1), "");
        assert_eq!(take_width("ab", 9), "ab");
    }

    #[test]
    fn highlight_splits_across_spans() {
        let line = Line::from(vec![Span::raw("ab"), Span::raw("cd")]);
        let hl = Style::default().bold();
        let out = highlight(&line, &[(1, 3, hl)]);
        let parts: Vec<(&str, bool)> = out
            .spans
            .iter()
            .map(|s| (s.content.as_ref(), s.style == hl))
            .collect();
        assert_eq!(
            parts,
            [("a", false), ("b", true), ("c", true), ("d", false)]
        );
    }
}
