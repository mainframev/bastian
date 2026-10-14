//! Restyling spans of laid-out text.

use ratatui::style::Style;
use ratatui::text::{Line, Span};

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
