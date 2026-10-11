use std::path::Path;
use std::time::SystemTime;

use jiff::Timestamp;
use jiff::tz::TimeZone;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::reader::MARGIN;
use super::status::{self, Left};
use crate::browser::Browser;
use crate::render::theme::Theme;

const HELP: &[(&str, &str)] = &[
    ("j/k  ↓/↑", "move"),
    ("ctrl-d/u", "half page"),
    ("ctrl-f/b", "full page"),
    ("g/G", "first/last"),
    ("/", "filter"),
    ("Enter, l", "open"),
    ("e", "edit in $VISUAL/$EDITOR"),
    ("r", "rescan for files"),
    ("Esc", "clear filter"),
    ("q", "quit"),
];

/// Rows per entry: title, path line, gap.
const ENTRY_ROWS: u16 = 3;

pub fn draw(frame: &mut Frame, b: &mut Browser, t: &Theme) {
    // Laid out like glow: a title badge (or the `Find:` line while
    // filtering), the document count, then the list.
    let [_, top, _, count_row, _, list, status] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    let inset = |r: Rect| Rect {
        x: r.x + MARGIN,
        width: r.width.saturating_sub(MARGIN * 2),
        ..r
    };

    let finding = b.prompt.is_some() || !b.filter.is_empty();
    if finding {
        let text = b
            .prompt
            .as_ref()
            .map_or(b.filter.as_str(), |p| p.text.as_str());
        frame.render_widget(status::find_line(t, text, b.prompt.is_some()), inset(top));
    } else {
        frame.render_widget(
            Span::styled(" bastian ", Style::default().fg(t.warn).bg(t.h1_bg).bold()),
            inset(top),
        );
    }

    let count = if b.filter.is_empty() {
        format!("{} documents", b.entries.len())
    } else {
        format!("{} of {} documents", b.visible.len(), b.entries.len())
    };
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
        &display_root(&b.root),
        (count_row.width as usize).saturating_sub(tail_w),
    );
    let mut spans = vec![Span::styled(root, Style::default().fg(t.heading).bold())];
    spans.extend(tail);
    frame.render_widget(Line::from(spans), count_row);

    b.layout(usize::from(list.height / ENTRY_ROWS));
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
    for (row, &idx) in b.visible.iter().enumerate().skip(b.offset).take(b.height) {
        let e = &b.entries[idx];
        let selected = row == b.selected;
        let bar = if selected {
            Span::styled("│ ", Style::default().fg(t.accent))
        } else {
            Span::raw("  ")
        };
        let title_style = if selected {
            Style::default().fg(t.accent).bold()
        } else {
            Style::default().fg(t.text)
        };
        // Like glow: the path (or the document's title) over its date.
        let title = e.title.clone().unwrap_or_else(|| e.rel.clone());
        lines.push(Line::from(vec![
            bar.clone(),
            Span::styled(title, title_style),
        ]));
        let mut meta = e.created.map(|c| format_time(c, &tz)).unwrap_or_default();
        if e.title.is_some() {
            if !meta.is_empty() {
                meta.push_str(" · ");
            }
            meta.push_str(&e.rel);
        }
        lines.push(Line::from(vec![
            bar,
            Span::styled(meta, Style::default().fg(t.muted)),
        ]));
        lines.push(Line::default());
    }
    frame.render_widget(Paragraph::new(lines), inset(list));

    let left = match (&b.prompt, &b.message) {
        (Some(_), _) => Left::Text("enter accept · esc clear"),
        (None, Some(m)) => Left::Error(m),
        (None, None) if !b.filter.is_empty() => Left::Text("/ edit filter · esc clear"),
        (None, None) => Left::Text("enter open · / filter · e edit · ? more"),
    };
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

/// Absolute path with the home directory shortened to `~`.
fn display_root(root: &Path) -> String {
    let abs = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    match std::env::var_os("HOME").and_then(|h| abs.strip_prefix(h).ok().map(Path::to_path_buf)) {
        Some(rel) if rel.as_os_str().is_empty() => "~".to_string(),
        Some(rel) => format!("~/{}", rel.display()),
        None => abs.display().to_string(),
    }
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

/// Absolute local time like glow shows it: `06 Aug 2026 02:20 CEST`.
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
    fn shortens_paths_from_the_left() {
        assert_eq!(shorten_left("~/code/app", 20), "~/code/app");
        assert_eq!(shorten_left("~/code/app", 6), "…e/app");
        assert_eq!(shorten_left("~/code/app", 0), "");
    }

    #[test]
    fn formats_like_glow() {
        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(1_786_000_000);
        let utc = TimeZone::UTC;
        assert_eq!(format_time(t, &utc), "06 Aug 2026 07:06 UTC");
        let berlin = TimeZone::get("Europe/Berlin").unwrap();
        assert_eq!(format_time(t, &berlin), "06 Aug 2026 09:06 CEST");
    }
}
