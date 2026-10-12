use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Padding, Paragraph};
use unicode_width::UnicodeWidthStr;

use crate::prompt::Prompt;
use crate::render::theme::Theme;

/// What the left side of the status bar shows.
#[derive(Clone, Copy)]
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

/// A thin vertical scrollbar in the last column of `area`, drawn only when
/// `content` doesn't fit in `viewport` (both in the same units).
pub fn scrollbar(
    frame: &mut Frame,
    t: &Theme,
    area: Rect,
    content: usize,
    viewport: usize,
    offset: usize,
) {
    if content <= viewport || area.width == 0 {
        return;
    }
    let bar = tui_scrollbar::ScrollBar::vertical(tui_scrollbar::ScrollLengths {
        content_len: content,
        viewport_len: viewport,
    })
    .offset(offset)
    .glyph_set(tui_scrollbar::GlyphSet::unicode())
    .track_style(Style::default().fg(t.surface))
    .thumb_style(Style::default().fg(t.muted));
    let column = Rect {
        x: area.right() - 1,
        width: 1,
        ..area
    };
    frame.render_widget(&bar, column);
}

/// The `Find:` line while searching: the prompt's text with its cursor
/// while editing, else the `query` last searched for.
pub fn find_prompt(t: &Theme, prompt: Option<&Prompt>, query: &str) -> Line<'static> {
    let text = prompt.map_or(query, Prompt::text);
    find_line(t, text, prompt.map(Prompt::cursor))
}

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

/// The `▶ ` in front of the selected row of a popup list, blank otherwise.
pub fn select_marker(t: &Theme, selected: bool) -> Span<'static> {
    Span::styled(
        if selected { "▶ " } else { "  " },
        Style::default().fg(t.accent),
    )
}

/// First of `rows` visible rows of a `len`-row list so `selected` shows:
/// in the middle when `centered`, else as low as it can go.
pub fn window(selected: usize, len: usize, rows: usize, centered: bool) -> usize {
    let above = if centered {
        rows / 2
    } else {
        rows.saturating_sub(1)
    };
    selected.saturating_sub(above).min(len.saturating_sub(rows))
}

/// The accent-bordered, padded block around popups.
pub fn panel_block<'a>(t: &Theme, title: impl Into<Line<'a>>) -> Block<'a> {
    Block::bordered()
        .title(title.into().style(Style::default().fg(t.accent)))
        .border_style(Style::default().fg(t.accent))
        .padding(Padding::horizontal(1))
        .style(Style::default().bg(t.surface))
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
        Paragraph::new(lines).block(panel_block(t, title.to_string())),
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
