//! Moving between `<details>` blocks and opening or closing them.

use super::Reader;

impl Reader {
    /// Focus the next or previous details block shown, starting from the
    /// view when none is focused.
    pub(super) fn move_details_focus(&mut self, forward: bool) {
        let spans = &self.rendered.details;
        if spans.is_empty() {
            self.message = Some("no details blocks".into());
            return;
        }
        let current = self
            .details_focus
            .and_then(|id| spans.iter().position(|d| d.id == id));
        let next = match (current, forward) {
            (Some(i), true) => (i + 1).min(spans.len() - 1),
            (Some(i), false) => i.saturating_sub(1),
            (None, true) => spans
                .iter()
                .position(|d| d.line >= self.scroll)
                .unwrap_or(spans.len() - 1),
            (None, false) => spans
                .iter()
                .rposition(|d| d.line < self.scroll + self.height)
                .unwrap_or(0),
        };
        let line = spans[next].line;
        self.details_focus = Some(spans[next].id);
        self.focus = None;
        self.link_focus = None;
        if line < self.scroll || line >= self.scroll + self.height {
            self.follow = None;
            self.scroll = line.saturating_sub(self.height / 3).min(self.max_scroll());
        }
    }

    /// Open the focused details block if it's closed, or close it.
    pub(super) fn toggle_details(&mut self) {
        let Some(id) = self.details_focus else {
            return;
        };
        if !self.toggled_details.remove(&id) {
            self.toggled_details.insert(id);
        }
        self.dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use crate::doc::parse;
    use crate::reader::Reader;
    use crate::render::{theme::Theme, to_plain};

    const SRC: &str = "\
<details>
<summary>First</summary>

hidden one

</details>

```sh
echo hi
```

<details open>
<summary>Second</summary>

shown two

</details>
";

    fn reader() -> Reader {
        let mut r = Reader::new("t".into(), parse(SRC), Theme::dark(), 80);
        r.layout(80, 40);
        r
    }

    fn press(r: &mut Reader, code: KeyCode) {
        r.on_key(KeyEvent::new(code, KeyModifiers::NONE));
        r.layout(80, 40);
    }

    fn text(r: &Reader) -> String {
        to_plain(&r.rendered.lines)
    }

    #[test]
    fn details_start_closed_unless_marked_open() {
        let r = reader();
        assert!(text(&r).contains("▶ First") && !text(&r).contains("hidden one"));
        assert!(text(&r).contains("▼ Second") && text(&r).contains("shown two"));
    }

    #[test]
    fn d_focuses_and_enter_toggles() {
        let mut r = reader();
        press(&mut r, KeyCode::Char('d'));
        assert_eq!(r.details_focus, Some(0));
        press(&mut r, KeyCode::Enter);
        assert!(text(&r).contains("▼ First") && text(&r).contains("hidden one"));
        press(&mut r, KeyCode::Char('d'));
        assert_eq!(r.details_focus, Some(1));
        press(&mut r, KeyCode::Enter);
        assert!(text(&r).contains("▶ Second") && !text(&r).contains("shown two"));
        press(&mut r, KeyCode::Char('D'));
        press(&mut r, KeyCode::Enter);
        assert!(!text(&r).contains("hidden one"), "{}", text(&r));
    }

    #[test]
    fn tab_moves_focus_to_code() {
        let mut r = reader();
        press(&mut r, KeyCode::Char('d'));
        press(&mut r, KeyCode::Tab);
        assert_eq!(r.details_focus, None);
        assert!(r.focus.is_some());
    }

    #[test]
    fn without_details_d_says_so() {
        let mut r = Reader::new("t".into(), parse("text\n"), Theme::dark(), 80);
        r.layout(80, 40);
        press(&mut r, KeyCode::Char('d'));
        assert_eq!(r.message.as_deref(), Some("no details blocks"));
    }
}
