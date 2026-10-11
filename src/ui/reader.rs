use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Padding, Paragraph};

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

    // Focus marker in the left margin, next to the focused block.
    if let Some(range) = r
        .focus
        .and_then(|id| r.rendered.code_blocks.iter().find(|c| c.id == id))
        .map(|c| c.lines.clone())
    {
        let top = range.start.max(r.scroll);
        let bottom = range.end.min(end);
        for line in top..bottom {
            let y = body.y + (line - r.scroll) as u16;
            let cell = Rect::new(body.x, y, 1, 1);
            frame.render_widget(Span::styled("▌", Style::default().fg(t.accent)), cell);
        }
    }

    let pct = if r.max_scroll() == 0 {
        100
    } else {
        r.scroll * 100 / r.max_scroll()
    };
    let right = match &r.search {
        _ if r.search.is_none() && r.focus.is_some() => {
            let (i, n) = r.focus_position().unwrap_or((0, 0));
            format!("block {i}/{n}  {pct:>3}%")
        }
        Some(s) if s.matches.is_empty() => format!("no matches  {pct:>3}%"),
        Some(s) => format!("{}/{}  {pct:>3}%", s.current + 1, s.matches.len()),
        None => format!("{} lines  {pct:>3}%", r.rendered.lines.len()),
    };
    let hint;
    let left = match (&r.prompt, &r.message) {
        (Some(p), _) => Left::Prompt(p),
        (None, Some(m)) => Left::Error(m),
        (None, None) if r.focus.is_some() => {
            hint = format!("{}  enter run · R all · x stop · o output", r.name);
            Left::Text(&hint)
        }
        (None, None) => Left::Text(&r.name),
    };
    status::draw(frame, status, &r.theme, left, &right);

    if let Some(ids) = &r.confirm {
        draw_confirm(frame, r, ids);
    }
}

fn draw_confirm(frame: &mut Frame, r: &Reader, ids: &[usize]) {
    let t = &r.theme;
    let what = match ids {
        [id] => r.label(*id),
        _ => format!("{} blocks, in order", ids.len()),
    };
    let cwd = ids
        .first()
        .and_then(|&id| r.code(id))
        .and_then(|c| c.attrs.cwd.clone())
        .map_or_else(
            || r.base_dir.display().to_string(),
            |d| r.base_dir.join(d).display().to_string(),
        );
    let lines = vec![
        Line::from(vec![
            Span::styled("Run ", Style::default().fg(t.text)),
            Span::styled(what, Style::default().fg(t.accent).bold()),
            Span::styled("?", Style::default().fg(t.text)),
        ]),
        Line::styled(format!("in {cwd}"), Style::default().fg(t.muted)),
        Line::default(),
        Line::from(vec![
            Span::styled("y", Style::default().fg(t.ok).bold()),
            Span::styled(" run   ", Style::default().fg(t.subtle)),
            Span::styled("n", Style::default().fg(t.err).bold()),
            Span::styled(" cancel", Style::default().fg(t.subtle)),
        ]),
    ];
    let area = frame.area();
    let width = lines
        .iter()
        .map(Line::width)
        .max()
        .unwrap_or(20)
        .clamp(24, area.width.saturating_sub(4) as usize) as u16
        + 4;
    let height = lines.len() as u16 + 2;
    let rect = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width.min(area.width),
        height.min(area.height),
    );
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::bordered()
                .border_style(Style::default().fg(t.accent))
                .padding(Padding::horizontal(1))
                .style(Style::default().bg(t.surface)),
        ),
        rect,
    );
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
