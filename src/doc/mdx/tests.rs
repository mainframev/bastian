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

/// Each paragraph's text and the links in it, as `(text, url)`.
fn paragraphs(doc: &Document) -> Vec<(String, Vec<(String, String)>)> {
    doc.blocks
        .iter()
        .filter_map(|b| match b {
            Block::Paragraph(inlines) => Some((
                plain_text(inlines),
                inlines
                    .iter()
                    .filter_map(|i| Some((i.text.clone(), i.style.link.clone()?)))
                    .collect(),
            )),
            _ => None,
        })
        .collect()
}

#[test]
fn soft_breaks_join_lines_as_in_markdown() {
    let src = "first line\nsecond line\n\nhard  \nbreak\n";
    let mdx = parse(src).unwrap();
    assert_eq!(paragraphs(&mdx), paragraphs(&crate::doc::parse(src)));
    assert_eq!(paragraphs(&mdx)[0].0, "first line second line");
    assert_eq!(paragraphs(&mdx)[1].0, "hard\nbreak");
}

#[test]
fn alerts_need_the_marker_alone_on_its_line_as_in_markdown() {
    for src in [
        "> [!NOTE]\n> body\n",
        "> [!NOTE] same line\n",
        "> [!NOTE]\n",
    ] {
        let mdx = parse(src).unwrap();
        let md = crate::doc::parse(src);
        let kind = |d: &Document| match &d.blocks[0] {
            Block::Callout { kind, blocks } => Some((*kind, blocks.len())),
            _ => None,
        };
        assert_eq!(kind(&mdx), kind(&md), "{src:?}: {:?}", mdx.blocks);
    }
}

#[test]
fn reference_links_and_images_resolve_as_in_markdown() {
    let src = "See [the guide][g], [g][] and [g].\n\n![logo][l]\n\n\
                   > [quoted][q]\n\n[g]: docs/guide.md\n[G]: ignored.md\n[l]: logo.png\n\n\
                   > [q]: https://q.dev\n";
    let mdx = parse(src).unwrap();
    let md = crate::doc::parse(src);
    assert_eq!(paragraphs(&mdx), paragraphs(&md));
    let links = &paragraphs(&mdx)[0].1;
    assert_eq!(links.len(), 3, "{links:?}");
    assert!(
        links.iter().all(|(_, url)| url == "docs/guide.md"),
        "{links:?}"
    );
    assert!(
        matches!(&mdx.blocks[1], Block::Image { url, alt } if url == "logo.png" && alt == "logo"),
        "{:?}",
        mdx.blocks[1]
    );
    let Block::Quote(quoted) = &mdx.blocks[2] else {
        panic!("{:?}", mdx.blocks[2]);
    };
    let Block::Paragraph(inlines) = &quoted[0] else {
        panic!("{quoted:?}");
    };
    assert_eq!(inlines[0].style.link.as_deref(), Some("https://q.dev"));
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
fn long_lines_that_open_no_containers_still_parse() {
    // Long rules and numbers aren't container prefixes.
    for line in ["-".repeat(300), "*".repeat(300), "1".repeat(300)] {
        let src = format!("<Note>\n\ntext\n\n{line}\n\n</Note>\n");
        assert!(parse(&src).is_some(), "{line}");
    }
    // Deeply indented nested lists still fall back, without parsing.
    let list: String = (0..5_000)
        .map(|n| format!("{}- x\n", "  ".repeat(n)))
        .collect();
    assert!(parse(&list).is_none());
    assert!(parse(&format!("{}x\n", "> 1. ".repeat(5_000))).is_none());
}

#[test]
fn rejected_deep_trees_drop_without_recursion() {
    // A recursive drop overflows this small stack at this depth; on the
    // default stack it'd take a depth that parses for seconds.
    let n = 2_000;
    let src = format!("{}x\n{}", "<a>\n".repeat(n), "</a>\n".repeat(n));
    std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(move || assert!(parse(&src).is_none()))
        .unwrap()
        .join()
        .unwrap();
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

#[test]
fn wiki_links_resolve_as_in_markdown() {
    let src = "[[Other Page]] [[notes/todo|My list]] [[Guide#Getting Started]] [[#Usage]] [[spec.pdf]]\n\n\
               `[[code]]` and \\[\\[escaped]] and **[[bold]]**.\n";
    let mdx = parse(src).unwrap();
    let md = crate::doc::parse(src);
    assert_eq!(paragraphs(&mdx), paragraphs(&md));
    assert_eq!(
        paragraphs(&mdx)[0].1,
        [
            ("Other Page".into(), "Other Page.md".into()),
            ("My list".into(), "notes/todo.md".into()),
            (
                "Guide#Getting Started".into(),
                "Guide.md#getting-started".into()
            ),
            ("#Usage".into(), "#usage".into()),
            ("spec.pdf".into(), "spec.pdf".into()),
        ]
    );
    // Code and escapes stay text; emphasis still links.
    assert_eq!(paragraphs(&mdx)[1].1, [("bold".into(), "bold.md".into())]);
}

/// Each details block: (id, open, summary, text of its paragraphs).
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
fn details_fold_as_in_markdown() {
    let src = "<details>\n<summary>More</summary>\n\nhidden body\n\n</details>\n\n\
               <details open>\n<summary>Two **b**</summary>\n\nx\n\n<details>\n<summary>Inner</summary>\n\ny\n\n</details>\n\n</details>\n\n\
               <details>\n\nno summary\n\n</details>\n";
    let mdx = parse(src).unwrap();
    let md = crate::doc::parse(src);
    // Same blocks, ids and open state. Only the summary may differ: MDX reads
    // Markdown inside JSX, while raw HTML in .md keeps `**b**` as written.
    let without_summary = |d: &Document| {
        details(d)
            .into_iter()
            .map(|(id, open, _, body)| (id, open, body))
            .collect::<Vec<_>>()
    };
    assert_eq!(without_summary(&mdx), without_summary(&md));
    assert_eq!(
        details(&mdx),
        [
            (0, false, "More".into(), vec!["hidden body".into()]),
            (1, true, "Two b".into(), vec!["x".into()]),
            (2, false, "Inner".into(), vec!["y".into()]),
            (3, false, "Details".into(), vec!["no summary".into()]),
        ]
    );
}

#[test]
fn escaped_wiki_brackets_stay_text_beside_real_wiki_links() {
    let src = "See [[Other Page]] and \\[\\[not]] and \\[[nor]] then [[Last]].\n";
    let mdx = parse(src).unwrap();
    assert_eq!(paragraphs(&mdx), paragraphs(&crate::doc::parse(src)));
    assert_eq!(
        paragraphs(&mdx)[0].1,
        [
            ("Other Page".into(), "Other Page.md".into()),
            ("Last".into(), "Last.md".into()),
        ]
    );
}

#[test]
fn html_formatting_elements_style_like_markdown() {
    let src = "a <b>x</b> <kbd>K</kbd> <mark>m</mark> <u>u</u><br />==y== <Badge>z</Badge>\n";
    let mdx = crate::doc::parse_for(src, Some(std::path::Path::new("x.mdx")));
    let md = crate::doc::parse_for(src, Some(std::path::Path::new("x.md")));
    let Block::Paragraph(inl) = &mdx.blocks[0] else {
        panic!("{:?}", mdx.blocks);
    };
    let style = |t: &str| &inl.iter().find(|i| i.text == t).unwrap().style;
    assert!(style("x").strong);
    assert!(style("K").code);
    assert!(style("m").mark);
    assert!(style("u").underline);
    assert!(style("y").mark);
    // Components stay labelled.
    assert!(style("<Badge>").muted);
    assert_eq!(plain_text(inl), "a x K m u\ny <Badge>z</Badge>");
    // The Markdown front end reads the same tags the same way, bar the
    // component, which is just an unknown tag there.
    let Block::Paragraph(md) = &md.blocks[0] else {
        panic!("{:?}", md.blocks);
    };
    assert_eq!(plain_text(md), plain_text(inl));
}
