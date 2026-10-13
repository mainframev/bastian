use pulldown_cmark::{
    Alignment, BlockQuoteKind, CodeBlockKind, Event, Options, Parser, Tag, TagEnd,
};

use super::{
    Align, Attrs, Block, CalloutKind, CodeBlock, Document, Footnotes, Inline, InlineStyle,
    MAX_DEPTH, push_inline,
};

pub fn parse(src: &str) -> Document {
    let opts = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
        | Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS
        | Options::ENABLE_GFM
        | Options::ENABLE_FOOTNOTES;
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
        // Footnote definitions are listed apart from where they sit, so
        // they keep their own line even inside a list or quote.
        if matches!(event, Event::Start(Tag::FootnoteDefinition(_))) {
            b.footnote_line = line_of(range.start);
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
    /// A block quote, or a GitHub alert when it has a kind.
    Quote(Option<CalloutKind>, Vec<Block>),
    List {
        start: Option<u64>,
        items: Vec<Vec<Block>>,
        tasks: Vec<Option<bool>>,
    },
    /// A list item's blocks and, for a task item, whether it's done.
    Item(Vec<Block>, Option<bool>),
    Footnote {
        label: String,
        blocks: Vec<Block>,
        /// Source line of the definition.
        line: usize,
    },
    /// An HTML `<details>` element, open until its `</details>`.
    Details {
        id: usize,
        open: bool,
        summary: Option<Vec<Inline>>,
        blocks: Vec<Block>,
        /// Source line of the opening tag.
        line: usize,
    },
}

impl From<BlockQuoteKind> for CalloutKind {
    fn from(k: BlockQuoteKind) -> Self {
        match k {
            BlockQuoteKind::Note => CalloutKind::Note,
            BlockQuoteKind::Tip => CalloutKind::Tip,
            BlockQuoteKind::Important => CalloutKind::Important,
            BlockQuoteKind::Warning => CalloutKind::Warning,
            BlockQuoteKind::Caution => CalloutKind::Caution,
        }
    }
}

impl From<Alignment> for Align {
    fn from(a: Alignment) -> Self {
        match a {
            Alignment::None => Align::None,
            Alignment::Left => Align::Left,
            Alignment::Center => Align::Center,
            Alignment::Right => Align::Right,
        }
    }
}

#[derive(Default)]
struct TableState {
    align: Vec<Align>,
    header: Vec<Vec<Inline>>,
    rows: Vec<Vec<Vec<Inline>>>,
    row: Vec<Vec<Inline>>,
}

#[derive(Default)]
struct Builder {
    root: Vec<Block>,
    stack: Vec<Frame>,
    inlines: Vec<Inline>,
    /// Image URLs among the pending `inlines`.
    images: Vec<String>,
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
    next_link_id: usize,
    footnotes: Footnotes,
    /// Source line of the footnote definition being read.
    footnote_line: usize,
    /// Inside a repeated footnote definition, which is skipped.
    discarding: bool,
    /// Open quotes, lists and items past `MAX_DEPTH`, which get no frame:
    /// their content lands in the deepest frame.
    flattened: u32,
    /// The HTML block being read, gathered to find `<details>` tags in it.
    html: Option<String>,
    next_details_id: usize,
    /// `<details>` opened past `MAX_DEPTH`, ignored along with their close.
    skipped_details: u32,
}

impl Builder {
    fn event(&mut self, event: Event) {
        if self.discarding {
            self.discarding = !matches!(event, Event::End(TagEnd::FootnoteDefinition));
            return;
        }
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
            Event::Html(t) => match &mut self.html {
                Some(buf) => buf.push_str(&t),
                None => self.push_text(&t),
            },
            Event::InlineHtml(t) => self.push_text(&t),
            Event::SoftBreak => self.push_text(" "),
            Event::HardBreak => self.push_text("\n"),
            Event::Rule => {
                self.flush();
                self.push_block(Block::Rule);
            }
            Event::TaskListMarker(done) => {
                if self.flattened == 0
                    && let Some(Frame::Item(_, task)) = self.stack.last_mut()
                {
                    *task = Some(done);
                }
            }
            Event::FootnoteReference(label) => self.footnote_ref(&label),
            Event::InlineMath(_) | Event::DisplayMath(_) => {}
        }
    }

    fn start(&mut self, tag: Tag) {
        if matches!(tag, Tag::BlockQuote(_) | Tag::List(_) | Tag::Item) && self.too_deep() {
            self.flush();
            self.flattened += 1;
            return;
        }
        match tag {
            Tag::Paragraph => self.flush(),
            Tag::Heading { level, .. } => {
                self.flush();
                self.heading = Some(level as u8);
            }
            Tag::BlockQuote(kind) => {
                self.flush();
                self.stack
                    .push(Frame::Quote(kind.map(CalloutKind::from), Vec::new()));
            }
            Tag::List(start) => {
                self.flush();
                self.stack.push(Frame::List {
                    start,
                    items: Vec::new(),
                    tasks: Vec::new(),
                });
            }
            Tag::Item => self.stack.push(Frame::Item(Vec::new(), None)),
            Tag::CodeBlock(kind) => {
                self.flush();
                let info = match kind {
                    CodeBlockKind::Fenced(info) => info.trim().to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                self.code = Some((info, String::new()));
            }
            Tag::Table(aligns) => {
                self.flush();
                self.table = Some(TableState {
                    align: aligns.into_iter().map(Align::from).collect(),
                    ..TableState::default()
                });
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
            Tag::Link { dest_url, .. } => {
                self.style.link = Some(dest_url.into_string());
                self.style.link_id = Some(self.next_link_id());
            }
            Tag::Image { dest_url, .. } => {
                self.images.push(dest_url.into_string());
                self.push_text(super::IMAGE_PREFIX);
            }
            Tag::MetadataBlock(_) => self.metadata = true,
            Tag::HtmlBlock => {
                self.flush();
                self.html = Some(String::new());
            }
            Tag::FootnoteDefinition(label) => {
                self.flush();
                if !self.footnotes.claim(&label) {
                    self.discarding = true;
                    return;
                }
                self.stack.push(Frame::Footnote {
                    label: label.into_string(),
                    blocks: Vec::new(),
                    line: self.footnote_line,
                });
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        // A `<details>` left open inside a container ends with it.
        if matches!(
            tag,
            TagEnd::BlockQuote(_) | TagEnd::Item | TagEnd::FootnoteDefinition
        ) && self.flattened == 0
        {
            self.flush();
            self.close_open_details();
        }
        if matches!(tag, TagEnd::BlockQuote(_) | TagEnd::List(_) | TagEnd::Item)
            && self.flattened > 0
        {
            self.flush();
            self.flattened -= 1;
            return;
        }
        match tag {
            TagEnd::Paragraph => self.flush(),
            TagEnd::Heading(_) => {
                let level = self.heading.take().unwrap_or(1);
                let inlines = std::mem::take(&mut self.inlines);
                self.images.clear();
                self.push_block(Block::Heading { level, inlines });
            }
            TagEnd::BlockQuote(_) => {
                self.flush();
                match self.stack.pop() {
                    Some(Frame::Quote(Some(kind), blocks)) => {
                        self.push_block(Block::Callout { kind, blocks });
                    }
                    Some(Frame::Quote(None, blocks)) => self.push_block(Block::Quote(blocks)),
                    _ => {}
                }
            }
            TagEnd::List(_) => {
                if let Some(Frame::List {
                    start,
                    items,
                    tasks,
                }) = self.stack.pop()
                {
                    self.push_block(Block::List {
                        start,
                        items,
                        tasks,
                    });
                }
            }
            TagEnd::Item => {
                self.flush();
                if let Some(Frame::Item(blocks, task)) = self.stack.pop()
                    && let Some(Frame::List { items, tasks, .. }) = self.stack.last_mut()
                {
                    items.push(blocks);
                    tasks.push(task);
                }
            }
            TagEnd::CodeBlock => {
                if let Some((info, src)) = self.code.take() {
                    let id = self.next_id();
                    self.push_block(Block::Code(CodeBlock {
                        id,
                        lang: lang_of(&info),
                        attrs: Attrs::parse(&info),
                        src,
                    }));
                }
            }
            TagEnd::TableCell => {
                let cell = std::mem::take(&mut self.inlines);
                self.images.clear();
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
                        align: t.align,
                        header: t.header,
                        rows: t.rows,
                    });
                }
            }
            TagEnd::Strong => self.set_count(|b| &mut b.strong, -1),
            TagEnd::Emphasis => self.set_count(|b| &mut b.emphasis, -1),
            TagEnd::Strikethrough => self.set_count(|b| &mut b.strike, -1),
            TagEnd::Link => {
                self.style.link = None;
                self.style.link_id = None;
            }
            TagEnd::Image => self.push_text("]"),
            TagEnd::MetadataBlock(_) => self.metadata = false,
            TagEnd::HtmlBlock => {
                if let Some(html) = self.html.take() {
                    self.html_block(&html);
                }
            }
            TagEnd::FootnoteDefinition => {
                self.flush();
                if let Some(Frame::Footnote {
                    label,
                    blocks,
                    line,
                }) = self.stack.pop()
                {
                    self.footnotes.define(label, blocks, line);
                }
            }
            _ => {}
        }
    }

    /// Whether a new quote, list or item would nest past `MAX_DEPTH`. Lists
    /// don't count, so the deepest frame is always a quote or an item,
    /// never a list that would send flattened blocks to the root.
    fn too_deep(&self) -> bool {
        self.flattened > 0
            || self
                .stack
                .iter()
                .filter(|f| {
                    matches!(
                        f,
                        Frame::Quote(..) | Frame::Item(..) | Frame::Details { .. }
                    )
                })
                .count()
                >= MAX_DEPTH
    }

    fn set_count(&mut self, field: fn(&mut Self) -> &mut u32, delta: i32) {
        let n = field(self);
        *n = n.saturating_add_signed(delta);
        self.style.strong = self.strong > 0;
        self.style.emphasis = self.emphasis > 0;
        self.style.strike = self.strike > 0;
    }

    fn push_text(&mut self, text: &str) {
        push_inline(&mut self.inlines, text, &self.style);
    }

    /// A footnote reference: `[n]`, linking to the footnote's anchor.
    fn footnote_ref(&mut self, label: &str) {
        let mut r = self.footnotes.reference(label, &self.style);
        r.style.link_id = Some(self.next_link_id());
        push_inline(&mut self.inlines, &r.text, &r.style);
    }

    fn next_link_id(&mut self) -> usize {
        self.next_link_id += 1;
        self.next_link_id - 1
    }

    fn next_id(&mut self) -> usize {
        self.next_code_id += 1;
        self.next_code_id - 1
    }

    /// Turn pending inline text into a paragraph. Tight list items emit text
    /// without paragraph tags, so this also runs at item and container edges.
    fn flush(&mut self) {
        if self.table.is_some() || self.heading.is_some() || self.inlines.is_empty() {
            return;
        }
        let inlines = std::mem::take(&mut self.inlines);
        let images = std::mem::take(&mut self.images);
        let block = match images.as_slice() {
            [url] => image_block(&inlines, url),
            _ => None,
        };
        self.push_block(block.unwrap_or(Block::Paragraph(inlines)));
    }

    fn push_block(&mut self, block: Block) {
        match self.stack.last_mut() {
            Some(
                Frame::Quote(_, blocks)
                | Frame::Item(blocks, _)
                | Frame::Footnote { blocks, .. }
                | Frame::Details { blocks, .. },
            ) => blocks.push(block),
            // Blocks never appear directly inside a list, only inside items.
            Some(Frame::List { .. }) | None => {
                self.root.push(block);
                self.lines.push(self.block_start);
            }
        }
    }

    /// An HTML block: `<details>` and `<summary>` tags open, label and close
    /// a details block; any other HTML shows as text, as before.
    fn html_block(&mut self, html: &str) {
        for part in html_parts(html) {
            match part {
                HtmlPart::Text(t) => {
                    if !t.trim().is_empty() {
                        self.push_text(t.trim_matches('\n'));
                    }
                }
                HtmlPart::Open { open } => {
                    self.flush();
                    if self.too_deep() {
                        self.skipped_details += 1;
                        continue;
                    }
                    let id = self.next_details_id;
                    self.next_details_id += 1;
                    self.stack.push(Frame::Details {
                        id,
                        open,
                        summary: None,
                        blocks: Vec::new(),
                        line: self.block_start,
                    });
                }
                HtmlPart::Summary(text) => {
                    self.flush();
                    match self.stack.last_mut() {
                        Some(Frame::Details { summary, .. }) if summary.is_none() => {
                            *summary = Some(vec![Inline {
                                text,
                                style: InlineStyle::default(),
                            }]);
                        }
                        _ => {
                            self.push_text(&text);
                            self.flush();
                        }
                    }
                }
                HtmlPart::Close => {
                    self.flush();
                    if self.skipped_details > 0 {
                        self.skipped_details -= 1;
                    } else if matches!(self.stack.last(), Some(Frame::Details { .. })) {
                        self.close_details();
                    }
                }
            }
        }
        self.flush();
    }

    /// Close the `<details>` frame on top of the stack.
    fn close_details(&mut self) {
        let Some(Frame::Details {
            id,
            open,
            summary,
            blocks,
            line,
        }) = self.stack.pop()
        else {
            return;
        };
        let summary = summary.unwrap_or_else(|| {
            vec![Inline {
                text: "Details".into(),
                style: InlineStyle::default(),
            }]
        });
        // At the root, the block belongs to the line of its opening tag.
        let start = std::mem::replace(&mut self.block_start, line);
        self.push_block(Block::Details {
            id,
            open,
            summary,
            blocks,
        });
        self.block_start = start;
    }

    /// Close every `<details>` still open on top of the stack.
    fn close_open_details(&mut self) {
        while matches!(self.stack.last(), Some(Frame::Details { .. })) {
            self.close_details();
        }
    }

    fn finish(mut self) -> Document {
        self.flush();
        self.close_open_details();
        std::mem::take(&mut self.footnotes).append_to(&mut self.root, &mut self.lines);
        Document {
            blocks: self.root,
            lines: self.lines,
            code_file: false,
        }
    }
}

/// A piece of an HTML block, as `<details>` handling sees it.
#[derive(Debug, PartialEq)]
enum HtmlPart<'a> {
    Text(&'a str),
    Open { open: bool },
    Summary(String),
    Close,
}

/// Split an HTML block into `<details>`, `<summary>…</summary>` and
/// `</details>` tags (any case) and the text between them.
fn html_parts(html: &str) -> Vec<HtmlPart<'_>> {
    let mut parts = Vec::new();
    let mut text_start = 0;
    let mut i = 0;
    while let Some(off) = html[i..].find('<') {
        let at = i + off;
        let rest = &html[at..];
        let tag = |name: &str| {
            rest.get(..name.len())
                .is_some_and(|t| t.eq_ignore_ascii_case(name))
                && rest[name.len()..]
                    .chars()
                    .next()
                    .is_some_and(|c| c == '>' || c == '/' || c.is_ascii_whitespace())
        };
        let end_of_tag = |from: usize| rest[from..].find('>').map(|e| at + from + e + 1);
        let (part, next) = if tag("<details") {
            let Some(end) = end_of_tag(8) else { break };
            let attrs = html[at + 8..end - 1].to_ascii_lowercase();
            let open = attrs
                .split(|c: char| c.is_ascii_whitespace() || c == '/')
                .any(|a| a == "open" || a.starts_with("open="));
            (HtmlPart::Open { open }, end)
        } else if tag("</details") {
            let Some(end) = end_of_tag(9) else { break };
            (HtmlPart::Close, end)
        } else if tag("<summary") {
            let Some(body) = end_of_tag(8) else { break };
            let lower = html[body..].to_ascii_lowercase();
            let (inner_end, end) = match lower.find("</summary") {
                Some(e) => {
                    let close = body + e;
                    let end = html[close..]
                        .find('>')
                        .map_or(html.len(), |g| close + g + 1);
                    (close, end)
                }
                None => (html.len(), html.len()),
            };
            (HtmlPart::Summary(html_text(&html[body..inner_end])), end)
        } else {
            i = at + 1;
            continue;
        };
        if text_start < at {
            parts.push(HtmlPart::Text(&html[text_start..at]));
        }
        parts.push(part);
        text_start = next;
        i = next;
    }
    if text_start < html.len() {
        parts.push(HtmlPart::Text(&html[text_start..]));
    }
    parts
}

/// The text of an HTML fragment: tags dropped, common entities decoded,
/// whitespace collapsed.
fn html_text(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    let out = out
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&");
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The image block for a paragraph that is nothing but `[image: alt]`
/// (not wrapped in a link, which would be lost).
fn image_block(inlines: &[Inline], url: &str) -> Option<Block> {
    if inlines.iter().any(|i| i.style.link.is_some()) {
        return None;
    }
    let text = super::plain_text(inlines);
    let alt = text
        .trim()
        .strip_prefix(super::IMAGE_PREFIX)?
        .strip_suffix(']')?;
    Some(Block::Image {
        alt: alt.to_string(),
        url: url.to_string(),
    })
}

/// Language token of a fence info string: `sh {name=x}` and `sh{name=x}` give `sh`.
pub(super) fn lang_of(info: &str) -> Option<String> {
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
        assert_eq!(codes[0].attrs.name.as_deref(), Some("a"));
        assert_eq!(codes[0].src, "echo a\n");
        assert_eq!(codes[1].id, 1);
        assert_eq!(codes[1].lang, None);
    }

    #[test]
    fn tight_and_nested_lists() {
        let doc = parse("- one\n- two\n  - nested\n");
        let Block::List { start, items, .. } = &doc.blocks[0] else {
            panic!("expected list");
        };
        assert_eq!(*start, None);
        assert_eq!(items.len(), 2);
        assert!(matches!(&items[0][0], Block::Paragraph(i) if plain_text(i) == "one"));
        assert!(matches!(&items[1][1], Block::List { items, .. } if items.len() == 1));
    }

    #[test]
    fn task_items_carry_their_state() {
        let doc = parse("- [x] done\n- [ ] todo\n- plain\n");
        let Block::List { items, tasks, .. } = &doc.blocks[0] else {
            panic!("expected list");
        };
        assert_eq!(tasks, &[Some(true), Some(false), None]);
        assert!(matches!(&items[0][0], Block::Paragraph(i) if plain_text(i) == "done"));
        let mdx =
            crate::doc::parse_for("- [x] done\n- plain\n", Some(std::path::Path::new("x.mdx")));
        let Block::List { items, tasks, .. } = &mdx.blocks[0] else {
            panic!("expected list");
        };
        assert_eq!(tasks, &[Some(true), None]);
        assert!(matches!(&items[0][0], Block::Paragraph(i) if plain_text(i) == "done"));
    }

    #[test]
    fn code_inside_list_item() {
        let doc = parse("1. run:\n\n   ```sh\n   ls\n   ```\n");
        let Block::List { start, items, .. } = &doc.blocks[0] else {
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
        let Block::Table {
            align,
            header,
            rows,
        } = &doc.blocks[0]
        else {
            panic!("expected table");
        };
        assert_eq!(align, &[Align::None, Align::None]);
        assert_eq!(header.len(), 2);
        assert_eq!(plain_text(&rows[0][1]), "2");
    }

    #[test]
    fn table_alignment() {
        let doc = parse("| a | b | c | d |\n|:--|:-:|--:|---|\n");
        let Block::Table { align, .. } = &doc.blocks[0] else {
            panic!("expected table");
        };
        assert_eq!(
            align,
            &[Align::Left, Align::Center, Align::Right, Align::None]
        );
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

    #[test]
    fn gfm_alerts_become_callouts() {
        let doc = parse("> [!WARNING]\n> Mind the *gap*.\n\n> [!bogus]\n> plain\n");
        let Block::Callout { kind, blocks } = &doc.blocks[0] else {
            panic!("expected callout, got {:?}", doc.blocks[0]);
        };
        assert_eq!(*kind, CalloutKind::Warning);
        assert!(matches!(&blocks[..], [Block::Paragraph(i)] if plain_text(i) == "Mind the gap."));
        assert!(matches!(&doc.blocks[1], Block::Quote(_)));

        // MDX too, where markdown-rs leaves the marker as text.
        let mdx = crate::doc::parse_for(
            "> [!tip]\n> Mind the *gap*.\n\n> [!TIP]\n\n> [!bogus]\n> plain\n",
            Some(std::path::Path::new("x.mdx")),
        );
        let Block::Callout { kind, blocks } = &mdx.blocks[0] else {
            panic!("expected callout, got {:?}", mdx.blocks[0]);
        };
        assert_eq!(*kind, CalloutKind::Tip);
        assert!(matches!(&blocks[..], [Block::Paragraph(i)] if plain_text(i) == "Mind the gap."));
        assert!(matches!(&mdx.blocks[1], Block::Callout { blocks, .. } if blocks.is_empty()));
        assert!(matches!(&mdx.blocks[2], Block::Quote(_)));
    }

    #[test]
    fn footnotes_are_numbered_and_listed_last() {
        let doc = parse(
            "[^b]: Bee.\n\nSee[^a] and[^B] again[^a].\n\n[^a]: Ay *one*.\n\n[^z]: Unused.\n\n# After\n",
        );
        let Block::Paragraph(inl) = &doc.blocks[0] else {
            panic!("expected paragraph, got {:?}", doc.blocks[0]);
        };
        assert_eq!(plain_text(inl), "See[1] and[2] again[1].");
        let refs: Vec<_> = inl.iter().filter_map(|i| i.style.link.as_deref()).collect();
        assert_eq!(refs, ["#fn:a", "#fn:b", "#fn:a"]);
        assert!(matches!(&doc.blocks[1], Block::Heading { .. }));
        assert_eq!(doc.blocks[2], Block::Rule);
        let notes: Vec<_> = doc.blocks[3..]
            .iter()
            .map(|b| match b {
                Block::Footnote {
                    label,
                    number,
                    blocks,
                } => (label.as_str(), *number, blocks.len()),
                other => panic!("expected footnote, got {other:?}"),
            })
            .collect();
        assert_eq!(notes, [("a", 1, 1), ("b", 2, 1), ("z", 3, 1)]);
        assert_eq!(doc.lines.len(), doc.blocks.len());
        assert_eq!(doc.lines[3], 4);

        let mdx = crate::doc::parse_for(
            "See[^a].\n\n[^a]: Ay.\n",
            Some(std::path::Path::new("x.mdx")),
        );
        assert!(matches!(&mdx.blocks[0], Block::Paragraph(i) if plain_text(i) == "See[1]."));
        assert!(matches!(&mdx.blocks[2], Block::Footnote { number: 1, .. }));
        assert_eq!(mdx.lines, [0, 2, 2]);
    }

    #[test]
    fn footnote_edge_cases() {
        // Distinct labels get distinct anchors.
        let doc = parse("A[^*] B[^†] C[^a b] D[^a-b].\n\n[^*]: 1\n[^†]: 2\n[^a b]: 3\n[^a-b]: 4\n");
        let Block::Paragraph(inl) = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        let mut refs: Vec<_> = inl.iter().filter_map(|i| i.style.link.clone()).collect();
        refs.dedup();
        assert_eq!(refs.len(), 4, "{refs:?}");

        // The first definition of a label wins, and a repeat counts for
        // nothing: not its references, not its code.
        for path in ["x.md", "x.mdx"] {
            let doc = crate::doc::parse_for(
                "A[^a].\n\n[^a]: one\n\n[^A]: dup[^b]\n\n    ```sh\n    no\n    ```\n\n[^b]: bee\n\nB[^b].\n",
                Some(std::path::Path::new(path)),
            );
            let numbers: Vec<_> = doc
                .blocks
                .iter()
                .filter_map(|b| match b {
                    Block::Footnote {
                        label,
                        number,
                        blocks,
                    } => Some((label.as_str(), *number, blocks.len())),
                    _ => None,
                })
                .collect();
            assert_eq!(numbers, [("a", 1, 1), ("b", 2, 1)], "{path}");
        }

        // Code ids follow the order blocks are shown in, even when a note
        // with code is defined before other code.
        for path in ["x.md", "x.mdx"] {
            let doc = crate::doc::parse_for(
                "x[^n]\n\n[^n]: note\n\n    ```sh\n    in note\n    ```\n\n```sh\nafter\n```\n",
                Some(std::path::Path::new(path)),
            );
            let mut codes = Vec::new();
            crate::doc::visit(&doc.blocks, &mut |b| {
                if let Block::Code(c) = b {
                    codes.push((c.id, c.src.clone()));
                }
            });
            assert_eq!(
                codes,
                [(0, "after\n".to_string()), (1, "in note\n".to_string())],
                "{path}"
            );
        }

        // A definition inside a list keeps its own source line.
        let doc = parse("- item[^n]\n\n  [^n]: note\n");
        assert_eq!(doc.lines, [0, 2, 2]);

        // Footnote references aren't part of a heading's anchor text.
        let doc = parse("# Setup[^h]\n\n[^h]: x\n");
        let Block::Heading { inlines, .. } = &doc.blocks[0] else {
            panic!("expected heading");
        };
        assert_eq!(crate::doc::anchor_text(inlines), "Setup");
        assert_eq!(plain_text(inlines), "Setup[1]");
    }

    /// Levels of quotes and lists down to the deepest block.
    fn depth(blocks: &[Block]) -> usize {
        blocks
            .iter()
            .map(|b| match b {
                Block::Quote(inner)
                | Block::Callout { blocks: inner, .. }
                | Block::Footnote { blocks: inner, .. } => 1 + depth(inner),
                Block::List { items, .. } => 1 + items.iter().map(|i| depth(i)).max().unwrap_or(0),
                _ => 0,
            })
            .max()
            .unwrap_or(0)
    }

    #[test]
    fn deep_nesting_is_flattened() {
        let theme = crate::render::theme::Theme::dark();
        for path in ["x.md", "x.mdx"] {
            let path = Some(std::path::Path::new(path));
            // Up to the cap, nesting is kept as written.
            for n in [MAX_DEPTH, MAX_DEPTH + 1] {
                let doc = crate::doc::parse_for(&format!("{}x\n", "> ".repeat(n)), path);
                assert_eq!(depth(&doc.blocks), MAX_DEPTH, "{path:?} {n}");
            }
            for src in [
                format!("{}x\n", "> ".repeat(10_000)),
                format!("{}x\n", "- ".repeat(10_000)),
                format!("{}x\n", "> 1. ".repeat(5_000)),
            ] {
                let doc = crate::doc::parse_for(&src, path);
                assert_eq!(depth(&doc.blocks), MAX_DEPTH, "{path:?}");
                let mut texts = Vec::new();
                crate::doc::visit(&doc.blocks, &mut |b| {
                    if let Block::Paragraph(i) = b {
                        texts.push(plain_text(i));
                    }
                });
                assert_eq!(texts, ["x"], "{path:?}");
                let r = crate::render::render(&doc, 80, &theme);
                assert!(
                    r.lines
                        .iter()
                        .any(|l| crate::render::plain(l).trim_end().ends_with('x')),
                    "{path:?}"
                );
            }
        }
    }

    #[test]
    fn image_only_paragraphs_become_image_blocks() {
        let doc = parse(
            "![Screenshot](img/shot.png)\n\nText ![inline](a.png) here\n\n[![linked](b.png)](https://x.dev)\n",
        );
        assert_eq!(
            doc.blocks[0],
            Block::Image {
                alt: "Screenshot".into(),
                url: "img/shot.png".into()
            }
        );
        assert!(matches!(doc.blocks[1], Block::Paragraph(_)));
        assert!(matches!(doc.blocks[2], Block::Paragraph(_)));
        let mdx = crate::doc::parse_for("![Logo](logo.png)\n", Some(std::path::Path::new("x.mdx")));
        assert!(matches!(&mdx.blocks[0], Block::Image { url, .. } if url == "logo.png"));
    }

    /// The details blocks of `doc`, in document order: (id, open, summary,
    /// plain text of their direct paragraphs).
    fn details(doc: &Document) -> Vec<(usize, bool, String, Vec<String>)> {
        let mut out = Vec::new();
        crate::doc::visit(&doc.blocks, &mut |b| {
            if let Block::Details {
                id,
                open,
                summary,
                blocks,
            } = b
            {
                let paras = blocks
                    .iter()
                    .filter_map(|b| match b {
                        Block::Paragraph(i) => Some(plain_text(i)),
                        _ => None,
                    })
                    .collect();
                out.push((*id, *open, plain_text(summary), paras));
            }
        });
        out
    }

    #[test]
    fn details_hold_their_blocks_and_summary() {
        let doc = parse(
            "<details>\n<summary>More <b>info</b> &amp; such</summary>\n\nHidden **text**.\n\n</details>\n\nAfter.\n",
        );
        assert_eq!(
            details(&doc),
            [(
                0,
                false,
                "More info & such".into(),
                vec!["Hidden text.".into()]
            )]
        );
        assert!(matches!(&doc.blocks[1], Block::Paragraph(i) if plain_text(i) == "After."));
        assert_eq!(doc.lines, [0, 7]);
    }

    #[test]
    fn details_open_attribute_nesting_and_case() {
        let doc = parse(
            "<DETAILS Open><Summary>Outer</Summary>\n\nA\n\n<details>\n<summary>Inner</summary>\n\nB\n\n</details>\n</DETAILS>\n",
        );
        assert_eq!(
            details(&doc),
            [
                (0, true, "Outer".into(), vec!["A".into()]),
                (1, false, "Inner".into(), vec!["B".into()]),
            ]
        );
    }

    #[test]
    fn details_without_summary_or_blank_lines() {
        let doc = parse("<details>\nRaw body.\n</details>\n");
        assert_eq!(
            details(&doc),
            [(0, false, "Details".into(), vec!["Raw body.".into()])]
        );
    }

    #[test]
    fn unclosed_details_end_with_their_container() {
        // Left open at the end of the document.
        let doc = parse("<details>\n<summary>S</summary>\n\ntail\n");
        assert_eq!(details(&doc), [(0, false, "S".into(), vec!["tail".into()])]);
        // Left open inside a list item: closed with it, the list intact.
        let doc = parse("- <details>\n  <summary>S</summary>\n\n  in\n- next\n");
        let Block::List { items, .. } = &doc.blocks[0] else {
            panic!("{:?}", doc.blocks);
        };
        assert_eq!(items.len(), 2);
        assert!(matches!(&items[0][..], [Block::Details { .. }]));
    }

    #[test]
    fn stray_close_tags_and_other_html_are_left_alone() {
        let doc = parse("</details>\n\n<div>hi</div>\n\n<summary>loose</summary>\n");
        assert!(details(&doc).is_empty());
        let text: Vec<String> = doc
            .blocks
            .iter()
            .filter_map(|b| match b {
                Block::Paragraph(i) => Some(plain_text(i)),
                _ => None,
            })
            .collect();
        assert_eq!(text, ["<div>hi</div>", "loose"]);
    }

    #[test]
    fn details_past_max_depth_are_flattened() {
        let open = "<details>\n<summary>s</summary>\n\n".repeat(MAX_DEPTH + 4);
        let close = "</details>\n\n".repeat(MAX_DEPTH + 4);
        let doc = parse(&format!("{open}deep\n\n{close}after\n"));
        assert_eq!(details(&doc).len(), MAX_DEPTH);
        assert!(matches!(doc.blocks.last(), Some(Block::Paragraph(i)) if plain_text(i) == "after"));
    }
}
