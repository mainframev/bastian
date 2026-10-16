mod rows;

use jiff::tz::TimeZone;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::inset;
use super::logo;
use super::status;
use crate::browser::Browser;
use crate::config::{ListLayout, Selector};
use crate::picker::ThemePicker;
use crate::render::theme::Theme;
use crate::text::shorten_left;
use rows::{card_lines, compact_line, page_indicator, spinner};

const HELP: &[status::HelpSection] = &[
    (
        "Navigation",
        &[
            ("j/k  ↓/↑", "move (ctrl-j/k also while filtering)"),
            ("h/l  ←/→", "previous/next page"),
            ("ctrl-d/u", "half page"),
            ("ctrl-f/b", "full page"),
            ("g/G", "first/last"),
        ],
    ),
    (
        "Files",
        &[
            ("/", "filter"),
            ("Esc", "clear filter"),
            ("Enter", "open"),
            ("e", "edit in $VISUAL/$EDITOR"),
            ("r", "rescan for files"),
        ],
    ),
    ("General", &[("T", "pick a theme"), ("q", "quit")]),
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

/// The file list, with the theme picker on the right of it when open.
pub fn draw(
    frame: &mut Frame,
    b: &mut Browser,
    t: &Theme,
    look: Look,
    mut picker: Option<&mut ThemePicker>,
) {
    super::paint_page(frame, t);
    let show_logo = look.logo;
    // The theme picker, when open, takes the right of everything but the
    // status bar, as in the reader, and the list narrows beside it.
    let [screen, status] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(frame.area());
    let (side, docked) = super::picker::dock(picker.as_deref_mut(), screen.width);
    let [main, _] =
        Layout::horizontal([Constraint::Min(1), Constraint::Length(docked)]).areas(screen);
    // Layout: the logo (or the `Find:` line while filtering; just that
    // line's row without the logo), the document count, then the list.
    // The page indicator goes under the list (each entry already ends with
    // a blank row) with a blank row between it and the status bar.
    let [_, top, _, count_row, _, list, pager_row, _] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(if show_logo { logo::HEIGHT } else { 1 }),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(main);

    let finding = b.prompt.is_some() || !b.filter.is_empty();
    if finding {
        let find = status::find_prompt(t, b.prompt.as_ref(), &b.filter);
        frame.render_widget(find, inset(top));
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
    for (row, &idx) in b
        .visible
        .iter()
        .enumerate()
        .skip(b.offset())
        .take(b.page_len())
    {
        let e = &b.entries[idx];
        let selected = row == b.selected.get();
        let start = lines.len();
        let date = look
            .date_format
            .zip(e.date(b.scan.sort))
            .map(|(f, time)| crate::config::format_time(time, &tz, f))
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
        status::scrollbar(frame, t, list, b.visible.len(), b.page_len(), b.offset());
    }

    let hints: &[(&str, &str)] = if b.filter.is_empty() {
        &[
            ("j/k", "move"),
            ("enter", "open"),
            ("/", "filter"),
            ("e", "edit"),
            ("T", "theme"),
            ("q", "quit"),
            ("?", "help"),
        ]
    } else {
        &[
            ("j/k", "move"),
            ("enter", "open"),
            ("/", "edit filter"),
            ("esc", "clear"),
        ]
    };
    // The picker's keys, typing or a message take the place of the hints.
    let left = if picker.is_some() {
        status::left(
            b.prompt.is_some(),
            b.message.as_deref(),
            super::picker::HINT,
        )
    } else if b.prompt.is_none() && b.message.is_none() {
        status::Left::Hints(hints)
    } else {
        status::left(b.prompt.is_some(), b.message.as_deref(), "")
    };
    let right = if b.visible.is_empty() {
        String::new()
    } else {
        format!("{}/{}", b.selected.get() + 1, b.visible.len())
    };
    status::draw(frame, status, t, left, &right);

    super::picker::draw_docked(frame, t, picker.as_deref(), screen, side);
    if b.help.open {
        let over = Rect {
            height: status.y - frame.area().y,
            ..frame.area()
        };
        status::help(frame, t, over, HELP, &mut b.help.scroll);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::rows;
    use std::time::Duration;

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
        styled_screen_with(files, look, h, None)
    }

    /// Like `styled_screen`, with the theme picker open or not.
    fn styled_screen_with(
        files: usize,
        look: Look,
        h: u16,
        picker: Option<&mut ThemePicker>,
    ) -> ratatui::buffer::Buffer {
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
        crate::browser::tests::wait_for_scan(&mut b);
        let mut terminal = Terminal::new(TestBackend::new(50, h)).unwrap();
        let t = Theme::dark();
        terminal
            .draw(|f| draw(f, &mut b, &t, look, picker))
            .unwrap();
        terminal.backend().buffer().clone()
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
    fn status_bar_hints_keys_and_keeps_help_when_narrow() {
        let t = Theme::dark();
        let buf = styled_screen(0, look(Selector::Bar, ListLayout::Cards), 30);
        let rows = rows(&buf);
        let bar = rows.last().unwrap();
        // 50 columns: some hints drop, `? help` stays last.
        assert!(bar.contains("j/k move · enter open"), "{bar}");
        assert!(bar.contains("· ? help"), "{bar}");
        assert!(!bar.contains("quit"), "{bar}");
        let x = bar[..bar.find("enter").unwrap()].chars().count() as u16;
        let y = buf.area.height - 1;
        assert_eq!(buf[(x, y)].fg, t.accent, "keys in the accent");
        assert_eq!(buf[(x + 6, y)].fg, t.muted, "what they do muted");
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
    fn theme_picker_narrows_the_list_beside_it() {
        let mut p = ThemePicker::new(None, "nord", crate::picker::DEFAULT_WIDTH);
        let look = look(Selector::Bar, ListLayout::Cards);
        let rows = rows(&styled_screen_with(0, look, 20, Some(&mut p)));
        let text = rows.join("\n");
        // Half of the 50 columns, above the status bar.
        let pane = |r: &String| r.chars().skip(25).collect::<String>();
        assert!(pane(&rows[0]).starts_with("│ Themes"), "{text}");
        assert!(
            rows[..19].iter().all(|r| pane(r).starts_with('│')),
            "{text}"
        );
        assert!(rows[19].contains("enter use"), "{text}");
        // The list goes on left of it.
        let guide = row_of(&rows, "Guide");
        assert!(rows[guide + 1].contains("docs/guide.md"), "{text}");
        assert!(rows[guide].find("Guide") < rows[guide].find('│'), "{text}");
    }

    #[test]
    fn the_theme_paints_the_page_behind_the_list() {
        let unpainted = |buf: &ratatui::buffer::Buffer, to: u16| {
            (0..buf.area.height)
                .flat_map(|y| (0..to).map(move |x| (x, y)))
                .filter(|&(x, y)| buf[(x, y)].bg == ratatui::style::Color::Reset)
                .count()
        };
        // The logo, the cards, the scrollbar and the status bar.
        let look = Look {
            logo: true,
            ..look(Selector::Bar, ListLayout::Cards)
        };
        let buf = styled_screen(30, look, 20);
        assert_eq!(unpainted(&buf, 50), 0);
        // Beside the theme picker too, up to its border.
        let mut p = ThemePicker::new(None, "nord", crate::picker::DEFAULT_WIDTH);
        let buf = styled_screen_with(0, look, 20, Some(&mut p));
        assert_eq!(unpainted(&buf, 27), 0);
    }
}
