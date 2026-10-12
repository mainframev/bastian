//! MDX front end: parse with markdown-rs and map its mdast into bastian's
//! document model. Front matter, `import`/`export` and `{/* comments */}`
//! are dropped; JSX components become muted label lines around their
//! rendered Markdown children.

use markdown::mdast::{AlignKind, AttributeContent, AttributeValue, Node};
use markdown::{Constructs, MdxSignal, ParseOptions};

use super::{
    Align, Attrs, Block, CalloutKind, CodeBlock, Document, Footnotes, Inline, InlineStyle,
    MAX_DEPTH, push_inline,
};

/// A quote's blocks as a GitHub alert when its first line is just a
/// marker like `[!NOTE]` (markdown-rs doesn't parse these), else a quote.
fn alert(mut blocks: Vec<Block>) -> Block {
    let kind = match blocks.first() {
        Some(Block::Paragraph(inl)) => inl.first().and_then(|first| {
            let line = first.text.split('\n').next()?;
            let marker = line.trim_end().strip_prefix("[!")?.strip_suffix(']')?;
            (first.style == InlineStyle::default())
                .then(|| CalloutKind::from_marker(marker))
                .flatten()
        }),
        _ => None,
    };
    let Some(kind) = kind else {
        return Block::Quote(blocks);
    };
    if let Some(Block::Paragraph(inl)) = blocks.first_mut() {
        let rest = inl[0].text.split_once('\n').map_or("", |(_, r)| r);
        if rest.is_empty() {
            inl.remove(0);
        } else {
            inl[0].text = rest.to_string();
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
        return None;
    }
    let mut m = Mapper::default();
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
    footnotes: Footnotes,
}

impl Mapper {
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
                children => out.push(Block::Paragraph(self.inlines(children))),
            },
            // Past `MAX_DEPTH`, quotes and lists are flattened into their
            // parent, as the plain Markdown parser does.
            Node::Blockquote(q) if depth >= MAX_DEPTH => {
                out.extend(self.blocks(&q.children, depth));
            }
            Node::Blockquote(q) => out.push(alert(self.blocks(&q.children, depth + 1))),
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
            Node::Text(t) => push_inline(out, &t.value, style),
            Node::Strong(s) => children(self, &s.children, &with(&|s| s.strong = true), out),
            Node::Emphasis(e) => children(self, &e.children, &with(&|s| s.emphasis = true), out),
            Node::Delete(d) => children(self, &d.children, &with(&|s| s.strike = true), out),
            Node::InlineCode(c) => push_inline(out, &c.value, &with(&|s| s.code = true)),
            Node::InlineMath(m) => push_inline(out, &m.value, &with(&|s| s.code = true)),
            Node::Break(_) => push_inline(out, "\n", style),
            Node::Link(l) => {
                let url = l.url.clone();
                children(
                    self,
                    &l.children,
                    &with(&|s| s.link = Some(url.clone())),
                    out,
                );
            }
            Node::LinkReference(l) => children(self, &l.children, style, out),
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
                out.push(self.footnotes.reference(label, style));
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

/// Whether a line's container prefix (the leading quote and list markers
/// and indentation) is longer than `MAX_NODE_DEPTH` columns. Each quote or
/// list level takes at least a column there, so this rejects deep quotes
/// and lists before markdown-rs spends long on them.
fn long_prefix(src: &str) -> bool {
    src.lines().any(|line| {
        line.bytes()
            .take_while(|b| {
                matches!(
                    b,
                    b' ' | b'\t' | b'>' | b'-' | b'*' | b'+' | b'.' | b')' | b'0'..=b'9'
                )
            })
            .map(|b| if b == b'\t' { 4 } else { 1 })
            .sum::<usize>()
            > MAX_NODE_DEPTH
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
mod tests {
    use super::*;
    use crate::doc::plain_text;

    const SAMPLE: &str = r#"---
title: MenuItemSwitch migration
---

import { Callout } from '@site/components';
export const meta = { draft: false };

# MenuItemSwitch migration

{/* internal note */}

<Callout type="warning" title="Breaking">
  The `checked` prop is now **controlled**.
</Callout>

<Tabs>
<Tab label="v8">

```sh {name=demo}
echo hi
```

</Tab>
</Tabs>

Use <Badge>new</Badge> for the inline API and {props.version} too.
"#;

    fn labels(doc: &Document) -> Vec<String> {
        doc.blocks
            .iter()
            .filter_map(|b| match b {
                Block::Label(l) => Some(l.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn hides_frontmatter_esm_and_comments() {
        let doc = parse(SAMPLE).unwrap();
        assert_eq!(doc.title().as_deref(), Some("MenuItemSwitch migration"));
        assert!(matches!(&doc.blocks[0], Block::Heading { level: 1, .. }));
        let all = format!("{doc:?}");
        assert!(!all.contains("import"), "{all}");
        assert!(!all.contains("internal note"), "{all}");
        assert!(!all.contains("sidebar"), "{all}");
    }

    #[test]
    fn jsx_components_become_labels_around_rendered_markdown() {
        let doc = parse(SAMPLE).unwrap();
        assert_eq!(
            labels(&doc),
            [
                r#"<Callout type="warning" title="Breaking">"#,
                "</Callout>",
                "<Tabs>",
                r#"<Tab label="v8">"#,
                "</Tab>",
                "</Tabs>",
            ]
        );
        // Markdown inside the component is parsed, not raw.
        let inner = doc
            .blocks
            .iter()
            .find_map(|b| match b {
                Block::Paragraph(i) if plain_text(i).contains("checked") => Some(i),
                _ => None,
            })
            .unwrap();
        assert!(inner.iter().any(|i| i.style.code && i.text == "checked"));
        assert!(
            inner
                .iter()
                .any(|i| i.style.strong && i.text == "controlled")
        );
    }

    #[test]
    fn code_blocks_inside_components_stay_runnable() {
        let doc = parse(SAMPLE).unwrap();
        let code = doc
            .blocks
            .iter()
            .find_map(|b| match b {
                Block::Code(c) => Some(c),
                _ => None,
            })
            .unwrap();
        assert_eq!(code.lang.as_deref(), Some("sh"));
        assert_eq!(code.attrs.name.as_deref(), Some("demo"));
        assert_eq!(code.src, "echo hi\n");
        assert_eq!(code.id, 0);
    }

    #[test]
    fn attributes_glued_to_the_language_are_not_part_of_it() {
        let doc = parse("```sh{name=x}\necho hi\n```\n").unwrap();
        let Some(Block::Code(code)) = doc.blocks.first() else {
            panic!("expected a code block: {doc:?}");
        };
        assert_eq!(code.lang.as_deref(), Some("sh"));
        assert_eq!(code.attrs.name.as_deref(), Some("x"));
    }

    #[test]
    fn inline_jsx_and_expressions_are_muted() {
        let doc = parse(SAMPLE).unwrap();
        let Some(Block::Paragraph(inl)) = doc.blocks.last() else {
            panic!("expected trailing paragraph");
        };
        let muted: Vec<&str> = inl
            .iter()
            .filter(|i| i.style.muted)
            .map(|i| i.text.as_str())
            .collect();
        assert_eq!(muted, ["<Badge>", "</Badge>", "{props.version}"]);
        assert!(plain_text(inl).contains("<Badge>new</Badge>"));
    }

    #[test]
    fn multi_line_attribute_values_are_shortened() {
        let doc = parse("<Meta title=\"x\" parameters={{\n  a: 1,\n}} on={true} />\n").unwrap();
        assert_eq!(
            labels(&doc),
            [r#"<Meta title="x" parameters={…} on={true} />"#]
        );
    }

    #[test]
    fn broken_jsx_returns_none() {
        // An opening tag that's never closed is an MDX syntax error.
        assert!(parse("<Callout>\n\ntext\n").is_none());
        // parse_for then falls back to plain Markdown instead of failing.
        let doc = crate::doc::parse_for("<Callout>\n\ntext\n", Some(std::path::Path::new("a.mdx")));
        assert!(!doc.blocks.is_empty(), "no blocks parsed");
    }

    #[test]
    fn deep_quotes_and_lists_are_flattened() {
        for src in [
            format!("{}x\n", "> ".repeat(40)),
            format!("{}x\n", "- ".repeat(40)),
        ] {
            let doc = parse(&src).unwrap();
            let mut levels = 0;
            let mut blocks = &doc.blocks[..];
            loop {
                match blocks {
                    [Block::Quote(inner)] => blocks = inner,
                    [Block::List { items, .. }] if items.len() == 1 => blocks = &items[0],
                    _ => break,
                }
                levels += 1;
            }
            assert_eq!(levels, MAX_DEPTH, "{src}");
            assert!(matches!(blocks, [Block::Paragraph(i)] if plain_text(i) == "x"));
        }
    }

    #[test]
    fn too_deep_returns_none() {
        // JSX nesting near the limit still maps.
        let jsx = |n| format!("{}x\n{}", "<a>\n".repeat(n), "</a>\n".repeat(n));
        assert!(parse(&jsx(MAX_NODE_DEPTH - 2)).is_some());
        // Past it, and for deep quotes and lists, the caller falls back.
        assert!(parse(&jsx(1_000)).is_none());
        assert!(parse(&format!("{}x\n", "> ".repeat(10_000))).is_none());
        assert!(parse(&format!("{}x\n", "- ".repeat(10_000))).is_none());
        let doc = crate::doc::parse_for(&jsx(1_000), Some(std::path::Path::new("a.mdx")));
        assert!(!doc.blocks.is_empty(), "no blocks parsed");
    }

    #[test]
    fn table_alignment() {
        let doc = parse("| a | b | c |\n|:--|:-:|--:|\n| 1 | 2 | 3 |\n").unwrap();
        let Block::Table { align, rows, .. } = &doc.blocks[0] else {
            panic!("expected table");
        };
        assert_eq!(align, &[Align::Left, Align::Center, Align::Right]);
        assert_eq!(plain_text(&rows[0][2]), "3");
    }
}
