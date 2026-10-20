use super::Reader;
use crate::pane;
use crate::toc::{self, Entry};

impl Reader {
    /// The table of contents for the document as last laid out.
    pub fn toc_entries(&self) -> Vec<Entry> {
        toc::entries(&self.doc, &self.rendered)
    }

    /// The width asked for the table of contents, kept for the session.
    pub fn toc_width(&self) -> u16 {
        self.toc_width
    }

    pub fn set_toc_width(&mut self, width: u16) {
        self.toc_width = width;
    }

    /// Columns the table of contents takes on a screen `screen` columns
    /// wide: 0 when it's closed or doesn't fit. Remembers the screen
    /// width, which `<`, `>` and clicks size the pane against.
    pub fn fit_toc(&mut self, screen: u16) -> u16 {
        self.fit_toc_beside(screen, 0)
    }

    /// Like `fit_toc`, beside the theme picker `taken` columns wide: left
    /// out when the document would be too narrow between them.
    pub fn fit_toc_beside(&mut self, screen: u16, taken: u16) -> u16 {
        self.screen = screen;
        if self.toc {
            pane::fit_beside(self.toc_width, screen, taken)
        } else {
            0
        }
    }

    pub(super) fn toggle_toc(&mut self) {
        if !self.toc && self.toc_entries().is_empty() {
            self.message = Some("no headings in this document".into());
        } else {
            self.toc = !self.toc;
        }
    }

    /// Narrow or widen the table of contents by `pane::STEP` columns.
    pub(super) fn resize_toc(&mut self, wider: bool) {
        self.toc_width = pane::resize(self.toc_width, self.screen, wider);
    }

    /// The line the table of contents marks the section of: the heading
    /// last jumped to while the view hasn't moved since, else the top.
    pub fn toc_line(&self) -> usize {
        self.toc_mark()
            .and_then(|i| self.toc_entries().get(i).map(|e| e.line))
            .unwrap_or(self.scroll)
    }

    /// The entry last jumped to, while the view hasn't moved since.
    fn toc_mark(&self) -> Option<usize> {
        match self.toc_jump {
            Some((entry, at)) if at == self.scroll => Some(entry),
            _ => None,
        }
    }

    /// Scroll entry `i`'s heading, at `line`, to the top, or as near as
    /// the end allows, and remember it as the heading jumped to.
    fn jump_to_heading(&mut self, i: usize, line: usize) {
        self.scroll_to(line);
        self.toc_jump = Some((i, self.scroll));
    }

    /// Scroll to the heading numbered `n` in the table of contents. Right
    /// after a jump by number (`walk`: the number pressed and the entry
    /// whose section it walks), the same `n` steps to the next heading of
    /// that section in document order, so `1 1 1` goes A, A1, A1a, and
    /// past its last heading back to the section's own. Any other `n`
    /// goes to the `n`th of the numbered level, so `2` then `3` is the
    /// third section, not a heading inside the second.
    pub(super) fn jump_to_entry(&mut self, n: usize, walk: Option<(usize, usize)>) {
        let entries = self.toc_entries();
        let Some(top) = toc::numbered_depth(&entries) else {
            self.message = Some("no numbered headings".into());
            return;
        };
        let walk = walk.filter(|&(_, root)| root < entries.len());
        let mark = self.toc_mark().filter(|_| walk.is_some());
        let again = walk.filter(|&(digit, _)| digit == n).zip(mark);
        if let Some(((_, root), at)) = again {
            // A section's headings follow it until one as shallow as it.
            let next = at + 1;
            let inside = entries
                .get(next)
                .is_some_and(|e| e.depth > entries[root].depth);
            let i = if inside { next } else { root };
            self.jump_to_heading(i, entries[i].line);
            self.toc_walk = Some((n, root));
            return;
        }
        let i = entries
            .iter()
            .position(|e| e.depth == top && e.number == Some(n));
        match i {
            Some(i) => {
                self.jump_to_heading(i, entries[i].line);
                self.toc_walk = Some((n, i));
            }
            None => self.message = Some(format!("no heading {n:02}")),
        }
    }

    /// Scroll to the next heading below the top of the view, or the one
    /// above it.
    pub(super) fn step_heading(&mut self, forward: bool) {
        let entries = self.toc_entries();
        // From the heading last jumped to, which may sit below the top.
        let from = self.toc_line();
        let i = if forward {
            entries.iter().position(|e| e.line > from)
        } else {
            entries.iter().rposition(|e| e.line < from)
        };
        match i {
            Some(i) => self.jump_to_heading(i, entries[i].line),
            None if entries.is_empty() => {
                self.message = Some("no headings in this document".into());
            }
            None => {}
        }
    }

    /// A left click at `column`, `row` of the screen: the `<` and `>`
    /// markers on the table of contents' header resize it. Returns whether
    /// it hit one.
    pub fn on_click(&mut self, column: u16, row: u16) -> bool {
        let width = self.fit_toc(self.screen);
        // Popups may cover the header.
        let hit = pane::marker_hit(width, column, row).filter(|_| !self.popup_open());
        let Some(wider) = hit else {
            return false;
        };
        self.resize_toc(wider);
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::doc::parse;
    use crate::reader::{Options, Reader, Source};
    use crate::{pane, toc};
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn reader(src: &str) -> Reader {
        let mut r = Reader::new(Source::in_memory("t", parse(src)), &Options::default());
        r.layout(80, 10);
        r.fit_toc(100);
        r
    }

    fn key(r: &mut Reader, c: char) {
        r.on_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }

    /// Twelve sections with filler between, so each can reach the top.
    fn sections() -> String {
        let filler: String = (0..12).map(|i| format!("para {i}\n\n")).collect();
        (1..=12).map(|i| format!("# S{i}\n\n{filler}")).collect()
    }

    fn top(r: &Reader) -> String {
        crate::render::plain(&r.rendered().lines[r.scroll()])
            .trim()
            .to_string()
    }

    #[test]
    fn t_toggles_the_pane_unless_there_are_no_headings() {
        let mut r = reader("# A\n\ntext\n");
        assert_eq!(r.fit_toc(100), 0);
        key(&mut r, 't');
        assert!(r.toc);
        assert_eq!(r.fit_toc(100), toc::DEFAULT_WIDTH);
        key(&mut r, 't');
        assert!(!r.toc);

        let mut plain = reader("just text\n");
        key(&mut plain, 't');
        assert!(!plain.toc);
        assert_eq!(plain.message(), Some("no headings in this document"));
    }

    #[test]
    fn digits_jump_to_numbered_entries_while_open() {
        let mut r = reader(&sections());
        // Closed, digits do nothing.
        key(&mut r, '3');
        assert_eq!(r.scroll(), 0);
        key(&mut r, 't');
        key(&mut r, '3');
        assert_eq!(top(&r), "S3");
        key(&mut r, '0');
        assert_eq!(top(&r), "S10");
        key(&mut r, '1');
        assert_eq!(top(&r), "S1");
        assert_eq!(toc::current(&r.toc_entries(), r.scroll()), Some(0));
    }

    #[test]
    fn missing_number_says_so() {
        let mut r = reader("# A\n\n# B\n");
        key(&mut r, 't');
        key(&mut r, '7');
        assert_eq!(r.message(), Some("no heading 07"));

        let mut lone = reader("# Only\n\ntext\n");
        key(&mut lone, 't');
        key(&mut lone, '1');
        assert_eq!(lone.message(), Some("no numbered headings"));
    }

    #[test]
    fn shift_j_and_k_step_through_headings() {
        let mut r = reader(&sections());
        key(&mut r, 'J');
        assert_eq!(top(&r), "S2");
        key(&mut r, 'J');
        assert_eq!(top(&r), "S3");
        key(&mut r, 'K');
        assert_eq!(top(&r), "S2");
        r.scroll_by(1);
        key(&mut r, 'K');
        assert_eq!(top(&r), "S2");
    }

    #[test]
    fn angle_brackets_resize_within_the_screen() {
        let mut r = reader("# A\n");
        key(&mut r, 't');
        // `<`/`>` only resize while the pane is open; this one is.
        key(&mut r, '>');
        assert_eq!(r.toc_width(), toc::DEFAULT_WIDTH + pane::STEP);
        for _ in 0..20 {
            key(&mut r, '>');
        }
        assert_eq!(r.toc_width(), 50, "half of the 100-column screen");
        for _ in 0..20 {
            key(&mut r, '<');
        }
        assert_eq!(r.toc_width(), pane::MIN_WIDTH);
        // On a smaller screen, widening starts from what fits.
        r.set_toc_width(50);
        r.fit_toc(60);
        key(&mut r, '<');
        assert_eq!(r.toc_width(), 26);
    }

    #[test]
    fn clicks_on_the_markers_resize() {
        let mut r = reader("# A\n");
        assert!(!r.on_click(0, 0), "closed");
        key(&mut r, 't');
        let width = r.fit_toc(100);
        let (narrower, wider) = pane::markers(width);
        assert!(r.on_click(wider, 0));
        assert_eq!(r.toc_width(), width + pane::STEP);
        let (narrower_now, _) = pane::markers(r.toc_width());
        assert!(r.on_click(narrower_now, 0));
        assert_eq!(r.toc_width(), width);
        assert!(!r.on_click(narrower, 1), "only the header row");
        assert!(!r.on_click(1, 0));
        assert_eq!(r.toc_width(), width);
    }

    #[test]
    fn jumps_near_the_end_mark_the_heading_they_aimed_at() {
        // Short last sections: their headings can't scroll to the top.
        let filler: String = (0..30).map(|i| format!("para {i}\n\n")).collect();
        let mut r = reader(&format!(
            "# A\n\n{filler}# B\n\nb\n\n# C\n\nc\n\n# D\n\nd\n"
        ));
        r.layout(60, 20);
        key(&mut r, 't');
        let current = |r: &Reader| {
            let e = r.toc_entries();
            toc::current(&e, r.toc_line()).map(|i| e[i].text.clone())
        };
        key(&mut r, '3');
        assert_eq!(r.scroll(), r.max_scroll(), "the end stops it short");
        assert_eq!(current(&r).as_deref(), Some("C"));
        key(&mut r, '4');
        assert_eq!(current(&r).as_deref(), Some("D"));
        // J and K step on from the heading jumped to, not the top line.
        key(&mut r, 'K');
        assert_eq!(current(&r).as_deref(), Some("C"));
        key(&mut r, 'K');
        assert_eq!(current(&r).as_deref(), Some("B"));
        key(&mut r, 'J');
        assert_eq!(current(&r).as_deref(), Some("C"));
        // Scrolling away drops the mark, also when the view comes back.
        key(&mut r, '3');
        r.scroll_by(-1);
        r.scroll_by(1);
        assert_eq!(r.scroll(), r.max_scroll());
        // A's filler still fills the top of the last screen.
        assert_eq!(current(&r).as_deref(), Some("A"), "the top's section");
        key(&mut r, '3');
        key(&mut r, 'g');
        assert_eq!(current(&r).as_deref(), Some("A"));
        key(&mut r, 'G');
        assert_eq!(current(&r).as_deref(), Some("A"));
        // And J steps from there, to B.
        key(&mut r, 'J');
        assert_eq!(current(&r).as_deref(), Some("B"));
    }

    #[test]
    fn a_new_layout_keeps_the_heading_jumped_to() {
        // Fits on screen, so the scroll stays at the top throughout.
        let long = "word ".repeat(40);
        let mut r = reader(&format!("# A\n\n{long}\n\n# B\n\nb\n\n# C\n\nc\n"));
        r.layout(80, 40);
        key(&mut r, 't');
        let current = |r: &Reader| {
            let e = r.toc_entries();
            toc::current(&e, r.toc_line()).map(|i| e[i].text.clone())
        };
        key(&mut r, '2');
        assert_eq!(current(&r).as_deref(), Some("B"));
        // Narrower: the text before B wraps more and B moves down.
        r.layout(30, 40);
        assert_eq!(r.scroll(), 0);
        assert_eq!(current(&r).as_deref(), Some("B"));
        key(&mut r, 'J');
        assert_eq!(current(&r).as_deref(), Some("C"));
    }

    #[test]
    fn another_number_goes_to_the_numbered_level() {
        let filler: String = (0..40).map(|i| format!("p{i}\n\n")).collect();
        let mut r = reader(&format!(
            "# A\n\n{filler}# B\n\n{filler}## B1\n\n{filler}## B2\n\n{filler}### B2a\n\n{filler}# C\n\n{filler}"
        ));
        r.layout(60, 20);
        key(&mut r, 't');
        let current = |r: &Reader| {
            let e = r.toc_entries();
            toc::current(&e, r.toc_line()).map(|i| e[i].text.clone())
        };
        key(&mut r, '2');
        assert_eq!(current(&r).as_deref(), Some("B"));
        // The same number again walks B's section.
        key(&mut r, '2');
        assert_eq!(current(&r).as_deref(), Some("B1"));
        key(&mut r, '2');
        assert_eq!(current(&r).as_deref(), Some("B2"));
        // Another number goes to the numbered level, not into B2.
        key(&mut r, '1');
        assert_eq!(current(&r).as_deref(), Some("A"));
        key(&mut r, '2');
        key(&mut r, '3');
        assert_eq!(current(&r).as_deref(), Some("C"));
        key(&mut r, '2');
        key(&mut r, '1');
        assert_eq!(current(&r).as_deref(), Some("A"));
        key(&mut r, '2');
        key(&mut r, '9');
        assert_eq!(r.message(), Some("no heading 09"));
        // Moving starts over at the numbered level.
        key(&mut r, '2');
        key(&mut r, 'j');
        key(&mut r, '1');
        assert_eq!(current(&r).as_deref(), Some("A"));
        // So does J, which isn't a jump by number.
        key(&mut r, '2');
        key(&mut r, 'J');
        key(&mut r, '3');
        assert_eq!(current(&r).as_deref(), Some("C"));
    }

    #[test]
    fn j_and_k_step_through_headings_while_the_toc_is_open() {
        let mut r = reader(&sections());
        let current = |r: &Reader| {
            let e = r.toc_entries();
            toc::current(&e, r.toc_line()).map(|i| e[i].text.clone())
        };
        // Closed, j scrolls a line.
        key(&mut r, 'j');
        assert_eq!(r.scroll(), 1);
        key(&mut r, 'g');
        key(&mut r, 't');
        key(&mut r, 'j');
        assert_eq!(current(&r).as_deref(), Some("S2"));
        key(&mut r, 'j');
        assert_eq!(current(&r).as_deref(), Some("S3"));
        key(&mut r, 'k');
        assert_eq!(current(&r).as_deref(), Some("S2"));
        let down = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
        r.on_key(down);
        assert_eq!(current(&r).as_deref(), Some("S3"));
        let up = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
        r.on_key(up);
        assert_eq!(current(&r).as_deref(), Some("S2"));
    }

    #[test]
    fn keys_that_cannot_move_still_end_the_drill_down() {
        // Fits on screen, so nothing scrolls and K has no heading before A.
        let mut r = reader("# A\n\n## A1\n\n## A2\n\n# B\n");
        r.layout(80, 40);
        key(&mut r, 't');
        let current = |r: &Reader| {
            let e = r.toc_entries();
            toc::current(&e, r.toc_line()).map(|i| e[i].text.clone())
        };
        for reset in ['g', 'k', 'K'] {
            key(&mut r, '1');
            key(&mut r, reset);
            key(&mut r, '2');
            assert_eq!(current(&r).as_deref(), Some("B"), "after {reset}");
        }
        // The wheel at the top, too.
        key(&mut r, '1');
        r.scroll_by(-1);
        key(&mut r, '2');
        assert_eq!(current(&r).as_deref(), Some("B"), "after the wheel");
    }

    #[test]
    fn the_same_number_again_walks_its_section() {
        let current = |r: &Reader| {
            let e = r.toc_entries();
            toc::current(&e, r.toc_line()).map(|i| e[i].text.clone())
        };
        let walk = |src: &str, keys: &str| {
            // Fits on screen, so every jump is told apart by its mark.
            let mut r = reader(src);
            r.layout(80, 60);
            key(&mut r, 't');
            keys.chars()
                .map(|c| {
                    key(&mut r, c);
                    current(&r).unwrap_or_default()
                })
                .collect::<Vec<_>>()
        };

        let flat = "# A\n\n## A1\n\n## A2\n\n## A3\n\n## A4\n\n# B\n";
        assert_eq!(walk(flat, "11111"), ["A", "A1", "A2", "A3", "A4"]);
        // Past the section's last heading, back to its own.
        assert_eq!(walk(flat, "111111"), ["A", "A1", "A2", "A3", "A4", "A"]);

        let nested = "# A\n\n## A1\n\n### A1a\n\n## A2\n\n### A2a\n\n# B\n\n## B1\n";
        assert_eq!(walk(nested, "11111"), ["A", "A1", "A1a", "A2", "A2a"]);
        assert_eq!(walk(nested, "111111"), ["A", "A1", "A1a", "A2", "A2a", "A"]);

        // Another number goes to the numbered level, even when the
        // heading reached has one of that number under it.
        assert_eq!(walk(nested, "112"), ["A", "A1", "B"]);
        assert_eq!(walk(nested, "12"), ["A", "B"]);
        assert_eq!(walk(nested, "121"), ["A", "B", "A"]);
        // A section with nothing under it stays put.
        assert_eq!(walk(flat, "22"), ["B", "B"]);
    }
}
