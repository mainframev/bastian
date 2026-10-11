use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthStr;

use crate::render::theme::Theme;

/// What the left side of the status bar shows.
pub enum Left<'a> {
    Text(&'a str),
    Error(&'a str),
}

pub fn draw(frame: &mut Frame, area: Rect, t: &Theme, left: Left, right: &str) {
    let mut spans = vec![Span::styled(
        " bastian ",
        Style::default().fg(t.h1_fg).bg(t.accent).bold(),
    )];
    match left {
        Left::Text(s) => spans.push(Span::styled(format!(" {s}"), Style::default().fg(t.text))),
        Left::Error(s) => spans.push(Span::styled(format!(" {s}"), Style::default().fg(t.err))),
    }

    // One column of padding on each side of the right-hand text.
    let right_w = right.width() as u16 + 2;
    let [l, r] = Layout::horizontal([Constraint::Min(1), Constraint::Length(right_w)]).areas(area);
    let bg = Style::default().bg(t.surface);
    frame.render_widget(Paragraph::new(Line::from(spans)).style(bg), l);
    frame.render_widget(
        Paragraph::new(Span::styled(
            format!(" {right} "),
            Style::default().fg(t.subtle),
        ))
        .style(bg),
        r,
    );
}

/// The `Find:` line shown above a list or document while searching, like glow.
/// `editing` draws a cursor after the text.
pub fn find_line(t: &Theme, text: &str, editing: bool) -> Line<'static> {
    let mut spans = vec![
        Span::styled("Find: ", Style::default().fg(t.warn).bold()),
        Span::styled(text.to_string(), Style::default().fg(t.text)),
    ];
    if editing {
        spans.push(Span::styled(" ", Style::default().bg(t.accent)));
    }
    Line::from(spans)
}
