mod code;
pub mod theme;
mod wrap;

use std::ops::Range;

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::doc::{Block, CodeBlock, Document, Inline, plain_text};
use theme::Theme;
use wrap::{hard_wrap, line_width, wrap};

/// A document laid out for a given width.
#[derive(Debug, Default)]
pub struct Rendered {
    pub lines: Vec<Line<'static>>,
    /// Line range occupied by each code block, in document order.
    pub code_blocks: Vec<CodeRange>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CodeRange {
    pub id: usize,
    pub lines: Range<usize>,
}

pub fn render(doc: &Document, width: u16, theme: &Theme) -> Rendered {
    let out = Renderer { theme }.blocks(&doc.blocks, usize::from(width).max(10), true);
    Rendered {
        lines: out.lines,
        code_blocks: out.codes,
    }
}

#[derive(Default)]
struct Out {
    lines: Vec<Line<'static>>,
    codes: Vec<CodeRange>,
}

impl Out {
    fn blank(&mut self) {
        self.lines.push(Line::default());
    }

    /// Append `other`, prefixing its first line with `first` and the rest with `rest`.
    fn append(&mut self, other: Out, first: &[Span<'static>], rest: &[Span<'static>]) {
        let offset = self.lines.len();
        for (i, line) in other.lines.into_iter().enumerate() {
            let prefix = if i == 0 { first } else { rest };
            let mut spans = prefix.to_vec();
            spans.extend(line.spans);
            self.lines.push(Line::from(spans));
        }
        self.codes
            .extend(other.codes.into_iter().map(|c| CodeRange {
                id: c.id,
                lines: c.lines.start + offset..c.lines.end + offset,
            }));
    }
}

struct Renderer<'a> {
    theme: &'a Theme,
}

impl Renderer<'_> {
    /// Render a sequence of blocks. `gap` puts a blank line between them.
    fn blocks(&self, blocks: &[Block], width: usize, gap: bool) -> Out {
        let mut out = Out::default();
        for (i, block) in blocks.iter().enumerate() {
            if gap && i > 0 {
                out.blank();
            }
            let part = self.block(block, width);
            out.append(part, &[], &[]);
        }
        out
    }

    fn block(&self, block: &Block, width: usize) -> Out {
        let t = self.theme;
        match block {
            Block::Heading { level, inlines } => self.heading(*level, inlines, width),
            Block::Paragraph(inlines) => Out {
                lines: wrap(&self.runs(inlines, Style::default().fg(t.text)), width),
                codes: Vec::new(),
            },
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
            Block::Table { header, rows } => self.table(header, rows, width),
            Block::Rule => Out {
                lines: vec![Line::styled(
                    "─".repeat(width),
                    Style::default().fg(t.muted),
                )],
                codes: Vec::new(),
            },
        }
    }

    fn heading(&self, level: u8, inlines: &[Inline], width: usize) -> Out {
        let t = self.theme;
        let lines = if level == 1 {
            // Pad each wrapped line inside the colored band, like glow's H1.
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
        Out {
            lines,
            codes: Vec::new(),
        }
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
        let pad_line = |spans: Vec<Span<'static>>| {
            let mut spans = spans;
            spans.insert(0, Span::raw(" "));
            let used = line_width(&Line::from(spans.clone()));
            spans.push(Span::raw(" ".repeat(width.saturating_sub(used))));
            Line::from(spans).style(bg)
        };

        let mut lines = Vec::new();
        let label = c.lang.clone().unwrap_or_default();
        lines.push(pad_line(vec![Span::styled(
            format!("{label:>inner_w$}"),
            Style::default().fg(t.muted),
        )]));
        for src_line in code::highlight(&c.src, c.lang.as_deref(), t) {
            for row in hard_wrap(src_line, inner_w) {
                lines.push(pad_line(row));
            }
        }
        lines.push(pad_line(Vec::new()));

        let len = lines.len();
        Out {
            lines,
            codes: vec![CodeRange {
                id: c.id,
                lines: 0..len,
            }],
        }
    }

    fn table(&self, header: &[Vec<Inline>], rows: &[Vec<Vec<Inline>>], width: usize) -> Out {
        let t = self.theme;
        let cols = header
            .len()
            .max(rows.iter().map(Vec::len).max().unwrap_or(0));
        if cols == 0 {
            return Out::default();
        }
        let cell =
            |row: &[Vec<Inline>], i: usize| row.get(i).map(|c| plain_text(c)).unwrap_or_default();

        let mut widths = vec![3usize; cols];
        for row in std::iter::once(header).chain(rows.iter().map(Vec::as_slice)) {
            for (i, w) in widths.iter_mut().enumerate() {
                *w = (*w).max(cell(row, i).width());
            }
        }
        // Shrink the widest column until the table fits.
        let sep_w = 3 * (cols - 1);
        while widths.iter().sum::<usize>() + sep_w > width {
            let (i, &max) = widths.iter().enumerate().max_by_key(|(_, w)| **w).unwrap();
            if max <= 3 {
                break;
            }
            widths[i] -= 1;
        }

        let sep = Style::default().fg(t.muted);
        let row_line = |row: &[Vec<Inline>], style: Style| {
            let mut spans = Vec::new();
            for (i, w) in widths.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::styled(" │ ", sep));
                }
                spans.push(Span::styled(fit(&cell(row, i), *w), style));
            }
            Line::from(spans)
        };

        let mut lines = vec![row_line(header, Style::default().fg(t.heading).bold())];
        let rule: Vec<String> = widths.iter().map(|w| "─".repeat(*w)).collect();
        lines.push(Line::styled(rule.join("─┼─"), sep));
        for row in rows {
            lines.push(row_line(row, Style::default().fg(t.text)));
        }
        Out {
            lines,
            codes: Vec::new(),
        }
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
                (i.text.clone(), s)
            })
            .collect()
    }
}

/// Pad or truncate `s` to exactly `w` columns.
fn fit(s: &str, w: usize) -> String {
    if s.width() <= w {
        return format!("{s}{}", " ".repeat(w - s.width()));
    }
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let cw = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
        if used + cw + 1 > w {
            break;
        }
        out.push(c);
        used += cw;
    }
    out.push('…');
    used += 1;
    format!("{out}{}", " ".repeat(w.saturating_sub(used)))
}

/// Plain text of rendered lines, for printing and snapshot tests.
pub fn to_plain(lines: &[Line]) -> String {
    lines
        .iter()
        .map(|l| {
            let s: String = l.spans.iter().map(|s| s.content.as_ref()).collect();
            s.trim_end().to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
