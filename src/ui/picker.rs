use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use unicode_width::UnicodeWidthStr;

use super::pane::{self, Side};
use super::status;
use crate::picker::ThemePicker;
use crate::render::theme::{Appearance, Theme};
use crate::text::take_width;

/// The status bar's hint while the picker is open.
pub const HINT: &str = "enter use · esc cancel · </> narrow/widen";

/// Columns of the mark before each name: `✓` on the theme in use.
const MARK_W: usize = 2;
/// Columns of the dark/light/any label.
const KIND_W: usize = 5;
/// The colors in a swatch, a column each.
const SWATCH_W: usize = 7;

/// The theme list in `area`, a pane right of the screen it previews: a
/// header with the position in the list and the `<` and `>` resize
/// markers, a rule, then the themes with the selected one highlighted and
/// kept in view. Each shows its dark/light/any and a swatch of its colors
/// when the pane is wide enough, the swatch going first.
pub fn draw(frame: &mut Frame, t: &Theme, p: &ThemePicker, area: Rect) {
    let info = format!("{}/{}", p.selected + 1, p.entries.len());
    let Some(mut list) = pane::draw(frame, t, area, Side::Right, "Themes", &info) else {
        return;
    };
    // Why the selected theme doesn't load, under the list.
    if let Err(e) = &p.entries[p.selected].theme {
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
    let swatch = MARK_W + name_w + 1 + KIND_W + 1 + SWATCH_W <= width;
    let rows = usize::from(list.height);
    let first = pane::first_row(Some(p.selected), p.entries.len(), rows);
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
                    status::row_style(t, i == p.selected),
                ),
            ];
            if !kind {
                return Line::from(spans);
            }
            match &e.theme {
                Ok(theme) => {
                    let label = match theme.appearance {
                        Appearance::Dark => "dark",
                        Appearance::Light => "light",
                        Appearance::Any => "any",
                    };
                    spans.push(Span::styled(
                        format!(" {label:<KIND_W$}"),
                        Style::default().fg(t.muted),
                    ));
                    if swatch {
                        spans.push(Span::raw(" "));
                        spans.extend(colors(theme));
                    }
                }
                Err(_) => spans.push(Span::styled(" broken", Style::default().fg(t.err))),
            }
            Line::from(spans)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), list);
}

/// A few of the theme's colors side by side.
fn colors(theme: &Theme) -> Vec<Span<'static>> {
    [
        theme.accent,
        theme.heading,
        theme.link,
        theme.code,
        theme.ok,
        theme.warn,
        theme.err,
    ]
    .into_iter()
    .map(|c: Color| Span::styled("█", Style::default().fg(c)))
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
        let buf = term.backend().buffer();
        (0..h)
            .map(|y| (0..w).map(|x| buf[(x, y)].symbol()).collect())
            .collect()
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
        let at = p.selected + 1;
        assert!(rows[0].starts_with("│ Themes"), "{rows:#?}");
        assert!(rows[0].ends_with(&format!("{at}/{n} < >  ")), "{rows:#?}");
        assert!(rows[1].contains("────"), "{rows:#?}");
        assert!(row(&rows, "nord").starts_with("│ ✓ nord"), "{rows:#?}");
        assert!(row(&rows, "dracula").starts_with("│   dracula"));
    }

    #[test]
    fn narrow_panes_drop_the_swatch_then_the_label() {
        let p = ThemePicker::new(None, "nord", DEFAULT_WIDTH);
        // The longest built-in name, its label and the swatch fit in 40.
        let wide = pane(&p, 40, 30);
        assert!(row(&wide, "nord").contains("dark  ███████"), "{wide:#?}");
        assert!(row(&wide, "rose-pine-dawn").contains("light ███████"));
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
}
