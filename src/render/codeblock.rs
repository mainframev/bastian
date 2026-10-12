use ratatui::style::Style;
use ratatui::text::{Line, Span};

use super::wrap::hard_wrap;
use super::{CodeRange, Out, Rendered, Renderer, code};
use crate::doc::CodeBlock;

impl Renderer<'_> {
    pub(super) fn code(&self, c: &CodeBlock, width: usize) -> Out {
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
            ..Out::default()
        }
    }

    pub(super) fn highlighted(&self, c: &CodeBlock) -> Vec<Vec<Span<'static>>> {
        self.highlights
            .0
            .borrow_mut()
            .entry(c.id)
            .or_insert_with(|| {
                code::highlight(&c.src, c.lang.as_deref(), self.theme, self.opts.tab_width)
            })
            .clone()
    }

    /// Append `c`'s run panel, if it has run, under its `lines` and return
    /// the range they cover.
    pub(super) fn add_run_panel(
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
    pub(super) fn code_file(&self, c: &CodeBlock, width: usize) -> Rendered {
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
            images: Vec::new(),
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
}
