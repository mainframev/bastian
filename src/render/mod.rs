mod ansi;
pub(crate) mod code;
pub mod theme;

pub use ansi::to_ansi;
pub use code::known_extension;
mod wrap;

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::time::Duration;

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::doc::{Align, Block, CodeBlock, Document, Inline, plain_text};
use crate::exec::{BlockRun, Interpreters, Status};
use theme::Theme;
use wrap::{hard_wrap, wrap};

/// A document laid out for a given width.
#[derive(Debug, Default)]
pub struct Rendered {
    pub lines: Vec<Line<'static>>,
    /// Source line (0-based) each rendered line starts, when known; wrapped
    /// continuations are `None`. Empty when the document has no mapping.
    /// Only code files fill this; it drives source-numbered gutters.
    pub src_line: Vec<Option<usize>>,
    /// `(rendered line, source line)` pairs, ascending, where top-level
    /// blocks (or code-file lines) start.
    pub anchors: Vec<(usize, usize)>,
    /// Line range occupied by each code block, in document order.
    pub code_blocks: Vec<CodeRange>,
    /// Where link text landed on screen, one entry per line a link covers.
    pub links: Vec<LinkSpan>,
    /// Rendered line where each top-level block starts.
    pub block_starts: Vec<usize>,
}

/// A link's text on one rendered line.
#[derive(Debug, Clone, PartialEq)]
pub struct LinkSpan {
    pub line: usize,
    /// First column and width of the link text on that line.
    pub col: usize,
    pub width: usize,
    /// The link target as written in the document.
    pub url: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CodeRange {
    pub id: usize,
    /// The code box itself.
    pub lines: Range<usize>,
    /// End of the run panel under the box, or `lines.end` without one.
    pub output_end: usize,
}

pub fn render(doc: &Document, width: u16, theme: &Theme) -> Rendered {
    render_with_runs(
        doc,
        width,
        theme,
        &HashMap::new(),
        &Interpreters::default(),
        &Highlights::default(),
    )
}

/// Highlighted source lines per code block id. Highlighting doesn't depend
/// on width or run state, so a reader keeps these across re-renders and
/// clears them when the document changes.
#[derive(Default)]
pub struct Highlights(RefCell<HashMap<usize, Vec<Vec<Span<'static>>>>>);

impl Highlights {
    pub fn clear(&self) {
        self.0.borrow_mut().clear();
    }
}

/// Render with each code block's run status and output drawn under it.
pub fn render_with_runs(
    doc: &Document,
    width: u16,
    theme: &Theme,
    runs: &HashMap<usize, BlockRun>,
    interpreters: &Interpreters,
    highlights: &Highlights,
) -> Rendered {
    let r = Renderer {
        theme,
        runs,
        interpreters,
        highlights,
        link_runs: RefCell::default(),
    };
    let width = usize::from(width).max(10);
    if doc.code_file
        && let [Block::Code(c)] = &doc.blocks[..]
    {
        return r.code_file(c, width);
    }
    let out = r.blocks(&doc.blocks, width, true);
    let anchors = out
        .starts
        .iter()
        .copied()
        .zip(doc.lines.iter().copied())
        .collect();
    let links = locate_links(&out.lines, &r.link_runs.into_inner(), theme);
    Rendered {
        block_starts: out.starts,
        lines: out.lines,
        src_line: Vec::new(),
        anchors,
        code_blocks: out.codes,
        links,
    }
}

/// Spans drawn in the link style.
fn is_link_style(style: Style, t: &Theme) -> bool {
    style.fg == Some(t.link) && style.add_modifier.contains(Modifier::UNDERLINED)
}

/// Match link-styled spans, in order, to the link runs the renderer styled
/// (`(url, non-space characters)`), and merge each run's spans on a line.
/// Wrapping only drops whitespace, so counting the other characters ties
/// every span back to its run, even across line breaks.
fn locate_links(lines: &[Line], runs: &[(String, usize)], t: &Theme) -> Vec<LinkSpan> {
    let mut out: Vec<LinkSpan> = Vec::new();
    let mut run = 0;
    let mut left = runs.first().map_or(0, |r| r.1);
    for (i, line) in lines.iter().enumerate() {
        let mut col = 0;
        for span in &line.spans {
            let w = span.width();
            if is_link_style(span.style, t) {
                let used = span.content.chars().filter(|c| !c.is_whitespace()).count();
                // Move on once the current run's text is used up; spaces
                // still belong to the run they follow.
                while used > 0 && left == 0 && run + 1 < runs.len() {
                    run += 1;
                    left = runs[run].1;
                }
                let Some((url, _)) = runs.get(run) else {
                    return out;
                };
                match out.last_mut() {
                    Some(l) if l.line == i && l.col + l.width == col && l.url == *url => {
                        l.width += w;
                    }
                    _ => out.push(LinkSpan {
                        line: i,
                        col,
                        width: w,
                        url: url.clone(),
                    }),
                }
                left = left.saturating_sub(used);
            }
            col += w;
        }
    }
    out
}

#[derive(Default)]
struct Out {
    lines: Vec<Line<'static>>,
    codes: Vec<CodeRange>,
    /// Rendered line where each block passed to `blocks` begins.
    starts: Vec<usize>,
}

impl From<Vec<Line<'static>>> for Out {
    fn from(lines: Vec<Line<'static>>) -> Self {
        Out {
            lines,
            ..Out::default()
        }
    }
}

impl Out {
    fn blank(&mut self) {
        self.lines.push(Line::default());
    }

    /// Append `other`, prefixing its first line with `first` and the rest with `rest`.
    fn append(&mut self, other: Out, first: &[Span<'static>], rest: &[Span<'static>]) {
        let offset = self.lines.len();
        if first.is_empty() && rest.is_empty() {
            self.lines.extend(other.lines);
        } else {
            for (i, line) in other.lines.into_iter().enumerate() {
                let prefix = if i == 0 { first } else { rest };
                let mut spans = prefix.to_vec();
                spans.extend(line.spans);
                self.lines.push(Line::from(spans).style(line.style));
            }
        }
        self.codes
            .extend(other.codes.into_iter().map(|c| CodeRange {
                id: c.id,
                lines: c.lines.start + offset..c.lines.end + offset,
                output_end: c.output_end + offset,
            }));
    }
}

struct Renderer<'a> {
    theme: &'a Theme,
    runs: &'a HashMap<usize, BlockRun>,
    interpreters: &'a Interpreters,
    highlights: &'a Highlights,
    /// Link runs in the order they were styled, for `locate_links`.
    link_runs: RefCell<Vec<(String, usize)>>,
}

impl Renderer<'_> {
    /// Render a sequence of blocks. `gap` puts a blank line between them.
    fn blocks(&self, blocks: &[Block], width: usize, gap: bool) -> Out {
        let mut out = Out::default();
        for (i, block) in blocks.iter().enumerate() {
            // MDX tags hug what they wrap: no gap after an opening tag or
            // before a closing one.
            let after_open = i > 0 && matches!(&blocks[i - 1], Block::Label(l) if is_open_tag(l));
            let before_close = matches!(block, Block::Label(l) if l.starts_with("</"));
            if gap && i > 0 && !after_open && !before_close {
                out.blank();
            }
            let part = self.block(block, width);
            out.starts.push(out.lines.len());
            out.append(part, &[], &[]);
        }
        out
    }

    fn block(&self, block: &Block, width: usize) -> Out {
        let t = self.theme;
        match block {
            Block::Heading { level, inlines } => self.heading(*level, inlines, width),
            Block::Paragraph(inlines) => Out::from(wrap(
                &self.runs(inlines, Style::default().fg(t.text)),
                width,
            )),
            Block::List { start, items } => self.list(*start, items, width),
            Block::Quote(inner) => {
                let mut out = Out::default();
                let bar = [Span::styled("│ ", Style::default().fg(t.muted))];
                out.append(
                    self.blocks(inner, width.saturating_sub(2), true),
                    &bar,
                    &bar,
                );
                out
            }
            Block::Code(c) => self.code(c, width),
            Block::Label(text) => {
                Out::from(wrap(&[(text.clone(), Style::default().fg(t.muted))], width))
            }
            Block::Table {
                align,
                header,
                rows,
            } => self.table(align, header, rows, width),
            Block::Rule => Out::from(vec![Line::styled(
                "─".repeat(width),
                Style::default().fg(t.muted),
            )]),
        }
    }

    fn heading(&self, level: u8, inlines: &[Inline], width: usize) -> Out {
        let t = self.theme;
        let lines = if level == 1 {
            // Pad each wrapped line inside the colored band.
            let style = Style::default().fg(t.h1_fg).bg(t.h1_bg).bold();
            wrap(&[(plain_text(inlines), style)], width.saturating_sub(2))
                .into_iter()
                .map(|line| {
                    let mut spans = vec![Span::styled(" ", style)];
                    spans.extend(line.spans);
                    spans.push(Span::styled(" ", style));
                    Line::from(spans)
                })
                .collect()
        } else {
            let marker = format!("{} ", "#".repeat(usize::from(level)));
            let style = Style::default().fg(t.heading).bold();
            let mut runs = vec![(marker, Style::default().fg(t.muted).bold())];
            runs.extend(self.runs(inlines, style));
            wrap(&runs, width)
        };
        Out::from(lines)
    }

    fn list(&self, start: Option<u64>, items: &[Vec<Block>], width: usize) -> Out {
        let t = self.theme;
        let markers: Vec<String> = match start {
            Some(n) => (0..items.len() as u64)
                .map(|i| format!("{}. ", n + i))
                .collect(),
            None => vec!["• ".to_string(); items.len()],
        };
        let marker_w = markers.iter().map(|m| m.width()).max().unwrap_or(2);
        let pad = [Span::raw(" ".repeat(marker_w))];

        let mut out = Out::default();
        for (item, marker) in items.iter().zip(markers) {
            let first = [Span::styled(
                format!("{marker:>marker_w$}"),
                Style::default().fg(t.accent),
            )];
            let inner = self.blocks(item, width.saturating_sub(marker_w), false);
            out.append(inner, &first, &pad);
        }
        out
    }

    fn code(&self, c: &CodeBlock, width: usize) -> Out {
        let t = self.theme;
        let bg = Style::default().bg(t.surface);
        let inner_w = width.saturating_sub(2).max(1);
        // Background goes on every span, not the line, so prefixes added by
        // enclosing lists and quotes stay unshaded.
        let pad_line = |spans: Vec<Span<'static>>| {
            let mut spans = spans;
            spans.insert(0, Span::raw(" "));
            let used: usize = spans.iter().map(Span::width).sum();
            spans.push(Span::raw(" ".repeat(width.saturating_sub(used))));
            Line::from(
                spans
                    .into_iter()
                    .map(|s| {
                        let style = bg.patch(s.style);
                        s.style(style)
                    })
                    .collect::<Vec<_>>(),
            )
        };

        let mut lines = Vec::new();
        let label = match (&c.attrs.name, &c.lang) {
            (Some(name), Some(lang)) => format!("{name} · {lang}"),
            (Some(name), None) => name.clone(),
            (None, lang) => lang.clone().unwrap_or_default(),
        };
        let label = if c
            .lang
            .as_deref()
            .and_then(|l| self.interpreters.get(l))
            .is_some()
            && !c.attrs.norun
        {
            format!("▶ {label}")
        } else {
            label
        };
        lines.push(pad_line(vec![Span::styled(
            format!("{label:>inner_w$}"),
            Style::default().fg(t.muted),
        )]));
        for src_line in self.highlighted(c) {
            for row in hard_wrap(src_line, inner_w) {
                lines.push(pad_line(row));
            }
        }
        lines.push(pad_line(Vec::new()));

        let range = self.add_run_panel(c, &mut lines, width);
        Out {
            lines,
            codes: vec![range],
            starts: Vec::new(),
        }
    }

    fn highlighted(&self, c: &CodeBlock) -> Vec<Vec<Span<'static>>> {
        self.highlights
            .0
            .borrow_mut()
            .entry(c.id)
            .or_insert_with(|| code::highlight(&c.src, c.lang.as_deref(), self.theme))
            .clone()
    }

    /// Append `c`'s run panel, if it has run, under its `lines` and return
    /// the range they cover.
    fn add_run_panel(
        &self,
        c: &CodeBlock,
        lines: &mut Vec<Line<'static>>,
        width: usize,
    ) -> CodeRange {
        let len = lines.len();
        if let Some(run) = self.runs.get(&c.id) {
            lines.extend(self.run_panel(run, width));
        }
        CodeRange {
            id: c.id,
            lines: 0..len,
            output_end: lines.len(),
        }
    }

    /// A whole source file: highlighted lines with no box, each wrapped
    /// segment mapped back to its source line.
    fn code_file(&self, c: &CodeBlock, width: usize) -> Rendered {
        let mut lines = Vec::new();
        let mut src_line = Vec::new();
        for (n, spans) in self.highlighted(c).into_iter().enumerate() {
            for (i, row) in hard_wrap(spans, width).into_iter().enumerate() {
                lines.push(Line::from(row));
                src_line.push((i == 0).then_some(n));
            }
        }
        let range = self.add_run_panel(c, &mut lines, width);
        src_line.resize(lines.len(), None);
        Rendered {
            block_starts: Vec::new(),
            links: Vec::new(),
            code_blocks: vec![range],
            anchors: src_line
                .iter()
                .enumerate()
                .filter_map(|(i, s)| s.map(|s| (i, s)))
                .collect(),
            lines,
            src_line,
        }
    }

    /// Status line and output shown under a code block that has run.
    fn run_panel(&self, run: &BlockRun, width: usize) -> Vec<Line<'static>> {
        let t = self.theme;
        let mut lines = vec![run_status(run, t)];
        if !run.expanded {
            return lines;
        }

        let bar = Span::styled("│ ", Style::default().fg(t.muted));
        if run.dropped > 0 {
            lines.push(Line::from(vec![
                bar.clone(),
                Span::styled(
                    format!("… {} earlier lines", run.dropped),
                    Style::default().fg(t.muted),
                ),
            ]));
        }
        for out in &run.output {
            let style = Style::default().fg(if out.stderr { t.err } else { t.subtle });
            for row in hard_wrap(
                vec![Span::styled(out.text.clone(), style)],
                width.saturating_sub(2),
            ) {
                let mut spans = vec![bar.clone()];
                spans.extend(row);
                lines.push(Line::from(spans));
            }
        }
        lines
    }

    fn table(
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
                let word = s
                    .split_whitespace()
                    .map(UnicodeWidthStr::width)
                    .max()
                    .unwrap_or(0);
                natural[i] = natural[i].max(line);
                min[i] = min[i].max(word.min(MAX_MIN_COLUMN));
            }
        }
        // One space of margin on each side plus " │ " between columns.
        let budget = width.saturating_sub(2 + 3 * (cols - 1));
        let widths = column_widths(&natural, &min, budget);

        let sep = Style::default().fg(t.muted);
        let row_lines = |row: &[Vec<Inline>], base: Style| -> Vec<Line<'static>> {
            let cells: Vec<Vec<Line<'static>>> = widths
                .iter()
                .enumerate()
                .map(|(i, &w)| {
                    let inlines = row.get(i).map_or(&[][..], Vec::as_slice);
                    wrap(&self.runs(inlines, base), w)
                })
                .collect();
            let height = cells.iter().map(Vec::len).max().unwrap_or(1);
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

    fn runs(&self, inlines: &[Inline], base: Style) -> Vec<(String, Style)> {
        let t = self.theme;
        inlines
            .iter()
            .map(|i| {
                let mut s = base;
                if i.style.strong {
                    s = s.add_modifier(Modifier::BOLD);
                }
                if i.style.emphasis {
                    s = s.add_modifier(Modifier::ITALIC);
                }
                if i.style.strike {
                    s = s.add_modifier(Modifier::CROSSED_OUT);
                }
                if i.style.code {
                    s = s.fg(t.code).bg(t.surface);
                }
                if i.style.link.is_some() {
                    s = s.fg(t.link).add_modifier(Modifier::UNDERLINED);
                }
                if i.style.muted {
                    s = s.fg(t.muted);
                }
                if let Some(url) = &i.style.link
                    && is_link_style(s, t)
                {
                    let chars = i.text.chars().filter(|c| !c.is_whitespace()).count();
                    self.link_runs.borrow_mut().push((url.clone(), chars));
                }
                (i.text.clone(), s)
            })
            .collect()
    }
}

/// First line of a run panel: status icon, text and elapsed time, plus a
/// hidden-line count when the output is collapsed.
pub fn run_status(run: &BlockRun, t: &Theme) -> Line<'static> {
    let (icon, text, color) = match &run.status {
        Status::Running(since) => ("●", format!("running {}", secs(since.elapsed())), t.warn),
        Status::Ok(took) => ("✓", format!("done in {}", secs(*took)), t.ok),
        Status::Failed {
            code: Some(c),
            took,
        } => ("✗", format!("exit {c} after {}", secs(*took)), t.err),
        Status::Failed { code: None, took } => (
            "✗",
            format!("killed by signal after {}", secs(*took)),
            t.err,
        ),
        Status::Cancelled(took) => ("■", format!("cancelled after {}", secs(*took)), t.warn),
        Status::TimedOut(took) => ("✗", format!("timed out after {}", secs(*took)), t.err),
        Status::Error(e) => ("✗", format!("could not start: {e}"), t.err),
    };
    let mut status = vec![Span::styled(
        format!("{icon} {text}"),
        Style::default().fg(color).bold(),
    )];
    if !run.expanded && !run.output.is_empty() {
        let n = run.output.len() + run.dropped;
        status.push(Span::styled(
            format!("  {n} line{} hidden", if n == 1 { "" } else { "s" }),
            Style::default().fg(t.muted),
        ));
    }
    Line::from(status)
}

/// A column's minimum width never exceeds this, so one long word can't
/// starve the other columns; longer words are split.
const MAX_MIN_COLUMN: usize = 12;

/// Fit columns with `natural` widths into `budget` columns. Columns that
/// fit within an even share keep their natural width; the rest split what's
/// left in proportion to their natural width, but not below `min` unless
/// the table can't fit otherwise.
fn column_widths(natural: &[usize], min: &[usize], budget: usize) -> Vec<usize> {
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
fn aligned(line: Line<'static>, w: usize, align: Align) -> Vec<Span<'static>> {
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

/// Text of one rendered line, without styling.
pub fn plain(line: &Line) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

/// Plain text of rendered lines, for printing and snapshot tests.
pub fn to_plain(lines: &[Line]) -> String {
    lines
        .iter()
        .map(|l| plain(l).trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn secs(d: Duration) -> String {
    format!("{:.1}s", d.as_secs_f64())
}

/// `<Name ...>` but not `</Name>` or a self-closing `<Name />`.
fn is_open_tag(label: &str) -> bool {
    label.starts_with('<') && !label.starts_with("</") && !label.ends_with("/>")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::parse;

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

    #[test]
    fn links_are_located_across_wraps_and_prefixes() {
        let src = "# Top [x](https://h1.dev)\n\nsee [the long guide](docs/guide.md) and [b](https://b.dev)\n\n- item [c](#c)\n";
        let r = render(&parse(src), 16, &Theme::dark());
        let text = |l: &LinkSpan| -> String {
            let line = plain(&r.lines[l.line]);
            line.chars().skip(l.col).take(l.width).collect()
        };
        let found: Vec<(String, String)> =
            r.links.iter().map(|l| (l.url.clone(), text(l))).collect();
        assert_eq!(
            found,
            [
                // The H1 band shows its text plainly, so only these count.
                ("docs/guide.md".to_string(), "the long".to_string()),
                ("docs/guide.md".to_string(), "guide".to_string()),
                ("https://b.dev".to_string(), "b".to_string()),
                ("#c".to_string(), "c".to_string()),
            ],
            "{:?}",
            to_plain(&r.lines)
        );
    }
}
