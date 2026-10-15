//! MDX front end: parse with markdown-rs and map its mdast into bastian's
//! document model. Front matter, `import`/`export` and `{/* comments */}`
//! are dropped; JSX components become muted label lines around their
//! rendered Markdown children.

use std::collections::HashMap;

use markdown::mdast::{AlignKind, AttributeContent, AttributeValue, Node};
use markdown::{Constructs, MdxSignal, ParseOptions};

use super::{
    Align, Attrs, Block, CalloutKind, CodeBlock, Document, Footnotes, Inline, InlineStyle,
    MAX_DEPTH, push_inline,
};

/// The GitHub alert a quote is, when its first line is just a marker like
/// `[!NOTE]` (markdown-rs doesn't parse these). Read from the raw tree,
/// where the line break after the marker is still a newline.
fn alert_kind(quote: &[Node]) -> Option<CalloutKind> {
    let [Node::Paragraph(p), ..] = quote else {
        return None;
    };
    let [Node::Text(t), ..] = p.children.as_slice() else {
        return None;
    };
    let line = t.value.split('\n').next()?;
    let marker = line.trim_end().strip_prefix("[!")?.strip_suffix(']')?;
    CalloutKind::from_marker(marker)
}

/// A quote's mapped `blocks` as a callout of `kind`, its marker dropped.
fn alert(kind: CalloutKind, mut blocks: Vec<Block>) -> Block {
    if let Some(Block::Paragraph(inl)) = blocks.first_mut()
        && let Some(first) = inl.first_mut()
    {
        // The marker, then the space its line break became.
        let rest = first.text.split_once(']').map_or("", |(_, r)| r);
        let rest = rest.strip_prefix(' ').unwrap_or(rest);
        if rest.is_empty() {
            inl.remove(0);
        } else {
            first.text = rest.to_string();
        }
        if inl.is_empty() {
            blocks.remove(0);
        }
    }
    Block::Callout { kind, blocks }
}

/// Deepest mdast nesting mapped. Deeper input, only ever pathological, is
/// left to the plain Markdown parser, which flattens it: markdown-rs slows
/// down quadratically with depth, and mapping recurses once per level.
const MAX_NODE_DEPTH: usize = 256;

/// Parse MDX, or `None` when the source isn't valid MDX (e.g. broken JSX)
/// or nests too deeply, so the caller can fall back to the plain Markdown
/// parser.
pub fn parse(src: &str) -> Option<Document> {
    if long_prefix(src) {
        return None;
    }
    let options = ParseOptions {
        constructs: Constructs {
            frontmatter: true,
            gfm_table: true,
            gfm_strikethrough: true,
            gfm_task_list_item: true,
            gfm_autolink_literal: true,
            gfm_footnote_definition: true,
            gfm_label_start_footnote: true,
            ..Constructs::mdx()
        },
        // ESM is only recognized with a parser; accept every import/export
        // as-is since bastian never evaluates it.
        mdx_esm_parse: Some(Box::new(|_| MdxSignal::Ok)),
        ..ParseOptions::mdx()
    };
    let root = markdown::to_mdast(src, &options).ok()?;
    if too_deep(&root) {
        drop_iteratively(root);
        return None;
    }
    let mut m = Mapper {
        definitions: definitions(&root),
        ..Mapper::default()
    };
    let mut blocks = Vec::new();
    let mut lines = Vec::new();
    for node in root.children()? {
        let before = blocks.len();
        m.block(node, &mut blocks, 0);
        // mdast lines are 1-based.
        let line = node
            .position()
            .map_or(0, |p| p.start.line.saturating_sub(1));
        lines.extend(std::iter::repeat_n(line, blocks.len() - before));
    }
    m.footnotes.append_to(&mut blocks, &mut lines);
    Some(Document {
        blocks,
        lines,
        code_file: false,
    })
}

#[derive(Default)]
struct Mapper {
    next_code_id: usize,
    next_link_id: usize,
    footnotes: Footnotes,
    /// Link reference definitions by normalized identifier, for
    /// `[text][id]` links and `![alt][id]` images.
    definitions: HashMap<String, String>,
}

/// Every `[id]: url` in the tree, the first of each identifier winning as
/// `CommonMark` says. markdown-rs leaves references unresolved.
fn definitions(root: &Node) -> HashMap<String, String> {
    let mut found = HashMap::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if let Node::Definition(d) = node {
            found
                .entry(d.identifier.clone())
                .or_insert_with(|| d.url.clone());
        }
        if let Some(children) = node.children() {
            // Reversed so the stack visits them in document order.
            stack.extend(children.iter().rev());
        }
    }
    found
}

impl Mapper {
    fn next_link_id(&mut self) -> usize {
        self.next_link_id += 1;
        self.next_link_id - 1
    }

    fn next_id(&mut self) -> usize {
        self.next_code_id += 1;
        self.next_code_id - 1
    }

    /// Map `nodes`, which sit inside `depth` quotes and list items.
    fn blocks(&mut self, nodes: &[Node], depth: usize) -> Vec<Block> {
        let mut out = Vec::new();
        for node in nodes {
            self.block(node, &mut out, depth);
        }
        out
    }

    fn block(&mut self, node: &Node, out: &mut Vec<Block>, depth: usize) {
        match node {
            Node::Heading(h) => out.push(Block::Heading {
                level: h.depth,
                inlines: self.inlines(&h.children),
            }),
            Node::Paragraph(p) => match p.children.as_slice() {
                [Node::Image(i)] => out.push(Block::Image {
                    alt: i.alt.clone(),
                    url: i.url.clone(),
                }),
                [Node::ImageReference(i)] if self.definitions.contains_key(&i.identifier) => {
                    out.push(Block::Image {
                        alt: i.alt.clone(),
                        url: self.definitions[&i.identifier].clone(),
                    });
                }
                children => out.push(Block::Paragraph(self.inlines(children))),
            },
            // Past `MAX_DEPTH`, quotes and lists are flattened into their
            // parent, as the plain Markdown parser does.
            Node::Blockquote(q) if depth >= MAX_DEPTH => {
                out.extend(self.blocks(&q.children, depth));
            }
            Node::Blockquote(q) => {
                let blocks = self.blocks(&q.children, depth + 1);
                out.push(match alert_kind(&q.children) {
                    Some(kind) => alert(kind, blocks),
                    None => Block::Quote(blocks),
                });
            }
            Node::List(l) if depth >= MAX_DEPTH => {
                for item in &l.children {
                    out.extend(self.list_item(item, depth));
                }
            }
            Node::List(l) => out.push(Block::List {
                start: l.ordered.then(|| u64::from(l.start.unwrap_or(1))),
                items: l
                    .children
                    .iter()
                    .map(|item| self.list_item(item, depth + 1))
                    .collect(),
                tasks: l
                    .children
                    .iter()
                    .map(|item| match item {
                        Node::ListItem(i) => i.checked,
                        _ => None,
                    })
                    .collect(),
            }),
            Node::Code(c) => {
                let info = match (&c.lang, &c.meta) {
                    (Some(lang), Some(meta)) => format!("{lang} {meta}"),
                    (Some(lang), None) => lang.clone(),
                    (None, _) => String::new(),
                };
                let id = self.next_id();
                out.push(Block::Code(CodeBlock {
                    id,
                    lang: super::parse::lang_of(&info),
                    attrs: Attrs::parse(&info),
                    src: format!("{}\n", c.value),
                }));
            }
            Node::Math(m) => out.push(Block::Code(CodeBlock {
                id: self.next_id(),
                lang: Some("math".into()),
                attrs: Attrs::default(),
                src: format!("{}\n", m.value),
            })),
            Node::Table(t) => {
                let mut rows = t.children.iter().filter_map(|r| match r {
                    Node::TableRow(row) => Some(
                        row.children
                            .iter()
                            .map(|cell| self.inlines(cell.children().map_or(&[][..], |c| c)))
                            .collect::<Vec<_>>(),
                    ),
                    _ => None,
                });
                let header = rows.next().unwrap_or_default();
                out.push(Block::Table {
                    align: t.align.iter().map(Align::from).collect(),
                    header,
                    rows: rows.collect(),
                });
            }
            Node::ThematicBreak(_) => out.push(Block::Rule),
            Node::MdxJsxFlowElement(el) => {
                let tag = open_tag(el.name.as_deref(), &el.attributes, el.children.is_empty());
                out.push(Block::Label(tag));
                if !el.children.is_empty() {
                    out.extend(self.blocks(&el.children, depth));
                    out.push(Block::Label(close_tag(el.name.as_deref())));
                }
            }
            Node::MdxFlowExpression(e) if !is_comment(&e.value) => {
                out.push(Block::Label(format!("{{{}}}", e.value)));
            }
            Node::Html(h) => out.push(Block::Label(h.value.clone())),
            // Front matter, ESM, comments and link definitions aren't shown.
            Node::Yaml(_)
            | Node::Toml(_)
            | Node::MdxjsEsm(_)
            | Node::MdxFlowExpression(_)
            | Node::Definition(_) => {}
            Node::FootnoteDefinition(f) => {
                let label = f.label.clone().unwrap_or_else(|| f.identifier.clone());
                if self.footnotes.claim(&label) {
                    let blocks = self.blocks(&f.children, depth);
                    let line = f.position.as_ref().map_or(0, |p| p.start.line - 1);
                    self.footnotes.define(label, blocks, line);
                }
            }
            // Stray phrasing content at block level: wrap it in a paragraph.
            other => {
                let inlines = self.inlines(std::slice::from_ref(other));
                if !inlines.is_empty() {
                    out.push(Block::Paragraph(inlines));
                }
            }
        }
    }

    /// An item's blocks, at the `depth` of the item itself.
    fn list_item(&mut self, node: &Node, depth: usize) -> Vec<Block> {
        match node {
            Node::ListItem(item) => self.blocks(&item.children, depth),
            _ => self.blocks(std::slice::from_ref(node), depth),
        }
    }

    /// `nodes` as the text of one link to `url`.
    fn link(&mut self, url: String, nodes: &[Node], style: &InlineStyle, out: &mut Vec<Inline>) {
        let mut s = style.clone();
        s.link = Some(url);
        s.link_id = Some(self.next_link_id());
        for n in nodes {
            self.inline(n, &s, out);
        }
    }

    fn inlines(&mut self, nodes: &[Node]) -> Vec<Inline> {
        let mut out = Vec::new();
        for n in nodes {
            self.inline(n, &InlineStyle::default(), &mut out);
        }
        out
    }

    fn inline(&mut self, node: &Node, style: &InlineStyle, out: &mut Vec<Inline>) {
        let with = |f: &dyn Fn(&mut InlineStyle)| {
            let mut s = style.clone();
            f(&mut s);
            s
        };
        let children = |m: &mut Self, nodes: &[Node], s: &InlineStyle, out: &mut Vec<Inline>| {
            for n in nodes {
                m.inline(n, s, out);
            }
        };
        match node {
            // A soft line break is a space, as in the Markdown front end;
            // only `Break` is a hard one.
            Node::Text(t) => {
                push_inline(out, &t.value.replace("\r\n", " ").replace('\n', " "), style);
            }
            Node::Strong(s) => children(self, &s.children, &with(&|s| s.strong = true), out),
            Node::Emphasis(e) => children(self, &e.children, &with(&|s| s.emphasis = true), out),
            Node::Delete(d) => children(self, &d.children, &with(&|s| s.strike = true), out),
            Node::InlineCode(c) => push_inline(out, &c.value, &with(&|s| s.code = true)),
            Node::InlineMath(m) => push_inline(out, &m.value, &with(&|s| s.code = true)),
            Node::Break(_) => push_inline(out, "\n", style),
            Node::Link(l) => self.link(l.url.clone(), &l.children, style, out),
            Node::LinkReference(l) => match self.definitions.get(&l.identifier).cloned() {
                Some(url) => self.link(url, &l.children, style, out),
                None => children(self, &l.children, style, out),
            },
            Node::Image(i) => push_inline(out, &super::image_placeholder(&i.alt), style),
            Node::ImageReference(i) => push_inline(out, &super::image_placeholder(&i.alt), style),
            Node::Html(h) => push_inline(out, &h.value, &with(&|s| s.muted = true)),
            Node::MdxJsxTextElement(el) => {
                let muted = with(&|s| s.muted = true);
                let self_closing = el.children.is_empty();
                let open = open_tag(el.name.as_deref(), &el.attributes, self_closing);
                push_inline(out, &open, &muted);
                if !self_closing {
                    children(self, &el.children, style, out);
                    push_inline(out, &close_tag(el.name.as_deref()), &muted);
                }
            }
            Node::MdxTextExpression(e) if !is_comment(&e.value) => {
                push_inline(out, &format!("{{{}}}", e.value), &with(&|s| s.muted = true));
            }
            Node::FootnoteReference(f) => {
                let label = f.label.as_deref().unwrap_or(&f.identifier);
                let mut r = self.footnotes.reference(label, style);
                r.style.link_id = Some(self.next_link_id());
                out.push(r);
            }
            Node::MdxTextExpression(_) => {}
            // Containers that can't appear inline: render their text.
            other => {
                if let Some(nodes) = other.children() {
                    children(self, nodes, style, out);
                }
            }
        }
    }
}

/// Whether a line's container prefix (its leading quote and list markers
/// and their indentation) opens more than `MAX_NODE_DEPTH` levels, or is
/// indented past `4 * MAX_NODE_DEPTH` columns, as deeply nested lists are.
/// This rejects deep quotes and lists before markdown-rs spends long on
/// them; text after the prefix, like a long `---` rule, isn't counted.
fn long_prefix(src: &str) -> bool {
    // A list marker is followed by a space or tab, or ends the line.
    let ends_marker = |b: Option<&u8>| matches!(b, None | Some(b' ' | b'\t'));
    src.lines().any(|line| {
        let b = line.as_bytes();
        let (mut i, mut levels, mut columns) = (0, 0, 0);
        loop {
            match b.get(i) {
                Some(b' ') => columns += 1,
                Some(b'\t') => columns += 4,
                Some(b'>') => levels += 1,
                Some(b'-' | b'*' | b'+') if ends_marker(b.get(i + 1)) => levels += 1,
                Some(b'0'..=b'9') => {
                    // An ordered marker: 1-9 digits, then `.` or `)`.
                    let digits = b[i..].iter().take(10).take_while(|b| b.is_ascii_digit());
                    let n = digits.count();
                    if n > 9
                        || !matches!(b.get(i + n), Some(b'.' | b')'))
                        || !ends_marker(b.get(i + n + 1))
                    {
                        return false;
                    }
                    levels += 1;
                    i += n;
                }
                _ => return false,
            }
            if levels > MAX_NODE_DEPTH || columns > 4 * MAX_NODE_DEPTH {
                return true;
            }
            i += 1;
        }
    })
}

/// Whether the tree nests deeper than `MAX_NODE_DEPTH`, walked without
/// recursion so any depth is safe to check.
fn too_deep(root: &Node) -> bool {
    let mut stack = vec![(root, 0)];
    while let Some((node, depth)) = stack.pop() {
        if depth > MAX_NODE_DEPTH {
            return true;
        }
        if let Some(children) = node.children() {
            stack.extend(children.iter().map(|c| (c, depth + 1)));
        }
    }
    false
}

/// Drop a tree of any depth: the default drop recurses once per level.
fn drop_iteratively(root: Node) {
    let mut stack = vec![root];
    while let Some(mut node) = stack.pop() {
        if let Some(children) = node.children_mut() {
            stack.append(children);
        }
    }
}

/// `{/* ... */}` holds only a comment.
fn is_comment(expr: &str) -> bool {
    let e = expr.trim();
    e.starts_with("/*") && e.ends_with("*/")
}

/// `<Callout type="warning" open>`, or `<Badge />` when self-closing.
fn open_tag(name: Option<&str>, attrs: &[AttributeContent], self_closing: bool) -> String {
    let mut tag = format!("<{}", name.unwrap_or(""));
    for attr in attrs {
        tag.push(' ');
        match attr {
            AttributeContent::Expression(e) => tag.push_str(&expr(&e.value)),
            AttributeContent::Property(p) => {
                tag.push_str(&p.name);
                match &p.value {
                    Some(AttributeValue::Literal(v)) => tag.push_str(&format!("=\"{v}\"")),
                    Some(AttributeValue::Expression(e)) => {
                        tag.push('=');
                        tag.push_str(&expr(&e.value));
                    }
                    None => {}
                }
            }
        }
    }
    tag.push_str(if self_closing { " />" } else { ">" });
    tag
}

/// `{value}`, shortened to `{…}` when it spans several lines (e.g. a
/// Storybook `parameters={{ ... }}` object) so tags stay one line.
fn expr(value: &str) -> String {
    if value.contains('\n') {
        "{…}".to_string()
    } else {
        format!("{{{value}}}")
    }
}

fn close_tag(name: Option<&str>) -> String {
    format!("</{}>", name.unwrap_or(""))
}

impl From<&AlignKind> for Align {
    fn from(a: &AlignKind) -> Self {
        match a {
            AlignKind::None => Align::None,
            AlignKind::Left => Align::Left,
            AlignKind::Center => Align::Center,
            AlignKind::Right => Align::Right,
        }
    }
}

#[cfg(test)]
mod tests;
