use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use unicode_width::UnicodeWidthStr;

use super::pane::{self, Side};
use super::status;
use crate::picker::ThemePicker;
use crate::render::theme::Theme;
use crate::text::take_width;

/// The status bar's hint while the picker is open.
pub const HINT: &str = "enter use · esc cancel · </> narrow/widen";

/// Columns of the mark before each name: `✓` on the theme in use.
const MARK_W: usize = 2;
/// Columns of the dark/light/any label.
const KIND_W: usize = 5;
/// The most and fewest colors in a swatch, a column each.
const SWATCH_MAX: usize = 7;
const SWATCH_MIN: usize = 3;

/// Columns the open picker takes on a screen `width` columns wide, and
/// how many of them the view beside it gives up: none on a screen too
/// narrow for both, where the picker covers it.
pub(crate) fn dock(picker: Option<&mut ThemePicker>, width: u16) -> (u16, u16) {
    let side = picker.map_or(0, |p| p.fit(width));
    (side, if side < width { side } else { 0 })
}

/// The open picker in the right `side` columns of `screen`.
pub(crate) fn draw_docked(
    frame: &mut Frame,
    t: &Theme,
    picker: Option<&ThemePicker>,
    screen: Rect,
    side: u16,
) {
    if let Some(p) = picker {
        let area = Rect {
            x: screen.right() - side,
            width: side,
            ..screen
        };
        draw(frame, t, p, area);
    }
}

/// The theme list in `area`, a pane right of the screen it previews: a
/// header with the position in the list and the `<` and `>` resize
/// markers, a rule, then the themes with the selected one highlighted and
/// kept in view. Each shows its dark/light/any and a swatch of as many of
/// its colors as fit, on its own surface so they read as they will; the
/// swatch goes first when the pane narrows.
pub fn draw(frame: &mut Frame, t: &Theme, p: &ThemePicker, area: Rect) {
    let info = format!("{}/{}", p.selected.get() + 1, p.entries.len());
    let Some(mut list) = pane::draw(frame, t, area, Side::Right, "Themes", &info) else {
        return;
    };
    // Why the selected theme doesn't load, under the list.
    if let Err(e) = &p.entries[p.selected.get()].theme {
        let rows = (list.height / 3).clamp(1, 4);
        list.height -= rows;
        let below = Rect {
            y: list.bottom(),
            height: rows,
            ..list
        };
        let error = Paragraph::new(e.as_str())
            .style(Style::default().fg(t.err))
            .wrap(Wrap { trim: true });
        frame.render_widget(error, below);
    }

    let width = usize::from(list.width);
    let name_w = p
        .entries
        .iter()
        .map(|e| e.name.width())
        .max()
        .unwrap_or(0)
        .min(width.saturating_sub(MARK_W));
    let kind = MARK_W + name_w + 1 + KIND_W <= width;
    // A space, then the colors with a space of the theme's surface on
    // either side.
    let swatch = width
        .saturating_sub(MARK_W + name_w + 1 + KIND_W + 1 + 2)
        .min(SWATCH_MAX);
    let swatch = if swatch >= SWATCH_MIN { swatch } else { 0 };
    let rows = usize::from(list.height);
    let first = pane::first_row(Some(p.selected.get()), p.entries.len(), rows);
    let lines: Vec<Line> = p
        .entries
        .iter()
        .enumerate()
        .skip(first)
        .take(rows)
        .map(|(i, e)| {
            let mark = if p.current == Some(i) { "✓ " } else { "  " };
            let name = take_width(&e.name, name_w);
            let mut spans = vec![
                Span::styled(mark, Style::default().fg(t.ok)),
                Span::styled(
                    format!("{name}{}", " ".repeat(name_w - name.width())),
                    status::row_style(t, i == p.selected.get()),
                ),
            ];
            if !kind {
                return Line::from(spans);
            }
            match &e.theme {
                Ok(theme) => {
                    let label = theme.appearance.label();
                    spans.push(Span::styled(
                        format!(" {label:<KIND_W$}"),
                        Style::default().fg(t.muted),
                    ));
                    if swatch > 0 {
                        spans.push(Span::raw(" "));
                        spans.extend(colors(theme, swatch));
                    }
                }
                Err(_) => spans.push(Span::styled(" broken", Style::default().fg(t.err))),
            }
            Line::from(spans)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), list);
}

/// The first `n` of the theme's main colors side by side, on its surface:
/// drawn on the picker's own background, a light theme's colors wash out
/// on a dark one and the other way round.
fn colors(theme: &Theme, n: usize) -> Vec<Span<'static>> {
    let surface = Style::default().bg(theme.surface);
    let pad = || Span::styled(" ", surface);
    let blocks = theme
        .swatch()
        .into_iter()
        .take(n)
        .map(|c| Span::styled("█", surface.fg(c)));
    std::iter::once(pad())
        .chain(blocks)
        .chain(std::iter::once(pad()))
        .collect()
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::*;
    use crate::picker::DEFAULT_WIDTH;

    /// `p` drawn in a `w`×`h` pane, row by row.
    fn pane(p: &ThemePicker, w: u16, h: u16) -> Vec<String> {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| draw(f, &Theme::dark(), p, f.area())).unwrap();
        crate::ui::rows(term.backend().buffer())
    }

    fn row<'a>(rows: &'a [String], name: &str) -> &'a str {
        rows.iter()
            .find(|r| r.contains(&format!(" {name} ")))
            .unwrap_or_else(|| panic!("no {name}:\n{}", rows.join("\n")))
    }

    #[test]
    fn header_counts_and_marks_the_current_theme() {
        let p = ThemePicker::new(None, "nord", DEFAULT_WIDTH);
        let rows = pane(&p, DEFAULT_WIDTH, 30);
        let n = p.entries.len();
        let at = p.selected.get() + 1;
        assert!(rows[0].starts_with("│ Themes"), "{rows:#?}");
        assert!(rows[0].ends_with(&format!("{at}/{n} < >  ")), "{rows:#?}");
        assert!(rows[1].contains("────"), "{rows:#?}");
        assert!(row(&rows, "nord").starts_with("│ ✓ nord"), "{rows:#?}");
        assert!(row(&rows, "dracula").starts_with("│   dracula"));
    }

    #[test]
    fn narrow_panes_shrink_the_swatch_then_drop_it_and_the_label() {
        let p = ThemePicker::new(None, "nord", DEFAULT_WIDTH);
        // The longest built-in name, its label and the full swatch fit in 42.
        let wide = pane(&p, 42, 30);
        assert!(row(&wide, "nord").contains("dark   ███████ "), "{wide:#?}");
        assert!(row(&wide, "rose-pine-dawn").contains("light  ███████ "));
        // Fewer colors as the pane narrows.
        let mid = pane(&p, 37, 30);
        assert!(row(&mid, "nord").contains("dark   ███ "), "{mid:#?}");
        assert!(!row(&mid, "nord").contains("████"), "{mid:#?}");
        let default = pane(&p, DEFAULT_WIDTH, 30);
        assert!(row(&default, "nord").trim_end().ends_with("dark"));
        assert!(!default.join("").contains('█'), "{default:#?}");
        let narrow = pane(&p, 20, 30);
        assert_eq!(row(&narrow, "nord").trim_end(), "│ ✓ nord");
        // Names are cut to fit, the count left out when it doesn't.
        assert!(narrow.iter().any(|r| r.contains("catppuccin-mac")));
        assert_eq!(narrow[0].trim_end(), "│ Themes       < >");
    }

    #[test]
    fn the_selection_stays_in_view() {
        let mut p = ThemePicker::new(None, "nord", DEFAULT_WIDTH);
        p.on_key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE));
        let rows = pane(&p, DEFAULT_WIDTH, 10);
        let last = &p.entries[p.entries.len() - 1].name;
        assert!(rows.iter().any(|r| r.contains(last.as_str())), "{rows:#?}");
        p.on_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
        let rows = pane(&p, DEFAULT_WIDTH, 10);
        assert!(rows[2].contains(&p.entries[0].name), "{rows:#?}");
    }

    #[test]
    fn the_count_keeps_the_title_in_the_accent() {
        let t = Theme::dark();
        let p = ThemePicker::new(None, "nord", DEFAULT_WIDTH);
        let mut term = Terminal::new(TestBackend::new(DEFAULT_WIDTH, 30)).unwrap();
        term.draw(|f| draw(f, &t, &p, f.area())).unwrap();
        let buf = term.backend().buffer();
        let row = &crate::ui::rows(buf)[0];
        let title = row.find("Themes").unwrap() as u16;
        assert_eq!(buf[(title, 0)].fg, t.accent);
        let count = row.find(&format!("/{}", p.entries.len())).unwrap() as u16;
        assert_eq!(buf[(count, 0)].fg, t.muted);
    }

    #[test]
    fn the_swatch_sits_on_the_themes_own_surface() {
        let p = ThemePicker::new(None, "nord", DEFAULT_WIDTH);
        let i = p
            .entries
            .iter()
            .position(|e| e.name == "rose-pine-dawn")
            .unwrap();
        let light = p.entries[i].theme.as_ref().unwrap();
        let mut term = Terminal::new(TestBackend::new(42, 40)).unwrap();
        term.draw(|f| draw(f, &Theme::dark(), &p, f.area()))
            .unwrap();
        let buf = term.backend().buffer();
        let y = crate::ui::rows(buf)
            .iter()
            .position(|row| row.contains(" rose-pine-dawn "))
            .unwrap() as u16;
        let x = (0..42).find(|&x| buf[(x, y)].symbol() == "█").unwrap();
        assert_eq!(buf[(x, y)].bg, light.surface);
        assert_eq!(buf[(x - 1, y)].bg, light.surface);
        assert_eq!(buf[(x, y)].fg, light.accent);
    }
}
