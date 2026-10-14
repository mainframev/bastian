mod media;
mod overlays;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

pub use super::MARGIN;
use super::status;
use super::text::highlight;
use crate::reader::Reader;
use media::{draw_hyperlinks, draw_images};
use overlays::{draw_confirm, draw_links, draw_open_confirm, draw_outline};

pub fn draw(frame: &mut Frame, r: &mut Reader) {
    // A blank row above the document (the `Find:` line takes it while
    // searching, with its own blank row below) and one above the status
    // bar, so the text never touches either edge.
    let finding = r.prompt.is_some() || r.search.is_some();
    let [find, body, _, status] = Layout::vertical([
        Constraint::Length(if finding { 2 } else { 1 }),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    if finding {
        draw_find(frame, r, find);
    }

    // The gutter width depends on the line count, which depends on the
    // width, so lay out again if the digit count changed.
    let mut gutter = gutter_width(r, r.rendered().lines.len());
    let max_width = r.max_width;
    let width = |g: u16| body.width.saturating_sub(MARGIN * 2 + g).min(max_width);
    r.layout(width(gutter), body.height);
    let fitted = gutter_width(r, r.rendered().lines.len());
    if fitted != gutter {
        gutter = fitted;
        r.layout(width(gutter), body.height);
    }

    let content = Rect {
        x: body.x + MARGIN + gutter,
        width: width(gutter),
        ..body
    };
    let end = (r.scroll() + content.height as usize).min(r.rendered().lines.len());
    // A copy (cheap: colors and an `Arc`), so `r` can be borrowed mutably
    // for the images below.
    let theme = r.theme().clone();
    let t = &theme;
    let visible: Vec<Line> = (r.scroll()..end)
        .map(|i| {
            let line = &r.rendered().lines[i];
            let Some(s) = &r.search else {
                return line.clone();
            };
            // Matches are in line order: take just this line's.
            let first = s.matches.partition_point(|m| m.line < i);
            let last = s.matches.partition_point(|m| m.line <= i);
            let ranges: Vec<_> = s.matches[first..last]
                .iter()
                .enumerate()
                .map(|(k, m)| {
                    let n = first + k;
                    let style = if n == s.current {
                        Style::default().fg(t.h1_fg).bg(t.current_bg)
                    } else {
                        Style::default().bg(t.match_bg)
                    };
                    (m.start, m.end, style)
                })
                .collect();
            highlight(line, &ranges)
        })
        .collect();
    frame.render_widget(Paragraph::new(visible), content);
    // The focused link: drawn plain (no hyperlink) and highlighted.
    let focused = r
        .link_focus
        .and_then(|i| r.link_stops().get(i).cloned())
        .unwrap_or_default();
    if r.hyperlinks {
        draw_hyperlinks(frame, r, content, end, &focused);
    }
    for link in focused.iter().map(|&i| &r.rendered().links[i]) {
        if (r.scroll()..end).contains(&link.line) {
            let x = content.x + link.col as u16;
            let width = (link.width as u16).min(content.right().saturating_sub(x));
            let y = content.y + (link.line - r.scroll()) as u16;
            frame.buffer_mut().set_style(
                Rect::new(x, y, width, 1),
                Style::default().fg(t.h1_fg).bg(t.current_bg),
            );
        }
    }
    // The focused details block's summary line.
    if let Some(span) = r
        .details_focus
        .and_then(|id| r.rendered().details.iter().find(|d| d.id == id))
        && (r.scroll()..end).contains(&span.line)
    {
        let y = content.y + (span.line - r.scroll()) as u16;
        frame.buffer_mut().set_style(
            Rect::new(content.x, y, content.width, 1),
            Style::default().fg(t.h1_fg).bg(t.current_bg),
        );
    }
    draw_images(frame, r, content, end);
    if r.scrollbar {
        status::scrollbar(
            frame,
            t,
            body,
            r.rendered().lines.len(),
            usize::from(body.height),
            r.scroll(),
        );
    }

    if gutter > 0 {
        let digits = usize::from(gutter - GUTTER_GAP);
        // Source line numbers when the document maps them (code files),
        // otherwise rendered line numbers. The gap to the text is the rest
        // of the gutter.
        let label = |i: usize| match r.rendered().src_line.get(i) {
            Some(Some(n)) => format!("{:>digits$}", n + 1),
            Some(None) => String::new(),
            None => format!("{:>digits$}", i + 1),
        };
        let numbers: Vec<Line> = (r.scroll()..end)
            .map(|i| Line::styled(label(i), Style::default().fg(t.muted)))
            .collect();
        let area = Rect {
            x: body.x + MARGIN,
            width: gutter,
            ..body
        };
        frame.render_widget(Paragraph::new(numbers), area);
    }

    // Focus marker in the left margin, next to the focused block.
    if let Some(range) = r
        .focus
        .and_then(|id| r.rendered().code_blocks.iter().find(|c| c.id == id))
        .map(|c| c.lines.clone())
    {
        let top = range.start.max(r.scroll());
        let bottom = range.end.min(end);
        for line in top..bottom {
            let y = body.y + (line - r.scroll()) as u16;
            let cell = Rect::new(body.x, y, 1, 1);
            frame.render_widget(Span::styled("▌", Style::default().fg(t.accent)), cell);
        }
    }

    let pct = if r.max_scroll() == 0 {
        100
    } else {
        r.scroll() * 100 / r.max_scroll()
    };
    let right = if let Some((i, n)) = r.link_position() {
        format!("link {i}/{n}  {pct:>3}%")
    } else if r.search.is_none() && r.focus.is_some() {
        let (i, n) = r.focus_position().unwrap_or((0, 0));
        format!("block {i}/{n}  {pct:>3}%")
    } else {
        format!("{} lines  {pct:>3}%", r.rendered().lines.len())
    };
    let hint = if r.link_focus.is_some() {
        format!("{}  · enter follow · s/S next/prev", r.link_preview)
    } else if r.details_focus.is_some() {
        format!("{}  enter open/close · d/D next/prev", r.name)
    } else if r.focus.is_some() {
        format!("{}  enter run · R all · x stop · y copy · ? more", r.name)
    } else {
        format!("{}  ctrl-e edit · ? more", r.name)
    };
    let left = status::left(r.prompt.is_some(), r.message(), &hint);
    status::draw(frame, status, r.theme(), left, &right);

    if let Some(ids) = &r.confirm {
        draw_confirm(frame, r, ids);
    } else if let Some(target) = &r.confirm_open {
        draw_open_confirm(frame, r, target);
    } else if let Some(picker) = &r.link_picker {
        draw_links(frame, r, picker);
    } else if r.outline.is_some() {
        draw_outline(frame, r);
    } else if r.help.open {
        let t = r.theme().clone();
        status::help(frame, &t, HELP, &mut r.help.scroll);
    }
}

/// Blank columns between the line numbers and the text.
pub const GUTTER_GAP: u16 = 3;

/// Columns for the line-number gutter: the digits of `lines` plus the gap
/// to the text, or nothing when line numbers are off.
pub fn gutter_width(r: &Reader, lines: usize) -> u16 {
    if !r.line_numbers {
        return 0;
    }
    lines.max(1).to_string().len() as u16 + GUTTER_GAP
}

const HELP: &[status::HelpSection] = &[
    (
        "Navigation",
        &[
            ("j/k  ↓/↑", "scroll"),
            ("ctrl-d/u", "half page"),
            ("space/b", "page down/up"),
            ("g/G", "top/bottom"),
            ("O", "jump to a heading"),
            ("Esc, h", "back"),
        ],
    ),
    ("Search", &[("/", "search"), ("n/N", "next/previous match")]),
    (
        "Code blocks",
        &[
            ("Tab/S-Tab", "focus next/previous block"),
            ("Enter, r", "run focused block"),
            ("R", "run from here to the end"),
            ("x", "stop running blocks"),
            ("o", "show/hide output"),
            ("y", "copy focused block"),
        ],
    ),
    (
        "Links & details",
        &[
            ("L", "list links"),
            ("s/S", "next/previous link"),
            ("d/D", "next/previous details"),
            ("Enter", "follow, fold/unfold"),
        ],
    ),
    (
        "Document",
        &[
            ("ctrl-e", "edit"),
            ("c", "copy whole document"),
            ("#", "toggle line numbers"),
            ("i", "show/hide images"),
        ],
    ),
    (
        "General",
        &[("T", "pick a theme"), ("?", "this help"), ("q", "quit")],
    ),
];

/// `Find:` line above the document, with the match count on the right.
fn draw_find(frame: &mut Frame, r: &Reader, area: Rect) {
    let t = r.theme();
    let area = Rect {
        height: 1,
        ..super::inset(area)
    };
    let count = match &r.search {
        Some(s) if s.matches.is_empty() => "no matches".to_string(),
        Some(s) => format!("{}/{}", s.current + 1, s.matches.len()),
        None => String::new(),
    };
    let [left, right] = Layout::horizontal([
        Constraint::Min(1),
        Constraint::Length(count.chars().count() as u16),
    ])
    .areas(area);
    let query = r.search.as_ref().map_or("", |s| s.query.as_str());
    frame.render_widget(status::find_prompt(t, r.prompt.as_ref(), query), left);
    frame.render_widget(Span::styled(count, Style::default().fg(t.muted)), right);
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::render::theme::Theme;

    fn help_screen(w: u16, h: u16, scroll: &mut usize) -> String {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| status::help(f, &Theme::dark(), HELP, scroll))
            .unwrap();
        let buf = term.backend().buffer();
        (0..h)
            .map(|y| (0..w).map(|x| buf[(x, y)].symbol()).collect::<String>() + "\n")
            .collect()
    }

    #[test]
    fn help_fits_a_standard_terminal() {
        let screen = help_screen(80, 24, &mut 0);
        for (title, _) in HELP {
            assert!(screen.contains(title), "{title} missing:\n{screen}");
        }
        assert!(screen.contains("any key to close"), "{screen}");
    }

    #[test]
    fn short_terminals_scroll_the_help() {
        let screen = help_screen(80, 12, &mut 0);
        assert!(screen.contains("j/k scroll"), "{screen}");
        assert!(!screen.contains("General"), "{screen}");
        let mut scroll = usize::MAX;
        let screen = help_screen(80, 12, &mut scroll);
        assert!(screen.contains("General"), "{screen}");
        assert!(scroll < usize::MAX, "scroll is kept in range");
    }
}
