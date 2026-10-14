use super::{Overlay, Reader};
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

    /// Scroll to the heading numbered `n` in the table of contents.
    pub(super) fn jump_to_entry(&mut self, n: usize) {
        let entries = self.toc_entries();
        match entries.iter().find(|e| e.number == Some(n)) {
            Some(e) => self.jump_to(e.line),
            None if entries.iter().all(|e| e.number.is_none()) => {
                self.message = Some("no numbered headings".into());
            }
            None => self.message = Some(format!("no heading {n:02}")),
        }
    }

    /// Scroll to the next heading below the top of the view, or the one
    /// above it.
    pub(super) fn step_heading(&mut self, forward: bool) {
        let entries = self.toc_entries();
        let line = if forward {
            entries.iter().map(|e| e.line).find(|&l| l > self.scroll)
        } else {
            entries.iter().map(|e| e.line).rfind(|&l| l < self.scroll)
        };
        match line {
            Some(line) => self.jump_to(line),
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
        // Popups may cover the header.
        let popup = self
            .overlay
            .as_ref()
            .is_some_and(|o| !matches!(o, Overlay::Prompt(_) | Overlay::GoTo(_)));
        let width = self.fit_toc(self.screen);
        if popup || width == 0 || row != 0 {
            return false;
        }
        let (narrower, wider) = pane::markers(width);
        if column == narrower || column == wider {
            self.resize_toc(column == wider);
            return true;
        }
        false
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
}
