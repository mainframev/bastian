use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Padding, Paragraph};
use unicode_width::UnicodeWidthStr;

use crate::render::theme::Theme;

/// What the left side of the status bar shows.
pub enum Left<'a> {
    Text(&'a str),
    Error(&'a str),
}

/// Left side of the status bar: prompt keys while typing, else a pending
/// message, else the screen's `idle` hint.
pub fn left<'a>(prompt_open: bool, message: Option<&'a str>, idle: &'a str) -> Left<'a> {
    match (prompt_open, message) {
        (true, _) => Left::Text("enter accept · esc clear"),
        (false, Some(m)) => Left::Error(m),
        (false, None) => Left::Text(idle),
    }
}

/// Style of a list row's text, highlighted when selected.
pub fn row_style(t: &Theme, selected: bool) -> Style {
    if selected {
        Style::default().fg(t.accent).bold()
    } else {
        Style::default().fg(t.text)
    }
}

pub fn draw(frame: &mut Frame, area: Rect, t: &Theme, left: Left, right: &str) {
    let mut spans = vec![Span::styled(
        " bastian ",
        Style::default().fg(t.h1_fg).bg(t.accent).bold(),
    )];
    let (s, fg) = match left {
        Left::Text(s) => (s, t.text),
        Left::Error(s) => (s, t.err),
    };
    spans.push(Span::styled(format!(" {s}"), Style::default().fg(fg)));

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

/// The `Find:` line shown above a list or document while searching.
/// `editing` draws a cursor after the text.
/// `Find: text`, with a block cursor at character `cursor` while editing.
pub fn find_line(t: &Theme, text: &str, cursor: Option<usize>) -> Line<'static> {
    let label = Span::styled("Find: ", Style::default().fg(t.warn).bold());
    let style = Style::default().fg(t.text);
    let Some(at) = cursor else {
        return Line::from(vec![label, Span::styled(text.to_string(), style)]);
    };
    let before: String = text.chars().take(at).collect();
    let under = text.chars().nth(at).map_or(" ".to_string(), String::from);
    let after: String = text.chars().skip(at + 1).collect();
    Line::from(vec![
        label,
        Span::styled(before, style),
        Span::styled(under, Style::default().fg(t.h1_fg).bg(t.accent)),
        Span::styled(after, style),
    ])
}

/// A bordered box centered on screen.
pub fn modal(frame: &mut Frame, t: &Theme, title: &str, lines: Vec<Line>) {
    let area = frame.area();
    let (width, height) = box_size(area, title, &lines);
    let rect = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    boxed(frame, t, title, lines, rect);
}

/// Like `modal`, but against the right edge (above the status bar), so the
/// screen behind stays visible.
pub fn side_panel(frame: &mut Frame, t: &Theme, title: &str, lines: Vec<Line>) {
    let area = frame.area();
    let (width, height) = box_size(area, title, &lines);
    let height = height.min(area.height.saturating_sub(2));
    let rect = Rect::new(
        area.right().saturating_sub(width + 1),
        area.y + 1,
        width,
        height,
    );
    boxed(frame, t, title, lines, rect);
}

/// Border, padding and title around `lines`, fitted to `area`.
fn box_size(area: Rect, title: &str, lines: &[Line]) -> (u16, u16) {
    let inner_w = lines
        .iter()
        .map(Line::width)
        .chain(std::iter::once(title.chars().count()))
        .max()
        .unwrap_or(20);
    let width = (inner_w as u16 + 4).min(area.width);
    let height = (lines.len() as u16 + 2).min(area.height);
    (width, height)
}

fn boxed(frame: &mut Frame, t: &Theme, title: &str, lines: Vec<Line>, rect: Rect) {
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::bordered()
                .title(Span::styled(
                    title.to_string(),
                    Style::default().fg(t.accent),
                ))
                .border_style(Style::default().fg(t.accent))
                .padding(Padding::horizontal(1))
                .style(Style::default().bg(t.surface)),
        ),
        rect,
    );
}

/// Key reference overlay: `(keys, what they do)` rows in a modal.
pub fn help(frame: &mut Frame, t: &Theme, keys: &[(&str, &str)]) {
    let key_w = keys
        .iter()
        .map(|(k, _)| k.chars().count())
        .max()
        .unwrap_or(0);
    let mut lines: Vec<Line> = keys
        .iter()
        .map(|(k, what)| {
            Line::from(vec![
                Span::styled(
                    format!("{k:<key_w$}  "),
                    Style::default().fg(t.accent).bold(),
                ),
                Span::styled(what.to_string(), Style::default().fg(t.text)),
            ])
        })
        .collect();
    lines.push(Line::default());
    lines.push(Line::styled(
        "any key to close",
        Style::default().fg(t.muted),
    ));
    modal(frame, t, " keys ", lines);
}
