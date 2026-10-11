use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::Reader;

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
    let visible: Vec<Line> = r.rendered.lines[r.scroll..end].to_vec();
    frame.render_widget(Paragraph::new(visible), content);

    draw_status(frame, r, status);
}

fn draw_status(frame: &mut Frame, r: &Reader, area: Rect) {
    let t = &r.theme;
    let total = r.rendered.lines.len();
    let pct = if r.max_scroll() == 0 {
        100
    } else {
        r.scroll * 100 / r.max_scroll()
    };
    let left = vec![
        Span::styled(
            " bastian ",
            Style::default().fg(t.h1_fg).bg(t.accent).bold(),
        ),
        Span::styled(format!(" {} ", r.name), Style::default().fg(t.text)),
    ];
    let right = format!(" {total} lines  {pct:>3}% ");
    let [l, rgt] = Layout::horizontal([Constraint::Min(1), Constraint::Length(right.len() as u16)])
        .areas(area);
    let bg = Style::default().bg(t.surface);
    frame.render_widget(Paragraph::new(Line::from(left)).style(bg), l);
    frame.render_widget(
        Paragraph::new(Span::styled(right, Style::default().fg(t.subtle))).style(bg),
        rgt,
    );
}
