//! The table of contents shown beside the document with `t`: the
//! document's headings, nested by level and numbered for the digit keys,
//! the outer ones in order and the ones under them within their section.

use crate::doc::{Block, Document, plain_text};
use crate::render::Rendered;

/// Columns the pane starts with.
pub const DEFAULT_WIDTH: u16 = 30;

/// A heading in the table of contents.
pub struct Entry {
    /// How many headings it's nested under; 0 for the outermost.
    pub depth: usize,
    /// The entry it's nested under.
    pub parent: Option<usize>,
    /// 1-based number the digit keys jump with: across the numbered
    /// level, then within its section below that.
    pub number: Option<usize>,
    pub text: String,
    /// Rendered line of the heading.
    pub line: usize,
}

/// The document's headings in order. A heading nests under the closest
/// one before it of a lower level. The outermost level with more than one
/// heading is numbered, so a lone title doesn't take the only number; with
/// no such level, as in a document of one heading, nothing is. Below it,
/// each heading is numbered among the ones in its section.
pub fn entries(doc: &Document, rendered: &Rendered) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Vec::new();
    // Levels and entries of the headings the next one may nest under.
    let mut open: Vec<(u8, usize)> = Vec::new();
    for (i, block) in doc.blocks.iter().enumerate() {
        let Block::Heading { level, inlines } = block else {
            continue;
        };
        let Some(&line) = rendered.block_starts.get(i) else {
            continue;
        };
        while open.last().is_some_and(|&(l, _)| l >= *level) {
            open.pop();
        }
        open.push((*level, entries.len()));
        entries.push(Entry {
            depth: open.len() - 1,
            parent: open.len().checked_sub(2).map(|i| open[i].1),
            number: None,
            text: plain_text(inlines),
            line,
        });
    }
    let max_depth = entries.iter().map(|e| e.depth).max().unwrap_or(0);
    let Some(numbered) =
        (0..=max_depth).find(|&d| entries.iter().filter(|e| e.depth == d).count() > 1)
    else {
        return entries;
    };
    let mut n = 0;
    for e in entries.iter_mut().filter(|e| e.depth == numbered) {
        n += 1;
        e.number = Some(n);
    }
    for i in 0..entries.len() {
        if entries[i].depth > numbered {
            let before = entries[..i]
                .iter()
                .filter(|e| e.parent == entries[i].parent)
                .count();
            entries[i].number = Some(before + 1);
        }
    }
    entries
}

/// The depth whose entries the digit keys reach first.
pub fn numbered_depth(entries: &[Entry]) -> Option<usize> {
    entries.iter().find(|e| e.number.is_some()).map(|e| e.depth)
}

/// The entry for the section being read with `scroll` at the top of the
/// view: the last heading at or above it. `None` before the first heading.
pub fn current(entries: &[Entry], scroll: usize) -> Option<usize> {
    entries.iter().rposition(|e| e.line <= scroll)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::parse;
    use crate::render::{render, theme::Theme};

    fn toc(src: &str) -> (Vec<Entry>, Rendered) {
        let r = render(&parse(src), 40, &Theme::dark());
        (entries(&parse(src), &r), r)
    }

    fn shape(entries: &[Entry]) -> Vec<(&str, usize, Option<usize>)> {
        entries
            .iter()
            .map(|e| (e.text.as_str(), e.depth, e.number))
            .collect()
    }

    #[test]
    fn nests_and_numbers_the_outer_level() {
        let (e, _) = toc("# A\n\nx\n\n## A1\n\n### A1a\n\n## A2\n\n# B\n\ny\n");
        assert_eq!(
            shape(&e),
            [
                ("A", 0, Some(1)),
                ("A1", 1, Some(1)),
                ("A1a", 2, Some(1)),
                ("A2", 1, Some(2)),
                ("B", 0, Some(2)),
            ]
        );
        let parents: Vec<_> = e.iter().map(|e| e.parent).collect();
        assert_eq!(parents, [None, Some(0), Some(1), Some(0), None]);
        assert!(toc("text\n").0.is_empty());
    }

    #[test]
    fn a_lone_title_leaves_the_numbers_to_its_sections() {
        let (e, _) = toc("# Title\n\n## One\n\n#### Deep\n\n## Two\n");
        assert_eq!(
            shape(&e),
            [
                ("Title", 0, None),
                ("One", 1, Some(1)),
                ("Deep", 2, Some(1)),
                ("Two", 1, Some(2)),
            ]
        );
        // With no level of more than one heading, nothing is numbered.
        assert_eq!(shape(&toc("## Only\n").0), [("Only", 0, None)]);
        assert_eq!(
            shape(&toc("# Book\n\n## Part\n").0),
            [("Book", 0, None), ("Part", 1, None)]
        );
    }

    #[test]
    fn current_is_the_last_heading_at_or_above_the_top() {
        let (e, r) = toc("intro\n\n# A\n\nx\n\n# B\n\ny\n");
        assert_eq!(current(&e, 0), None);
        let b = r.block_starts[3];
        assert_eq!(e[1].line, b);
        assert_eq!(current(&e, b - 1), Some(0));
        assert_eq!(current(&e, b), Some(1));
        assert_eq!(current(&e, usize::MAX), Some(1));
    }
}
