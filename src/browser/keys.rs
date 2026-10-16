use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::{Action, Browser};
use crate::nav;
use crate::prompt::{self, Edited, Prompt};

impl Browser {
    pub fn on_key(&mut self, key: KeyEvent) -> Action {
        self.message = None;
        // Navigation keys end filter input and act right away, so moving
        // through the results doesn't need an Enter first.
        if self.prompt.is_some() && is_navigation(&key) {
            self.prompt = None;
        }
        if self.prompt.is_some() {
            match prompt::drive(&mut self.prompt, key) {
                Some(Edited::Text(text)) => self.filter = text,
                Some(Edited::Cancelled) => self.filter.clear(),
                None => return Action::None,
            }
            self.refilter();
            return Action::None;
        }

        if self.help.open {
            self.help.on_key(&key);
            return Action::None;
        }

        if let Some(motion) = nav::list_motion(&key) {
            self.selected.apply(motion, self.visible.len());
            return Action::None;
        }
        let page = self.height as isize;
        let ctrl = nav::ctrl(&key);
        match key.code {
            KeyCode::Char('d') if ctrl => self.move_by(page / 2),
            KeyCode::Char('u') if ctrl => self.move_by(-page / 2),
            KeyCode::Char('f') if ctrl => self.move_by(page),
            KeyCode::Char('b') if ctrl => self.move_by(-page),
            KeyCode::Esc if !self.filter.is_empty() => {
                self.filter.clear();
                self.refilter();
            }
            KeyCode::Char('q') | KeyCode::Esc => return Action::Quit,
            KeyCode::Char('/') => self.prompt = Some(Prompt::new(&self.filter)),
            KeyCode::Char('l') | KeyCode::Right | KeyCode::PageDown => self.move_by(page),
            KeyCode::Char('h') | KeyCode::Left | KeyCode::PageUp => self.move_by(-page),
            KeyCode::Enter => {
                if let Some(e) = self.selected_entry() {
                    return Action::Open(e.path.clone());
                }
            }
            KeyCode::Char('e') => {
                if let Some(e) = self.selected_entry() {
                    return Action::Edit(e.path.clone());
                }
            }
            KeyCode::Char('r') => self.rescan(),
            KeyCode::Char('?') => self.help.show(),
            KeyCode::Char('T') => return Action::PickTheme,
            _ => {}
        }
        Action::None
    }
}

/// Keys that move the selection rather than edit filter text. Plain
/// letters (even `j`/`k`) stay text, and ctrl-u keeps clearing the input.
fn is_navigation(key: &KeyEvent) -> bool {
    let ctrl = nav::ctrl(key);
    match key.code {
        // ←/→ move the cursor in the filter text instead.
        KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown => true,
        KeyCode::Char('j' | 'k' | 'n' | 'p' | 'd' | 'f' | 'b') => ctrl,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::Scan;
    use crate::browser::tests::wait_for_scan;
    use ratatui::crossterm::event::KeyModifiers;

    #[test]
    fn ctrl_paging_moves_selection() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..40 {
            std::fs::write(dir.path().join(format!("f{i}.md")), "x\n").unwrap();
        }
        let mut b = Browser::new(dir.path().to_path_buf(), Scan::default());
        wait_for_scan(&mut b);
        b.layout(10);
        let ctrl = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL);
        b.on_key(ctrl('d'));
        assert_eq!(b.selected.get(), 5);
        b.on_key(ctrl('f'));
        assert_eq!(b.selected.get(), 15);
        b.on_key(ctrl('u'));
        assert_eq!(b.selected.get(), 10);
        b.on_key(ctrl('b'));
        assert_eq!(b.selected.get(), 0);
        // Clamped at both ends.
        b.on_key(ctrl('b'));
        assert_eq!(b.selected.get(), 0);
        for _ in 0..10 {
            b.on_key(ctrl('f'));
        }
        assert_eq!(b.selected.get(), 39);
    }

    #[test]
    fn ctrl_u_in_filter_clears_text_instead_of_paging() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..20 {
            std::fs::write(dir.path().join(format!("f{i}.md")), "x\n").unwrap();
        }
        let mut b = Browser::new(dir.path().to_path_buf(), Scan::default());
        wait_for_scan(&mut b);
        b.layout(10);
        b.selected = nav::Selection::at(8);
        b.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        b.on_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE));
        b.on_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
        assert_eq!(b.prompt.as_ref().unwrap().text(), "");
    }

    #[test]
    fn edit_rescan_and_help_keys() {
        let key = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE);
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.md"), "# A\n").unwrap();
        let mut b = Browser::new(dir.path().to_path_buf(), Scan::default());
        wait_for_scan(&mut b);
        assert!(matches!(b.on_key(key('e')), Action::Edit(p) if p == dir.path().join("a.md")));

        std::fs::write(dir.path().join("b.md"), "# B\n").unwrap();
        b.on_key(key('r'));
        wait_for_scan(&mut b);
        assert_eq!(b.entries.len(), 2);

        b.on_key(key('?'));
        assert!(b.help.open);
        // The key that closes help does nothing else.
        assert!(matches!(b.on_key(key('q')), Action::None));
        assert!(!b.help.open);
    }

    #[test]
    fn list_turns_whole_pages() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..25 {
            std::fs::write(dir.path().join(format!("f{i}.md")), "x\n").unwrap();
        }
        let mut b = Browser::new(dir.path().to_path_buf(), Scan::default());
        wait_for_scan(&mut b);
        b.layout(10);
        assert_eq!((b.offset(), b.page()), (0, (1, 3)));
        b.selected = nav::Selection::at(9);
        b.layout(10);
        assert_eq!((b.offset(), b.page()), (0, (1, 3)));
        // One step past the last row turns to the next page.
        b.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        b.layout(10);
        assert_eq!((b.offset(), b.page()), (10, (2, 3)));
        b.selected = nav::Selection::at(24);
        b.layout(10);
        assert_eq!((b.offset(), b.page()), (20, (3, 3)));

        // h/l and ←/→ turn pages, keeping the row within the page.
        b.selected = nav::Selection::at(12);
        let press = |b: &mut Browser, code| {
            b.on_key(KeyEvent::new(code, KeyModifiers::NONE));
            b.layout(10);
        };
        press(&mut b, KeyCode::Char('h'));
        assert_eq!((b.selected.get(), b.page()), (2, (1, 3)));
        press(&mut b, KeyCode::Char('l'));
        assert_eq!((b.selected.get(), b.page()), (12, (2, 3)));
        press(&mut b, KeyCode::Right);
        assert_eq!((b.selected.get(), b.page()), (22, (3, 3)));
        press(&mut b, KeyCode::Left);
        assert_eq!(b.page(), (2, 3));
    }

    #[test]
    fn navigation_keys_leave_filter_input() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..30 {
            std::fs::write(dir.path().join(format!("doc{i}.md")), "x\n").unwrap();
        }
        let mut b = Browser::new(dir.path().to_path_buf(), Scan::default());
        wait_for_scan(&mut b);
        b.layout(10);
        let key = |c, m| KeyEvent::new(c, m);
        b.on_key(key(KeyCode::Char('/'), KeyModifiers::NONE));
        for c in "doc".chars() {
            b.on_key(key(KeyCode::Char(c), KeyModifiers::NONE));
        }
        // Plain j/k are still filter text.
        b.on_key(key(KeyCode::Char('j'), KeyModifiers::NONE));
        assert_eq!(b.prompt.as_ref().unwrap().text(), "docj");
        b.on_key(key(KeyCode::Backspace, KeyModifiers::NONE));

        // ctrl-j closes the input, keeps the filter and moves.
        b.on_key(key(KeyCode::Char('j'), KeyModifiers::CONTROL));
        assert!(b.prompt.is_none());
        assert_eq!(b.filter, "doc");
        assert_eq!(b.selected.get(), 1);

        // ←/→ move the cursor and keep the input open.
        b.on_key(key(KeyCode::Char('/'), KeyModifiers::NONE));
        b.on_key(key(KeyCode::Left, KeyModifiers::NONE));
        b.on_key(key(KeyCode::Char('_'), KeyModifiers::NONE));
        assert_eq!(b.prompt.as_ref().unwrap().text(), "do_c");
        b.on_key(key(KeyCode::Backspace, KeyModifiers::NONE));

        // Paging straight from the input closes it, like ctrl-j. (Editing
        // the filter put the selection back at the top.)
        b.on_key(key(KeyCode::PageDown, KeyModifiers::NONE));
        assert!(b.prompt.is_none());
        assert_eq!(b.selected.get(), 10);

        // ctrl-u still clears the text rather than paging.
        b.on_key(key(KeyCode::Char('/'), KeyModifiers::NONE));
        b.on_key(key(KeyCode::Char('u'), KeyModifiers::CONTROL));
        assert_eq!(b.prompt.as_ref().unwrap().text(), "");
    }
}
