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
    let mdx = crate::doc::parse_for("- [x] done\n- plain\n", Some(std::path::Path::new("x.mdx")));
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
    assert_eq!(details(&doc), []);
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
fn html_comments_are_not_shown() {
    let doc = parse(concat!(
        "Before <!-- inline --> after.\n\n",
        "<!-- prettier-ignore -->\n| a |\n|---|\n| 1 |\n\n",
        "<!--\nmulti\nline\n-->\n\n",
        "- item <!-- x\n  y --> end\n\n",
        "<!-- <details> --><div>kept</div>\n\n",
        "<!-- left open\n",
    ));
    let mut text = Vec::new();
    crate::doc::visit(&doc.blocks, &mut |b| {
        if let Block::Paragraph(i) = b {
            text.push(plain_text(i));
        }
    });
    assert_eq!(text, ["Before  after.", "item  end", "<div>kept</div>"]);
    assert!(
        matches!(doc.blocks[1], Block::Table { .. }),
        "{:?}",
        doc.blocks
    );
    assert_eq!(details(&doc), []);
}

#[test]
fn skipped_details_end_with_their_container() {
    // A quote holding more details than fit: the last gets no frame and
    // is left unclosed. Once the quote ends, its missing close tag must
    // not be paid for with the close of a later, valid details block.
    let inner = "> <details>\n> <summary>s</summary>\n>\n".repeat(MAX_DEPTH);
    let doc = parse(&format!(
        "{inner}\n<details>\n<summary>valid</summary>\n\nbody\n\n</details>\n\nafter\n"
    ));
    let last = details(&doc).pop().unwrap();
    assert_eq!(
        (last.2.as_str(), last.3),
        ("valid", vec!["body".to_string()])
    );
    assert!(
        matches!(doc.blocks.last(), Some(Block::Paragraph(i)) if plain_text(i) == "after"),
        "{:?}",
        doc.blocks.last()
    );
}

#[test]
fn details_past_max_depth_are_flattened() {
    let open = "<details>\n<summary>s</summary>\n\n".repeat(MAX_DEPTH + 4);
    let close = "</details>\n\n".repeat(MAX_DEPTH + 4);
    let doc = parse(&format!("{open}deep\n\n{close}after\n"));
    assert_eq!(details(&doc).len(), MAX_DEPTH);
    assert!(matches!(doc.blocks.last(), Some(Block::Paragraph(i)) if plain_text(i) == "after"));
}

/// Each link's text and target in `src`.
fn links(src: &str) -> Vec<(String, String)> {
    let doc = parse(src);
    let Block::Paragraph(inlines) = &doc.blocks[0] else {
        panic!("{:?}", doc.blocks);
    };
    inlines
        .iter()
        .filter_map(|i| Some((i.text.clone(), i.style.link.clone()?)))
        .collect()
}

#[test]
fn wiki_links_point_at_markdown_files_next_to_the_document() {
    assert_eq!(
        links(
            "[[Other Page]] [[notes/todo|My list]] [[Guide#Getting Started]] [[#Usage]] [[spec.pdf]]"
        ),
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
}

#[test]
fn wiki_link_syntax_in_code_stays_text() {
    let doc = parse("`[[not a link]]`\n");
    let Block::Paragraph(inlines) = &doc.blocks[0] else {
        panic!("{:?}", doc.blocks);
    };
    assert!(inlines.iter().all(|i| i.style.link.is_none()));
}

/// The first paragraph's runs as `(text, style)`.
fn runs(src: &str) -> Vec<(String, InlineStyle)> {
    let doc = parse(src);
    let Some(Block::Paragraph(inlines)) = doc.blocks.first() else {
        panic!("{:?}", doc.blocks);
    };
    inlines
        .iter()
        .map(|i| (i.text.clone(), i.style.clone()))
        .collect()
}

/// The style of the run whose text is `text`.
fn style_of(src: &str, text: &str) -> InlineStyle {
    runs(src)
        .into_iter()
        .find(|(t, _)| t == text)
        .unwrap_or_else(|| panic!("no run {text:?} in {:?}", runs(src)))
        .1
}

/// Whether a style has the one property a test looks for.
type Has = fn(&InlineStyle) -> bool;

#[test]
fn inline_html_formatting_tags_become_styles() {
    let cases: [(&str, Has); 11] = [
        ("<b>x</b>", |s| s.strong),
        ("<strong>x</strong>", |s| s.strong),
        ("<i>x</i>", |s| s.emphasis),
        ("<em>x</em>", |s| s.emphasis),
        ("<u>x</u>", |s| s.underline),
        ("<s>x</s>", |s| s.strike),
        ("<del>x</del>", |s| s.strike),
        ("<strike>x</strike>", |s| s.strike),
        ("<code>x</code>", |s| s.code),
        ("<kbd>x</kbd>", |s| s.code),
        ("<mark>x</mark>", |s| s.mark),
    ];
    for (tag, has) in cases {
        let src = format!("a {tag} b\n");
        let runs = runs(&src);
        let text: String = runs.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(text, "a x b", "{tag}");
        let x = style_of(&src, "x");
        assert!(has(&x), "{tag}: {x:?}");
        assert!(!x.raw_html);
        assert!(runs.iter().all(|(t, s)| t == "x" || !has(s)), "{tag}");
    }
}

#[test]
fn inline_html_tags_nest_ignore_case_and_attributes() {
    let x = style_of("<b><i>x</i></b>\n", "x");
    assert!(x.strong && x.emphasis);
    let x = style_of("<B>x</b> and <Mark class=\"hi\">y</MARK>\n", "x");
    assert!(x.strong);
    assert!(style_of("<B>x</b> and <Mark class=\"hi\">y</MARK>\n", "y").mark);
    // Markdown inside tags keeps its own style too.
    let x = style_of("<u>a *x* b</u>\n", "x");
    assert!(x.underline && x.emphasis);
    // Tags that cross still each pair by name.
    let x = style_of("<b><i>x</b></i>\n", "x");
    assert!(x.strong && x.emphasis);
    // Inline code keeps tags inside it as text.
    let r = runs("`<b>x</b>`\n");
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].0, "<b>x</b>");
}

#[test]
fn unpaired_and_unknown_inline_tags_stay_as_text() {
    for src in [
        "a <b>open\n",
        "close</i> b\n",
        "<span>x</span>\n",
        "<b>x</strong>\n",
        "<b/> x\n",
    ] {
        let doc = parse(src);
        let Block::Paragraph(inl) = &doc.blocks[0] else {
            panic!("{:?}", doc.blocks);
        };
        assert_eq!(plain_text(inl), src.trim_end(), "{src}");
        assert!(
            inl.iter().all(|i| i.style == InlineStyle::default()),
            "{src}: {inl:?}"
        );
    }
    // An unpaired tag inside a paired one is shown, and styled.
    let r = runs("<b>a <span>x</b>\n");
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].0, "a <span>x");
    assert!(r[0].1.strong);
}

#[test]
fn br_tags_are_line_breaks() {
    for src in ["a<br>b\n", "a<br/>b\n", "a<BR />b\n"] {
        let doc = parse(src);
        let Block::Paragraph(inl) = &doc.blocks[0] else {
            panic!("{:?}", doc.blocks);
        };
        assert_eq!(plain_text(inl), "a\nb", "{src}");
    }
}

#[test]
fn inline_html_in_headings_and_cells() {
    let doc = parse("# <b>Title</b>\n\n| <i>h</i> |\n|---|\n| <mark>c</mark> |\n");
    assert_eq!(doc.title().as_deref(), Some("Title"));
    let Block::Table { header, rows, .. } = &doc.blocks[1] else {
        panic!("{:?}", doc.blocks);
    };
    assert!(header[0].iter().any(|i| i.text == "h" && i.style.emphasis));
    assert!(rows[0][0].iter().any(|i| i.text == "c" && i.style.mark));
}

#[test]
fn double_equals_mark_text() {
    let r = runs("a ==hi there== b\n");
    let text: String = r.iter().map(|(t, _)| t.as_str()).collect();
    assert_eq!(text, "a hi there b");
    assert!(style_of("a ==hi there== b\n", "hi there").mark);
    assert!(!style_of("a ==hi there== b\n", "a ").mark);
    // Across other styles.
    let src = "==a **b** c==\n";
    assert!(style_of(src, "b").mark && style_of(src, "b").strong);
    assert!(style_of(src, "a ").mark);
    // Several on one line.
    let src = "==x== and ==y==\n";
    assert!(style_of(src, "x").mark && style_of(src, "y").mark);
    assert!(!style_of(src, " and ").mark);
}

#[test]
fn double_equals_outside_marks_stay_text() {
    for src in [
        "a == b\n",
        "unmatched ==here\n",
        "a ==b\n",
        "x === y === z\n",
        "`==code==`\n",
        "== spaced ==\n",
    ] {
        let doc = parse(src);
        let Block::Paragraph(inl) = &doc.blocks[0] else {
            panic!("{:?}", doc.blocks);
        };
        assert_eq!(plain_text(inl), src.trim_end().trim_matches('`'), "{src}");
        assert!(inl.iter().all(|i| !i.style.mark), "{src}: {inl:?}");
    }
    // A code span inside a mark keeps its markers' text out of play.
    let src = "==a `==` b==\n";
    let r = runs(src);
    let text: String = r.iter().map(|(t, _)| t.as_str()).collect();
    assert_eq!(text, "a == b");
    assert!(r.iter().all(|(_, s)| s.mark));
}
