mod attrs;
mod mdx;
mod parse;

pub use attrs::Attrs;
pub use parse::parse;

use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Parse a document, choosing the MDX front end for `.mdx` files. Invalid
/// MDX falls back to the plain Markdown parser so the file still shows.
pub fn parse_for(src: &str, path: Option<&Path>) -> Document {
    let ext = path
        .and_then(|p| p.extension())
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    match ext.as_deref() {
        Some("mdx") => mdx::parse(src).unwrap_or_else(|| parse(src)),
        Some(e) if !is_markdown_ext(e) && crate::syntax::known_extension(e) => code_file(src, e),
        _ => parse(src),
    }
}

/// Deepest nesting of quotes and list items kept. Containers past it are
/// flattened into their parent, so recursion over the document (rendering,
/// `visit`, drop) stays shallow however the source nests.
pub(crate) const MAX_DEPTH: usize = 16;

/// Start of the text standing in for an image: `[image: alt]`.
pub(crate) const IMAGE_PREFIX: &str = "[image: ";

/// The text standing in for an image with `alt` text.
pub(crate) fn image_placeholder(alt: &str) -> String {
    format!("{IMAGE_PREFIX}{alt}]")
}

/// File extensions of Markdown documents, in lowercase.
pub const MARKDOWN_EXTENSIONS: &[&str] = &["md", "markdown", "mdx", "mdown", "mkdn", "mkd"];

/// Extensions treated as Markdown, in any case.
pub fn is_markdown_ext(ext: &str) -> bool {
    MARKDOWN_EXTENSIONS
        .iter()
        .any(|m| m.eq_ignore_ascii_case(ext))
}

/// A source file shown as one highlighted code block with source line
/// numbers, rather than as Markdown.
fn code_file(src: &str, lang: &str) -> Document {
    let mut src = src.to_string();
    if !src.ends_with('\n') {
        src.push('\n');
    }
    Document {
        blocks: vec![Block::Code(CodeBlock {
            id: 0,
            lang: Some(lang.to_string()),
            attrs: Attrs::default(),
            src,
        })],
        lines: vec![0],
        code_file: true,
    }
}

/// A parsed Markdown document, reduced to the block structure bastian renders.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Document {
    pub blocks: Vec<Block>,
    /// Source line (0-based) where each top-level block starts, parallel to
    /// `blocks`. May be empty when unknown.
    pub lines: Vec<usize>,
    /// A source file rather than Markdown: rendered bare, with source
    /// line numbers.
    pub code_file: bool,
}

impl Document {
    /// Text of the first level-1 heading, if any.
    pub fn title(&self) -> Option<String> {
        self.blocks.iter().find_map(|b| match b {
            Block::Heading { level: 1, inlines } => Some(plain_text(inlines)),
            _ => None,
        })
    }
}

/// Call `f` on every block in document order, descending into quotes and
/// list items.
pub fn visit<'a>(blocks: &'a [Block], f: &mut impl FnMut(&'a Block)) {
    for b in blocks {
        f(b);
        match b {
            Block::Quote(inner)
            | Block::Callout { blocks: inner, .. }
            | Block::Footnote { blocks: inner, .. }
            | Block::Details { blocks: inner, .. } => visit(inner, f),
            Block::List { items, .. } => items.iter().for_each(|i| visit(i, f)),
            _ => {}
        }
    }
}

/// Append `text` to `out`, merging with the last run when the style matches
/// to keep spans few.
fn push_inline(out: &mut Vec<Inline>, text: &str, style: &InlineStyle) {
    if let Some(last) = out.last_mut()
        && last.style == *style
    {
        last.text.push_str(text);
    } else {
        out.push(Inline {
            text: text.to_string(),
            style: style.clone(),
        });
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading {
        level: u8,
        inlines: Vec<Inline>,
    },
    Paragraph(Vec<Inline>),
    List {
        /// Starting number for ordered lists, `None` for bullet lists.
        start: Option<u64>,
        items: Vec<Vec<Block>>,
        /// Per item, parallel to `items`: `Some(done)` for task items.
        tasks: Vec<Option<bool>>,
    },
    Quote(Vec<Block>),
    /// A GitHub alert: `> [!NOTE]` and friends.
    Callout {
        kind: CalloutKind,
        blocks: Vec<Block>,
    },
    Code(CodeBlock),
    /// A footnote definition, listed at the end of the document.
    Footnote {
        label: String,
        /// Shown as `[number]`, in order of first reference.
        number: usize,
        blocks: Vec<Block>,
    },
    Table {
        /// Per-column alignment; may be shorter than the widest row.
        align: Vec<Align>,
        header: Vec<Vec<Inline>>,
        rows: Vec<Vec<Vec<Inline>>>,
    },
    Rule,
    /// A muted single-line marker, e.g. an MDX component's opening or
    /// closing tag.
    Label(String),
    /// A paragraph holding just an image, shown as the picture where the
    /// terminal can.
    Image {
        alt: String,
        url: String,
    },
    /// An HTML `<details>` disclosure: the summary line, with `blocks`
    /// shown when it's open.
    Details {
        /// Index among the document's details blocks, in display order.
        id: usize,
        /// Open to begin with (`<details open>`).
        open: bool,
        summary: Vec<Inline>,
        blocks: Vec<Block>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalloutKind {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

impl CalloutKind {
    pub fn title(self) -> &'static str {
        match self {
            CalloutKind::Note => "Note",
            CalloutKind::Tip => "Tip",
            CalloutKind::Important => "Important",
            CalloutKind::Warning => "Warning",
            CalloutKind::Caution => "Caution",
        }
    }

    /// The kind an alert marker names: `NOTE` in `> [!NOTE]`, any case.
    pub fn from_marker(marker: &str) -> Option<Self> {
        Some(match marker.to_ascii_uppercase().as_str() {
            "NOTE" => CalloutKind::Note,
            "TIP" => CalloutKind::Tip,
            "IMPORTANT" => CalloutKind::Important,
            "WARNING" => CalloutKind::Warning,
            "CAUTION" => CalloutKind::Caution,
            _ => return None,
        })
    }
}

/// Horizontal alignment of a table column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    None,
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CodeBlock {
    /// Document-order index, stable across re-renders.
    pub id: usize,
    pub lang: Option<String>,
    pub attrs: Attrs,
    pub src: String,
}

/// A code block from its fence's `info` string, e.g. `sh {name=x}`.
fn code_block(info: &str, src: String) -> Block {
    Block::Code(CodeBlock {
        id: 0,
        lang: lang_of(info),
        attrs: Attrs::parse(info),
        src,
    })
}

/// A details block with its `blocks`, summed up as "Details" when it has
/// no `<summary>`.
fn details(open: bool, summary: Option<Vec<Inline>>, blocks: Vec<Block>) -> Block {
    let summary = summary.unwrap_or_else(|| {
        vec![Inline {
            text: "Details".into(),
            style: InlineStyle::default(),
        }]
    });
    Block::Details {
        id: 0,
        open,
        summary,
        blocks,
    }
}

/// Language token of a fence info string: `sh {name=x}` and `sh{name=x}` give `sh`.
fn lang_of(info: &str) -> Option<String> {
    let lang = info
        .split(|c: char| c.is_whitespace() || c == '{' || c == ',')
        .next()
        .unwrap_or("");
    (!lang.is_empty()).then(|| lang.to_string())
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Inline {
    pub text: String,
    pub style: InlineStyle,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct InlineStyle {
    pub strong: bool,
    pub emphasis: bool,
    pub strike: bool,
    pub code: bool,
    /// Underlined by an HTML `<u>` tag.
    pub underline: bool,
    /// Highlighted: `==text==` or an HTML `<mark>` tag.
    pub mark: bool,
    pub link: Option<String>,
    /// Which link occurrence this text belongs to, numbered through the
    /// document: the parts of `[the **docs**](x)` share one, while
    /// `[a](x)[b](x)` are two.
    pub link_id: Option<usize>,
    /// De-emphasized syntax such as inline JSX tags and expressions.
    pub muted: bool,
    /// A footnote reference, left out of heading anchors.
    pub footnote: bool,
    /// One inline HTML tag as the Markdown front end read it, until
    /// `finish` turns formatting tags into styles. Never set in a finished
    /// document.
    pub(crate) raw_html: bool,
}

/// Anchor (without `#`) that a footnote's references link to. Labels
/// match case-insensitively; anything else in them is kept, so distinct
/// labels never share an anchor. The `:` keeps it apart from heading
/// anchors, which never contain one.
pub fn footnote_anchor(label: &str) -> String {
    format!("fn:{}", label.to_lowercase())
}

/// A heading's text for its anchor: without footnote references, as
/// GitHub slugs it.
pub fn anchor_text(inlines: &[Inline]) -> String {
    inlines
        .iter()
        .filter(|i| !i.style.footnote)
        .map(|i| i.text.as_str())
        .collect()
}

/// Footnotes gathered while parsing. References are numbered in the order
/// they first appear; definitions are held back and listed at the end.
#[derive(Default)]
struct Footnotes {
    /// Lowercased label → number, in order of first reference.
    refs: HashMap<String, usize>,
    /// `(label, blocks, source line)` of each definition.
    defs: Vec<(String, Vec<Block>, usize)>,
    /// Lowercased labels of `defs`.
    defined: HashSet<String>,
}

impl Footnotes {
    /// The `[n]` text for a reference to `label`, linking to its note as
    /// link occurrence `link_id`.
    fn reference(&mut self, label: &str, style: &InlineStyle, link_id: usize) -> Inline {
        let next = self.refs.len() + 1;
        let n = *self.refs.entry(label.to_lowercase()).or_insert(next);
        let mut style = style.clone();
        style.link = Some(format!("#{}", footnote_anchor(label)));
        style.link_id = Some(link_id);
        style.footnote = true;
        Inline {
            text: format!("[{n}]"),
            style,
        }
    }

    /// Whether a definition of `label` should be read: as on GitHub, the
    /// first one wins, and later ones are skipped whole, so nothing in them
    /// (references, code blocks) counts.
    fn claim(&mut self, label: &str) -> bool {
        self.defined.insert(label.to_lowercase())
    }

    /// Record a definition `claim` allowed.
    fn define(&mut self, label: String, blocks: Vec<Block>, line: usize) {
        self.defs.push((label, blocks, line));
    }

    /// Append a rule and the notes to a document's top-level `blocks` (and
    /// their source `lines`): referenced ones by number, then any that are
    /// never referenced.
    fn append_to(self, blocks: &mut Vec<Block>, lines: &mut Vec<usize>) {
        let mut unreferenced = self.refs.len();
        let mut notes: Vec<_> = self
            .defs
            .into_iter()
            .map(|(label, inner, line)| {
                let number = if let Some(&n) = self.refs.get(&label.to_lowercase()) {
                    n
                } else {
                    unreferenced += 1;
                    unreferenced
                };
                (number, label, inner, line)
            })
            .collect();
        notes.sort_by_key(|n| n.0);
        let Some(first) = notes.first() else {
            return;
        };
        blocks.push(Block::Rule);
        lines.push(first.3);
        for (number, label, inner, line) in notes {
            blocks.push(Block::Footnote {
                label,
                number,
                blocks: inner,
            });
            lines.push(line);
        }
    }
}

/// The document a front end built: its top-level `blocks` with their
/// source `lines`, then the `footnotes`. Front ends leave code and details
/// ids at 0; they are numbered here, in the order blocks are shown in.
fn finish(mut blocks: Vec<Block>, mut lines: Vec<usize>, footnotes: Footnotes) -> Document {
    footnotes.append_to(&mut blocks, &mut lines);
    renumber(&mut blocks, &mut Ids::default());
    each_inlines(&mut blocks, &mut |inlines| {
        *inlines = marks(inline_html(std::mem::take(inlines)));
    });
    Document {
        blocks,
        lines,
        code_file: false,
    }
}

/// The next id of each numbered thing, handed out in document order: link
/// ids by the front ends as they read, code and details ids by `renumber`.
#[derive(Default)]
pub(super) struct Ids {
    code: usize,
    link: usize,
    details: usize,
}

impl Ids {
    fn code(&mut self) -> usize {
        self.code += 1;
        self.code - 1
    }

    /// One link occurrence, shared by every inline it spans.
    pub(super) fn link(&mut self) -> usize {
        self.link += 1;
        self.link - 1
    }

    fn details(&mut self) -> usize {
        self.details += 1;
        self.details - 1
    }
}

/// Number code and details blocks in document order, as `visit` walks them.
fn renumber(blocks: &mut [Block], next: &mut Ids) {
    for b in blocks {
        match b {
            Block::Code(c) => c.id = next.code(),
            Block::Details { id, blocks, .. } => {
                *id = next.details();
                renumber(blocks, next);
            }
            Block::Quote(inner)
            | Block::Callout { blocks: inner, .. }
            | Block::Footnote { blocks: inner, .. } => renumber(inner, next),
            Block::List { items, .. } => {
                for item in items {
                    renumber(item, next);
                }
            }
            _ => {}
        }
    }
}

/// Call `f` on every run of inlines in `blocks`: headings, paragraphs,
/// table cells and details summaries, descending into containers.
fn each_inlines(blocks: &mut [Block], f: &mut impl FnMut(&mut Vec<Inline>)) {
    for b in blocks {
        match b {
            Block::Heading { inlines, .. } | Block::Paragraph(inlines) => f(inlines),
            Block::Table { header, rows, .. } => {
                for cell in header.iter_mut().chain(rows.iter_mut().flatten()) {
                    f(cell);
                }
            }
            Block::Details {
                summary, blocks, ..
            } => {
                f(summary);
                each_inlines(blocks, f);
            }
            Block::Quote(inner)
            | Block::Callout { blocks: inner, .. }
            | Block::Footnote { blocks: inner, .. } => each_inlines(inner, f),
            Block::List { items, .. } => {
                for item in items {
                    each_inlines(item, f);
                }
            }
            Block::Code(_) | Block::Rule | Block::Label(_) | Block::Image { .. } => {}
        }
    }
}

/// The style an inline HTML formatting element gives its content. Both
/// front ends map tags through it, so `.md` and `.mdx` agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Format {
    Strong,
    Emphasis,
    Underline,
    Strike,
    Code,
    Mark,
}

impl Format {
    const ALL: [Format; 6] = [
        Format::Strong,
        Format::Emphasis,
        Format::Underline,
        Format::Strike,
        Format::Code,
        Format::Mark,
    ];

    /// The format of the HTML element `name`, in any case.
    pub(crate) fn of_tag(name: &str) -> Option<Self> {
        Some(match name.to_ascii_lowercase().as_str() {
            "b" | "strong" => Format::Strong,
            "i" | "em" => Format::Emphasis,
            "u" => Format::Underline,
            "s" | "del" | "strike" => Format::Strike,
            // A key is shown as a code chip, the terminal's nearest thing
            // to the box a browser draws around it.
            "code" | "kbd" => Format::Code,
            "mark" => Format::Mark,
            _ => return None,
        })
    }

    pub(crate) fn apply(self, s: &mut InlineStyle) {
        match self {
            Format::Strong => s.strong = true,
            Format::Emphasis => s.emphasis = true,
            Format::Underline => s.underline = true,
            Format::Strike => s.strike = true,
            Format::Code => s.code = true,
            Format::Mark => s.mark = true,
        }
    }
}

/// Whether the HTML element `name` is a line break, `<br>`.
pub(crate) fn is_break_tag(name: &str) -> bool {
    name.eq_ignore_ascii_case("br")
}

/// One HTML tag's source, read as `(name, closing, self_closing)`:
/// `<b class="x">`, `</b>` and `<br/>`. `None` for anything else.
fn html_tag(raw: &str) -> Option<(&str, bool, bool)> {
    let body = raw.strip_prefix('<')?.strip_suffix('>')?;
    let (body, closing) = match body.strip_prefix('/') {
        Some(rest) => (rest, true),
        None => (body, false),
    };
    let end = body
        .find(|c: char| !c.is_ascii_alphanumeric())
        .unwrap_or(body.len());
    let name = &body[..end];
    if !name.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return None;
    }
    let rest = &body[end..];
    if !rest.is_empty() && !rest.starts_with(|c: char| c.is_whitespace() || c == '/') {
        return None;
    }
    Some((name, closing, rest.trim_end().ends_with('/')))
}

/// `inlines` with the Markdown front end's inline HTML read: paired
/// formatting tags (`<b>x</b>`) style the inlines between them and are
/// dropped, and `<br>` is a line break. Tags pair by name, the nearest
/// open one first; unpaired and unknown tags stay as their source text.
fn inline_html(inlines: Vec<Inline>) -> Vec<Inline> {
    if !inlines.iter().any(|i| i.style.raw_html) {
        return inlines;
    }
    // Per inline: the format a paired tag there starts (`true`) or ends.
    let mut role: Vec<Option<(Format, bool)>> = vec![None; inlines.len()];
    let mut open: HashMap<String, Vec<usize>> = HashMap::new();
    let mut breaks = HashSet::new();
    for (i, inl) in inlines.iter().enumerate() {
        if !inl.style.raw_html {
            continue;
        }
        let Some((name, closing, self_closing)) = html_tag(&inl.text) else {
            continue;
        };
        if is_break_tag(name) && !closing {
            breaks.insert(i);
            continue;
        }
        let Some(format) = Format::of_tag(name) else {
            continue;
        };
        let stack = open.entry(name.to_ascii_lowercase()).or_default();
        match (closing, self_closing) {
            (false, false) => stack.push(i),
            (true, _) => {
                if let Some(start) = stack.pop() {
                    role[start] = Some((format, true));
                    role[i] = Some((format, false));
                }
            }
            // `<b/>` holds nothing; it stays as text.
            (false, true) => {}
        }
    }
    // Pairs open around each inline, by format.
    let mut depth = [0u32; Format::ALL.len()];
    let mut out = Vec::with_capacity(inlines.len());
    for (i, inl) in inlines.into_iter().enumerate() {
        if let Some((format, starts)) = role[i] {
            let d = &mut depth[format as usize];
            *d = if starts { *d + 1 } else { d.saturating_sub(1) };
            continue;
        }
        let mut style = inl.style;
        style.raw_html = false;
        for format in Format::ALL {
            if depth[format as usize] > 0 {
                format.apply(&mut style);
            }
        }
        let text = if breaks.contains(&i) { "\n" } else { &inl.text };
        push_inline(&mut out, text, &style);
    }
    out
}

/// `inlines` with each `==text==` highlighted and its markers dropped. A
/// marker is exactly two `=` outside code and muted syntax; it opens
/// before a non-space and closes after one, so `a == b` stays text, as
/// does a marker left unmatched.
fn marks(inlines: Vec<Inline>) -> Vec<Inline> {
    let plain = |i: &Inline| !i.style.code && !i.style.muted;
    if !inlines.iter().any(|i| plain(i) && i.text.contains("==")) {
        return inlines;
    }
    let flat: String = inlines.iter().map(|i| i.text.as_str()).collect();
    let b = flat.as_bytes();
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    let mut opener = None;
    let mut start = 0;
    for inl in &inlines {
        let end = start + inl.text.len();
        if plain(inl) {
            for p in start..end.saturating_sub(1) {
                let marker = b[p] == b'='
                    && b[p + 1] == b'='
                    && (p == 0 || b[p - 1] != b'=')
                    && b.get(p + 2) != Some(&b'=');
                if !marker {
                    continue;
                }
                let before = flat[..p].chars().next_back();
                let after = flat[p + 2..].chars().next();
                let solid = |c: Option<char>| c.is_some_and(|c| !c.is_whitespace());
                match opener {
                    Some(o) if solid(before) => {
                        pairs.push((o, p));
                        opener = None;
                    }
                    _ if solid(after) => opener = Some(p),
                    _ => {}
                }
            }
        }
        start = end;
    }
    if pairs.is_empty() {
        return inlines;
    }
    let cuts: Vec<usize> = pairs
        .iter()
        .flat_map(|&(o, c)| [o, o + 2, c, c + 2])
        .collect();
    // Cuts before a segment, mod 4: 0 plain, 1 opening marker, 2 marked,
    // 3 closing marker.
    let mut out = Vec::with_capacity(inlines.len());
    let (mut start, mut k) = (0, 0);
    for inl in inlines {
        let end = start + inl.text.len();
        let mut s = start;
        while s < end {
            while cuts.get(k).is_some_and(|&c| c <= s) {
                k += 1;
            }
            let e = cuts.get(k).map_or(end, |&c| c.min(end));
            if k % 2 == 0 {
                let mut style = inl.style.clone();
                style.mark |= k % 4 == 2;
                push_inline(&mut out, &inl.text[s - start..e - start], &style);
            }
            s = e;
        }
        start = end;
    }
    out
}

/// The relative link a wiki link's target stands for: `Page` is
/// `Page.md` next to the document, and a `#Heading` part becomes that
/// heading's anchor. Targets with an extension are kept as they are.
pub(super) fn wiki_target(target: &str) -> String {
    let (path, heading) = match target.split_once('#') {
        Some((path, heading)) => (path.trim(), Some(heading)),
        None => (target.trim(), None),
    };
    let mut url = path.to_string();
    if !path.is_empty() && Path::new(path).extension().is_none() {
        url.push_str(".md");
    }
    if let Some(heading) = heading {
        url.push('#');
        url.push_str(&crate::links::slug(heading));
    }
    url
}

pub fn plain_text(inlines: &[Inline]) -> String {
    inlines.iter().map(|i| i.text.as_str()).collect()
}
