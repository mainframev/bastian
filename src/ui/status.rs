use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthStr;

use crate::prompt::Prompt;
use crate::render::theme::Theme;

/// What the left side of the status bar shows.
pub enum Left<'a> {
    Text(&'a str),
    Prompt(&'a Prompt),
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
        Left::Prompt(p) => {
            spans.push(Span::styled(
                format!(" {}", p.label),
                Style::default().fg(t.accent),
            ));
            spans.push(Span::styled(p.text.clone(), Style::default().fg(t.text)));
            spans.push(Span::styled("▏", Style::default().fg(t.accent)));
        }
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
