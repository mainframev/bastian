use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Clear;
use unicode_width::UnicodeWidthStr;

use crate::reader::{LinkPicker, Reader};
use crate::ui::status::{self, Place};
use crate::ui::text::{shorten_left, take_width};

/// The outline tree, docked to the right.
pub(super) fn draw_outline(frame: &mut Frame, r: &mut Reader) {
    let t = r.theme.clone();
    let Some(outline) = &mut r.outline else {
        return;
    };
    let area = frame.area();
    // Border and padding around the widest heading, at most half the screen.
    let title = " outline · enter jump · esc close ";
    let width = (outline.width.max(title.width()) as u16 + 4)
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
        .block(status::panel_block(&t, title))
        .style(Style::default().fg(t.text))
        .highlight_style(status::row_style(&t, true))
        .highlight_symbol("▶ ")
        .node_open_symbol("▾ ")
        .node_closed_symbol("▸ ")
        .node_no_children_symbol("  ");
    frame.render_widget(Clear, rect);
    frame.render_stateful_widget(tree, rect, &mut outline.state);
}

pub(super) fn draw_links(frame: &mut Frame, r: &Reader, picker: &LinkPicker) {
    let t = &r.theme;
    let area = frame.area();
    // Leave room for the border and a margin around the box.
    let rows = area.height.saturating_sub(6).max(1) as usize;
    let max_w = area.width.saturating_sub(10) as usize;
    let lines: Vec<Line> = picker
        .links
        .iter()
        .enumerate()
        .map(|(i, link)| {
            let selected = i == picker.selected;
            let text_style = status::row_style(t, selected);
            let entry = format!("{}  {}", link.text, link.url);
            let shown = take_width(&entry, max_w.saturating_sub(2));
            // Both are starts of `entry`, so the shorter one ends on a char.
            let (text, url) = shown.split_at(link.text.len().min(shown.len()));
            Line::from(vec![
                Span::styled(text.to_string(), text_style),
                Span::styled(url.to_string(), Style::default().fg(t.muted)),
            ])
        })
        .collect();
    let title = format!(
        " links {}/{} · enter open · esc close ",
        picker.selected + 1,
        picker.links.len()
    );
    status::list_popup(
        frame,
        t,
        &title,
        lines,
        picker.selected,
        rows,
        Place::Center,
    );
}

pub(super) fn draw_confirm(frame: &mut Frame, r: &Reader, ids: &[usize]) {
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

/// Ask before handing a non-web link to the system opener. The target is
/// shown whole, wrapped to the screen, so nothing hides past the edge.
pub(super) fn draw_open_confirm(frame: &mut Frame, r: &Reader, target: &str) {
    let t = &r.theme;
    let area = frame.area();
    // Border and padding, at least one column for the text.
    let max_w = (area.width.saturating_sub(4) as usize).max(1);
    // Inside the border; the box can't grow past the screen.
    let avail = area.height.saturating_sub(2) as usize;
    let target_style = Style::default().fg(t.accent).bold();
    let keys = Line::from(vec![
        Span::styled("y", Style::default().fg(t.ok).bold()),
        Span::styled(" open   ", Style::default().fg(t.subtle)),
        Span::styled("n", Style::default().fg(t.err).bold()),
        Span::styled(" cancel", Style::default().fg(t.subtle)),
    ]);
    // Rows left for the target beside the question, a blank and the keys.
    let Some(budget) = avail.checked_sub(3).filter(|&b| b > 0) else {
        // Too short for the full prompt: the target on one row, if there's
        // room, and always the keys.
        let mut lines = Vec::new();
        if avail >= 2 {
            lines.push(Line::styled(elide_middle(target, max_w), target_style));
        }
        lines.push(keys);
        status::modal(frame, t, " open ", lines);
        return;
    };
    let mut lines = vec![Line::styled(
        "Open with the system opener?",
        Style::default().fg(t.text),
    )];
    let rows = wrap_width(target, max_w);
    if rows.len() <= budget {
        lines.extend(rows.into_iter().map(|row| Line::styled(row, target_style)));
    } else if budget < 3 {
        lines.push(Line::styled(elide_middle(target, max_w), target_style));
    } else {
        // Keep the start (scheme, host) and the end (file name) in view.
        let head = (budget - 1).div_ceil(2);
        let tail = budget - 1 - head;
        let row = |s: &str| Line::styled(s.to_string(), target_style);
        lines.extend(rows[..head].iter().map(|s| row(s)));
        lines.push(Line::styled("…", Style::default().fg(t.subtle)));
        lines.extend(rows[rows.len() - tail..].iter().map(|s| row(s)));
    }
    lines.extend([Line::default(), keys]);
    status::modal(frame, t, " open ", lines);
}

/// `s` cut into rows of at most `max` columns.
fn wrap_width(s: &str, max: usize) -> Vec<&str> {
    let mut rows = Vec::new();
    let mut rest = s;
    while !rest.is_empty() {
        let mut chunk = take_width(rest, max);
        if chunk.is_empty() {
            // A character wider than the box: show it anyway.
            let end = rest.chars().next().map_or(rest.len(), char::len_utf8);
            chunk = &rest[..end];
        }
        rows.push(chunk);
        rest = &rest[chunk.len()..];
    }
    rows
}

/// `s` in at most `max` columns, its middle replaced with `…` if it's too long.
fn elide_middle(s: &str, max: usize) -> String {
    if s.width() <= max {
        return s.to_string();
    }
    let head = take_width(s, max.saturating_sub(1) / 2);
    format!("{head}{}", shorten_left(s, max - head.width()))
}
