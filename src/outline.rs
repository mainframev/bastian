//! The heading outline opened with `O`: a tree of the document's headings
//! that jumps to the chosen one.

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use tui_tree_widget::{TreeItem, TreeState};
use unicode_width::UnicodeWidthStr;

use crate::doc::{Block, Document, plain_text};
use crate::render::Rendered;

pub struct Outline {
    pub items: Vec<TreeItem<'static, usize>>,
    pub state: TreeState<usize>,
    /// Rendered line of each heading, by id.
    lines: Vec<usize>,
    /// Width of the longest entry, indentation included.
    pub width: usize,
}

pub enum Event {
    None,
    /// Scroll to this rendered line and close.
    Jump(usize),
    Close,
}

/// A heading and the ones nested under it.
struct Node {
    id: usize,
    level: u8,
    text: String,
    children: Vec<Node>,
}

impl Outline {
    /// The outline of top-level headings, with every section open and the
    /// one containing `scroll` selected. `None` when there are no headings.
    pub fn new(doc: &Document, rendered: &Rendered, scroll: usize) -> Option<Self> {
        let mut lines = Vec::new();
        let mut roots: Vec<Node> = Vec::new();
        let mut width = 0;
        for (i, block) in doc.blocks.iter().enumerate() {
            let Block::Heading { level, inlines } = block else {
                continue;
            };
            let Some(&line) = rendered.block_starts.get(i) else {
                continue;
            };
            let node = Node {
                id: lines.len(),
                level: *level,
                text: plain_text(inlines),
                children: Vec::new(),
            };
            lines.push(line);
            let depth = insert(&mut roots, node);
            width = width.max(depth * 2 + 4 + plain_text(inlines).width());
        }
        if roots.is_empty() {
            return None;
        }

        let mut state = TreeState::default();
        let mut paths = vec![Vec::new(); lines.len()];
        let items = roots
            .into_iter()
            .map(|n| to_item(n, &[], &mut paths, &mut state))
            .collect();
        let current = lines.iter().rposition(|&l| l <= scroll).unwrap_or(0);
        state.select(paths[current].clone());
        Some(Self {
            items,
            state,
            lines,
            width,
        })
    }

    pub fn on_key(&mut self, key: KeyEvent) -> Event {
        let s = &mut self.state;
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => s.key_down(),
            KeyCode::Char('k') | KeyCode::Up => s.key_up(),
            KeyCode::Char('h') | KeyCode::Left => s.key_left(),
            KeyCode::Char('l') | KeyCode::Right => s.key_right(),
            KeyCode::Char('g') | KeyCode::Home => s.select_first(),
            KeyCode::Char('G') | KeyCode::End => s.select_last(),
            KeyCode::Enter => {
                return match s.selected().last() {
                    Some(&id) => Event::Jump(self.lines[id]),
                    None => Event::None,
                };
            }
            KeyCode::Esc | KeyCode::Char('q' | 'O') => return Event::Close,
            _ => false,
        };
        Event::None
    }
}

/// Add `node` under the last heading of a lower level; returns its depth.
fn insert(siblings: &mut Vec<Node>, node: Node) -> usize {
    match siblings.last_mut() {
        Some(last) if last.level < node.level => 1 + insert(&mut last.children, node),
        _ => {
            siblings.push(node);
            0
        }
    }
}

/// Tree items for `node`, recording each heading's path and opening every
/// section.
fn to_item(
    node: Node,
    parent: &[usize],
    paths: &mut [Vec<usize>],
    state: &mut TreeState<usize>,
) -> TreeItem<'static, usize> {
    let mut path = parent.to_vec();
    path.push(node.id);
    paths[node.id].clone_from(&path);
    if node.children.is_empty() {
        return TreeItem::new_leaf(node.id, node.text);
    }
    state.open(path.clone());
    let children = node
        .children
        .into_iter()
        .map(|c| to_item(c, &path, paths, state))
        .collect();
    TreeItem::new(node.id, node.text, children).expect("heading ids are unique")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::parse;
    use crate::render::{render, theme::Theme};
    use ratatui::crossterm::event::KeyModifiers;

    const DOC: &str = "# A\n\nx\n\n## A1\n\n### A1a\n\n## A2\n\n# B\n\ny\n";

    fn outline(scroll: usize) -> (Outline, Rendered) {
        let r = render(&parse(DOC), 40, &Theme::dark());
        (Outline::new(&parse(DOC), &r, scroll).unwrap(), r)
    }

    #[test]
    fn width_counts_columns_not_chars() {
        let src = "# 日本語\n";
        let r = render(&parse(src), 40, &Theme::dark());
        // Marker and padding (4) plus three double-width characters.
        assert_eq!(Outline::new(&parse(src), &r, 0).unwrap().width, 10);
    }

    #[test]
    fn nests_headings_by_level() {
        let (o, _) = outline(0);
        let shape: Vec<(usize, Vec<usize>)> = o
            .items
            .iter()
            .map(|i| {
                (
                    *i.identifier(),
                    i.children().iter().map(|c| *c.identifier()).collect(),
                )
            })
            .collect();
        assert_eq!(shape, [(0, vec![1, 3]), (4, vec![])]);
        assert_eq!(o.items[0].children()[0].children().len(), 1);
        assert!(
            Outline::new(
                &parse("text\n"),
                &render(&parse("text\n"), 40, &Theme::dark()),
                0
            )
            .is_none()
        );
    }

    #[test]
    fn selects_the_current_section_and_jumps() {
        let (mut o, r) = outline(0);
        assert_eq!(o.state.selected(), [0]);
        // `# B` is the sixth block.
        let b_line = r.block_starts[5];
        let (mut later, _) = outline(b_line);
        assert_eq!(later.state.selected(), [4]);
        match later.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)) {
            Event::Jump(line) => assert_eq!(line, b_line),
            _ => panic!("expected a jump"),
        }
        // Moving needs the rows from a first render, as in the app.
        let area = ratatui::layout::Rect::new(0, 0, 30, 10);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        ratatui::widgets::StatefulWidget::render(
            tui_tree_widget::Tree::new(&o.items).unwrap(),
            area,
            &mut buf,
            &mut o.state,
        );
        o.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        assert_eq!(o.state.selected(), [0, 1]);
        assert!(matches!(
            o.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Event::Close
        ));
    }
}
