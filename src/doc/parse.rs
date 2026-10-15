use pulldown_cmark::{
    Alignment, BlockQuoteKind, CodeBlockKind, Event, LinkType, Options, Parser, Tag, TagEnd,
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
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_WIKILINKS;
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
    /// Depth (frames plus flattened containers) at which each `<details>`
    /// past `MAX_DEPTH` was opened. It gets no frame and its close tag is
    /// ignored; it's forgotten when the container it was opened in ends.
    skipped_details: Vec<usize>,
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
            Tag::Link {
                link_type,
                dest_url,
                ..
            } => {
                let url = match link_type {
                    LinkType::WikiLink { .. } => wiki_target(&dest_url),
                    _ => dest_url.into_string(),
                };
                self.style.link = Some(url);
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
            self.drop_skipped_details();
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
        if matches!(
            tag,
            TagEnd::BlockQuote(_) | TagEnd::List(_) | TagEnd::Item | TagEnd::FootnoteDefinition
        ) {
            self.drop_skipped_details();
        }
    }

    /// Containers open, with or without a frame.
    fn depth(&self) -> usize {
        self.stack.len() + self.flattened as usize
    }

    /// Forget skipped `<details>` opened inside a container that has ended:
    /// their close tags, if they ever come, belong to no block.
    fn drop_skipped_details(&mut self) {
        let depth = self.depth();
        self.skipped_details.retain(|&d| d <= depth);
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
                        self.skipped_details.push(self.depth());
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
                    if self.skipped_details.pop().is_none()
                        && matches!(self.stack.last(), Some(Frame::Details { .. }))
                    {
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

/// The relative link a wiki link's target stands for: `Page` is
/// `Page.md` next to the document, and a `#Heading` part becomes that
/// heading's anchor. Targets with an extension are kept as they are.
fn wiki_target(target: &str) -> String {
    let (path, heading) = match target.split_once('#') {
        Some((path, heading)) => (path.trim(), Some(heading)),
        None => (target.trim(), None),
    };
    let mut url = path.to_string();
    if !path.is_empty() && std::path::Path::new(path).extension().is_none() {
        url.push_str(".md");
    }
    if let Some(heading) = heading {
        url.push('#');
        url.push_str(&crate::links::slug(heading));
    }
    url
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
mod tests;
