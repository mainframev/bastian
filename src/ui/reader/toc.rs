use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph};

use crate::reader::Reader;
use crate::text::take_width;
use crate::toc;
use crate::ui::status;

/// Columns the numbers take: two digits and a space.
const NUMBER_W: usize = 3;

/// The table of contents in `area`, left of the document: a header with
/// the `<` and `>` resize markers, a rule, then the headings with the one
/// being read highlighted and kept in view.
pub(super) fn draw_toc(frame: &mut Frame, r: &Reader, area: Rect) {
    let t = r.theme();
    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(t.muted))
        .padding(Padding::left(1));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height < 3 || inner.width == 0 {
        return;
    }

    let (narrower, wider) = toc::markers(area.width);
    let title_w = usize::from(narrower.saturating_sub(2));
    let header = Line::styled(
        take_width("Table of Contents", title_w).to_string(),
        Style::default().fg(t.accent).bold(),
    );
    frame.render_widget(header, Rect { height: 1, ..inner });
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

    let list = Rect {
        y: inner.y + 2,
        height: inner.height - 2,
        ..inner
    };
    let entries = r.toc_entries();
    let current = toc::current(&entries, r.scroll());
    let numbered = entries
        .iter()
        .find(|e| e.number.is_some())
        .map_or(0, |e| e.depth);
    let rows = usize::from(list.height);
    // Keep the current entry about mid-list.
    let first = current
        .map_or(0, |c| c.saturating_sub(rows / 2))
        .min(entries.len().saturating_sub(rows));
    let width = usize::from(list.width);
    let lines: Vec<Line> = entries
        .iter()
        .enumerate()
        .skip(first)
        .take(rows)
        .map(|(i, e)| {
            let selected = current == Some(i);
            let style = status::row_style(t, selected);
            let lead = match e.number {
                Some(n) => Span::styled(
                    format!("{n:02} "),
                    if selected {
                        style
                    } else {
                        Style::default().fg(t.muted)
                    },
                ),
                // Headings above the numbered level (a title) start flush
                // left; deeper ones indent under the text of their section.
                None if e.depth < numbered => Span::raw(""),
                None => Span::raw(" ".repeat(NUMBER_W + 2 * (e.depth - numbered))),
            };
            let room = width.saturating_sub(lead.width());
            Line::from(vec![
                lead,
                Span::styled(take_width(&e.text, room).to_string(), style),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), list);
}
