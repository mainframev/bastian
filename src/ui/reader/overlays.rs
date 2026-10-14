use ratatui::Frame;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::reader::{LinkPicker, Location, Reader};
use crate::render::theme::Theme;
use crate::text::{shorten_left, take_width};
use crate::ui::status;

pub(super) fn draw_links(frame: &mut Frame, r: &Reader, picker: &LinkPicker) {
    let t = r.theme();
    let area = frame.area();
    // Leave room for the border and a margin around the box.
    let rows = area.height.saturating_sub(6).max(1) as usize;
    let max_w = area.width.saturating_sub(10) as usize;
    let lines: Vec<Line> = picker
        .links
        .iter()
        .enumerate()
        .map(|(i, link)| {
            let selected = i == picker.selected;
            let text_style = status::row_style(t, selected);
            let entry = format!("{}  {}", link.text, link.url);
            let shown = take_width(&entry, max_w.saturating_sub(2));
            // Both are starts of `entry`, so the shorter one ends on a char.
            let (text, url) = shown.split_at(link.text.len().min(shown.len()));
            Line::from(vec![
                Span::styled(text.to_string(), text_style),
                Span::styled(url.to_string(), Style::default().fg(t.muted)),
            ])
        })
        .collect();
    let title = format!(
        " links {}/{} · enter open · esc close ",
        picker.selected + 1,
        picker.links.len()
    );
    status::list_popup(frame, t, &title, lines, picker.selected, rows);
}

pub(super) fn draw_confirm(frame: &mut Frame, r: &Reader, ids: &[usize]) {
    let t = r.theme();
    let what = match ids {
        [id] => r.label(*id),
        _ => format!("{} blocks, in order", ids.len()),
    };
    let cwd = ids
        .first()
        .map_or_else(|| r.base_dir().to_path_buf(), |&id| r.cwd(id))
        .display()
        .to_string();
    let lines = vec![
        Line::from(vec![
            Span::styled("Run ", Style::default().fg(t.text)),
            Span::styled(what, Style::default().fg(t.accent).bold()),
            Span::styled("?", Style::default().fg(t.text)),
        ]),
        Line::styled(format!("in {cwd}"), Style::default().fg(t.muted)),
        Line::default(),
        Line::from(vec![
            Span::styled("y", Style::default().fg(t.ok).bold()),
            Span::styled(" run   ", Style::default().fg(t.subtle)),
            Span::styled("n", Style::default().fg(t.err).bold()),
            Span::styled(" cancel", Style::default().fg(t.subtle)),
        ]),
    ];
    status::modal(frame, t, " run ", lines);
}

/// Ask before handing a non-web link to the system opener. The target is
/// wrapped to the screen; one too long to fit keeps its first and last rows
/// (scheme and host, file name) with `…` between, and the y/n row stays.
pub(super) fn draw_open_confirm(frame: &mut Frame, r: &Reader, target: &str) {
    let t = r.theme();
    let area = frame.area();
    // Border and padding, at least one column for the text.
    let max_w = (area.width.saturating_sub(4) as usize).max(1);
    // Inside the border; the box can't grow past the screen.
    let avail = area.height.saturating_sub(2) as usize;
    let target_style = Style::default().fg(t.accent).bold();
    let keys = Line::from(vec![
        Span::styled("y", Style::default().fg(t.ok).bold()),
        Span::styled(" open   ", Style::default().fg(t.subtle)),
        Span::styled("n", Style::default().fg(t.err).bold()),
        Span::styled(" cancel", Style::default().fg(t.subtle)),
    ]);
    // Rows left for the target beside the question, a blank and the keys.
    let Some(budget) = avail.checked_sub(3).filter(|&b| b > 0) else {
        // Too short for the full prompt: the target on one row, if there's
        // room, and always the keys.
        let mut lines = Vec::new();
        if avail >= 2 {
            lines.push(Line::styled(elide_middle(target, max_w), target_style));
        }
        lines.push(keys);
        status::modal(frame, t, " open ", lines);
        return;
    };
    let mut lines = vec![Line::styled(
        "Open with the system opener?",
        Style::default().fg(t.text),
    )];
    lines.extend(fit_rows(t, target, target_style, max_w, budget));
    lines.extend([Line::default(), keys]);
    status::modal(frame, t, " open ", lines);
}

/// Where the document came from, with the keys that copy it. Long paths
/// wrap to the screen; on a screen too short for them, each keeps its
/// first and last rows with `…` between, and the labels and keys stay.
pub(super) fn draw_location(frame: &mut Frame, r: &Reader, location: &Location) {
    let t = r.theme();
    let area = frame.area();
    // Border and padding, at least one column for the text.
    let max_w = (area.width.saturating_sub(4) as usize).max(1);
    // Inside the border; the box can't grow past the screen.
    let avail = area.height.saturating_sub(2) as usize;
    let label = Style::default().fg(t.muted);
    let value = Style::default().fg(t.accent).bold();
    let (entries, keys): (Vec<Pair>, &[Pair]) = match location {
        Location::File { relative, absolute } => (
            vec![("Relative:", relative), ("Absolute:", absolute)],
            &[("R", " copy relative · "), ("A", " copy absolute · ")],
        ),
        Location::Url(url) => (vec![("URL:", url)], &[("R/A", " copy URL · ")]),
        Location::Unnamed(_) => (Vec::new(), &[]),
    };
    let footer = location_keys(t, keys, max_w);
    let mut lines = Vec::new();
    if let Location::Unnamed(name) = location {
        lines.push(Line::styled(
            format!("({name})"),
            Style::default().fg(t.text),
        ));
        lines.push(Line::styled("no local file", label));
    }
    // Rows left for the values beside their labels and the keys, and the
    // blank row above the keys when there's room for it.
    let spare = avail.saturating_sub(entries.len() + footer.len());
    let blank = spare > entries.len();
    let budgets = share_rows(
        &entries
            .iter()
            .map(|(_, text)| wrap_width(text, max_w).len())
            .collect::<Vec<_>>(),
        spare - usize::from(blank),
    );
    for ((name, text), budget) in entries.iter().zip(budgets) {
        lines.push(Line::styled(name.to_string(), label));
        lines.extend(fit_rows(t, text, value, max_w, budget));
    }
    if blank || entries.is_empty() {
        lines.push(Line::default());
    }
    lines.extend(footer);
    status::modal(frame, t, " File Path ", lines);
}

/// A label and its text, or a key and what it does.
type Pair<'a> = (&'a str, &'a str);

/// The key row of the path popup, or a key per row when it's too narrow
/// for one.
fn location_keys<'a>(t: &Theme, keys: &[Pair<'a>], max_w: usize) -> Vec<Line<'a>> {
    let pairs: Vec<[Span; 2]> = keys
        .iter()
        .copied()
        .chain([("esc", " close")])
        .map(|(key, what)| {
            [
                Span::styled(key, Style::default().fg(t.accent).bold()),
                Span::styled(what, Style::default().fg(t.subtle)),
            ]
        })
        .collect();
    let row: Line = pairs.iter().flatten().cloned().collect();
    if row.width() <= max_w {
        return vec![row];
    }
    // Without the separators.
    pairs
        .into_iter()
        .map(|[key, what]| {
            let what = what.content.trim_end_matches(" · ").to_string();
            Line::from(vec![key, Span::styled(what, Style::default().fg(t.subtle))])
        })
        .collect()
}

/// `total` rows shared between values that want `wants` rows each: all
/// they want when it fits, else an even share, with what one doesn't use
/// going to the others.
fn share_rows(wants: &[usize], total: usize) -> Vec<usize> {
    let mut budgets = vec![0; wants.len()];
    let mut left = total;
    // Smallest first, so the rows a short value leaves go to longer ones.
    let mut order: Vec<usize> = (0..wants.len()).collect();
    order.sort_by_key(|&i| wants[i]);
    for (n, &i) in order.iter().enumerate() {
        let share = left / (wants.len() - n);
        budgets[i] = wants[i].min(share);
        left -= budgets[i];
    }
    budgets
}

/// `text` wrapped to `max_w` in at most `budget` rows. Too many rows keep
/// the first and last (scheme and host, file name) with `…` between; with
/// fewer than three, one row with its middle elided.
fn fit_rows<'a>(
    t: &Theme,
    text: &'a str,
    style: Style,
    max_w: usize,
    budget: usize,
) -> Vec<Line<'a>> {
    let rows = wrap_width(text, max_w);
    if rows.len() <= budget {
        return rows
            .into_iter()
            .map(|row| Line::styled(row, style))
            .collect();
    }
    match budget {
        0 => Vec::new(),
        1 | 2 => vec![Line::styled(elide_middle(text, max_w), style)],
        _ => {
            let head = (budget - 1).div_ceil(2);
            let tail = budget - 1 - head;
            let row = |s: &&'a str| Line::styled(*s, style);
            rows[..head]
                .iter()
                .map(row)
                .chain([Line::styled("…", Style::default().fg(t.subtle))])
                .chain(rows[rows.len() - tail..].iter().map(row))
                .collect()
        }
    }
}

/// `s` cut into rows of at most `max` columns.
fn wrap_width(s: &str, max: usize) -> Vec<&str> {
    let mut rows = Vec::new();
    let mut rest = s;
    while !rest.is_empty() {
        let mut chunk = take_width(rest, max);
        if chunk.is_empty() {
            // A character wider than the box: show it anyway.
            let end = rest.chars().next().map_or(rest.len(), char::len_utf8);
            chunk = &rest[..end];
        }
        rows.push(chunk);
        rest = &rest[chunk.len()..];
    }
    rows
}

/// `s` in at most `max` columns, its middle replaced with `…` if it's too long.
fn elide_middle(s: &str, max: usize) -> String {
    if s.width() <= max {
        return s.to_string();
    }
    let head = take_width(s, max.saturating_sub(1) / 2);
    format!("{head}{}", shorten_left(s, max - head.width()))
}
