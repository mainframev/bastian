use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::reader::Reader;
use crate::text::take_width;
use crate::toc;
use crate::ui::pane::{self, Side};
use crate::ui::status;

/// Columns the numbers take: two digits and a space.
const NUMBER_W: usize = 3;

/// The table of contents in `area`, left of the document: a header with
/// the `<` and `>` resize markers, a rule, then the headings with the one
/// being read highlighted and kept in view.
pub(super) fn draw_toc(frame: &mut Frame, r: &Reader, area: Rect) {
    let t = r.theme();
    let Some(list) = pane::draw(frame, t, area, Side::Left, "Table of Contents", "") else {
        return;
    };
    let entries = r.toc_entries();
    let current = toc::current(&entries, r.scroll());
    let numbered = entries
        .iter()
        .find(|e| e.number.is_some())
        // With nothing numbered, every heading indents by depth alone.
        .map_or(usize::MAX, |e| e.depth);
    let rows = usize::from(list.height);
    let first = pane::first_row(current, entries.len(), rows);
    let width = usize::from(list.width);
    let lines: Vec<Line> = entries
        .iter()
        .enumerate()
        .skip(first)
        .take(rows)
        .map(|(i, e)| {
            let selected = current == Some(i);
            let style = status::row_style(t, selected);
            let lead = match e.number {
                Some(n) => Span::styled(
                    format!("{n:02} "),
                    if selected {
                        style
                    } else {
                        Style::default().fg(t.muted)
                    },
                ),
                // Headings above the numbered level (a title, or a book
                // and its part) indent by depth from flush left; deeper
                // ones indent under the text of their section.
                None if e.depth < numbered => Span::raw(" ".repeat(2 * e.depth)),
                None => Span::raw(" ".repeat(NUMBER_W + 2 * (e.depth - numbered))),
            };
            let room = width.saturating_sub(lead.width());
            Line::from(vec![
                lead,
                Span::styled(take_width(&e.text, room).to_string(), style),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), list);
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use crate::doc::parse;
    use crate::reader::{Options, Reader, Source};

    /// The pane's entry rows for `src`, without the border or status bar.
    fn pane(src: &str) -> Vec<String> {
        let mut r = Reader::new(Source::in_memory("t.md", parse(src)), &Options::default());
        let mut term = Terminal::new(TestBackend::new(80, 12)).unwrap();
        term.draw(|f| super::super::draw(f, &mut r)).unwrap();
        r.on_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
        term.draw(|f| super::super::draw(f, &mut r)).unwrap();
        let width = r.fit_toc(80);
        let buf = term.backend().buffer();
        (2..11)
            .map(|y| {
                let row: String = (0..width).map(|x| buf[(x, y)].symbol()).collect();
                row.trim_end_matches([' ', '│']).to_string()
            })
            .filter(|row| !row.is_empty())
            .collect()
    }

    #[test]
    fn headings_above_the_numbered_level_indent_by_depth() {
        assert_eq!(
            pane("# Book\n\n## Part\n\n### One\n\n### Two\n"),
            [" Book", "   Part", " 01 One", " 02 Two"]
        );
        // Nothing is numbered without a level of two or more headings.
        assert_eq!(pane("# Only\n\n## Sub\n"), [" Only", "   Sub"]);
        // A lone title stays flush left.
        assert_eq!(
            pane("# Title\n\n## One\n\n## Two\n"),
            [" Title", " 01 One", " 02 Two"]
        );
    }
}
