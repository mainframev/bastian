//! The table of contents shown beside the document with `t`: the
//! document's headings, nested by level, with the outer ones numbered for
//! the digit keys.

use crate::doc::{Block, Document, plain_text};
use crate::render::Rendered;

/// Columns the pane starts with.
pub const DEFAULT_WIDTH: u16 = 30;
/// The narrowest the pane gets.
pub const MIN_WIDTH: u16 = 16;
/// Columns `<` and `>` take off or add.
pub const STEP: u16 = 4;
/// Screens narrower than this leave the pane out, so the document keeps
/// room to read.
pub const MIN_SCREEN: u16 = 40;

/// A heading in the table of contents.
pub struct Entry {
    /// How many headings it's nested under; 0 for the outermost.
    pub depth: usize,
    /// 1-based number the digit keys jump with, on the numbered level only.
    pub number: Option<usize>,
    pub text: String,
    /// Rendered line of the heading.
    pub line: usize,
}

/// The document's headings in order. A heading nests under the closest
/// one before it of a lower level. The outermost level with more than one
/// heading is numbered, so a lone title doesn't take the only number; with
/// no such level, as in a document of one heading, nothing is.
pub fn entries(doc: &Document, rendered: &Rendered) -> Vec<Entry> {
    let mut entries = Vec::new();
    // Levels of the headings the next one may nest under.
    let mut open: Vec<u8> = Vec::new();
    for (i, block) in doc.blocks.iter().enumerate() {
        let Block::Heading { level, inlines } = block else {
            continue;
        };
        let Some(&line) = rendered.block_starts.get(i) else {
            continue;
        };
        while open.last().is_some_and(|&l| l >= *level) {
            open.pop();
        }
        entries.push(Entry {
            depth: open.len(),
            number: None,
            text: plain_text(inlines),
            line,
        });
        open.push(*level);
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
    entries
}

/// The entry for the section being read with `scroll` at the top of the
/// view: the last heading at or above it. `None` before the first heading.
pub fn current(entries: &[Entry], scroll: usize) -> Option<usize> {
    entries.iter().rposition(|e| e.line <= scroll)
}

/// The pane's width on a screen `screen` columns wide when asked for
/// `width`: kept to at least `MIN_WIDTH` and at most half the screen, or 0
/// when the screen is too narrow for it.
pub fn fit(width: u16, screen: u16) -> u16 {
    if screen < MIN_SCREEN {
        return 0;
    }
    width.clamp(MIN_WIDTH, max_width(screen))
}

/// The widest the pane gets on a screen `screen` columns wide.
pub fn max_width(screen: u16) -> u16 {
    (screen / 2).max(MIN_WIDTH)
}

/// Columns of the `<` and `>` markers on the header row of a pane `width`
/// wide, from its left edge: before a space and the right border.
pub fn markers(width: u16) -> (u16, u16) {
    (width.saturating_sub(5), width.saturating_sub(3))
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
                ("A1", 1, None),
                ("A1a", 2, None),
                ("A2", 1, None),
                ("B", 0, Some(2)),
            ]
        );
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
                ("Deep", 2, None),
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

    #[test]
    fn fit_keeps_the_pane_within_the_screen() {
        assert_eq!(fit(30, 100), 30);
        assert_eq!(fit(4, 100), MIN_WIDTH);
        assert_eq!(fit(90, 100), 50);
        assert_eq!(fit(30, MIN_SCREEN - 1), 0);
        assert_eq!(fit(30, MIN_SCREEN), 20);
    }
}
