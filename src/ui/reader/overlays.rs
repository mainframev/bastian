use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::reader::{HistoryPicker, LinkPicker, Location, Reader};
use crate::render::theme::Theme;
use crate::text::{shorten_left, shorten_right, take_width};
use crate::ui::status;

pub(super) fn draw_links(frame: &mut Frame, over: Rect, r: &Reader, picker: &LinkPicker) {
    let t = r.theme();
    let area = frame.area();
    // Leave room for the border and a margin around the box.
    let rows = area.height.saturating_sub(6).max(1) as usize;
    let max_w = area.width.saturating_sub(10) as usize;
    let shown = picker.shown();
    let lines: Vec<Line> = shown
        .iter()
        .map(|&i| &picker.links[i])
        .enumerate()
        .map(|(row, link)| {
            let selected = row == picker.selected.get();
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
    let at = if shown.is_empty() {
        0
    } else {
        picker.selected.get() + 1
    };
    let count = format!("{at}/{}", shown.len());
    // The title sits in the top border, between its corners.
    let max_title = usize::from(area.width.saturating_sub(2));
    let title = match &picker.prompt {
        Some(prompt) => filter_title(prompt.text(), Some(prompt.cursor()), &count, max_title),
        None if picker.filter.is_empty() => {
            format!(" links {count} · / filter · enter open · esc close ")
        }
        None => filter_title(&picker.filter, None, &count, max_title),
    };
    picker.rows.set(rows);
    status::list_popup(frame, t, over, &title, lines, picker.selected.get(), rows);
}

/// ` /filter · n/m · keys `, in at most `max` columns: the keys go
/// first, then the filter keeps the columns around `cursor`, where a bar
/// marks it while typing.
fn filter_title(filter: &str, cursor: Option<usize>, count: &str, max: usize) -> String {
    const KEYS: &str = " · enter open · esc clear";
    let bar = if cursor.is_some() { "▏" } else { "" };
    let fixed =
        |keys: &str| " /".width() + bar.width() + " · ".width() + count.width() + keys.width() + 1;
    // Room for at least a few characters of the filter, else no keys.
    let keys = if fixed(KEYS) + 8 <= max { KEYS } else { "" };
    let room = max.saturating_sub(fixed(keys));
    let at = cursor.map_or(filter.len(), |c| {
        filter
            .char_indices()
            .nth(c)
            .map_or(filter.len(), |(i, _)| i)
    });
    let (before, after) = filter.split_at(at);
    // Up to a third of the room for the text after the cursor.
    let before = shorten_left(before, room - after.width().min(room / 3));
    let after = shorten_right(after, room.saturating_sub(before.width()));
    format!(" /{before}{bar}{after} · {count}{keys} ")
}

/// The documents opened this session, most recent first, with paths
/// shortened from the left to fit and this one marked.
pub(super) fn draw_history(frame: &mut Frame, over: Rect, r: &Reader, picker: &HistoryPicker) {
    let t = r.theme();
    let area = frame.area();
    // Leave room for the border and a margin around the box.
    let rows = area.height.saturating_sub(6).max(1) as usize;
    let here = "  current";
    let max_w = (area.width.saturating_sub(10) as usize).saturating_sub(2 + here.width());
    let lines: Vec<Line> = picker
        .paths
        .iter()
        .enumerate()
        .map(|(i, path)| {
            let selected = i == picker.selected.get();
            let shown = shorten_left(&crate::paths::tilde(path), max_w);
            let mut spans = vec![Span::styled(shown, status::row_style(t, selected))];
            if picker.current == Some(i) {
                spans.push(Span::styled(here, Style::default().fg(t.muted)));
            }
            Line::from(spans)
        })
        .collect();
    let title = format!(
        " history {}/{} · enter open · esc close ",
        picker.selected.get() + 1,
        picker.paths.len()
    );
    status::list_popup(frame, t, over, &title, lines, picker.selected.get(), rows);
}

pub(super) fn draw_confirm(frame: &mut Frame, over: Rect, r: &Reader, ids: &[usize]) {
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
        yes_no_row(t, "run"),
    ];
    status::modal(frame, t, over, " run ", lines);
}

/// The `y verb   n cancel` row of a yes/no popup.
fn yes_no_row(t: &Theme, verb: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled("y", Style::default().fg(t.ok).bold()),
        Span::styled(format!(" {verb}   "), Style::default().fg(t.subtle)),
        Span::styled("n", Style::default().fg(t.err).bold()),
        Span::styled(" cancel", Style::default().fg(t.subtle)),
    ])
}

/// Ask before handing a non-web link to the system opener. The target is
/// wrapped to the screen; one too long to fit keeps its first and last rows
/// (scheme and host, file name) with `…` between, and the y/n row stays.
pub(super) fn draw_open_confirm(frame: &mut Frame, over: Rect, r: &Reader, target: &str) {
    let t = r.theme();
    let area = frame.area();
    // Border and padding, at least one column for the text.
    let max_w = (area.width.saturating_sub(4) as usize).max(1);
    // Inside the border; the box can't grow past the screen.
    let avail = area.height.saturating_sub(2) as usize;
    let target_style = Style::default().fg(t.accent).bold();
    let keys = yes_no_row(t, "open");
    // Rows left for the target beside the question, a blank and the keys.
    let Some(budget) = avail.checked_sub(3).filter(|&b| b > 0) else {
        // Too short for the full prompt: the target on one row, if there's
        // room, and always the keys.
        let mut lines = Vec::new();
        if avail >= 2 {
            lines.push(Line::styled(elide_middle(target, max_w), target_style));
        }
        lines.push(keys);
        status::modal(frame, t, over, " open ", lines);
        return;
    };
    let mut lines = vec![Line::styled(
        "Open with the system opener?",
        Style::default().fg(t.text),
    )];
    lines.extend(fit_rows(t, target, target_style, max_w, budget));
    lines.extend([Line::default(), keys]);
    status::modal(frame, t, over, " open ", lines);
}

/// Where the document came from, with the keys that copy it. Long paths
/// wrap to the screen; on a screen too short for them, each keeps its
/// first and last rows with `…` between, and the labels and keys stay.
pub(super) fn draw_location(frame: &mut Frame, over: Rect, r: &Reader, location: &Location) {
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
    status::modal(frame, t, over, " File Path ", lines);
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

#[cfg(test)]
mod tests {
    use unicode_width::UnicodeWidthStr;

    use super::filter_title;

    #[test]
    fn long_filters_keep_the_cursor_and_count_in_view() {
        let filter = "x".repeat(80);
        let title = filter_title(&filter, Some(80), "1/3", 78);
        assert!(title.width() <= 78, "{title}");
        assert!(title.contains("x▏ · 1/3"), "{title}");
        assert!(title.starts_with(" /…"), "{title}");
        // Typing mid-filter keeps text on both sides of the bar.
        let title = filter_title(&filter, Some(40), "1/3", 40);
        assert!(title.width() <= 40, "{title}");
        assert!(title.contains("x▏x"), "{title}");
        assert!(
            title.starts_with(" /…") && title.contains("x… · 1/3"),
            "{title}"
        );
        // Short filters show whole, with the keys.
        let title = filter_title("guide", Some(5), "2/9", 78);
        assert_eq!(title, " /guide▏ · 2/9 · enter open · esc clear ");
    }
}
