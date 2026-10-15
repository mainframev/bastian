mod ansi;
pub(crate) mod code;
mod codeblock;
#[cfg(feature = "mermaid")]
mod diagram;
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

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::ops::Range;

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::doc::{Block, CalloutKind, Document, Inline, plain_text};
use crate::exec::{BlockRun, Interpreters};
use links::{LinkRun, is_link_style, locate_links};
use theme::Theme;
use wrap::{PAD, wrap};

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
    /// Pad inline code into a chip. Off for plain text, where the padding
    /// has no background and reads as doubled spaces.
    pub chips: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            image_rows: 24,
            tab_width: 4,
            chips: true,
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

/// Whether a code fence's language names a Mermaid diagram.
pub(crate) fn is_mermaid(lang: Option<&str>) -> bool {
    lang.is_some_and(|l| l.eq_ignore_ascii_case("mermaid"))
}

/// Bullets of unordered lists, by nesting depth; deeper lists cycle.
const BULLETS: [&str; 3] = ["• ", "◦ ", "▸ "];

/// The bar down the left of quotes and run output.
const QUOTE_BAR: &str = "│ ";

pub fn render(doc: &Document, width: u16, theme: &Theme) -> Rendered {
    render_with(doc, width, theme, Options::default())
}

/// Render without run output or pictures, with `opts`.
pub fn render_with(doc: &Document, width: u16, theme: &Theme, opts: Options) -> Rendered {
    let (runs, interpreters) = (HashMap::new(), Interpreters::default());
    let (highlights, images) = (Highlights::default(), ImageSizes::new());
    let as_source = HashSet::new();
    let cx = Context {
        runs: &runs,
        interpreters: &interpreters,
        highlights: &highlights,
        images: &images,
        toggled_details: None,
        as_source: &as_source,
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
    /// Diagram blocks the reader switched to show their source.
    pub as_source: &'a HashSet<usize>,
    pub opts: Options,
}

/// Render with each code block's run status and output drawn under it.
pub(crate) fn render_with_runs(doc: &Document, width: u16, theme: &Theme, cx: Context) -> Rendered {
    let r = Renderer {
        theme,
        cx,
        link_runs: RefCell::default(),
        list_depth: Cell::default(),
        in_quote: Cell::default(),
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
    /// Lists enclosing the block being rendered.
    list_depth: Cell<usize>,
    /// Whether the block being rendered is in a block quote.
    in_quote: Cell<bool>,
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
                let outer = self.in_quote.replace(true);
                let inner = self.blocks(inner, width.saturating_sub(2), true);
                self.in_quote.set(outer);
                out.append(inner, &bar, &bar);
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
                // Upright even inside a quote.
                let outer = self.in_quote.replace(false);
                let body = self.blocks(blocks, width.saturating_sub(2), true);
                self.in_quote.set(outer);
                inner.append(body, &[], &[]);
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
            // Color tells the levels apart; bold down to 5, italic for 6.
            let mut style = Style::default().fg(t.heading_color(level));
            style = if level < 6 {
                style.bold()
            } else {
                style.italic()
            };
            let mut lines = wrap(&self.runs(inlines, style), width);
            if level == 2 {
                let w = lines.iter().map(Line::width).max().unwrap_or(0);
                lines.push(Line::styled(
                    "─".repeat(w),
                    Style::default().fg(t.heading_underline),
                ));
            }
            lines
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
        let depth = self.list_depth.get();
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
                        let i = depth % BULLETS.len();
                        spans.push(Span::styled(BULLETS[i], Style::default().fg(t.bullets[i])));
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
        self.list_depth.set(depth + 1);
        for (item, mut first) in items.iter().zip(markers) {
            let gap = marker_w - width_of(&first);
            if gap > 0 {
                first.insert(0, Span::raw(" ".repeat(gap)));
            }
            let inner = self.blocks(item, width.saturating_sub(marker_w), false);
            out.append(non_empty(inner), &first, &pad);
        }
        self.list_depth.set(depth);
        out
    }

    /// Styled text of `inlines`, with inline code padded into a chip.
    fn runs(&self, inlines: &[Inline], base: Style) -> Vec<(String, Style)> {
        self.styled_runs(inlines, base, self.cx.opts.chips)
    }

    /// Styled text of `inlines`. With `chips`, inline code gets a space of
    /// its background on each side; `wrap` keeps that padding with the code.
    fn styled_runs(&self, inlines: &[Inline], base: Style, chips: bool) -> Vec<(String, Style)> {
        let t = self.theme;
        let mut out = Vec::with_capacity(inlines.len());
        for i in inlines {
            // Quoted text is italic; code keeps its upright face.
            let base = if self.in_quote.get() && !i.style.code {
                base.add_modifier(Modifier::ITALIC)
            } else {
                base
            };
            let (text, s) = self.styled(i, base);
            if chips && i.style.code {
                // Not underlined or struck, so a link's padding isn't part
                // of the link.
                let pad = (PAD.to_string(), Style::default().bg(t.surface));
                out.push(pad.clone());
                out.push((text, s));
                out.push(pad);
            } else {
                out.push((text, s));
            }
        }
        out
    }

    /// One inline's text and style; records the link it belongs to, if any.
    fn styled(&self, i: &Inline, base: Style) -> (String, Style) {
        let t = self.theme;
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
    fn headings_take_their_level_colour_and_h2_an_underline() {
        let t = crate::themes::load("catppuccin-mocha", None).unwrap();
        let src = "## Two words\n\n### Three\n\n#### Four\n\n##### Five\n\n###### Six\n";
        let r = render(&parse(src), 40, &t);
        let text: Vec<String> = r.lines.iter().map(|l| plain(l)).collect();
        assert_eq!(
            text,
            [
                "Two words",
                "─────────",
                "",
                "Three",
                "",
                "Four",
                "",
                "Five",
                "",
                "Six"
            ]
        );
        let style = |i: usize| r.lines[i].style.patch(r.lines[i].spans[0].style);
        assert_eq!(style(1).fg, Some(t.heading_underline));
        for (line, level) in [(0, 2), (3, 3), (5, 4), (7, 5), (9, 6)] {
            assert_eq!(
                style(line).fg,
                Some(t.heading_color(level)),
                "level {level}"
            );
            let bold = style(line).add_modifier.contains(Modifier::BOLD);
            let italic = style(line).add_modifier.contains(Modifier::ITALIC);
            assert_eq!((bold, italic), (level < 6, level == 6), "level {level}");
        }
        // The levels differ in this theme.
        assert_ne!(t.heading_color(2), t.heading_color(3));
        assert_ne!(t.heading_color(3), t.heading_color(4));
        // Block starts point at the heading text, not its underline.
        assert_eq!(r.block_starts[..2], [0, 3]);

        // A wrapped heading is underlined as wide as its widest line.
        let r = render(&parse("## one two three four\n"), 12, &t);
        let text: Vec<String> = r.lines.iter().map(|l| plain(l)).collect();
        assert_eq!(text, ["one two", "three four", "──────────"]);
    }

    #[test]
    fn bullets_cycle_by_depth_in_their_colours() {
        use ratatui::style::Color;
        let mut t = Theme::dark();
        // Without bullet colours the theme uses its accent for all of them.
        assert_eq!(t.bullets, [t.accent; 3]);
        t.bullets = [Color::Red, Color::Green, Color::Blue];
        let src = "- a\n  - b\n    - c\n      - d\n- e\n\n1. one\n   - f\n";
        let r = render(&parse(src), 40, &t);
        let bullets: Vec<(String, Option<Color>)> = r
            .lines
            .iter()
            .filter_map(|l| l.spans.iter().find(|s| !s.content.trim().is_empty()))
            .map(|s| (s.content.to_string(), s.style.fg))
            .collect();
        let b = |s: &str, c| (s.to_string(), Some(c));
        assert_eq!(
            bullets,
            [
                b("• ", Color::Red),
                b("◦ ", Color::Green),
                b("▸ ", Color::Blue),
                b("• ", Color::Red),
                b("• ", Color::Red),
                b("1. ", t.accent),
                // Nested in a numbered list, a level down.
                b("◦ ", Color::Green),
            ]
        );
    }

    #[test]
    fn inline_code_is_padded_into_a_chip() {
        let t = Theme::dark();
        let r = render(&parse("Run `cargo build` now\n"), 40, &t);
        assert_eq!(plain(&r.lines[0]), "Run  cargo build  now");
        // Which columns sit on the code background.
        let on_chip: String = r.lines[0]
            .spans
            .iter()
            .flat_map(|s| {
                let mark = if s.style.bg == Some(t.surface) {
                    '#'
                } else {
                    '.'
                };
                std::iter::repeat_n(mark, s.content.chars().count())
            })
            .collect();
        assert_eq!(on_chip, "....#############....");

        // The padding stays with the code when it wraps, and lines keep
        // to the width.
        let r = render(&parse("aaaa `bbbb` cc `dd`.\n"), 10, &t);
        let text: Vec<String> = r.lines.iter().map(|l| plain(l)).collect();
        assert_eq!(text, ["aaaa", " bbbb  cc", " dd ."], "{text:?}");
        assert!(r.lines.iter().all(|l| l.width() <= 10));

        // A link's padding isn't part of the link.
        let r = render(&parse("see [`docs`](https://x.dev)\n"), 40, &t);
        let link = &r.links[0];
        let text: String = plain(&r.lines[0])
            .chars()
            .skip(link.col)
            .take(link.width)
            .collect();
        assert_eq!(text, "docs");

        // Tables size columns by the text, so cells aren't padded.
        let r = render(&parse("| a |\n|---|\n| `x` |\n"), 40, &t);
        assert_eq!(plain(&r.lines[2]).trim(), "x");
    }

    #[test]
    fn quoted_text_is_italic_but_its_code_and_bar_are_not() {
        let r = render(&parse("> some `code` here\n\nafter\n"), 40, &Theme::dark());
        let italic = |s: &Span| s.style.add_modifier.contains(Modifier::ITALIC);
        let spans = &r.lines[0].spans;
        let style_of = |text: &str| spans.iter().find(|s| s.content.contains(text)).unwrap();
        assert!(
            italic(style_of("some")) && italic(style_of("here")),
            "{spans:?}"
        );
        assert!(!italic(style_of("code")));
        assert!(!italic(&spans[0]), "bar");
        assert!(
            r.lines[2].spans.iter().all(|s| !italic(s)),
            "after the quote"
        );
    }

    #[test]
    fn callouts_in_quotes_stay_upright() {
        let src = "> quoted\n>\n> > [!NOTE]\n> > inside\n";
        let r = render(&parse(src), 40, &Theme::dark());
        let span = |text: &str| {
            r.lines
                .iter()
                .flat_map(|l| &l.spans)
                .find(|s| s.content.contains(text))
                .unwrap_or_else(|| panic!("no {text}"))
                .style
        };
        assert!(span("quoted").add_modifier.contains(Modifier::ITALIC));
        assert!(!span("inside").add_modifier.contains(Modifier::ITALIC));
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
    fn chips_can_be_left_off_for_plain_text() {
        let doc = parse("Run `cargo build` now.\n");
        let text = |chips| {
            let opts = Options {
                chips,
                ..Options::default()
            };
            to_plain(&render_with(&doc, 40, &Theme::dark(), opts).lines)
        };
        assert!(
            text(true).contains("Run  cargo build  now."),
            "{}",
            text(true)
        );
        assert!(
            text(false).contains("Run cargo build now."),
            "{}",
            text(false)
        );
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
        let (runs, interpreters, highlights, as_source) = Default::default();
        let cx = Context {
            runs: &runs,
            interpreters: &interpreters,
            highlights: &highlights,
            images: &sizes,
            toggled_details: None,
            as_source: &as_source,
            opts: short,
        };
        let r = render_with_runs(&doc, 80, &Theme::dark(), cx);
        assert_eq!(r.images[0].rows, 10);
    }
}
