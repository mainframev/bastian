use std::path::Path;
use std::time::SystemTime;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::reader::MARGIN;
use super::status::{self, Left};
use crate::browser::Browser;
use crate::render::theme::Theme;

/// Rows per entry: title, path line, gap.
const ENTRY_ROWS: u16 = 3;

pub fn draw(frame: &mut Frame, b: &mut Browser, t: &Theme) {
    let [header, list, status] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    let inset = |r: Rect| Rect {
        x: r.x + MARGIN,
        width: r.width.saturating_sub(MARGIN * 2),
        ..r
    };

    let mut count = if b.filter.is_empty() {
        format!("{} documents", b.entries.len())
    } else {
        format!("{} of {} documents", b.visible.len(), b.entries.len())
    };
    if b.is_scanning() {
        count = format!("{} {count}", spinner(b));
    }
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(display_root(&b.root), Style::default().fg(t.heading).bold()),
            Span::styled(format!("  {count}"), Style::default().fg(t.muted)),
        ])),
        inset(header),
    );

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
    let now = SystemTime::now();
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
        let title = e.title.clone().unwrap_or_else(|| {
            e.path
                .file_name()
                .map_or_else(|| e.rel.clone(), |n| n.to_string_lossy().into_owned())
        });
        lines.push(Line::from(vec![
            bar.clone(),
            Span::styled(title, title_style),
        ]));
        let mut meta = e.rel.clone();
        if let Some(m) = e.modified {
            meta.push_str(&format!(" · {}", ago(now, m)));
        }
        lines.push(Line::from(vec![
            bar,
            Span::styled(meta, Style::default().fg(t.muted)),
        ]));
        lines.push(Line::default());
    }
    frame.render_widget(Paragraph::new(lines), inset(list));

    let left = match (&b.prompt, &b.message) {
        (Some(p), _) => Left::Prompt(p),
        (None, Some(m)) => Left::Error(m),
        (None, None) if !b.filter.is_empty() => Left::Text(&b.filter),
        (None, None) => Left::Text("enter open · / filter · q quit"),
    };
    let right = if b.visible.is_empty() {
        String::new()
    } else {
        format!("{}/{}", b.selected + 1, b.visible.len())
    };
    status::draw(frame, status, t, left, &right);
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

/// Braille spinner frame for the running scan, advancing every 80 ms.
fn spinner(b: &Browser) -> char {
    const FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    FRAMES[(b.started.elapsed().as_millis() / 80) as usize % FRAMES.len()]
}

/// Coarse relative time, e.g. "3 days ago".
pub fn ago(now: SystemTime, then: SystemTime) -> String {
    let secs = now.duration_since(then).map_or(0, |d| d.as_secs());
    let (n, unit) = match secs {
        0..60 => return "just now".to_string(),
        60..3600 => (secs / 60, "minute"),
        3600..86_400 => (secs / 3600, "hour"),
        86_400..2_592_000 => (secs / 86_400, "day"),
        2_592_000..31_536_000 => (secs / 2_592_000, "month"),
        _ => (secs / 31_536_000, "year"),
    };
    format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn relative_times() {
        let now = SystemTime::now();
        assert_eq!(ago(now, now), "just now");
        assert_eq!(ago(now, now - Duration::from_secs(60)), "1 minute ago");
        assert_eq!(
            ago(now, now - Duration::from_secs(3 * 86_400)),
            "3 days ago"
        );
        assert_eq!(ago(now, now + Duration::from_secs(10)), "just now");
    }
}
