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

    /// Scroll by `delta` lines (negative is up).
    pub fn scroll_by(&mut self, delta: isize) {
        self.follow = None;
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
