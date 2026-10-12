//! MDX front end: parse with markdown-rs and map its mdast into bastian's
//! document model. Front matter, `import`/`export` and `{/* comments */}`
//! are dropped; JSX components become muted label lines around their
//! rendered Markdown children.

use markdown::mdast::{AlignKind, AttributeContent, AttributeValue, Node};
use markdown::{Constructs, MdxSignal, ParseOptions};

use super::{Align, Attrs, Block, CodeBlock, Document, Inline, InlineStyle, push_inline};

/// Parse MDX, or `None` when the source isn't valid MDX (e.g. broken JSX),
/// so the caller can fall back to the plain Markdown parser.
pub fn parse(src: &str) -> Option<Document> {
    let options = ParseOptions {
        constructs: Constructs {
            frontmatter: true,
            gfm_table: true,
            gfm_strikethrough: true,
            gfm_task_list_item: true,
            gfm_autolink_literal: true,
            ..Constructs::mdx()
        },
        // ESM is only recognized with a parser; accept every import/export
        // as-is since bastian never evaluates it.
        mdx_esm_parse: Some(Box::new(|_| MdxSignal::Ok)),
        ..ParseOptions::mdx()
    };
    let root = markdown::to_mdast(src, &options).ok()?;
    let mut m = Mapper::default();
    let mut blocks = Vec::new();
    let mut lines = Vec::new();
    for node in root.children()? {
        let before = blocks.len();
        m.block(node, &mut blocks);
        // mdast lines are 1-based.
        let line = node
            .position()
            .map_or(0, |p| p.start.line.saturating_sub(1));
        lines.extend(std::iter::repeat_n(line, blocks.len() - before));
    }
    Some(Document {
        blocks,
        lines,
        code_file: false,
    })
}

#[derive(Default)]
struct Mapper {
    next_code_id: usize,
}

impl Mapper {
    fn next_id(&mut self) -> usize {
        self.next_code_id += 1;
        self.next_code_id - 1
    }

    fn blocks(&mut self, nodes: &[Node]) -> Vec<Block> {
        let mut out = Vec::new();
        for node in nodes {
            self.block(node, &mut out);
        }
        out
    }

    fn block(&mut self, node: &Node, out: &mut Vec<Block>) {
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
            Node::Blockquote(q) => out.push(Block::Quote(self.blocks(&q.children))),
            Node::List(l) => out.push(Block::List {
                start: l.ordered.then(|| u64::from(l.start.unwrap_or(1))),
                items: l.children.iter().map(|item| self.list_item(item)).collect(),
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
                    out.extend(self.blocks(&el.children));
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
            | Node::Definition(_)
            | Node::FootnoteDefinition(_) => {}
            // Stray phrasing content at block level: wrap it in a paragraph.
            other => {
                let inlines = self.inlines(std::slice::from_ref(other));
                if !inlines.is_empty() {
                    out.push(Block::Paragraph(inlines));
                }
            }
        }
    }

    fn list_item(&mut self, node: &Node) -> Vec<Block> {
        let Node::ListItem(item) = node else {
            return self.blocks(std::slice::from_ref(node));
        };
        let mut blocks = self.blocks(&item.children);
        if let Some(done) = item.checked {
            let mark = Inline {
                text: if done { "[x] " } else { "[ ] " }.into(),
                style: InlineStyle::default(),
            };
            match blocks.first_mut() {
                Some(Block::Paragraph(inl)) => inl.insert(0, mark),
                _ => blocks.insert(0, Block::Paragraph(vec![mark])),
            }
        }
        blocks
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
            Node::MdxTextExpression(_) | Node::FootnoteReference(_) => {}
            // Containers that can't appear inline: render their text.
            other => {
                if let Some(nodes) = other.children() {
                    children(self, nodes, style, out);
                }
            }
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
    fn table_alignment() {
        let doc = parse("| a | b | c |\n|:--|:-:|--:|\n| 1 | 2 | 3 |\n").unwrap();
        let Block::Table { align, rows, .. } = &doc.blocks[0] else {
            panic!("expected table");
        };
        assert_eq!(align, &[Align::Left, Align::Center, Align::Right]);
        assert_eq!(plain_text(&rows[0][2]), "3");
    }
}
