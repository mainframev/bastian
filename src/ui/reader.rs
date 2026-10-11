use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::status::{self, Left};
use crate::reader::Reader;

/// Columns of left margin around the document.
pub const MARGIN: u16 = 2;

pub fn draw(frame: &mut Frame, r: &mut Reader) {
    let [body, status] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(frame.area());

    let width = body.width.saturating_sub(MARGIN * 2).min(r.max_width);
    r.layout(width, body.height);

    let content = Rect {
        x: body.x + MARGIN,
        width,
        ..body
    };
    let end = (r.scroll + content.height as usize).min(r.rendered.lines.len());
    let t = &r.theme;
    let visible: Vec<Line> = (r.scroll..end)
        .map(|i| {
            let line = &r.rendered.lines[i];
            let Some(s) = &r.search else {
                return line.clone();
            };
            let ranges: Vec<_> = s
                .matches
                .iter()
                .enumerate()
                .filter(|(_, m)| m.line == i)
                .map(|(n, m)| {
                    let style = if n == s.current {
                        Style::default().fg(t.h1_fg).bg(t.current_bg)
                    } else {
                        Style::default().bg(t.match_bg)
                    };
                    (m.start, m.end, style)
                })
                .collect();
            highlight(line, &ranges)
        })
        .collect();
    frame.render_widget(Paragraph::new(visible), content);

    let pct = if r.max_scroll() == 0 {
        100
    } else {
        r.scroll * 100 / r.max_scroll()
    };
    let right = match &r.search {
        Some(s) if s.matches.is_empty() => format!("no matches  {pct:>3}%"),
        Some(s) => format!("{}/{}  {pct:>3}%", s.current + 1, s.matches.len()),
        None => format!("{} lines  {pct:>3}%", r.rendered.lines.len()),
    };
    let left = match &r.prompt {
        Some(p) => Left::Prompt(p),
        None => Left::Text(&r.name),
    };
    status::draw(frame, status, &r.theme, left, &right);
}

/// Restyle char ranges of `line`, splitting spans where needed.
fn highlight(line: &Line<'static>, ranges: &[(usize, usize, Style)]) -> Line<'static> {
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
