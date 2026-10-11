use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::status::{self, Left};
use crate::reader::{LinkPicker, Reader};

/// Columns of left margin around the document.
pub const MARGIN: u16 = 2;

pub fn draw(frame: &mut Frame, r: &mut Reader) {
    // While searching, a `Find:` line sits above the document.
    let finding = r.prompt.is_some() || r.search.is_some();
    let [find, body, status] = Layout::vertical([
        Constraint::Length(if finding { 2 } else { 0 }),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    if finding {
        draw_find(frame, r, find);
    }

    // The gutter width depends on the line count, which depends on the
    // width, so lay out again if the digit count changed.
    let mut gutter = gutter_width(r, r.rendered.lines.len());
    let max_width = r.max_width;
    let width = |g: u16| body.width.saturating_sub(MARGIN * 2 + g).min(max_width);
    r.layout(width(gutter), body.height);
    let fitted = gutter_width(r, r.rendered.lines.len());
    if fitted != gutter {
        gutter = fitted;
        r.layout(width(gutter), body.height);
    }

    let content = Rect {
        x: body.x + MARGIN + gutter,
        width: width(gutter),
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

    if gutter > 0 {
        let digits = usize::from(gutter - 1);
        let numbers: Vec<Line> = (r.scroll..end)
            .map(|i| Line::styled(format!("{:>digits$} ", i + 1), Style::default().fg(t.muted)))
            .collect();
        let area = Rect {
            x: body.x + MARGIN,
            width: gutter,
            ..body
        };
        frame.render_widget(Paragraph::new(numbers), area);
    }

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
        _ => format!("{} lines  {pct:>3}%", r.rendered.lines.len()),
    };
    let hint;
    let left = match (&r.prompt, &r.message) {
        (Some(_), _) => Left::Text("enter accept · esc clear"),
        (None, Some(m)) => Left::Error(m),
        (None, None) if r.focus.is_some() => {
            hint = format!("{}  enter run · R all · x stop · y copy · ? more", r.name);
            Left::Text(&hint)
        }
        (None, None) => {
            hint = format!("{}  e edit · ? more", r.name);
            Left::Text(&hint)
        }
    };
    status::draw(frame, status, &r.theme, left, &right);

    if let Some(ids) = &r.confirm {
        draw_confirm(frame, r, ids);
    } else if let Some(picker) = &r.link_picker {
        draw_links(frame, r, picker);
    } else if r.help {
        draw_help(frame, r);
    }
}

/// Columns for the line-number gutter: the digits of `lines` plus a space,
/// or nothing when line numbers are off.
pub fn gutter_width(r: &Reader, lines: usize) -> u16 {
    if !r.line_numbers {
        return 0;
    }
    lines.max(1).to_string().len() as u16 + 1
}

const HELP: &[(&str, &str)] = &[
    ("j/k  ↓/↑", "scroll"),
    ("ctrl-d/u", "half page"),
    ("space/b", "page down/up"),
    ("g/G", "top/bottom"),
    ("/  n/N", "search, next/previous match"),
    ("Tab/S-Tab", "focus next/previous code block"),
    ("Enter, r", "run focused block"),
    ("R", "run all blocks from the focused one"),
    ("x", "stop running blocks"),
    ("o", "show/hide output"),
    ("y", "copy focused block"),
    ("L", "list links"),
    ("e", "edit in $VISUAL/$EDITOR"),
    ("#", "toggle line numbers"),
    ("Esc, h", "back"),
    ("q", "quit"),
];

fn draw_help(frame: &mut Frame, r: &Reader) {
    status::help(frame, &r.theme, HELP);
}

fn draw_links(frame: &mut Frame, r: &Reader, picker: &LinkPicker) {
    let t = &r.theme;
    let area = frame.area();
    // Leave room for the border and a margin around the box.
    let rows = area.height.saturating_sub(6).max(1) as usize;
    let start = picker.selected.saturating_sub(rows.saturating_sub(1));
    let max_w = area.width.saturating_sub(10) as usize;
    let lines: Vec<Line> = picker
        .links
        .iter()
        .enumerate()
        .skip(start)
        .take(rows)
        .map(|(i, link)| {
            let selected = i == picker.selected;
            let marker = if selected { "▶ " } else { "  " };
            let text_style = if selected {
                Style::default().fg(t.accent).bold()
            } else {
                Style::default().fg(t.text)
            };
            let entry = format!("{}  {}", link.text, link.url);
            let shown: String = entry.chars().take(max_w.saturating_sub(2)).collect();
            let split = link.text.chars().count().min(shown.chars().count());
            let (text, url): (String, String) = (
                shown.chars().take(split).collect(),
                shown.chars().skip(split).collect(),
            );
            Line::from(vec![
                Span::styled(marker, Style::default().fg(t.accent)),
                Span::styled(text, text_style),
                Span::styled(url, Style::default().fg(t.muted)),
            ])
        })
        .collect();
    let title = format!(
        " links {}/{} · enter open · esc close ",
        picker.selected + 1,
        picker.links.len()
    );
    status::modal(frame, t, &title, lines);
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
    status::modal(frame, t, " run ", lines);
}

/// `Find:` line above the document, with the match count on the right.
fn draw_find(frame: &mut Frame, r: &Reader, area: Rect) {
    let t = &r.theme;
    let area = Rect {
        x: area.x + MARGIN,
        width: area.width.saturating_sub(MARGIN * 2),
        height: 1,
        ..area
    };
    let text = match (&r.prompt, &r.search) {
        (Some(p), _) => p.text.as_str(),
        (None, Some(s)) => s.query.as_str(),
        (None, None) => "",
    };
    let count = match &r.search {
        Some(s) if s.matches.is_empty() => "no matches".to_string(),
        Some(s) => format!("{}/{}", s.current + 1, s.matches.len()),
        None => String::new(),
    };
    let [left, right] = Layout::horizontal([
        Constraint::Min(1),
        Constraint::Length(count.chars().count() as u16),
    ])
    .areas(area);
    frame.render_widget(status::find_line(t, text, r.prompt.is_some()), left);
    frame.render_widget(Span::styled(count, Style::default().fg(t.muted)), right);
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
