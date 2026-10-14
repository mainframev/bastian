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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyModifiers;

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
