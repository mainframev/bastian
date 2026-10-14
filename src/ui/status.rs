use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, HighlightSpacing, List, ListState, Padding, Paragraph};
use unicode_width::UnicodeWidthStr;

use crate::prompt::Prompt;
use crate::render::theme::Theme;

/// What the left side of the status bar shows.
#[derive(Clone, Copy)]
pub enum Left<'a> {
    Text(&'a str),
    Error(&'a str),
    /// Key hints, `(key, what it does)`, the last one kept however
    /// narrow the bar gets.
    Hints(&'a [(&'a str, &'a str)]),
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
    const NAME: &str = " bastian ";
    // One column of padding on each side of the right-hand text.
    let mut right = right;
    let mut right_w = right.width() as u16 + 2;
    let mut name = NAME;
    // Hints keep their last one, giving up the right-hand text and then
    // the name when the bar is too narrow for it.
    if let Left::Hints(&[.., (key, what)]) = left {
        let need = 1 + key.width() + 1 + what.width();
        let room = |name: &str, right_w: u16| {
            usize::from(area.width.saturating_sub(right_w)).saturating_sub(name.width())
        };
        if room(name, right_w) < need {
            (right, right_w) = ("", 0);
        }
        if room(name, right_w) < need {
            name = "";
        }
    }
    let mut spans = vec![Span::styled(
        name,
        Style::default().fg(t.h1_fg).bg(t.accent).bold(),
    )];
    match left {
        Left::Text(s) => spans.push(Span::styled(format!(" {s}"), Style::default().fg(t.text))),
        Left::Error(s) => spans.push(Span::styled(format!(" {s}"), Style::default().fg(t.err))),
        Left::Hints(hints) => {
            let room = usize::from(area.width.saturating_sub(right_w)).saturating_sub(name.width());
            spans.extend(hint_spans(t, hints, room));
        }
    }

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

/// `key what · key what` in `width` columns: keys in the accent, what
/// they do muted. Hints before the last are dropped from the end until
/// the rest fit.
fn hint_spans(t: &Theme, hints: &[(&str, &str)], width: usize) -> Vec<Span<'static>> {
    const SEP: &str = " · ";
    let Some((last, rest)) = hints.split_last() else {
        return Vec::new();
    };
    let w = |(k, v): &(&str, &str)| k.width() + 1 + v.width();
    // The leading space, then the last hint.
    let mut used = 1 + w(last);
    let mut shown: Vec<&(&str, &str)> = rest
        .iter()
        .take_while(|h| {
            used += w(h) + SEP.width();
            used <= width
        })
        .collect();
    shown.push(last);
    let mut spans = vec![Span::raw(" ")];
    for (i, (key, what)) in shown.into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(SEP, Style::default().fg(t.subtle)));
        }
        spans.push(Span::styled(
            (*key).to_string(),
            Style::default().fg(t.accent).bold(),
        ));
        spans.push(Span::styled(
            format!(" {what}"),
            Style::default().fg(t.muted),
        ));
    }
    spans
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
    prompt_line(t, "Find: ", text, cursor)
}

/// `label` then `text`, with a block cursor at character `cursor` while
/// editing.
pub fn prompt_line(t: &Theme, label: &str, text: &str, cursor: Option<usize>) -> Line<'static> {
    let label = Span::styled(label.to_string(), Style::default().fg(t.warn).bold());
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

/// The accent-bordered, padded block around popups.
pub fn panel_block<'a>(t: &Theme, title: impl Into<Line<'a>>) -> Block<'a> {
    Block::bordered()
        .title(title.into().style(Style::default().fg(t.accent)))
        .border_style(Style::default().fg(t.accent))
        .padding(Padding::horizontal(1))
        .style(Style::default().bg(t.surface))
}

/// A bordered box centered on `over`.
pub fn modal(frame: &mut Frame, t: &Theme, over: Rect, title: &str, lines: Vec<Line>) {
    let rect = popup_rect(frame.area(), over, title, &lines, 0, lines.len());
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines).block(panel_block(t, title.to_string())),
        rect,
    );
}

/// A centered popup list of `lines` showing at most `rows` of them, with
/// `selected` marked `▶ ` and kept in view.
pub fn list_popup(
    frame: &mut Frame,
    t: &Theme,
    over: Rect,
    title: &str,
    lines: Vec<Line>,
    selected: usize,
    rows: usize,
) {
    let marker = Span::styled("▶ ", Style::default().fg(t.accent));
    let rect = popup_rect(frame.area(), over, title, &lines, marker.width(), rows);
    let list = List::new(lines)
        .block(panel_block(t, title.to_string()))
        .highlight_symbol(marker)
        .highlight_spacing(HighlightSpacing::Always);
    let mut state = ListState::default().with_selected(Some(selected));
    frame.render_widget(Clear, rect);
    frame.render_stateful_widget(list, rect, &mut state);
}

/// The box for at most `rows` of `lines`, each `extra` columns wider:
/// border, padding and title, fitted to `area` and centered in it.
fn popup_rect(
    area: Rect,
    over: Rect,
    title: &str,
    lines: &[Line],
    extra: usize,
    rows: usize,
) -> Rect {
    let inner_w = lines
        .iter()
        .map(|l| l.width() + extra)
        .chain(std::iter::once(title.width()))
        .max()
        .unwrap_or(20);
    let width = (inner_w as u16 + 4).min(area.width);
    let height = (lines.len().min(rows) as u16 + 2).min(area.height);
    // Half the spare space on each side, moved back on screen when the
    // box is bigger than `over`.
    let mid = |start: u16, len: u16, size: u16, lo: u16, hi: u16| {
        (start + len.saturating_sub(size) / 2)
            .saturating_sub(size.saturating_sub(len) / 2)
            .clamp(lo, hi.saturating_sub(size).max(lo))
    };
    Rect::new(
        mid(over.x, over.width, width, area.x, area.right()),
        mid(over.y, over.height, height, area.y, area.bottom()),
        width,
        height,
    )
}

/// A titled group of `(keys, what they do)` rows in the key help.
pub type HelpSection = (&'static str, &'static [(&'static str, &'static str)]);

/// Key reference overlay: the version, then `sections` in two columns
/// when they fit across the screen, else one. Rows that don't fit scroll,
/// from `scroll`, which is kept in range.
pub fn help(
    frame: &mut Frame,
    t: &Theme,
    over: Rect,
    sections: &[HelpSection],
    scroll: &mut usize,
) {
    let key_w = sections
        .iter()
        .flat_map(|(_, keys)| keys.iter())
        .map(|(k, _)| k.width())
        .max()
        .unwrap_or(0);
    let blocks: Vec<Vec<Line>> = sections
        .iter()
        .map(|(title, keys)| {
            let mut lines = vec![Line::styled(
                title.to_string(),
                Style::default().fg(t.heading).bold(),
            )];
            lines.extend(keys.iter().map(|(k, what)| {
                Line::from(vec![
                    Span::styled(
                        format!("{k:<key_w$}  "),
                        Style::default().fg(t.accent).bold(),
                    ),
                    Span::styled(what.to_string(), Style::default().fg(t.text)),
                ])
            }));
            lines
        })
        .collect();
    let col_w = blocks.iter().flatten().map(Line::width).max().unwrap_or(0);
    let area = frame.area();
    // Border and padding take 4 columns, the gap between columns 3.
    let body = if usize::from(area.width) >= col_w * 2 + 3 + 4 {
        let (left, right) = blocks.split_at(balanced_split(&blocks));
        let (left, right) = (stack(left), stack(right));
        (0..left.len().max(right.len()))
            .map(|i| {
                let mut line = left.get(i).cloned().unwrap_or_default();
                if let Some(r) = right.get(i) {
                    let pad = col_w + 3 - line.width();
                    line.spans.push(Span::raw(" ".repeat(pad)));
                    line.spans.extend(r.spans.iter().cloned());
                }
                line
            })
            .collect()
    } else {
        stack(&blocks)
    };
    let mut lines = vec![
        Line::styled(
            format!("bastian {}", env!("CARGO_PKG_VERSION")),
            Style::default().fg(t.muted),
        ),
        Line::default(),
    ];
    lines.extend(body);

    let rows = usize::from(area.height.saturating_sub(2));
    let overflow = lines.len().saturating_sub(rows);
    *scroll = (*scroll).min(overflow);
    let hint = if overflow > 0 {
        " j/k scroll · any other key closes "
    } else {
        " any key to close "
    };
    let rect = popup_rect(area, over, " Help ", &lines, 0, rows);
    let block = panel_block(t, " Help ")
        .title_bottom(Line::styled(hint, Style::default().fg(t.muted)).right_aligned());
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .scroll((*scroll as u16, 0)),
        rect,
    );
}

/// Sections one under another, a blank line between them.
fn stack(blocks: &[Vec<Line<'static>>]) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (i, block) in blocks.iter().enumerate() {
        if i > 0 {
            lines.push(Line::default());
        }
        lines.extend(block.iter().cloned());
    }
    lines
}

/// Where to split `blocks`, in order, into two columns of about the same
/// height.
fn balanced_split(blocks: &[Vec<Line>]) -> usize {
    let height = |bs: &[Vec<Line>]| bs.iter().map(|b| b.len() + 1).sum::<usize>();
    (1..blocks.len())
        .min_by_key(|&k| height(&blocks[..k]).max(height(&blocks[k..])))
        .unwrap_or(blocks.len())
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;

    fn rows(t: &Terminal<TestBackend>) -> Vec<String> {
        let buf = t.backend().buffer();
        (0..buf.area.height)
            .map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect())
            .collect()
    }

    const SECTIONS: &[HelpSection] = &[
        ("Move", &[("j/k", "scroll")]),
        ("Search", &[("/", "search"), ("n/N", "next/previous")]),
        ("Other", &[("q", "quit")]),
    ];

    #[test]
    fn help_shows_the_version_and_pairs_sections() {
        let mut term = Terminal::new(TestBackend::new(80, 20)).unwrap();
        term.draw(|f| help(f, &Theme::dark(), f.area(), SECTIONS, &mut 0))
            .unwrap();
        let screen = rows(&term);
        assert!(screen.iter().any(|r| r.contains(" Help ")), "{screen:#?}");
        let version = format!("bastian {}", env!("CARGO_PKG_VERSION"));
        assert!(screen.iter().any(|r| r.contains(&version)), "{screen:#?}");
        let paired = screen
            .iter()
            .any(|r| r.contains("Move") && r.contains("Search"));
        assert!(paired, "{screen:#?}");
        assert!(
            !screen
                .iter()
                .any(|r| r.contains("Search") && r.contains("Other"))
        );
    }

    #[test]
    fn narrow_help_stacks_sections() {
        let mut term = Terminal::new(TestBackend::new(30, 20)).unwrap();
        term.draw(|f| help(f, &Theme::dark(), f.area(), SECTIONS, &mut 0))
            .unwrap();
        let screen = rows(&term);
        let move_row = screen.iter().position(|r| r.contains("Move"));
        let search_row = screen.iter().position(|r| r.contains("Search"));
        assert!(move_row < search_row, "{screen:#?}");
    }

    #[test]
    fn list_popup_keeps_the_selection_in_view() {
        let theme = Theme::dark();
        let lines: Vec<Line> = (0..20).map(|i| Line::raw(format!("item {i}"))).collect();
        let mut term = Terminal::new(TestBackend::new(30, 10)).unwrap();
        term.draw(|f| list_popup(f, &theme, f.area(), " t ", lines.clone(), 15, 4))
            .unwrap();
        let screen = rows(&term);
        let marked: Vec<&String> = screen.iter().filter(|r| r.contains('▶')).collect();
        assert_eq!(marked.len(), 1, "{screen:#?}");
        assert!(marked[0].contains("▶ item 15"), "{screen:#?}");
        // Four rows inside the border, ending on the selection.
        assert!(
            screen.iter().any(|r| r.contains("  item 12")),
            "{screen:#?}"
        );
        assert!(!screen.iter().any(|r| r.contains("item 16")), "{screen:#?}");
    }

    #[test]
    fn popups_center_on_the_area_given_and_stay_on_screen() {
        let lines = vec![Line::raw("x".repeat(16)); 4];
        let screen = Rect::new(0, 0, 120, 30);
        // The document beside a 29-column pane, above the status bar.
        let doc = Rect::new(29, 0, 91, 29);
        let rect = popup_rect(screen, doc, " t ", &lines, 0, 4);
        assert_eq!((rect.width, rect.height), (20, 6));
        let (left, right) = (rect.x - doc.x, doc.right() - rect.right());
        let (top, bottom) = (rect.y - doc.y, doc.bottom() - rect.bottom());
        assert!(left.abs_diff(right) <= 1, "{left} vs {right}");
        assert!(top.abs_diff(bottom) <= 1, "{top} vs {bottom}");
        // Wider than the area: centered on it as far as the screen allows.
        let narrow = Rect::new(110, 0, 10, 29);
        let rect = popup_rect(screen, narrow, " t ", &lines, 0, 4);
        assert_eq!(rect.right(), screen.right());
    }

    #[test]
    fn narrow_bars_give_up_the_counter_then_the_name_for_the_last_hint() {
        let hints: &[(&str, &str)] = &[("j/k", "move"), ("?", "help")];
        let bar = |w: u16| {
            let mut term = Terminal::new(TestBackend::new(w, 1)).unwrap();
            term.draw(|f| draw(f, f.area(), &Theme::dark(), Left::Hints(hints), "1/2"))
                .unwrap();
            rows(&term).remove(0)
        };
        let wide = bar(40);
        assert!(wide.contains("bastian") && wide.contains("1/2"), "{wide}");
        let twenty = bar(20);
        assert!(twenty.contains("bastian  ? help"), "{twenty}");
        assert!(!twenty.contains("1/2"), "{twenty}");
        let fourteen = bar(14);
        assert_eq!(fourteen.trim(), "? help", "{fourteen}");
    }
}
