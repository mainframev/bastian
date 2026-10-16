mod media;
mod overlays;
mod toc;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

pub use super::MARGIN;
use super::status;
use super::text::highlight;
use crate::picker::ThemePicker;
use crate::reader::{Overlay, Reader};
use media::{draw_hyperlinks, draw_images, link_rect};
use overlays::{draw_confirm, draw_history, draw_links, draw_location, draw_open_confirm};
use toc::draw_toc;

pub fn draw(frame: &mut Frame, r: &mut Reader) {
    draw_with(frame, r, None);
}

/// `draw`, with the theme picker beside the document when it's open.
pub fn draw_with(frame: &mut Frame, r: &mut Reader, mut picker: Option<&mut ThemePicker>) {
    super::paint_page(frame, r.theme());
    // The table of contents, when open, takes the left of everything but
    // the status bar, and the theme picker the right; the document lays
    // out in what's left, so the picker previews it as it will read. On a
    // screen too narrow for the document beside it, the picker covers it.
    let [top, status] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(frame.area());
    let (side, docked) = super::picker::dock(picker.as_deref_mut(), top.width);
    let pane = r.fit_toc_beside(top.width, docked);
    let [toc, main, _] = Layout::horizontal([
        Constraint::Length(pane),
        Constraint::Min(1),
        Constraint::Length(docked),
    ])
    .areas(top);
    // A blank row above the document (the `Find:` line takes it while
    // searching, with its own blank row below) and one above the status
    // bar, so the text never touches either edge.
    let finding = r.prompt().is_some() || r.search.is_some() || r.goto().is_some();
    let [find, body, _] = Layout::vertical([
        Constraint::Length(if finding { 2 } else { 1 }),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(main);
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
    // A hyperlink's first cell holds its whole label, which the terminal
    // prints over whatever a popup put in the cells after it.
    if r.hyperlinks && !r.popup_open() {
        draw_hyperlinks(frame, r, content, end, &focused);
    }
    for link in focused.iter().map(|&i| &r.rendered().links[i]) {
        if (r.scroll()..end).contains(&link.line)
            && let Some(area) = link_rect(content, link, r.scroll())
        {
            frame
                .buffer_mut()
                .set_style(area, Style::default().fg(t.h1_fg).bg(t.current_bg));
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
    // The line `:` went to.
    if let Some(rows) = r.goto_mark() {
        for line in rows.start.max(r.scroll())..rows.end.min(end) {
            let y = content.y + (line - r.scroll()) as u16;
            frame.buffer_mut().set_style(
                Rect::new(content.x, y, content.width, 1),
                Style::default().fg(t.h1_fg).bg(t.current_bg),
            );
        }
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

    // The focused block's frame in the accent color, or a marker in the
    // left margin next to a block without one.
    if let Some(block) = r
        .focus
        .and_then(|id| r.rendered().code_blocks.iter().find(|c| c.id == id))
    {
        let accent = Style::default().fg(t.accent);
        let range = &block.lines;
        for line in range.start.max(r.scroll())..range.end.min(end) {
            let y = body.y + (line - r.scroll()) as u16;
            let Some(cols) = &block.frame else {
                frame.render_widget(Span::styled("▌", accent), Rect::new(body.x, y, 1, 1));
                continue;
            };
            let cells = if line == range.start || line + 1 == range.end {
                vec![(cols.start, cols.len())]
            } else {
                vec![(cols.start, 1), (cols.end - 1, 1)]
            };
            for (col, width) in cells {
                let Some(x) = media::column(content, col) else {
                    continue;
                };
                let width = u16::try_from(width)
                    .unwrap_or(u16::MAX)
                    .min(content.right() - x);
                frame
                    .buffer_mut()
                    .set_style(Rect::new(x, y, width, 1), accent);
            }
        }
    }

    // After the layout, so its entries point at the lines just laid out.
    if pane > 0 {
        draw_toc(frame, r, toc);
    }
    super::picker::draw_docked(frame, r.theme(), picker.as_deref(), top, side);

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
    let hint = if picker.is_some() {
        super::picker::HINT.to_string()
    } else if r.link_focus.is_some() {
        format!("{}  · enter follow · s/S next/prev", r.link_preview)
    } else if r.details_focus.is_some() {
        format!("{}  enter open/close · d/D next/prev", r.name)
    } else if r.focus.is_some() {
        format!("{}  enter run · R all · x stop · y copy · ? more", r.name)
    } else {
        format!("{}  ctrl-e edit · ? more", r.name)
    };
    // The picker owns the keys while it's open, so a message (from a
    // reload, say) can't be cleared and mustn't hide its hint.
    let left = if picker.is_some() {
        status::Left::Text(&hint)
    } else {
        status::left(
            r.prompt().is_some() || r.goto().is_some(),
            r.message(),
            &hint,
        )
    };
    status::draw(frame, status, r.theme(), left, &right);

    // Popups center on the document, beside the table of contents.
    match &r.overlay {
        Some(Overlay::RunConfirm(ids)) => draw_confirm(frame, main, r, ids),
        Some(Overlay::OpenConfirm(target)) => draw_open_confirm(frame, main, r, target),
        Some(Overlay::Links(picker)) => draw_links(frame, main, r, picker),
        Some(Overlay::History(picker)) => draw_history(frame, main, r, picker),
        Some(Overlay::Location(location)) => draw_location(frame, main, r, location),
        Some(Overlay::Help(_) | Overlay::Prompt(_) | Overlay::GoTo(_)) | None => {}
    }
    // The help keeps its scroll in range while drawing.
    let t = r.theme().clone();
    if let Some(Overlay::Help(help)) = &mut r.overlay {
        status::help(frame, &t, main, HELP, &mut help.scroll);
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
            ("j/k  ↓/↑", "scroll; ToC: heading"),
            ("ctrl-d/u", "half page"),
            ("space/b", "page down/up"),
            ("g/G", "top/bottom"),
            (":, ctrl-g", "go to line"),
            ("J/K", "next/previous heading"),
            ("h, Esc", "back; Esc clears first"),
        ],
    ),
    ("Search", &[("/", "search"), ("n/N", "next/previous match")]),
    (
        "Table of contents",
        &[
            ("t, </>", "show/hide, narrow/widen"),
            ("1-9, 0", "jump; again: walk section"),
        ],
    ),
    ("General", &[("T, H", "themes, history"), ("q", "quit")]),
    (
        "Code blocks",
        &[
            ("Tab/S-Tab", "focus next/previous block"),
            ("Enter, r", "run focused block"),
            ("R", "run from here to the end"),
            ("x", "stop running blocks"),
            ("o, v", "output, diagram/source"),
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
            ("p", "file path"),
            ("#", "toggle line numbers"),
            ("i", "show/hide images"),
        ],
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
    if let Some(goto) = r.goto() {
        let line = status::prompt_line(t, "Line: ", goto.text(), Some(goto.cursor()));
        frame.render_widget(line, area);
        return;
    }
    let query = r.search.as_ref().map_or("", |s| s.query.as_str());
    frame.render_widget(status::find_prompt(t, r.prompt(), query), left);
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
        term.draw(|f| status::help(f, &Theme::dark(), f.area(), HELP, scroll))
            .unwrap();
        crate::ui::rows(term.backend().buffer())
            .into_iter()
            .map(|row| row + "\n")
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

    #[test]
    fn the_picker_hint_shows_over_a_message() {
        use crate::picker::{DEFAULT_WIDTH, ThemePicker};
        use crate::reader::{Options, Source};

        let doc = crate::doc::parse("# A\n\ntext\n");
        let mut r = Reader::new(Source::in_memory("t", doc), &Options::default());
        r.set_message("reloaded");
        let mut p = ThemePicker::new(None, "nord", DEFAULT_WIDTH);
        let mut term = Terminal::new(TestBackend::new(100, 20)).unwrap();
        term.draw(|f| draw_with(f, &mut r, Some(&mut p))).unwrap();
        let buf = term.backend().buffer();
        let status = &crate::ui::rows(buf)[19];
        assert!(status.contains(super::super::picker::HINT), "{status}");
        assert!(!status.contains("reloaded"), "{status}");
    }

    /// A table wide enough to push a link to the edge of u16 still draws,
    /// and the link, off screen past the right edge, draws nothing.
    #[test]
    fn links_far_past_the_right_edge_are_skipped() {
        use crate::reader::{Options, Source};

        // Each `a` column takes 6 cells; the last, right-aligned column is
        // as wide as its header, which puts the link where adding the left
        // margin to its column overflowed u16.
        let cells = 10_922;
        let row = |cell: &str| format!("|{} {cell} |\n", " a |".repeat(cells));
        let src = format!(
            "{}|{}---:|\n{}",
            row("hhhhhh"),
            "---|".repeat(cells),
            row("[link](https://x.dev)")
        );
        let doc = crate::doc::parse(&src);
        let mut r = Reader::new(Source::in_memory("t", doc), &Options::default());
        let mut term = Terminal::new(TestBackend::new(80, 20)).unwrap();
        term.draw(|f| draw(f, &mut r)).unwrap();
        let col = r.rendered().links.first().map(|l| l.col);
        assert!(col.is_some_and(|c| c > 65_500), "{col:?}");
        // The document's rows, without the status bar.
        let page = |term: &Terminal<TestBackend>| {
            let buf = term.backend().buffer();
            let area = Rect::new(0, 0, buf.area.width, buf.area.height - 1);
            area.positions().map(|p| buf[p].clone()).collect::<Vec<_>>()
        };
        let rows = crate::ui::rows(term.backend().buffer());
        assert!(!rows.concat().contains("link"), "{rows:#?}");
        let plain = page(&term);
        // Focused, it would be highlighted: still nothing on screen.
        r.link_focus = Some(0);
        term.draw(|f| draw(f, &mut r)).unwrap();
        assert_eq!(page(&term), plain);
    }
}
