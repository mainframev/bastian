//! Cursor movement shared by the selectable lists.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Whether ctrl is held.
pub fn ctrl(key: &KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL)
}

/// A move of a list's selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    Up,
    Down,
    Top,
    Bottom,
}

/// The keys every list understands: j/k ↓/↑ (ctrl-n/p too), g/G Home/End.
pub fn list_motion(key: &KeyEvent) -> Option<Motion> {
    let ctrl = ctrl(key);
    Some(match key.code {
        KeyCode::Char('j') | KeyCode::Down => Motion::Down,
        KeyCode::Char('n') if ctrl => Motion::Down,
        KeyCode::Char('k') | KeyCode::Up => Motion::Up,
        KeyCode::Char('p') if ctrl => Motion::Up,
        KeyCode::Char('g') | KeyCode::Home => Motion::Top,
        KeyCode::Char('G') | KeyCode::End => Motion::Bottom,
        _ => return None,
    })
}

/// The highlighted row of a list. It holds only the row: a list's length
/// changes as it's filtered or fills in, so each move takes the length the
/// list has now and keeps the row in range of it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Selection(usize);

impl Selection {
    pub fn at(row: usize) -> Self {
        Self(row)
    }

    pub fn get(self) -> usize {
        self.0
    }

    /// The selection after `motion` in a list of `len` rows.
    #[must_use]
    pub fn moved(self, motion: Motion, len: usize) -> Self {
        Self(step(self.0, len, motion))
    }

    pub fn apply(&mut self, motion: Motion, len: usize) {
        *self = self.moved(motion, len);
    }

    /// Move `delta` rows (a page, half a page), stopping at either end.
    pub fn by(&mut self, delta: isize, len: usize) {
        self.0 = self
            .0
            .saturating_add_signed(delta)
            .min(len.saturating_sub(1));
    }
}

/// `selected` after `motion` in a list of `len` rows, kept in range.
pub fn step(selected: usize, len: usize, motion: Motion) -> usize {
    let last = len.saturating_sub(1);
    match motion {
        Motion::Up => selected.saturating_sub(1).min(last),
        Motion::Down => (selected + 1).min(last),
        Motion::Top => 0,
        Motion::Bottom => last,
    }
}

/// The key help overlay: whether it's open and how far it's scrolled.
#[derive(Debug, Default)]
pub struct Help {
    pub open: bool,
    /// First row shown; drawing keeps it in range.
    pub scroll: usize,
}

impl Help {
    pub fn show(&mut self) {
        *self = Help {
            open: true,
            scroll: 0,
        };
    }

    /// List motions scroll the help; any other key closes it.
    pub fn on_key(&mut self, key: &KeyEvent) {
        match list_motion(key) {
            Some(motion) => self.scroll = step(self.scroll, usize::MAX, motion),
            None => self.open = false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyModifiers;

    #[test]
    fn selection_stays_in_range_of_the_current_length() {
        let mut s = Selection::at(2);
        s.apply(Motion::Down, 3);
        assert_eq!(s.get(), 2);
        s.by(-10, 3);
        assert_eq!(s.get(), 0);
        s.by(10, 3);
        assert_eq!(s.get(), 2);
        // The list shrank (a filter): the next move lands inside it.
        s.apply(Motion::Up, 1);
        assert_eq!(s.get(), 0);
        s.by(5, 0);
        assert_eq!(s.get(), 0);
    }

    #[test]
    fn steps_stay_in_range() {
        assert_eq!(step(0, 3, Motion::Up), 0);
        assert_eq!(step(2, 3, Motion::Down), 2);
        assert_eq!(step(1, 3, Motion::Bottom), 2);
        assert_eq!(step(0, 0, Motion::Down), 0);
    }

    #[test]
    fn ctrl_n_and_p_move_but_plain_n_does_not() {
        let key = |c, m| KeyEvent::new(KeyCode::Char(c), m);
        assert_eq!(
            list_motion(&key('n', KeyModifiers::CONTROL)),
            Some(Motion::Down)
        );
        assert_eq!(
            list_motion(&key('p', KeyModifiers::CONTROL)),
            Some(Motion::Up)
        );
        assert_eq!(list_motion(&key('n', KeyModifiers::NONE)), None);
    }
}
