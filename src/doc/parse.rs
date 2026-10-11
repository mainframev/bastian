use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

use super::{Attrs, Block, CodeBlock, Document, Inline, InlineStyle};

pub fn parse(src: &str) -> Document {
    let opts = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
        | Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS;
    let newlines: Vec<usize> = src.match_indices('\n').map(|(i, _)| i).collect();
    let line_of = |offset: usize| newlines.partition_point(|&n| n < offset);
    let mut b = Builder::default();
    let mut depth = 0usize;
    for (event, range) in Parser::new_ext(src, opts).into_offset_iter() {
        // Remember where each top-level construct starts, for mapping
        // rendered lines back to the source (e.g. to open an editor there).
        if depth == 0 && matches!(event, Event::Start(_) | Event::Rule) {
            b.block_start = line_of(range.start);
        }
        match &event {
            Event::Start(_) => depth += 1,
            Event::End(_) => depth = depth.saturating_sub(1),
            _ => {}
        }
        b.event(event);
    }
    b.finish()
}

/// Open container blocks while walking the event stream.
enum Frame {
    Quote(Vec<Block>),
    List {
        start: Option<u64>,
        items: Vec<Vec<Block>>,
    },
    Item(Vec<Block>),
}

#[derive(Default)]
struct TableState {
    header: Vec<Vec<Inline>>,
    rows: Vec<Vec<Vec<Inline>>>,
    row: Vec<Vec<Inline>>,
}

#[derive(Default)]
struct Builder {
    root: Vec<Block>,
    stack: Vec<Frame>,
    inlines: Vec<Inline>,
    style: InlineStyle,
    strong: u32,
    emphasis: u32,
    strike: u32,
    heading: Option<u8>,
    code: Option<(String, String)>,
    /// Source line where the current top-level construct starts.
    block_start: usize,
    /// Source line of each root block, parallel to `root`.
    lines: Vec<usize>,
    /// Inside YAML/TOML front matter, which isn't shown.
    metadata: bool,
    table: Option<TableState>,
    next_code_id: usize,
}

impl Builder {
    fn event(&mut self, event: Event) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(_) if self.metadata => {}
            Event::Text(t) => match &mut self.code {
                Some((_, buf)) => buf.push_str(&t),
                None => self.push_text(&t),
            },
            Event::Code(t) => {
                let mut style = self.style.clone();
                style.code = true;
                self.inlines.push(Inline {
                    text: t.into_string(),
                    style,
                });
            }
            Event::Html(t) | Event::InlineHtml(t) => self.push_text(&t),
            Event::SoftBreak => self.push_text(" "),
            Event::HardBreak => self.push_text("\n"),
            Event::Rule => {
                self.flush();
                self.push_block(Block::Rule);
            }
            Event::TaskListMarker(done) => self.push_text(if done { "[x] " } else { "[ ] " }),
            Event::FootnoteReference(_) | Event::InlineMath(_) | Event::DisplayMath(_) => {}
        }
    }

    fn start(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph => self.flush(),
            Tag::Heading { level, .. } => {
                self.flush();
                self.heading = Some(level as u8);
            }
            Tag::BlockQuote(_) => {
                self.flush();
                self.stack.push(Frame::Quote(Vec::new()));
            }
            Tag::List(start) => {
                self.flush();
                self.stack.push(Frame::List {
                    start,
                    items: Vec::new(),
                });
            }
            Tag::Item => self.stack.push(Frame::Item(Vec::new())),
            Tag::CodeBlock(kind) => {
                self.flush();
                let info = match kind {
                    CodeBlockKind::Fenced(info) => info.trim().to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                self.code = Some((info, String::new()));
            }
            Tag::Table(_) => {
                self.flush();
                self.table = Some(TableState::default());
            }
            Tag::TableHead | Tag::TableRow => {
                if let Some(t) = &mut self.table {
                    t.row.clear();
                }
            }
            Tag::TableCell => self.inlines.clear(),
            Tag::Strong => self.set_count(|b| &mut b.strong, 1),
            Tag::Emphasis => self.set_count(|b| &mut b.emphasis, 1),
            Tag::Strikethrough => self.set_count(|b| &mut b.strike, 1),
            Tag::Link { dest_url, .. } => self.style.link = Some(dest_url.into_string()),
            Tag::Image { .. } => self.push_text("[image: "),
            Tag::MetadataBlock(_) => self.metadata = true,
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.flush(),
            TagEnd::Heading(_) => {
                let level = self.heading.take().unwrap_or(1);
                let inlines = std::mem::take(&mut self.inlines);
                self.push_block(Block::Heading { level, inlines });
            }
            TagEnd::BlockQuote(_) => {
                self.flush();
                if let Some(Frame::Quote(blocks)) = self.stack.pop() {
                    self.push_block(Block::Quote(blocks));
                }
            }
            TagEnd::List(_) => {
                if let Some(Frame::List { start, items }) = self.stack.pop() {
                    self.push_block(Block::List { start, items });
                }
            }
            TagEnd::Item => {
                self.flush();
                if let Some(Frame::Item(blocks)) = self.stack.pop()
                    && let Some(Frame::List { items, .. }) = self.stack.last_mut()
                {
                    items.push(blocks);
                }
            }
            TagEnd::CodeBlock => {
                if let Some((info, src)) = self.code.take() {
                    let id = self.next_code_id;
                    self.next_code_id += 1;
                    self.push_block(Block::Code(CodeBlock {
                        id,
                        lang: lang_of(&info),
                        attrs: Attrs::parse(&info),
                        info,
                        src,
                    }));
                }
            }
            TagEnd::TableCell => {
                let cell = std::mem::take(&mut self.inlines);
                if let Some(t) = &mut self.table {
                    t.row.push(cell);
                }
            }
            TagEnd::TableHead => {
                if let Some(t) = &mut self.table {
                    t.header = std::mem::take(&mut t.row);
                }
            }
            TagEnd::TableRow => {
                if let Some(t) = &mut self.table {
                    let row = std::mem::take(&mut t.row);
                    t.rows.push(row);
                }
            }
            TagEnd::Table => {
                if let Some(t) = self.table.take() {
                    self.push_block(Block::Table {
                        header: t.header,
                        rows: t.rows,
                    });
                }
            }
            TagEnd::Strong => self.set_count(|b| &mut b.strong, -1),
            TagEnd::Emphasis => self.set_count(|b| &mut b.emphasis, -1),
            TagEnd::Strikethrough => self.set_count(|b| &mut b.strike, -1),
            TagEnd::Link => self.style.link = None,
            TagEnd::Image => self.push_text("]"),
            TagEnd::MetadataBlock(_) => self.metadata = false,
            _ => {}
        }
    }

    fn set_count(&mut self, field: fn(&mut Self) -> &mut u32, delta: i32) {
        let n = field(self);
        *n = n.saturating_add_signed(delta);
        self.style.strong = self.strong > 0;
        self.style.emphasis = self.emphasis > 0;
        self.style.strike = self.strike > 0;
    }

    fn push_text(&mut self, text: &str) {
        // Merge with the previous run when the style matches to keep spans few.
        if let Some(last) = self.inlines.last_mut()
            && last.style == self.style
        {
            last.text.push_str(text);
            return;
        }
        self.inlines.push(Inline {
            text: text.to_string(),
            style: self.style.clone(),
        });
    }

    /// Turn pending inline text into a paragraph. Tight list items emit text
    /// without paragraph tags, so this also runs at item and container edges.
    fn flush(&mut self) {
        if self.table.is_some() || self.heading.is_some() || self.inlines.is_empty() {
            return;
        }
        let inlines = std::mem::take(&mut self.inlines);
        self.push_block(Block::Paragraph(inlines));
    }

    fn push_block(&mut self, block: Block) {
        match self.stack.last_mut() {
            Some(Frame::Quote(blocks) | Frame::Item(blocks)) => blocks.push(block),
            // Blocks never appear directly inside a list, only inside items.
            Some(Frame::List { .. }) | None => {
                self.root.push(block);
                self.lines.push(self.block_start);
            }
        }
    }

    fn finish(mut self) -> Document {
        self.flush();
        Document {
            blocks: self.root,
            lines: self.lines,
            code_file: false,
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::plain_text;

    #[test]
    fn heading_and_paragraph() {
        let doc = parse("# Title\n\nHello *world*.\n");
        assert_eq!(doc.title().as_deref(), Some("Title"));
        let Block::Paragraph(inl) = &doc.blocks[1] else {
            panic!("expected paragraph, got {:?}", doc.blocks[1]);
        };
        assert_eq!(plain_text(inl), "Hello world.");
        assert!(inl[1].style.emphasis);
    }

    #[test]
    fn code_blocks_get_ids_and_lang() {
        let doc = parse("```sh {name=a}\necho a\n```\n\n```\nplain\n```\n");
        let codes: Vec<_> = doc
            .blocks
            .iter()
            .filter_map(|b| match b {
                Block::Code(c) => Some(c),
                _ => None,
            })
            .collect();
        assert_eq!(codes.len(), 2);
        assert_eq!(codes[0].id, 0);
        assert_eq!(codes[0].lang.as_deref(), Some("sh"));
        assert_eq!(codes[0].info, "sh {name=a}");
        assert_eq!(codes[0].src, "echo a\n");
        assert_eq!(codes[1].id, 1);
        assert_eq!(codes[1].lang, None);
    }

    #[test]
    fn tight_and_nested_lists() {
        let doc = parse("- one\n- two\n  - nested\n");
        let Block::List { start, items } = &doc.blocks[0] else {
            panic!("expected list");
        };
        assert_eq!(*start, None);
        assert_eq!(items.len(), 2);
        assert!(matches!(&items[0][0], Block::Paragraph(i) if plain_text(i) == "one"));
        assert!(matches!(&items[1][1], Block::List { items, .. } if items.len() == 1));
    }

    #[test]
    fn code_inside_list_item() {
        let doc = parse("1. run:\n\n   ```sh\n   ls\n   ```\n");
        let Block::List { start, items } = &doc.blocks[0] else {
            panic!("expected list");
        };
        assert_eq!(*start, Some(1));
        assert!(matches!(&items[0][1], Block::Code(c) if c.src == "ls\n"));
    }

    #[test]
    fn front_matter_is_hidden() {
        let doc = parse("---\ntitle: x\ntags: [a]\n---\n\n# Real\n");
        assert_eq!(doc.blocks.len(), 1);
        assert_eq!(doc.title().as_deref(), Some("Real"));
        let toml = parse("+++\ntitle = 'x'\n+++\n\ntext\n");
        assert!(matches!(&toml.blocks[..], [Block::Paragraph(_)]));
    }

    #[test]
    fn table() {
        let doc = parse("| a | b |\n|---|---|\n| 1 | 2 |\n");
        let Block::Table { header, rows } = &doc.blocks[0] else {
            panic!("expected table");
        };
        assert_eq!(header.len(), 2);
        assert_eq!(plain_text(&rows[0][1]), "2");
    }

    #[test]
    fn quote_and_link() {
        let doc = parse("> see [docs](https://x.dev)\n");
        let Block::Quote(inner) = &doc.blocks[0] else {
            panic!("expected quote");
        };
        let Block::Paragraph(inl) = &inner[0] else {
            panic!("expected paragraph");
        };
        assert_eq!(inl[1].style.link.as_deref(), Some("https://x.dev"));
    }
}
