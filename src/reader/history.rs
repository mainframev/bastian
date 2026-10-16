use std::path::{Path, PathBuf};

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::{Action, Overlay, Reader};
use crate::nav;

/// The list of documents opened this session, shown with `H`.
pub struct HistoryPicker {
    /// Most recent first.
    pub paths: Vec<PathBuf>,
    pub selected: nav::Selection,
    /// Where this document is in `paths`.
    pub current: Option<usize>,
}

impl Reader {
    /// The file the document was read from, if any.
    pub fn file(&self) -> Option<&Path> {
        self.source.as_deref()
    }

    /// Show `paths`, most recent first and as they were opened, with this
    /// document marked and the one read before it selected.
    pub fn show_history(&mut self, paths: Vec<PathBuf>) {
        if paths.is_empty() {
            self.message = Some("no documents opened yet".into());
            return;
        }
        let here = self.source.as_deref().map(canonical);
        let current = here.and_then(|h| paths.iter().position(|p| canonical(p) == h));
        let selected = match current {
            Some(0) if paths.len() > 1 => 1,
            _ => 0,
        };
        self.overlay = Some(Overlay::History(HistoryPicker {
            paths,
            selected: nav::Selection::at(selected),
            current,
        }));
    }

    pub(super) fn on_history_key(&mut self, key: KeyEvent) -> Action {
        let Some(Overlay::History(picker)) = &mut self.overlay else {
            return Action::None;
        };
        if let Some(motion) = nav::list_motion(&key) {
            picker.selected.apply(motion, picker.paths.len());
            return Action::None;
        }
        match key.code {
            KeyCode::Enter => {
                let path = picker.paths[picker.selected.get()].clone();
                let here = picker.current == Some(picker.selected.get());
                self.overlay = None;
                if !here {
                    return Action::OpenDocument { path, anchor: None };
                }
            }
            KeyCode::Esc | KeyCode::Char('q' | 'H') => self.overlay = None,
            _ => {}
        }
        Action::None
    }
}

/// `path` made absolute and resolved, so one file is one history entry
/// however it was reached.
pub fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::*;
    use crate::doc::parse;
    use crate::reader::{Options, Source};

    fn press(r: &mut Reader, code: KeyCode) -> Action {
        r.on_key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn reader() -> Reader {
        Reader::new(Source::in_memory("t", parse("# A\n")), &Options::default())
    }

    #[test]
    fn h_asks_for_the_history_and_enter_opens_the_selection() {
        let mut r = reader();
        assert!(matches!(
            press(&mut r, KeyCode::Char('H')),
            Action::ShowHistory
        ));
        r.show_history(vec![PathBuf::from("/a.md"), PathBuf::from("/b.md")]);
        press(&mut r, KeyCode::Char('j'));
        match press(&mut r, KeyCode::Enter) {
            Action::OpenDocument { path, anchor: None } => {
                assert_eq!(path, PathBuf::from("/b.md"));
            }
            _ => panic!("expected OpenDocument"),
        }
        assert!(r.overlay.is_none());
    }

    #[test]
    fn esc_closes_and_an_empty_history_says_so() {
        let mut r = reader();
        r.show_history(Vec::new());
        assert!(r.overlay.is_none());
        assert_eq!(r.message(), Some("no documents opened yet"));
        r.show_history(vec![PathBuf::from("/a.md")]);
        press(&mut r, KeyCode::Esc);
        assert!(r.overlay.is_none());
    }

    #[test]
    fn the_history_blocks_clicks_on_the_table_of_contents() {
        let mut r = reader();
        press(&mut r, KeyCode::Char('t'));
        let (_, wider) = crate::pane::markers(r.fit_toc(100));
        r.show_history(vec![PathBuf::from("/a.md")]);
        let width = r.toc_width();
        assert!(!r.on_click(wider, 0));
        assert_eq!(r.toc_width(), width);
    }
}
