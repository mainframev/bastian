use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Clear;
use unicode_width::UnicodeWidthStr;

use crate::reader::{LinkPicker, Reader};
use crate::ui::status;
use crate::ui::text::take_width;

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
    let start = status::window(picker.selected, picker.links.len(), rows, false);
    let max_w = area.width.saturating_sub(10) as usize;
    let lines: Vec<Line> = picker
        .links
        .iter()
        .enumerate()
        .skip(start)
        .take(rows)
        .map(|(i, link)| {
            let selected = i == picker.selected;
            let text_style = status::row_style(t, selected);
            let entry = format!("{}  {}", link.text, link.url);
            let shown = take_width(&entry, max_w.saturating_sub(2));
            // Both are starts of `entry`, so the shorter one ends on a char.
            let (text, url) = shown.split_at(link.text.len().min(shown.len()));
            Line::from(vec![
                status::select_marker(t, selected),
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
    status::modal(frame, t, &title, lines);
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
