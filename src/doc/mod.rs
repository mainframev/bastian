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
        Some(e) if !is_markdown_ext(e) && crate::render::known_extension(e) => code_file(src, e),
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
    pub link: Option<String>,
    /// Which link occurrence this text belongs to, numbered through the
    /// document: the parts of `[the **docs**](x)` share one, while
    /// `[a](x)[b](x)` are two.
    pub link_id: Option<usize>,
    /// De-emphasized syntax such as inline JSX tags and expressions.
    pub muted: bool,
    /// A footnote reference, left out of heading anchors.
    pub footnote: bool,
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
    /// The `[n]` text for a reference to `label`, linking to its note.
    fn reference(&mut self, label: &str, style: &InlineStyle) -> Inline {
        let next = self.refs.len() + 1;
        let n = *self.refs.entry(label.to_lowercase()).or_insert(next);
        let mut style = style.clone();
        style.link = Some(format!("#{}", footnote_anchor(label)));
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
        // Code and details in notes were numbered where they were defined;
        // ids follow the order blocks are shown in.
        renumber(blocks, &mut Ids::default());
    }
}

/// Next code block and details ids while renumbering.
#[derive(Default)]
struct Ids {
    code: usize,
    details: usize,
}

/// Number code and details blocks in document order, as `visit` walks them.
fn renumber(blocks: &mut [Block], next: &mut Ids) {
    for b in blocks {
        match b {
            Block::Code(c) => {
                c.id = next.code;
                next.code += 1;
            }
            Block::Details { id, blocks, .. } => {
                *id = next.details;
                next.details += 1;
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

pub fn plain_text(inlines: &[Inline]) -> String {
    inlines.iter().map(|i| i.text.as_str()).collect()
}
