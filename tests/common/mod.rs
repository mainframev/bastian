//! Helpers shared by the interface test crates.

use bastian::reader::{Action, Overlay, Reader};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Press `code` with no modifiers.
pub fn key(r: &mut Reader, code: KeyCode) -> Action {
    r.on_key(KeyEvent::new(code, KeyModifiers::NONE))
}

/// The blocks a run confirmation is waiting on.
pub fn confirm(r: &Reader) -> Option<&[usize]> {
    match &r.overlay {
        Some(Overlay::RunConfirm(ids)) => Some(ids),
        _ => None,
    }
}
