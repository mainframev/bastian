use std::time::SystemTime;

use jiff::Timestamp;
use jiff::tz::TimeZone;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::inset;
use super::logo;
use super::status;
use crate::browser::{Browser, Entry};
use crate::config::{ListLayout, Selector};
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

/// Rows per card at most: title, path, date, gap. Pages hold as many
/// entries as fit at this height; cards without a path line are a row
/// shorter, so the gap after every card stays one row.
const CARD_ROWS: u16 = 4;

/// How the file list looks, from the settings.
#[derive(Debug, Clone, Copy, Default)]
pub struct Look<'a> {
    /// The logo at the top.
    pub logo: bool,
    /// strftime format of each file's date; `None` leaves dates out.
    pub date_format: Option<&'a str>,
    pub selector: Selector,
    pub layout: ListLayout,
    /// A scrollbar beside the list when it's longer than a page.
    pub scrollbar: bool,
}

pub fn draw(frame: &mut Frame, b: &mut Browser, t: &Theme, look: Look) {
    let show_logo = look.logo;
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
        let cursor = b.prompt.as_ref().map(super::super::prompt::Prompt::cursor);
        frame.render_widget(status::find_line(t, text, cursor), inset(top));
    } else if show_logo {
        logo::draw(frame, inset(top), t);
    }

    let count = if b.filter.is_empty() {
        format!("{} documents", b.entries.len())
    } else {
        format!("{} of {} documents", b.visible.len(), b.entries.len())
    };
    let entry_rows = match look.layout {
        // A row less without the date line.
        ListLayout::Cards => CARD_ROWS - u16::from(look.date_format.is_none()),
        ListLayout::Compact => 1,
    };
    b.layout(usize::from(list.height / entry_rows));

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
    let list_area = inset(list);
    // Rows of the selected entry, for the highlight selector.
    let mut selected_rows = 0..0;
    for (row, &idx) in b.visible.iter().enumerate().skip(b.offset()).take(b.height) {
        let e = &b.entries[idx];
        let selected = row == b.selected;
        let start = lines.len();
        let date = look
            .date_format
            .zip(e.date(b.scan.sort))
            .map(|(f, time)| format_time(time, &tz, f))
            .unwrap_or_default();
        match look.layout {
            ListLayout::Cards => {
                lines.extend(card_lines(e, &date, t, look.selector, selected));
            }
            ListLayout::Compact => lines.push(compact_line(
                e,
                &date,
                t,
                look.selector,
                selected,
                usize::from(list_area.width),
            )),
        }
        if selected {
            selected_rows = start..lines.len();
        }
        if look.layout == ListLayout::Cards {
            lines.push(Line::default());
        }
    }
    frame.render_widget(Paragraph::new(lines), list_area);
    if look.selector == Selector::Highlight && !selected_rows.is_empty() {
        let band = Rect {
            y: list_area.y + selected_rows.start as u16,
            height: selected_rows.len() as u16,
            ..list_area
        }
        .intersection(list_area);
        frame
            .buffer_mut()
            .set_style(band, Style::default().bg(t.surface));
    }
    // In entries, so the thumb moves a page at a time like the list.
    if look.scrollbar {
        status::scrollbar(frame, t, list, b.visible.len(), b.height, b.offset());
    }

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

/// What goes beside each row of an entry: `first` is its title row.
fn marker(selector: Selector, selected: bool, first: bool, t: &Theme) -> Span<'static> {
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
fn card_lines(
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
fn compact_line(
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
        spans.push(Span::raw(" ".repeat(room - used + GAP)));
        spans.push(Span::styled(
            date.to_string(),
            Style::default().fg(t.muted).add_modifier(Modifier::DIM),
        ));
    }
    Line::from(spans)
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

/// Keep the first `max` columns of `s`, marking the cut with `…`.
fn shorten_right(s: &str, max: usize) -> String {
    if s.width() <= max {
        return s.to_string();
    }
    let mut out = String::new();
    let mut w = 0;
    for c in s.chars() {
        let cw = c.width().unwrap_or(0);
        if w + cw + 1 > max {
            break;
        }
        out.push(c);
        w += cw;
    }
    if max > 0 {
        out.push('…');
    }
    out
}

/// Braille spinner frame for the running scan, advancing every 80 ms.
fn spinner(b: &Browser) -> char {
    const FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    FRAMES[(b.started.elapsed().as_millis() / 80) as usize % FRAMES.len()]
}

/// Local time in strftime `format`, e.g. `06 Aug 2026 02:20 CEST`.
/// Empty when it can't be shown.
pub fn format_time(t: SystemTime, tz: &TimeZone, format: &str) -> String {
    Timestamp::try_from(t)
        .ok()
        .and_then(|ts| jiff::fmt::strtime::format(format, &ts.to_zoned(tz.clone())).ok())
        .unwrap_or_default()
}

/// An error naming `date_format` when `format` can't format a time.
pub fn check_date_format(format: &str) -> anyhow::Result<()> {
    let zoned = Timestamp::UNIX_EPOCH.to_zoned(TimeZone::UTC);
    jiff::fmt::strtime::format(format, &zoned)
        .map(drop)
        .map_err(|e| anyhow::anyhow!("bad `date_format` {format:?}: {e}"))
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
        assert_eq!(
            format_time(t, &utc, crate::config::DATE_FORMAT),
            "06 Aug 2026 07:06 UTC"
        );
        let berlin = TimeZone::get("Europe/Berlin").unwrap();
        assert_eq!(
            format_time(t, &berlin, crate::config::DATE_FORMAT),
            "06 Aug 2026 09:06 CEST"
        );
    }

    /// Browse a tree with `top.md` (no title) and `docs/guide.md` (titled),
    /// then draw the browser into a `w`×`h` screen.
    fn screen(files: usize, logo: bool, h: u16) -> Vec<String> {
        let look = Look {
            logo,
            date_format: Some(crate::config::DATE_FORMAT),
            scrollbar: true,
            ..Look::default()
        };
        rows(&styled_screen(files, look, h))
    }

    /// Like `screen`, with any look, keeping the cells' styles.
    fn styled_screen(files: usize, look: Look, h: u16) -> ratatui::buffer::Buffer {
        use ratatui::{Terminal, backend::TestBackend};
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("docs")).unwrap();
        std::fs::write(dir.path().join("docs/guide.md"), "# Guide\n").unwrap();
        // Newest first: the untitled file is listed above the titled one.
        std::thread::sleep(Duration::from_millis(20));
        std::fs::write(dir.path().join("top.md"), "no title\n").unwrap();
        for i in 0..files {
            std::fs::write(dir.path().join(format!("n{i}.md")), "x\n").unwrap();
        }
        let mut b = Browser::new(dir.path().to_path_buf(), crate::browser::Scan::default());
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while b.is_scanning() {
            b.tick();
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        let mut terminal = Terminal::new(TestBackend::new(50, h)).unwrap();
        let t = Theme::dark();
        terminal.draw(|f| draw(f, &mut b, &t, look)).unwrap();
        terminal.backend().buffer().clone()
    }

    fn rows(buf: &ratatui::buffer::Buffer) -> Vec<String> {
        let area = buf.area;
        (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect()
    }

    /// Index of the first row containing `s`.
    fn row_of(rows: &[String], s: &str) -> usize {
        rows.iter()
            .position(|r| r.contains(s))
            .unwrap_or_else(|| panic!("no {s}:\n{}", rows.join("\n")))
    }

    fn look(selector: Selector, layout: ListLayout) -> Look<'static> {
        Look {
            logo: false,
            date_format: Some(crate::config::DATE_FORMAT),
            selector,
            layout,
            scrollbar: true,
        }
    }

    #[test]
    fn selectors_mark_only_the_selected_card() {
        // top.md (selected) is a name and a date; Guide adds a path row.
        for (selector, glyph, marked) in [
            (Selector::Bar, '│', 2),
            (Selector::Thick, '┃', 2),
            (Selector::Block, '▌', 2),
            (Selector::Arrow, '❯', 1),
        ] {
            let rows = rows(&styled_screen(0, look(selector, ListLayout::Cards), 30));
            let text = rows.join("\n");
            let with: Vec<usize> = (0..rows.len())
                .filter(|&i| rows[i].starts_with(&format!("  {glyph} ")))
                .collect();
            let top = row_of(&rows, "top.md");
            assert_eq!(
                with,
                (top..top + marked).collect::<Vec<_>>(),
                "{selector:?}:\n{text}"
            );
        }
    }

    #[test]
    fn highlight_selector_colors_the_selected_rows() {
        let t = Theme::dark();
        let buf = styled_screen(0, look(Selector::Highlight, ListLayout::Cards), 30);
        let rows = rows(&buf);
        let (top, guide) = (row_of(&rows, "top.md"), row_of(&rows, "Guide"));
        let bg = |y: usize| buf[(10, y as u16)].bg;
        assert_eq!(bg(top), t.surface);
        assert_eq!(bg(top + 1), t.surface, "the date row too");
        assert_ne!(bg(top + 2), t.surface, "not the gap after it");
        assert_ne!(bg(guide), t.surface);
        assert!(!rows.join("").contains('│'));
    }

    #[test]
    fn compact_layout_lists_one_file_per_row() {
        let rows = rows(&styled_screen(
            0,
            look(Selector::Bar, ListLayout::Compact),
            30,
        ));
        let text = rows.join("\n");
        let (top, guide) = (row_of(&rows, "top.md"), row_of(&rows, "Guide"));
        assert_eq!(guide, top + 1, "{text}");
        assert!(rows[top].starts_with("  │ top.md"), "{text}");
        // The path follows the title; the date ends at the right margin.
        assert!(rows[guide].contains("Guide  docs/guide.md"), "{text}");
        let last = 50 - 1 - usize::from(crate::ui::MARGIN);
        assert_ne!(rows[guide].chars().nth(last), Some(' '), "{text}");
        assert_eq!(rows[guide].chars().nth(last + 1), Some(' '), "{text}");
    }

    #[test]
    fn compact_pages_hold_a_file_per_list_row() {
        let rows = rows(&styled_screen(
            40,
            look(Selector::Bar, ListLayout::Compact),
            30,
        ));
        let text = rows.join("\n");
        let listed = rows
            .iter()
            .filter(|r| r.contains(".md") || r.contains("Guide"))
            .count();
        // 30 rows less the eight around the list (padding, find/logo row,
        // count, gaps, page indicator, status bar).
        assert_eq!(listed, 22, "{text}");
        let first = rows.iter().position(|r| r.contains(".md")).unwrap();
        assert!(
            rows[first..first + listed]
                .iter()
                .all(|r| r.contains(".md") || r.contains("Guide")),
            "{text}"
        );
        assert!(text.contains('━'), "more than one page:\n{text}");
    }

    #[test]
    fn shortens_titles_from_the_right() {
        assert_eq!(shorten_right("Guide", 10), "Guide");
        assert_eq!(shorten_right("A long title", 6), "A lon…");
        assert_eq!(shorten_right("Guide", 0), "");
    }

    #[test]
    fn entries_hide_a_path_that_repeats_the_name() {
        let rows = screen(0, false, 30);
        let text = rows.join("\n");
        let at = |s: &str| {
            rows.iter()
                .position(|r| r.contains(s))
                .unwrap_or_else(|| panic!("{s}:\n{text}"))
        };
        // A titled entry shows its path under the title.
        assert_eq!(at("docs/guide.md"), at("Guide") + 1);
        // An untitled top-level file shows its name once, then the date,
        // with one blank row before the next entry like any other.
        assert_eq!(rows.iter().filter(|r| r.contains("top.md")).count(), 1);
        assert!(at("top.md") < at("Guide"), "{text}");
        assert_eq!(
            at("Guide"),
            at("top.md") + 3,
            "name, date, one blank row:\n{text}"
        );
    }

    #[test]
    fn logo_can_be_turned_off() {
        let with = screen(0, true, 30);
        let without = screen(0, false, 30);
        let first_doc =
            |rows: &[String]| rows.iter().position(|r| r.contains("documents")).unwrap();
        assert_eq!(
            first_doc(&with) - first_doc(&without),
            usize::from(logo::HEIGHT) - 1
        );
    }

    #[test]
    fn page_indicator_shows_with_more_than_one_page() {
        // The page track; the status bar's "1/2" counts entries instead.
        let one = screen(0, false, 30).join("\n");
        assert!(!one.contains('━'), "{one}");
        let many = screen(20, false, 30).join("\n");
        assert!(many.contains('━'), "{many}");
    }

    #[test]
    fn dates_can_be_left_out() {
        let look = Look {
            date_format: None,
            ..look(Selector::Bar, ListLayout::Cards)
        };
        let rows = rows(&styled_screen(0, look, 30));
        let text = rows.join("\n");
        // top.md is just its name now, then the gap.
        assert_eq!(
            row_of(&rows, "Guide"),
            row_of(&rows, "top.md") + 2,
            "{text}"
        );
        assert!(!text.contains(" 20"), "no year anywhere:\n{text}");
    }

    #[test]
    fn date_formats_are_checked() {
        assert!(check_date_format("%Y-%m-%d").is_ok());
        let err = check_date_format("%Y %").unwrap_err().to_string();
        assert!(
            err.contains("`date_format`") && err.contains("%Y %"),
            "{err}"
        );
        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(1_786_000_000);
        assert_eq!(format_time(t, &TimeZone::UTC, "%Y-%m-%d"), "2026-08-06");
    }
}
