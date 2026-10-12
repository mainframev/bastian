use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::browser::{Browser, Entry};
use crate::config::Selector;
use crate::render::theme::Theme;
use crate::ui::status;
use crate::ui::text::{shorten_left, shorten_right};

/// What goes beside each row of an entry: `first` is its title row.
pub(super) fn marker(selector: Selector, selected: bool, first: bool, t: &Theme) -> Span<'static> {
    let glyph = match selector {
        _ if !selected => None,
        Selector::Bar => Some("│"),
        Selector::Thick => Some("┃"),
        Selector::Block => Some("▌"),
        Selector::Arrow if first => Some("❯"),
        Selector::Arrow | Selector::Highlight => None,
    };
    match glyph {
        Some(g) => Span::styled(format!("{g} "), Style::default().fg(t.accent)),
        None => Span::raw("  "),
    }
}

/// Title, then progressively quieter path and date lines. The path is left
/// out when it would just repeat the title (a top-level file without a
/// title).
pub(super) fn card_lines(
    e: &Entry,
    date: &str,
    t: &Theme,
    selector: Selector,
    selected: bool,
) -> Vec<Line<'static>> {
    let mark = |first| marker(selector, selected, first, t);
    let title = e.name();
    let show_path = e.rel != title;
    let mut lines = vec![Line::from(vec![
        mark(true),
        Span::styled(title, status::row_style(t, selected)),
    ])];
    if show_path {
        lines.push(Line::from(vec![
            mark(false),
            Span::styled(e.rel.clone(), Style::default().fg(t.subtle)),
        ]));
    }
    if !date.is_empty() {
        lines.push(Line::from(vec![
            mark(false),
            Span::styled(
                date.to_string(),
                Style::default().fg(t.muted).add_modifier(Modifier::DIM),
            ),
        ]));
    }
    lines
}

/// `❯ Title  path/to/file.md          06 Aug 2026 09:06 CEST`: one row in
/// `width` columns. The date stays at the right edge; the title is cut
/// before it, and the path only gets the room left over.
pub(super) fn compact_line(
    e: &Entry,
    date: &str,
    t: &Theme,
    selector: Selector,
    selected: bool,
    width: usize,
) -> Line<'static> {
    const GAP: usize = 2;
    let mark = marker(selector, selected, true, t);
    let title = e.name();
    let show_path = e.rel != title;
    let date_w = if date.is_empty() {
        0
    } else {
        date.width() + GAP
    };
    let room = width.saturating_sub(mark.width() + date_w);
    let title = shorten_right(&title, room);
    let mut spans = vec![mark];
    let mut used = title.width();
    spans.push(Span::styled(title, status::row_style(t, selected)));
    let path_room = room.saturating_sub(used + GAP);
    // A path cut down to a few columns says nothing; leave it out.
    if show_path && path_room >= 8 {
        let path = shorten_left(&e.rel, path_room);
        used += GAP + path.width();
        spans.push(Span::raw(" ".repeat(GAP)));
        spans.push(Span::styled(path, Style::default().fg(t.subtle)));
    }
    if date_w > 0 && room > 0 {
        spans.push(Span::raw(" ".repeat((room + GAP).saturating_sub(used))));
        spans.push(Span::styled(
            date.to_string(),
            Style::default().fg(t.muted).add_modifier(Modifier::DIM),
        ));
    }
    Line::from(spans)
}

/// Width of the page track, in cells.
pub(super) const TRACK: usize = 12;

/// `━━━━━━──────  4/12`: a track filled up to the current page, then the
/// page number.
pub(super) fn page_indicator(t: &Theme, page: usize, pages: usize) -> Line<'static> {
    let filled = (page * TRACK).div_ceil(pages).clamp(1, TRACK);
    Line::from(vec![
        Span::styled("━".repeat(filled), Style::default().fg(t.accent)),
        Span::styled("─".repeat(TRACK - filled), Style::default().fg(t.muted)),
        Span::styled(format!("  {page}/{pages}"), Style::default().fg(t.subtle)),
    ])
}

/// Braille spinner frame for the running scan, advancing every 80 ms.
pub(super) fn spinner(b: &Browser) -> char {
    const FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    FRAMES[(b.started.elapsed().as_millis() / 80) as usize % FRAMES.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_track_fills_with_progress() {
        let t = Theme::dark();
        let text = |p, n| -> String {
            page_indicator(&t, p, n)
                .spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect()
        };
        assert_eq!(text(1, 12), "━───────────  1/12");
        assert_eq!(text(6, 12), "━━━━━━──────  6/12");
        assert_eq!(text(95, 95), "━━━━━━━━━━━━  95/95");
        assert_eq!(text(1, 95), "━───────────  1/95");
    }
}
