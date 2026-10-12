use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::links::runs_by_line;
use super::wrap::{words, wrap};
use super::{Out, Renderer};
use crate::doc::{Align, Inline, plain_text};

impl Renderer<'_> {
    pub(super) fn table(
        &self,
        align: &[Align],
        header: &[Vec<Inline>],
        rows: &[Vec<Vec<Inline>>],
        width: usize,
    ) -> Out {
        let t = self.theme;
        let cols = header
            .len()
            .max(rows.iter().map(Vec::len).max().unwrap_or(0));
        if cols == 0 {
            return Out::default();
        }
        let all = || std::iter::once(header).chain(rows.iter().map(Vec::as_slice));
        let text =
            |row: &[Vec<Inline>], i: usize| row.get(i).map(|c| plain_text(c)).unwrap_or_default();

        let mut natural = vec![3usize; cols];
        let mut min = vec![3usize; cols];
        for row in all() {
            for i in 0..cols {
                let s = text(row, i);
                let line = s.lines().map(UnicodeWidthStr::width).max().unwrap_or(0);
                let word = words(&s).map(UnicodeWidthStr::width).max().unwrap_or(0);
                natural[i] = natural[i].max(line);
                min[i] = min[i].max(word.min(MAX_MIN_COLUMN));
            }
        }
        // One space of margin on each side plus " │ " between columns.
        let budget = width.saturating_sub(2 + 3 * (cols - 1));
        let widths = column_widths(&natural, &min, budget);

        let sep = Style::default().fg(t.muted);
        let row_lines = |row: &[Vec<Inline>], base: Style| -> Vec<Line<'static>> {
            let mut cell_links = Vec::new();
            let cells: Vec<Vec<Line<'static>>> = widths
                .iter()
                .enumerate()
                .map(|(i, &w)| {
                    let inlines = row.get(i).map_or(&[][..], Vec::as_slice);
                    let before = self.link_runs.borrow().len();
                    let lines = wrap(&self.runs(inlines, base), w);
                    let runs = self.link_runs.borrow_mut().split_off(before);
                    cell_links.push(runs_by_line(&lines, &runs, t));
                    lines
                })
                .collect();
            let height = cells.iter().map(Vec::len).max().unwrap_or(1);
            // Cells are styled one after another but drawn side by side, a
            // line at a time: list their link runs in that order.
            let mut link_runs = self.link_runs.borrow_mut();
            for k in 0..height {
                for runs in &mut cell_links {
                    if let Some(line) = runs.get_mut(k) {
                        link_runs.append(line);
                    }
                }
            }
            drop(link_runs);
            (0..height)
                .map(|k| {
                    let mut spans = vec![Span::raw(" ")];
                    for (i, &w) in widths.iter().enumerate() {
                        if i > 0 {
                            spans.push(Span::styled(" │ ", sep));
                        }
                        let line = cells[i].get(k).cloned().unwrap_or_default();
                        let a = align.get(i).copied().unwrap_or_default();
                        spans.extend(aligned(line, w, a));
                    }
                    Line::from(spans)
                })
                .collect()
        };

        let mut lines = row_lines(header, Style::default().fg(t.heading).bold());
        let rule: Vec<String> = widths.iter().map(|w| "─".repeat(*w)).collect();
        lines.push(Line::styled(format!("─{}─", rule.join("─┼─")), sep));
        for row in rows {
            lines.extend(row_lines(row, Style::default().fg(t.text)));
        }
        Out::from(lines)
    }
}

/// A column's minimum width never exceeds this, so one long word can't
/// starve the other columns; longer words are split.
pub(super) const MAX_MIN_COLUMN: usize = 12;

/// Fit columns with `natural` widths into `budget` columns. Columns that
/// fit within an even share keep their natural width; the rest split what's
/// left in proportion to their natural width, but not below `min` unless
/// the table can't fit otherwise.
pub(super) fn column_widths(natural: &[usize], min: &[usize], budget: usize) -> Vec<usize> {
    if natural.iter().sum::<usize>() <= budget {
        return natural.to_vec();
    }
    let mut widths = vec![0; natural.len()];
    let mut open: Vec<usize> = (0..natural.len()).collect();
    let mut left = budget;
    loop {
        let share = left / open.len();
        let (fixed, rest): (Vec<usize>, Vec<usize>) =
            open.iter().partition(|&&i| natural[i] <= share);
        if fixed.is_empty() {
            break;
        }
        for i in fixed {
            widths[i] = natural[i];
            left -= natural[i];
        }
        open = rest;
    }
    let total: usize = open.iter().map(|&i| natural[i]).sum();
    for &i in &open {
        widths[i] = left * natural[i] / total;
    }
    // Hand out the rounding remainder, widest columns first.
    let rem = left - open.iter().map(|&i| widths[i]).sum::<usize>();
    open.sort_by_key(|&i| std::cmp::Reverse(natural[i]));
    for &i in open.iter().cycle().take(rem) {
        widths[i] += 1;
    }
    for (w, &m) in widths.iter_mut().zip(min) {
        *w = (*w).max(m);
    }
    // Minimums may overshoot; take it back from the widest columns that
    // are still above the floor of 3.
    while widths.iter().sum::<usize>() > budget {
        let Some((i, _)) = widths
            .iter()
            .enumerate()
            .filter(|(_, w)| **w > 3)
            .max_by_key(|(_, w)| **w)
        else {
            break;
        };
        widths[i] -= 1;
    }
    widths
}

/// Pad a wrapped cell line to `w` columns according to `align`.
pub(super) fn aligned(line: Line<'static>, w: usize, align: Align) -> Vec<Span<'static>> {
    let pad = w.saturating_sub(line.width());
    let (before, after) = match align {
        Align::Right => (pad, 0),
        Align::Center => (pad / 2, pad - pad / 2),
        Align::None | Align::Left => (0, pad),
    };
    let mut spans = Vec::with_capacity(line.spans.len() + 2);
    if before > 0 {
        spans.push(Span::raw(" ".repeat(before)));
    }
    spans.extend(line.spans);
    if after > 0 {
        spans.push(Span::raw(" ".repeat(after)));
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::parse;
    use crate::render::*;

    #[test]
    fn widths_fit_unchanged() {
        assert_eq!(column_widths(&[5, 10], &[3, 3], 20), [5, 10]);
    }

    #[test]
    fn widths_shrink_only_long_column() {
        // Short columns keep their width; the long one takes the rest.
        assert_eq!(column_widths(&[4, 6, 80], &[3, 3, 5], 40), [4, 6, 30]);
    }

    #[test]
    fn widths_split_long_columns_proportionally() {
        assert_eq!(column_widths(&[3, 60, 30], &[3, 3, 3], 48), [3, 30, 15]);
    }

    #[test]
    fn widths_respect_minimum_then_budget() {
        let w = column_widths(&[20, 20], &[12, 12], 30);
        assert_eq!(w, [15, 15]);
        let w = column_widths(&[20, 20, 20], &[12, 12, 12], 24);
        assert_eq!(w.iter().sum::<usize>(), 24);
        assert!(w.iter().all(|&c| c >= 3));
    }

    fn table(src: &str, width: u16) -> Vec<Line<'static>> {
        render(&parse(src), width, &Theme::dark()).lines
    }

    #[test]
    fn table_aligns_columns() {
        let lines = table("| left | right | mid |\n|:--|--:|:-:|\n| a | b | c |\n", 40);
        assert_eq!(to_plain(&lines[2..3]), " a    │     b │  c");
    }

    #[test]
    fn table_wraps_long_cells() {
        let src = "| k | v |\n|---|---|\n| x | one two three four five six |\n";
        let plain = to_plain(&table(src, 20));
        assert_eq!(
            plain,
            " k   │ v\n─────┼──────────────\n x   │ one two\n     │ three four\n     │ five six"
        );
        assert!(!plain.contains('…'));
    }

    #[test]
    fn table_keeps_inline_styles() {
        let lines = table("| a | b |\n|---|---|\n| **bold** | `code` |\n", 40);
        let spans = &lines[2].spans;
        let bold = spans.iter().find(|s| s.content == "bold").unwrap();
        assert!(bold.style.add_modifier.contains(Modifier::BOLD));
        let code = spans.iter().find(|s| s.content == "code").unwrap();
        assert_eq!(code.style.bg, Some(Theme::dark().surface));
    }
}
