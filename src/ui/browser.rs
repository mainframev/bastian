use std::time::SystemTime;

use jiff::Timestamp;
use jiff::tz::TimeZone;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::inset;
use super::logo;
use super::status;
use crate::browser::Browser;
use crate::render::theme::Theme;

const HELP: &[(&str, &str)] = &[
    ("j/k  ↓/↑", "move (ctrl-j/k also while filtering)"),
    ("h/l  ←/→", "previous/next page"),
    ("ctrl-d/u", "half page"),
    ("ctrl-f/b", "full page"),
    ("g/G", "first/last"),
    ("/", "filter"),
    ("Enter", "open"),
    ("e", "edit in $VISUAL/$EDITOR"),
    ("r", "rescan for files"),
    ("T", "pick a theme"),
    ("Esc", "clear filter"),
    ("q", "quit"),
];

/// Rows per entry at most: title, path, date, gap. Pages hold as many
/// entries as fit at this height; entries without a path line are a row
/// shorter, so the gap after every entry stays one row.
const ENTRY_ROWS: u16 = 4;

pub fn draw(frame: &mut Frame, b: &mut Browser, t: &Theme, show_logo: bool) {
    // Layout: the logo (or the `Find:` line while filtering; just that
    // line's row without the logo), the document count, then the list.
    // The page indicator goes under the list (each entry already ends with
    // a blank row) with a blank row between it and the status bar.
    let [_, top, _, count_row, _, list, pager_row, _, status] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(if show_logo { logo::HEIGHT } else { 1 }),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    let finding = b.prompt.is_some() || !b.filter.is_empty();
    if finding {
        let text = b.prompt.as_ref().map_or(b.filter.as_str(), |p| p.text());
        let cursor = b.prompt.as_ref().map(|p| p.cursor());
        frame.render_widget(status::find_line(t, text, cursor), inset(top));
    } else if show_logo {
        logo::draw(frame, inset(top), t);
    }

    let count = if b.filter.is_empty() {
        format!("{} documents", b.entries.len())
    } else {
        format!("{} of {} documents", b.visible.len(), b.entries.len())
    };
    b.layout(usize::from(list.height / ENTRY_ROWS));

    let (page, pages) = b.page();
    if pages > 1 {
        frame.render_widget(page_indicator(t, page, pages), inset(pager_row));
    }

    // `~/project  1953 documents`: the directory, then its count. A long
    // path is shortened from the left so the count always fits.
    let count_row = inset(count_row);
    let mut tail = vec![Span::raw("  ")];
    if b.is_scanning() {
        tail.push(Span::styled(
            format!("{} ", spinner(b)),
            Style::default().fg(t.accent),
        ));
    }
    tail.push(Span::styled(count, Style::default().fg(t.muted)));
    let tail_w: usize = tail.iter().map(Span::width).sum();
    let root = shorten_left(
        &b.root_label,
        (count_row.width as usize).saturating_sub(tail_w),
    );
    let mut spans = vec![Span::styled(root, Style::default().fg(t.heading).bold())];
    spans.extend(tail);
    frame.render_widget(Line::from(spans), count_row);
    let mut lines = Vec::new();
    if b.visible.is_empty() {
        let msg = if b.is_scanning() {
            "Looking for Markdown files…"
        } else if b.entries.is_empty() {
            "No Markdown files here."
        } else {
            "Nothing matches the filter."
        };
        lines.push(Line::styled(msg, Style::default().fg(t.muted)));
    }
    let tz = TimeZone::system();
    for (row, &idx) in b.visible.iter().enumerate().skip(b.offset()).take(b.height) {
        let e = &b.entries[idx];
        let selected = row == b.selected;
        let bar = if selected {
            Span::styled("│ ", Style::default().fg(t.accent))
        } else {
            Span::raw("  ")
        };
        let title_style = status::row_style(t, selected);
        // Title, then progressively quieter path and date lines. The path
        // is left out when it would just repeat the title (a top-level file
        // without a title).
        let title = e.title.clone().unwrap_or_else(|| {
            e.path
                .file_name()
                .map_or_else(|| e.rel.clone(), |n| n.to_string_lossy().into_owned())
        });
        let show_path = e.rel != title;
        lines.push(Line::from(vec![
            bar.clone(),
            Span::styled(title, title_style),
        ]));
        if show_path {
            lines.push(Line::from(vec![
                bar.clone(),
                Span::styled(e.rel.clone(), Style::default().fg(t.subtle)),
            ]));
        }
        let date = e.created.map(|c| format_time(c, &tz)).unwrap_or_default();
        lines.push(Line::from(vec![
            bar,
            Span::styled(
                date,
                Style::default().fg(t.muted).add_modifier(Modifier::DIM),
            ),
        ]));
        lines.push(Line::default());
    }
    frame.render_widget(Paragraph::new(lines), inset(list));
    // In entries, so the thumb moves a page at a time like the list.
    status::scrollbar(frame, t, list, b.visible.len(), b.height, b.offset());

    let idle = if b.filter.is_empty() {
        "enter open · / filter · e edit · ? more"
    } else {
        "/ edit filter · esc clear"
    };
    let left = status::left(b.prompt.is_some(), b.message.as_deref(), idle);
    let right = if b.visible.is_empty() {
        String::new()
    } else {
        format!("{}/{}", b.selected + 1, b.visible.len())
    };
    status::draw(frame, status, t, left, &right);

    if b.help {
        status::help(frame, t, HELP);
    }
}

/// Width of the page track, in cells.
const TRACK: usize = 12;

/// `━━━━━━──────  4/12`: a track filled up to the current page, then the
/// page number.
fn page_indicator(t: &Theme, page: usize, pages: usize) -> Line<'static> {
    let filled = (page * TRACK).div_ceil(pages).clamp(1, TRACK);
    Line::from(vec![
        Span::styled("━".repeat(filled), Style::default().fg(t.accent)),
        Span::styled("─".repeat(TRACK - filled), Style::default().fg(t.muted)),
        Span::styled(format!("  {page}/{pages}"), Style::default().fg(t.subtle)),
    ])
}

/// Keep the last `max` columns of `s`, marking the cut with `…`.
fn shorten_left(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let keep = &chars[chars.len() - (max - 1)..];
    format!("…{}", keep.iter().collect::<String>())
}

/// Braille spinner frame for the running scan, advancing every 80 ms.
fn spinner(b: &Browser) -> char {
    const FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    FRAMES[(b.started.elapsed().as_millis() / 80) as usize % FRAMES.len()]
}

/// Absolute local time, e.g. `06 Aug 2026 02:20 CEST`.
pub fn format_time(t: SystemTime, tz: &TimeZone) -> String {
    match Timestamp::try_from(t) {
        Ok(ts) => ts
            .to_zoned(tz.clone())
            .strftime("%d %b %Y %H:%M %Z")
            .to_string(),
        Err(_) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

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

    #[test]
    fn shortens_paths_from_the_left() {
        assert_eq!(shorten_left("~/code/app", 20), "~/code/app");
        assert_eq!(shorten_left("~/code/app", 6), "…e/app");
        assert_eq!(shorten_left("~/code/app", 0), "");
    }

    #[test]
    fn formats_absolute_local_time() {
        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(1_786_000_000);
        let utc = TimeZone::UTC;
        assert_eq!(format_time(t, &utc), "06 Aug 2026 07:06 UTC");
        let berlin = TimeZone::get("Europe/Berlin").unwrap();
        assert_eq!(format_time(t, &berlin), "06 Aug 2026 09:06 CEST");
    }
}
