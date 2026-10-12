mod ansi;
pub(crate) mod code;
mod codeblock;
mod image;
mod links;
mod plain;
mod run;
mod table;
pub mod theme;
mod wrap;

pub use ansi::to_ansi;
pub use code::known_extension;
pub(crate) use plain::strip_controls;
pub use plain::{plain, to_plain};
pub(crate) use run::run_status;

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::doc::{Block, Document, Inline, plain_text};
use crate::exec::{BlockRun, Interpreters};
use links::{is_link_style, locate_links};
use theme::Theme;
use wrap::wrap;

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
    /// Rows reserved for pictures, to draw over.
    pub images: Vec<ImageSlot>,
}

/// Where a picture goes: `rows` blank lines from `line`, `cols` wide,
/// starting `col` columns in (after list or quote prefixes).
#[derive(Debug, Clone, PartialEq)]
pub struct ImageSlot {
    pub url: String,
    pub line: usize,
    pub col: usize,
    pub cols: u16,
    pub rows: u16,
}

/// Size in terminal cells of each image that's ready to show, by URL as
/// written in the document.
pub type ImageSizes = HashMap<String, (u16, u16)>;

/// Settings that change how documents render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// Pictures are scaled down to at most this many rows.
    pub image_rows: u16,
    /// Columns a tab in code takes up.
    pub tab_width: u16,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            image_rows: 24,
            tab_width: 4,
        }
    }
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

/// The bar down the left of quotes and run output.
const QUOTE_BAR: &str = "│ ";

pub fn render(doc: &Document, width: u16, theme: &Theme) -> Rendered {
    render_with(doc, width, theme, Options::default())
}

/// Render without run output or pictures, with `opts`.
pub fn render_with(doc: &Document, width: u16, theme: &Theme, opts: Options) -> Rendered {
    let (runs, interpreters) = (HashMap::new(), Interpreters::default());
    let (highlights, images) = (Highlights::default(), ImageSizes::new());
    let cx = Context {
        runs: &runs,
        interpreters: &interpreters,
        highlights: &highlights,
        images: &images,
        opts,
    };
    render_with_runs(doc, width, theme, cx)
}

/// Highlighted source lines per code block id. Highlighting doesn't depend
/// on width or run state, so a reader keeps these across re-renders and
/// clears them when the document changes.
#[derive(Default)]
pub(crate) struct Highlights(RefCell<HashMap<usize, Vec<Vec<Span<'static>>>>>);

impl Highlights {
    pub fn clear(&self) {
        self.0.borrow_mut().clear();
    }
}

/// The reader state a render draws on besides the document.
#[derive(Clone, Copy)]
pub(crate) struct Context<'a> {
    /// Run status and output per code block id.
    pub runs: &'a HashMap<usize, BlockRun>,
    pub interpreters: &'a Interpreters,
    pub highlights: &'a Highlights,
    /// Sizes of the pictures loaded so far.
    pub images: &'a ImageSizes,
    pub opts: Options,
}

/// Render with each code block's run status and output drawn under it.
pub(crate) fn render_with_runs(doc: &Document, width: u16, theme: &Theme, cx: Context) -> Rendered {
    let r = Renderer {
        theme,
        cx,
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
        images: out.images,
        lines: out.lines,
        src_line: Vec::new(),
        anchors,
        code_blocks: out.codes,
        links,
    }
}

#[derive(Default)]
struct Out {
    lines: Vec<Line<'static>>,
    codes: Vec<CodeRange>,
    images: Vec<ImageSlot>,
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
        // Prefixes are as wide on every line of a block, so either will do.
        let indent: usize = rest.iter().chain(first).map(Span::width).max().unwrap_or(0);
        self.images
            .extend(other.images.into_iter().map(|s| ImageSlot {
                line: s.line + offset,
                col: s.col + indent,
                ..s
            }));
    }
}

struct Renderer<'a> {
    theme: &'a Theme,
    cx: Context<'a>,
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
                let bar = [Span::styled(QUOTE_BAR, Style::default().fg(t.muted))];
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
            Block::Image { alt, url } => self.image(alt, url, width),
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

/// `<Name ...>` but not `</Name>` or a self-closing `<Name />`.
fn is_open_tag(label: &str) -> bool {
    label.starts_with('<') && !label.starts_with("</") && !label.ends_with("/>")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::parse;

    #[test]
    fn options_set_tab_width_and_picture_height() {
        let doc = parse("```go\nfunc main() {\n\treturn\n}\n```\n");
        let text = |opts| to_plain(&render_with(&doc, 40, &Theme::dark(), opts).lines);
        assert!(
            text(Options::default()).contains("     return"),
            "{}",
            text(Options::default())
        );
        let two = Options {
            tab_width: 2,
            ..Options::default()
        };
        assert!(
            text(two).contains("   return") && !text(two).contains("     return"),
            "{}",
            text(two)
        );

        let doc = parse("![A cat](cat.png)\n");
        let sizes = ImageSizes::from([("cat.png".to_string(), (60, 40))]);
        let short = Options {
            image_rows: 10,
            ..Options::default()
        };
        let (runs, interpreters, highlights) = Default::default();
        let cx = Context {
            runs: &runs,
            interpreters: &interpreters,
            highlights: &highlights,
            images: &sizes,
            opts: short,
        };
        let r = render_with_runs(&doc, 80, &Theme::dark(), cx);
        assert_eq!(r.images[0].rows, 10);
    }
}
