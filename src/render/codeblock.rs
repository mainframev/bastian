use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::wrap::hard_wrap;
use super::{CodeRange, Out, Rendered, Renderer, code};
use crate::doc::CodeBlock;
use crate::text::shorten_right;

/// Narrowest code block that gets a frame: the frame and its padding take
/// four columns, leaving at least four for code.
const MIN_FRAME_WIDTH: usize = 8;

impl Renderer<'_> {
    pub(super) fn code(&self, c: &CodeBlock, width: usize) -> Out {
        let t = self.theme;
        let bg = Style::default().bg(t.surface);
        let edge = Style::default().fg(t.muted);
        // Too narrow for a frame: just the shaded code.
        let framed = width >= MIN_FRAME_WIDTH;
        let inner_w = if framed { width - 4 } else { width.max(1) };
        // Background goes on every span inside the frame, not the line, so
        // the frame and prefixes added by enclosing lists and quotes stay
        // unshaded.
        let pad_line = |mut spans: Vec<Span<'static>>, bar: &'static str| {
            let fill = inner_w.saturating_sub(spans.iter().map(Span::width).sum());
            if framed {
                spans.insert(0, Span::raw(" "));
                spans.push(Span::raw(" ".repeat(fill + 1)));
            } else {
                spans.push(Span::raw(" ".repeat(fill)));
            }
            let mut line: Vec<Span<'static>> = spans
                .into_iter()
                .map(|s| {
                    let style = bg.patch(s.style);
                    s.style(style)
                })
                .collect();
            if framed {
                line.insert(0, Span::styled(bar, edge));
                line.push(Span::styled("│", edge));
            }
            Line::from(line)
        };

        let mut lines = Vec::new();
        if framed {
            lines.push(self.code_top(c, width));
        }
        if let Some(diagram) = self.diagram(c, inner_w) {
            let style = Style::default().fg(t.text);
            lines.extend(
                diagram
                    .into_iter()
                    .map(|l| pad_line(vec![Span::styled(l, style)], "│")),
            );
        } else {
            self.with_highlighted(c, |src| {
                for src_line in src {
                    // Wrapped continuations get a dotted bar.
                    for (i, row) in hard_wrap(src_line, inner_w).into_iter().enumerate() {
                        lines.push(pad_line(row, if i == 0 { "│" } else { "┊" }));
                    }
                }
            });
        }
        if framed {
            lines.push(Line::styled(format!("└{}┘", "─".repeat(width - 2)), edge));
        }

        let mut range = self.add_run_panel(c, &mut lines, width);
        range.frame = framed.then_some(0..width);
        Out {
            lines,
            codes: vec![range],
            ..Out::default()
        }
    }

    /// Top of a code block's frame, with its label (name, language, and
    /// `▶` when it can run) set into the border.
    fn code_top(&self, c: &CodeBlock, width: usize) -> Line<'static> {
        let edge = Style::default().fg(self.theme.muted);
        let label = match (&c.attrs.name, &c.lang) {
            (Some(name), Some(lang)) => format!("{name} · {lang}"),
            (Some(name), None) => name.clone(),
            (None, lang) => lang.clone().unwrap_or_default(),
        };
        let runnable = !c.attrs.norun
            && c.lang
                .as_deref()
                .is_some_and(|l| self.cx.interpreters.get(l).is_some());
        let label = if runnable {
            format!("▶ {label}")
        } else {
            label
        };
        // `┌─ label ─┐`: the corners, a rule either side and a space around
        // the label take six columns.
        if label.is_empty() {
            return Line::styled(format!("┌{}┐", "─".repeat(width - 2)), edge);
        }
        let label = shorten_right(&label, width - 6);
        let fill = width - 5 - label.width();
        Line::styled(format!("┌─ {label} {}┐", "─".repeat(fill)), edge)
    }

    /// `c` drawn as a diagram `width` columns wide, unless it isn't one,
    /// can't be drawn, or the reader switched it to its source.
    fn diagram(&self, c: &CodeBlock, width: usize) -> Option<Vec<String>> {
        if !super::is_mermaid(c.lang.as_deref()) || self.cx.as_source.contains(&c.id) {
            return None;
        }
        #[cfg(feature = "mermaid")]
        return super::diagram::draw(&c.src, width);
        #[cfg(not(feature = "mermaid"))]
        {
            let _ = width;
            None
        }
    }

    /// Call `f` with the highlighted source lines of `c`, highlighting them
    /// on first use. They stay in the cache rather than being copied out.
    pub(super) fn with_highlighted<R>(
        &self,
        c: &CodeBlock,
        f: impl FnOnce(&[Vec<Span<'static>>]) -> R,
    ) -> R {
        let mut cache = self.cx.highlights.0.borrow_mut();
        let lines = cache.entry(c.id).or_insert_with(|| {
            code::highlight(
                &c.src,
                c.lang.as_deref(),
                self.theme,
                self.cx.opts.tab_width,
            )
        });
        f(lines)
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
        if let Some(run) = self.cx.runs.get(&c.id) {
            lines.extend(self.run_panel(run, width));
        }
        CodeRange {
            id: c.id,
            lines: 0..len,
            output_end: lines.len(),
            frame: None,
        }
    }

    /// A whole source file: highlighted lines with no box, each wrapped
    /// segment mapped back to its source line.
    pub(super) fn code_file(&self, c: &CodeBlock, width: usize) -> Rendered {
        let mut lines = Vec::new();
        let mut src_line = Vec::new();
        self.with_highlighted(c, |src| {
            for (n, spans) in src.iter().enumerate() {
                for (i, row) in hard_wrap(spans, width).into_iter().enumerate() {
                    lines.push(Line::from(row));
                    src_line.push((i == 0).then_some(n));
                }
            }
        });
        let range = self.add_run_panel(c, &mut lines, width);
        src_line.resize(lines.len(), None);
        Rendered {
            images: Vec::new(),
            block_starts: Vec::new(),
            links: Vec::new(),
            details: Vec::new(),
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
