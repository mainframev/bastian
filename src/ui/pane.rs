//! The frame of a pane docked beside the document: the table of contents
//! on the left, the theme picker on the right.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Padding};
use unicode_width::UnicodeWidthStr;

use crate::pane;
use crate::render::theme::Theme;
use crate::text::take_width;

/// A pane's frame in `area`, left of the document: a border on its right, a
/// header with `title`, `info` at its right when both fit, and the `<`
/// and `>` resize markers, then a rule. Returns the area under the rule
/// for the pane's rows, or `None` when there's no room for any.
pub fn draw(frame: &mut Frame, t: &Theme, area: Rect, title: &str, info: &str) -> Option<Rect> {
    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(t.muted))
        .padding(Padding::left(1));
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    if inner.height < 3 || inner.width == 0 {
        return None;
    }

    let (narrower, wider) = pane::markers(area.width);
    // Up to a space before the `<`.
    let room = usize::from(narrower.saturating_sub(inner.x - area.x + 1));
    let title = take_width(title, room);
    let header = Rect {
        width: room as u16,
        height: 1,
        ..inner
    };
    frame.render_widget(
        Line::styled(title.to_string(), Style::default().fg(t.accent).bold()),
        header,
    );
    if title.width() + 2 + info.width() <= room {
        frame.render_widget(
            Line::styled(info.to_string(), Style::default().fg(t.muted)).right_aligned(),
            header,
        );
    }
    for (col, marker) in [(narrower, "<"), (wider, ">")] {
        let cell = Rect::new(area.x + col, area.y, 1, 1);
        frame.render_widget(Span::styled(marker, Style::default().fg(t.muted)), cell);
    }
    let rule = "─".repeat(usize::from(inner.width));
    frame.render_widget(
        Span::styled(rule, Style::default().fg(t.muted)),
        Rect {
            y: inner.y + 1,
            height: 1,
            ..inner
        },
    );
    Some(Rect {
        y: inner.y + 2,
        height: inner.height - 2,
        ..inner
    })
}

/// The first of `len` rows to show when `rows` fit, keeping `selected`
/// about mid-list.
pub fn first_row(selected: Option<usize>, len: usize, rows: usize) -> usize {
    selected
        .map_or(0, |s| s.saturating_sub(rows / 2))
        .min(len.saturating_sub(rows))
}
