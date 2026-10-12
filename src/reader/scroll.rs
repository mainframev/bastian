use super::Reader;

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

    /// Scroll to the block that holds 1-based source `line`: the one that
    /// starts closest before it. Blocks aren't always drawn in source order
    /// (footnotes go last), so this doesn't assume they are.
    pub(super) fn scroll_to_source_line(&mut self, line: usize) {
        if let Some(&(rendered, _)) = self
            .rendered
            .anchors
            .iter()
            .filter(|(_, src)| *src < line)
            .max_by_key(|&&(rendered, src)| (src, std::cmp::Reverse(rendered)))
        {
            self.scroll = rendered;
        }
    }

    /// Save where this document is being read, for next time it opens. At
    /// the top there's nothing to keep, so any old position is forgotten.
    pub fn remember_position(&self) {
        if let (Some(store), Some(file)) = (&self.positions, &self.source) {
            let line = (self.scroll > 0).then(|| self.source_line()).flatten();
            crate::positions::save(store, file, line);
        }
    }

    /// Scroll by `delta` lines (negative is up).
    pub fn scroll_by(&mut self, delta: isize) {
        self.follow = None;
        self.restore = None;
        self.scroll = self
            .scroll
            .saturating_add_signed(delta)
            .min(self.max_scroll());
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
        self.scroll = line.min(self.max_scroll());
    }
}

#[cfg(test)]
mod tests {
    use crate::doc::parse;
    use crate::reader::Reader;
    use crate::render::theme::Theme;

    #[test]
    fn restore_finds_the_closest_block_in_any_order() {
        let src: String = (0..50).map(|i| format!("para {i}\n\n")).collect();
        let mut r = Reader::new("t".into(), parse(&src), Theme::dark(), 80);
        r.layout(80, 10);
        // As with footnotes: a block drawn last that starts early in the
        // source.
        let last = r.rendered.lines.len() - 1;
        r.rendered.anchors.push((last, 1));
        r.scroll_to_source_line(61);
        assert_eq!(r.scroll, 60);
    }
}
