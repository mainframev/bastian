//! Moving between `<details>` blocks and opening or closing them.

use std::collections::HashSet;

use super::Reader;
use crate::doc::Block;

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
        // Pictures inside it may be on show now; the next layout asks for
        // the ones not loaded yet.
        self.images_requested = false;
        self.dirty = true;
    }

    /// URLs of the image blocks on show. Those inside closed details blocks
    /// aren't fetched until the block is opened.
    pub(super) fn visible_images(&self) -> Vec<String> {
        let mut urls = Vec::new();
        shown_images(&self.doc.blocks, &self.toggled_details, &mut urls);
        urls
    }
}

fn shown_images(blocks: &[Block], toggled: &HashSet<usize>, out: &mut Vec<String>) {
    for b in blocks {
        match b {
            Block::Image { url, .. } => out.push(url.clone()),
            Block::Details {
                id, open, blocks, ..
            } => {
                if toggled.contains(id) != *open {
                    shown_images(blocks, toggled, out);
                }
            }
            Block::Quote(inner)
            | Block::Callout { blocks: inner, .. }
            | Block::Footnote { blocks: inner, .. } => shown_images(inner, toggled, out),
            Block::List { items, .. } => items.iter().for_each(|i| shown_images(i, toggled, out)),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use crate::doc::parse;
    use crate::reader::{Action, Reader};
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
    fn esc_drops_details_focus_before_going_back() {
        let mut r = reader();
        press(&mut r, KeyCode::Char('d'));
        let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert!(matches!(r.on_key(esc), Action::None));
        assert_eq!(r.details_focus, None);
        assert!(matches!(r.on_key(esc), Action::Back));
    }

    #[test]
    fn a_reload_forgets_opened_and_focused_details() {
        let mut r = reader();
        press(&mut r, KeyCode::Char('d'));
        press(&mut r, KeyCode::Enter);
        assert!(text(&r).contains("hidden one"));
        // An edit puts a new block first, so id 0 names it now.
        r.replace_doc(parse(&format!(
            "<details>\n<summary>New</summary>\n\nnew body\n\n</details>\n\n{SRC}"
        )));
        r.layout(80, 40);
        assert_eq!(r.details_focus, None);
        assert!(
            !text(&r).contains("new body") && !text(&r).contains("hidden one"),
            "{}",
            text(&r)
        );
    }

    #[test]
    fn pictures_in_closed_details_load_once_opened() {
        use std::time::{Duration, Instant};

        use crate::images::{Images, Support};
        use ratatui_image::picker::Picker;

        let dir = tempfile::tempdir().unwrap();
        for name in ["shown.png", "hidden.png"] {
            image::RgbImage::from_pixel(4, 4, image::Rgb([1, 2, 3]))
                .save(dir.path().join(name))
                .unwrap();
        }
        let src = "![a](shown.png)\n\n<details>\n<summary>S</summary>\n\n![b](hidden.png)\n\n</details>\n";
        let mut r = Reader::new("t".into(), parse(src), Theme::dark(), 80);
        r.base_dir = dir.path().to_path_buf();
        r.images = Images::new(Some(Support::fixed(Picker::halfblocks())));
        let loaded = |r: &mut Reader, key: &str| {
            let deadline = Instant::now() + Duration::from_secs(5);
            while !r.images.sizes().contains_key(key) {
                assert!(Instant::now() < deadline, "{key} did not load");
                r.layout(80, 40);
                r.tick();
                std::thread::sleep(Duration::from_millis(10));
            }
        };
        loaded(&mut r, "shown.png");
        assert!(!r.images.sizes().contains_key("hidden.png"));
        press(&mut r, KeyCode::Char('d'));
        press(&mut r, KeyCode::Enter);
        loaded(&mut r, "hidden.png");
    }

    #[test]
    fn without_details_d_says_so() {
        let mut r = Reader::new("t".into(), parse("text\n"), Theme::dark(), 80);
        r.layout(80, 40);
        press(&mut r, KeyCode::Char('d'));
        assert_eq!(r.message.as_deref(), Some("no details blocks"));
    }
}
