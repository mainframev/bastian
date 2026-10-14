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
pub(crate) use plain::strip_controls;
pub use plain::{plain, to_plain};
pub(crate) use run::run_status;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ops::Range;

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::doc::{Block, CalloutKind, Document, Inline, plain_text};
use crate::exec::{BlockRun, Interpreters};
use links::{LinkRun, is_link_style, locate_links};
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
    /// Summary line of each details block shown, in document order.
    pub details: Vec<DetailsSpan>,
}

/// Where a details block's summary landed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetailsSpan {
    pub id: usize,
    /// First line of the summary.
    pub line: usize,
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
    /// Which link this is, in document order; the pieces of a link that
    /// wraps share it.
    pub id: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CodeRange {
    pub id: usize,
    /// The code box itself.
    pub lines: Range<usize>,
    /// End of the run panel under the box, or `lines.end` without one.
    pub output_end: usize,
    /// Columns the box's frame spans, after any prefixes from enclosing
    /// lists or quotes; `None` when it has no frame (a code file, or too
    /// narrow for one).
    pub frame: Option<Range<usize>>,
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
        toggled_details: None,
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
    /// Ids of the details blocks opened or closed from how they start;
    /// `None` shows them all open, as printed output can't open one.
    pub toggled_details: Option<&'a HashSet<usize>>,
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
        details: out.details,
    }
}

#[derive(Default)]
struct Out {
    lines: Vec<Line<'static>>,
    codes: Vec<CodeRange>,
    images: Vec<ImageSlot>,
    /// Rendered line where each block passed to `blocks` begins.
    starts: Vec<usize>,
    details: Vec<DetailsSpan>,
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
        // Prefixes are as wide on every line of a block, so either will do.
        let indent: usize = rest.iter().chain(first).map(Span::width).max().unwrap_or(0);
        self.codes
            .extend(other.codes.into_iter().map(|c| CodeRange {
                id: c.id,
                lines: c.lines.start + offset..c.lines.end + offset,
                output_end: c.output_end + offset,
                frame: c.frame.map(|f| f.start + indent..f.end + indent),
            }));
        self.details
            .extend(other.details.into_iter().map(|d| DetailsSpan {
                line: d.line + offset,
                ..d
            }));
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
    link_runs: RefCell<Vec<LinkRun>>,
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
            Block::List {
                start,
                items,
                tasks,
            } => self.list(*start, items, tasks, width),
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
            Block::Callout { kind, blocks } => {
                let color = match kind {
                    CalloutKind::Note => t.link,
                    CalloutKind::Tip => t.ok,
                    CalloutKind::Important => t.accent,
                    CalloutKind::Warning => t.warn,
                    CalloutKind::Caution => t.err,
                };
                let style = Style::default().fg(color);
                let mut inner = Out::from(vec![Line::styled(kind.title(), style.bold())]);
                inner.append(self.blocks(blocks, width.saturating_sub(2), true), &[], &[]);
                let bar = [Span::styled("▌ ", style)];
                let mut out = Out::default();
                out.append(inner, &bar, &bar);
                out
            }
            Block::Footnote { number, blocks, .. } => {
                let marker = format!("[{number}] ");
                let pad = [Span::raw(" ".repeat(marker.width()))];
                let first = [Span::styled(marker, Style::default().fg(t.muted))];
                let mut out = Out::default();
                out.append(
                    non_empty(self.blocks(blocks, width.saturating_sub(pad[0].width()), true)),
                    &first,
                    &pad,
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
            Block::Details {
                id,
                open,
                summary,
                blocks,
            } => self.details(*id, *open, summary, blocks, width),
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

    fn list(
        &self,
        start: Option<u64>,
        items: &[Vec<Block>],
        tasks: &[Option<bool>],
        width: usize,
    ) -> Out {
        let t = self.theme;
        let numbers: Vec<String> = match start {
            Some(n) => (0..items.len() as u64)
                .map(|i| format!("{}. ", n + i))
                .collect(),
            None => vec![String::new(); items.len()],
        };
        let num_w = numbers.iter().map(|m| m.width()).max().unwrap_or(0);
        let has_tasks = tasks.iter().any(Option::is_some);
        // Bullets give way to checkboxes; numbers keep theirs in front.
        let markers: Vec<Vec<Span<'static>>> = numbers
            .into_iter()
            .enumerate()
            .map(|(i, number)| {
                let number = format!("{number:>num_w$}");
                let task = match tasks.get(i).copied().flatten() {
                    Some(true) => Some(Span::styled("☑ ", Style::default().fg(t.ok))),
                    Some(false) => Some(Span::styled("☐ ", Style::default().fg(t.muted))),
                    None if has_tasks && start.is_some() => Some(Span::raw("  ")),
                    None => None,
                };
                let mut spans = Vec::new();
                if start.is_some() {
                    spans.push(Span::styled(number, Style::default().fg(t.accent)));
                }
                match task {
                    Some(task) => spans.push(task),
                    None if start.is_none() => {
                        spans.push(Span::styled("• ", Style::default().fg(t.accent)));
                    }
                    None => {}
                }
                spans
            })
            .collect();
        let width_of = |m: &[Span]| m.iter().map(Span::width).sum::<usize>();
        let marker_w = markers.iter().map(|m| width_of(m)).max().unwrap_or(2);
        let pad = [Span::raw(" ".repeat(marker_w))];

        let mut out = Out::default();
        for (item, mut first) in items.iter().zip(markers) {
            let gap = marker_w - width_of(&first);
            if gap > 0 {
                first.insert(0, Span::raw(" ".repeat(gap)));
            }
            let inner = self.blocks(item, width.saturating_sub(marker_w), false);
            out.append(non_empty(inner), &first, &pad);
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
                if let (Some(url), Some(id)) = (&i.style.link, i.style.link_id)
                    && is_link_style(s, t)
                {
                    let chars = i.text.chars().filter(|c| !c.is_whitespace()).count();
                    let mut link_runs = self.link_runs.borrow_mut();
                    // `[the **docs**](url)` is several inlines but one link.
                    match link_runs.last_mut() {
                        Some(run) if run.id == id => run.chars += chars,
                        _ => link_runs.push(LinkRun {
                            url: url.clone(),
                            chars,
                            id,
                        }),
                    }
                }
                (i.text.clone(), s)
            })
            .collect()
    }
}

/// `out`, or a blank line when it has none, so a marker put in front of an
/// empty list item or footnote still shows.
impl Renderer<'_> {
    /// A details block: `▶ summary` when closed, `▼ summary` with its
    /// blocks indented underneath when open.
    fn details(
        &self,
        id: usize,
        open: bool,
        summary: &[Inline],
        blocks: &[Block],
        width: usize,
    ) -> Out {
        let t = self.theme;
        let shown = self
            .cx
            .toggled_details
            .is_none_or(|toggled| toggled.contains(&id) != open);
        let marker = Span::styled(
            if shown { "▼ " } else { "▶ " },
            Style::default().fg(t.accent),
        );
        let pad = [Span::raw("  ")];
        let summary = wrap(
            &self.runs(
                summary,
                Style::default().fg(t.text).add_modifier(Modifier::BOLD),
            ),
            width.saturating_sub(2).max(1),
        );
        let mut out = Out::default();
        out.append(Out::from(summary), &[marker], &pad);
        out.details.push(DetailsSpan { id, line: 0 });
        if shown {
            out.append(
                self.blocks(blocks, width.saturating_sub(2), true),
                &pad,
                &pad,
            );
        }
        out
    }
}

fn non_empty(out: Out) -> Out {
    if out.lines.is_empty() {
        Out::from(vec![Line::default()])
    } else {
        out
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
    fn callouts_take_their_colour_from_the_theme() {
        let t = Theme::dark();
        let src = "> [!NOTE]\n> a\n\n> [!TIP]\n> a\n\n> [!IMPORTANT]\n> a\n\n> [!WARNING]\n> a\n\n> [!CAUTION]\n> a\n";
        let r = render(&parse(src), 40, &t);
        let titles: Vec<_> = r
            .lines
            .iter()
            .filter(|l| l.spans.len() == 2 && l.spans[0].content == "▌ ")
            .filter(|l| !l.spans[1].content.starts_with('a'))
            .map(|l| {
                // The title's colour may sit on the line rather than the span.
                let text = l.style.patch(l.spans[1].style);
                (l.spans[1].content.to_string(), l.spans[0].style.fg, text)
            })
            .collect();
        let expect = [
            ("Note", t.link),
            ("Tip", t.ok),
            ("Important", t.accent),
            ("Warning", t.warn),
            ("Caution", t.err),
        ];
        assert_eq!(titles.len(), expect.len(), "{titles:?}");
        for ((title, bar, text), (want, color)) in titles.iter().zip(expect) {
            assert_eq!(title, want);
            assert_eq!(*bar, Some(color), "{title} bar");
            assert_eq!(text.fg, Some(color), "{title} title");
            assert!(
                text.add_modifier.contains(Modifier::BOLD),
                "{title} not bold"
            );
        }
    }

    #[test]
    fn empty_task_items_and_footnotes_keep_their_marker() {
        let r = render(
            &parse("- [ ]\n- [x] \n- c\n\nx[^a]\n\n[^a]:\n"),
            40,
            &Theme::dark(),
        );
        let lines: Vec<String> = r
            .lines
            .iter()
            .map(|l| plain(l).trim_end().to_string())
            .collect();
        assert_eq!(lines[..3], ["☐", "☑", "• c"], "{lines:?}");
        assert_eq!(lines.last().map(String::as_str), Some("[1]"), "{lines:?}");
    }

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
            toggled_details: None,
            opts: short,
        };
        let r = render_with_runs(&doc, 80, &Theme::dark(), cx);
        assert_eq!(r.images[0].rows, 10);
    }
}
