use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Padding, Paragraph};

pub use super::MARGIN;
use super::status;
use crate::reader::{LinkPicker, Reader};

pub fn draw(frame: &mut Frame, r: &mut Reader) {
    // A blank row above the document (the `Find:` line takes it while
    // searching, with its own blank row below) and one above the status
    // bar, so the text never touches either edge.
    let finding = r.prompt.is_some() || r.search.is_some();
    let [find, body, _, status] = Layout::vertical([
        Constraint::Length(if finding { 2 } else { 1 }),
        Constraint::Min(1),
        Constraint::Length(1),
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
    // A copy (cheap: colors and an `Arc`), so `r` can be borrowed mutably
    // for the images below.
    let theme = r.theme.clone();
    let t = &theme;
    let visible: Vec<Line> = (r.scroll..end)
        .map(|i| {
            let line = &r.rendered.lines[i];
            let Some(s) = &r.search else {
                return line.clone();
            };
            // Matches are in line order: take just this line's.
            let first = s.matches.partition_point(|m| m.line < i);
            let last = s.matches.partition_point(|m| m.line <= i);
            let ranges: Vec<_> = s.matches[first..last]
                .iter()
                .enumerate()
                .map(|(k, m)| {
                    let n = first + k;
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
    if r.hyperlinks {
        draw_hyperlinks(frame, r, content, end);
    }
    draw_images(frame, r, content, end);
    if r.scrollbar {
        status::scrollbar(
            frame,
            t,
            body,
            r.rendered.lines.len(),
            usize::from(body.height),
            r.scroll,
        );
    }

    if gutter > 0 {
        let digits = usize::from(gutter - GUTTER_GAP);
        // Source line numbers when the document maps them (code files),
        // otherwise rendered line numbers. The gap to the text is the rest
        // of the gutter.
        let label = |i: usize| match r.rendered.src_line.get(i) {
            Some(Some(n)) => format!("{:>digits$}", n + 1),
            Some(None) => String::new(),
            None => format!("{:>digits$}", i + 1),
        };
        let numbers: Vec<Line> = (r.scroll..end)
            .map(|i| Line::styled(label(i), Style::default().fg(t.muted)))
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
    let right = if r.search.is_none() && r.focus.is_some() {
        let (i, n) = r.focus_position().unwrap_or((0, 0));
        format!("block {i}/{n}  {pct:>3}%")
    } else {
        format!("{} lines  {pct:>3}%", r.rendered.lines.len())
    };
    let hint = if r.focus.is_some() {
        format!("{}  enter run · R all · x stop · y copy · ? more", r.name)
    } else {
        format!("{}  e edit · ? more", r.name)
    };
    let left = status::left(r.prompt.is_some(), r.message.as_deref(), &hint);
    status::draw(frame, status, &r.theme, left, &right);

    if let Some(ids) = &r.confirm {
        draw_confirm(frame, r, ids);
    } else if let Some(picker) = &r.link_picker {
        draw_links(frame, r, picker);
    } else if r.outline.is_some() {
        draw_outline(frame, r);
    } else if r.help {
        status::help(frame, &r.theme, HELP);
    }
}

/// Blank columns between the line numbers and the text.
pub const GUTTER_GAP: u16 = 3;

/// Columns for the line-number gutter: the digits of `lines` plus the gap
/// to the text, or nothing when line numbers are off.
pub fn gutter_width(r: &Reader, lines: usize) -> u16 {
    if !r.line_numbers {
        return 0;
    }
    lines.max(1).to_string().len() as u16 + GUTTER_GAP
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
    ("c", "copy whole document"),
    ("L", "list links"),
    ("e", "edit at this line in $VISUAL/$EDITOR"),
    ("ctrl-z", "suspend (resume with fg)"),
    ("#", "toggle line numbers"),
    ("O", "outline: jump to a heading"),
    ("i", "show/hide images"),
    ("T", "pick a theme"),
    ("Esc, h", "back"),
    ("q", "quit"),
];

/// The outline tree, docked to the right.
fn draw_outline(frame: &mut Frame, r: &mut Reader) {
    let t = r.theme.clone();
    let Some(outline) = &mut r.outline else {
        return;
    };
    let area = frame.area();
    // Border and padding around the widest heading, at most half the screen.
    let title = " outline · enter jump · esc close ";
    let width = (outline.width.max(title.chars().count()) as u16 + 4)
        .min(area.width / 2)
        .max(area.width.min(24));
    let rect = Rect::new(
        area.right().saturating_sub(width + 1),
        area.y + 1,
        width,
        area.height.saturating_sub(3),
    );
    let tree = tui_tree_widget::Tree::new(&outline.items)
        .expect("heading ids are unique")
        .block(
            Block::bordered()
                .title(Span::styled(title, Style::default().fg(t.accent)))
                .border_style(Style::default().fg(t.accent))
                .padding(Padding::horizontal(1))
                .style(Style::default().bg(t.surface)),
        )
        .style(Style::default().fg(t.text))
        .highlight_style(status::row_style(&t, true))
        .highlight_symbol("▶ ")
        .node_open_symbol("▾ ")
        .node_closed_symbol("▸ ")
        .node_no_children_symbol("  ");
    frame.render_widget(Clear, rect);
    frame.render_stateful_widget(tree, rect, &mut outline.state);
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
            let text_style = status::row_style(t, selected);
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
        .map_or_else(|| r.base_dir.clone(), |&id| r.cwd(id))
        .display()
        .to_string();
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

/// Pictures over the rows reserved for them. A picture partly scrolled
/// off shows its visible rows: the slot-sized picture is cropped from the
/// top or the bottom.
fn draw_images(frame: &mut Frame, r: &mut Reader, content: Rect, end: usize) {
    let scroll = r.scroll;
    let slots: Vec<_> = r
        .rendered
        .images
        .iter()
        .filter(|s| s.line < end && s.line + usize::from(s.rows) > scroll)
        .cloned()
        .collect();
    for slot in slots {
        let Some(protocol) = r.images.protocol(&slot.url, slot.cols, slot.rows) else {
            continue;
        };
        let top = slot.line.max(scroll);
        let bottom = (slot.line + usize::from(slot.rows)).min(end);
        let x = content.x + slot.col as u16;
        let area = Rect::new(
            x,
            content.y + (top - scroll) as u16,
            slot.cols.min(content.right().saturating_sub(x)),
            (bottom - top) as u16,
        );
        let crop = ratatui_image::CropOptions {
            clip_top: slot.line < scroll,
            clip_left: false,
        };
        frame.render_stateful_widget(
            ratatui_image::StatefulImage::default().resize(ratatui_image::Resize::Crop(Some(crop))),
            area,
            protocol,
        );
    }
}

/// Make visible links clickable: an OSC 8 hyperlink over each link's text.
/// Lines with search matches keep their highlighting instead.
fn draw_hyperlinks(frame: &mut Frame, r: &Reader, content: Rect, end: usize) {
    let first = r.rendered.links.partition_point(|l| l.line < r.scroll);
    for link in r.rendered.links[first..]
        .iter()
        .take_while(|l| l.line < end)
    {
        if r.search
            .as_ref()
            .is_some_and(|s| s.matches.iter().any(|m| m.line == link.line))
        {
            continue;
        }
        let Some(target) = r.hyperlink_target(&link.url) else {
            continue;
        };
        let line = &r.rendered.lines[link.line];
        let (mut label, mut style, mut col) = (String::new(), None, 0);
        for span in &line.spans {
            let w = span.width();
            if col + w > link.col && col < link.col + link.width {
                label.push_str(&span.content);
                style.get_or_insert(span.style);
            }
            col += w;
        }
        let x = content.x + link.col as u16;
        let width = (link.width as u16).min(content.right().saturating_sub(x));
        let y = content.y + (link.line - r.scroll) as u16;
        let area = Rect::new(x, y, width, 1);
        frame.render_widget(
            hyperrat::Link::new(label, target).style(style.unwrap_or_default()),
            area,
        );
        // The first cell holds the whole escape sequence plus the label.
        // Tell the diff how many columns it really covers, or it measures the
        // escape sequence and skips the rest of the line.
        if let Some(width) = std::num::NonZeroU16::new(width)
            && let Some(cell) = frame.buffer_mut().cell_mut((x, y))
        {
            cell.set_diff_option(ratatui::buffer::CellDiffOption::ForcedWidth(width));
        }
    }
}

/// `Find:` line above the document, with the match count on the right.
fn draw_find(frame: &mut Frame, r: &Reader, area: Rect) {
    let t = &r.theme;
    let area = Rect {
        height: 1,
        ..super::inset(area)
    };
    let text = match (&r.prompt, &r.search) {
        (Some(p), _) => p.text(),
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
    frame.render_widget(
        status::find_line(
            t,
            text,
            r.prompt.as_ref().map(super::super::prompt::Prompt::cursor),
        ),
        left,
    );
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
