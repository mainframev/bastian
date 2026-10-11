mod parse;

pub use parse::parse;

/// A parsed Markdown document, reduced to the block structure bastian renders.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Document {
    pub blocks: Vec<Block>,
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct CodeBlock {
    /// Document-order index, stable across re-renders.
    pub id: usize,
    pub lang: Option<String>,
    /// Full fence info string, e.g. `sh {name=deploy confirm}`.
    pub info: String,
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
}

pub fn plain_text(inlines: &[Inline]) -> String {
    inlines.iter().map(|i| i.text.as_str()).collect()
}
