use super::Reader;
use crate::positions::Position;

impl Reader {
    pub fn max_scroll(&self) -> usize {
        self.rendered.lines.len().saturating_sub(self.height)
    }

    /// Scroll down so the followed block's output stays visible, without
    /// pushing the top of the block off screen.
    pub(super) fn follow_output(&mut self) {
        let Some(range) = self
            .follow
            .and_then(|id| self.rendered.code_blocks.iter().find(|c| c.id == id))
        else {
            return;
        };
        let height = self.height.max(1);
        if range.output_end > self.scroll + height {
            let want = range.output_end - height;
            self.scroll = want.min(range.lines.start).max(self.scroll);
        }
    }

    /// Scroll to a remembered position: the block starting closest before
    /// its line (blocks aren't always drawn in source order; footnotes go
    /// last), then as far into that block as it was.
    pub(super) fn scroll_to_position(&mut self, p: Position) {
        if let Some(&(start, _)) = self
            .rendered
            .anchors
            .iter()
            .filter(|(_, src)| *src < p.line)
            .max_by_key(|&&(rendered, src)| (src, std::cmp::Reverse(rendered)))
        {
            let rows = self.block_rows(start);
            self.scroll = start + ((rows as f32 * p.within) as usize).min(rows.saturating_sub(1));
        }
    }

    /// Rendered rows of the block starting at row `start`, up to the next
    /// block (blocks aren't always in source order, so not just the next
    /// anchor in the list).
    fn block_rows(&self, start: usize) -> usize {
        let end = self
            .rendered
            .anchors
            .iter()
            .map(|&(row, _)| row)
            .filter(|&row| row > start)
            .min()
            .unwrap_or(self.rendered.lines.len());
        end - start
    }

    /// Where this document is being read: the block at the top of the view
    /// and how far into it.
    pub fn position(&self) -> Option<Position> {
        let &(start, src) = self
            .rendered
            .anchors
            .iter()
            .filter(|(row, _)| *row <= self.scroll)
            .max_by_key(|(row, _)| *row)?;
        let rows = self.block_rows(start).max(1);
        Some(Position {
            line: src + 1,
            within: (self.scroll - start) as f32 / rows as f32,
        })
    }

    /// Save where this document is being read, for next time it opens. At
    /// the top there's nothing to keep, so any old position is forgotten.
    pub fn remember_position(&self) {
        if let (Some(store), Some(file)) = (&self.positions, &self.source) {
            let position = (self.scroll > 0).then(|| self.position()).flatten();
            crate::positions::save(store, file, position);
        }
    }

    /// Scroll by `delta` lines (negative is up).
    pub fn scroll_by(&mut self, delta: isize) {
        self.follow = None;
        let to = self
            .scroll
            .saturating_add_signed(delta)
            .min(self.max_scroll());
        self.move_view(to);
    }

    /// Scroll so `line` is visible, placing it a third of the way down.
    pub(super) fn reveal(&mut self, line: usize) {
        let visible = self.scroll..self.scroll + self.height;
        if !visible.contains(&line) {
            self.scroll = line.saturating_sub(self.height / 3).min(self.max_scroll());
        }
    }

    /// Scroll so `line` is at the top, as far as the document allows.
    pub(super) fn jump_to(&mut self, line: usize) {
        self.follow = None;
        self.move_view(line.min(self.max_scroll()));
    }

    /// Scroll to `to`; a view that moves forgets the heading last jumped
    /// to, even if it comes back to the same place later.
    fn move_view(&mut self, to: usize) {
        if to != self.scroll {
            self.toc_jump = None;
            self.goto_mark = None;
        }
        self.scroll = to;
    }
}

#[cfg(test)]
mod tests {
    use crate::doc::parse;
    use crate::positions::Position;
    use crate::reader::{Options, Reader, Source};

    #[test]
    fn restore_near_the_end_survives_re_renders() {
        let src: String = (0..50).map(|i| format!("para {i}\n\n")).collect();
        let mut r = Reader::new(Source::in_memory("t", parse(&src)), &Options::default());
        // The last paragraph: its block can't reach the top of the view.
        r.restore = Some(Position {
            line: 99,
            within: 0.0,
        });
        r.layout(80, 10);
        let at = r.scroll;
        assert_eq!(at, r.max_scroll());
        r.layout(80, 10);
        r.layout(70, 10);
        assert!(r.restore.is_some());
        assert_eq!(r.scroll, r.max_scroll());
    }

    #[test]
    fn position_inside_a_long_block_survives_a_new_width() {
        let items: String = (0..60).map(|i| format!("- item {i}\n")).collect();
        let src = format!("intro\n\n{items}\nafter\n");
        let mut r = Reader::new(Source::in_memory("t", parse(&src)), &Options::default());
        r.layout(80, 10);
        // Deep inside the one list block.
        r.scroll = 2 + 40;
        let p = r.position().unwrap();
        assert_eq!(p.line, 3);
        assert!(p.within > 0.6 && p.within < 0.7, "{p:?}");
        let mut again = Reader::new(Source::in_memory("t", parse(&src)), &Options::default());
        again.restore = Some(p);
        again.layout(80, 10);
        assert_eq!(
            crate::render::plain(&again.rendered.lines[again.scroll]),
            "• item 40"
        );
    }

    #[test]
    fn restore_finds_the_closest_block_in_any_order() {
        let src: String = (0..50).map(|i| format!("para {i}\n\n")).collect();
        let mut r = Reader::new(Source::in_memory("t", parse(&src)), &Options::default());
        r.layout(80, 10);
        // As with footnotes: a block drawn last that starts early in the
        // source.
        let last = r.rendered.lines.len() - 1;
        r.rendered.anchors.push((last, 1));
        r.scroll_to_position(Position {
            line: 61,
            within: 0.0,
        });
        assert_eq!(r.scroll, 60);
    }
}
