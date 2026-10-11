mod attrs;
mod mdx;
mod parse;

pub use attrs::Attrs;
pub use parse::parse;

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

/// Extensions treated as Markdown.
pub fn is_markdown_ext(ext: &str) -> bool {
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "md" | "markdown" | "mdx" | "mdown" | "mkdn" | "mkd"
    )
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
            info: lang.to_string(),
            src,
        })],
        code_file: true,
    }
}

/// A parsed Markdown document, reduced to the block structure bastian renders.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Document {
    pub blocks: Vec<Block>,
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
    },
    Quote(Vec<Block>),
    Code(CodeBlock),
    Table {
        header: Vec<Vec<Inline>>,
        rows: Vec<Vec<Vec<Inline>>>,
    },
    Rule,
    /// A muted single-line marker, e.g. an MDX component's opening or
    /// closing tag.
    Label(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CodeBlock {
    /// Document-order index, stable across re-renders.
    pub id: usize,
    pub lang: Option<String>,
    /// Full fence info string, e.g. `sh {name=deploy confirm}`.
    pub info: String,
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
    /// De-emphasized syntax such as inline JSX tags and expressions.
    pub muted: bool,
}

pub fn plain_text(inlines: &[Inline]) -> String {
    inlines.iter().map(|i| i.text.as_str()).collect()
}
