use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;

/// Whether ctrl is held.
pub fn ctrl(key: &KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL)
}

/// Single-line text input with a cursor (search and filter), backed by
/// `tui_input`: ←/→, Home/End, ctrl-a/e, alt-b/f and word deletion work
/// as in a shell.
pub struct Prompt {
    input: Input,
}

#[derive(Debug, PartialEq)]
pub enum PromptEvent {
    None,
    Changed,
    Submit,
    Cancel,
}

impl Prompt {
    /// Start editing `text`, with the cursor at its end.
    pub fn new(text: &str) -> Self {
        Self {
            input: Input::new(text.to_string()),
        }
    }

    pub fn text(&self) -> &str {
        self.input.value()
    }

    /// Cursor position, in characters.
    pub fn cursor(&self) -> usize {
        self.input.cursor()
    }

    pub fn on_key(&mut self, key: KeyEvent) -> PromptEvent {
        match key.code {
            KeyCode::Enter => PromptEvent::Submit,
            KeyCode::Esc => PromptEvent::Cancel,
            _ => match self.input.handle_event(&Event::Key(key)) {
                Some(changed) if changed.value => PromptEvent::Changed,
                _ => PromptEvent::None,
            },
        }
    }
}

/// What came of a key typed into an open prompt.
#[derive(Debug, PartialEq)]
pub enum Edited {
    /// The text changed to this.
    Text(String),
    /// Esc: the prompt closed and its text should be dropped.
    Cancelled,
}

/// Feed `key` to the prompt in `slot`, closing it on Enter or Esc.
pub fn drive(slot: &mut Option<Prompt>, key: KeyEvent) -> Option<Edited> {
    let prompt = slot.as_mut()?;
    match prompt.on_key(key) {
        PromptEvent::Changed => Some(Edited::Text(prompt.text().to_string())),
        PromptEvent::Submit => {
            *slot = None;
            None
        }
        PromptEvent::Cancel => {
            *slot = None;
            Some(Edited::Cancelled)
        }
        PromptEvent::None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(p: &mut Prompt, code: KeyCode, mods: KeyModifiers) -> PromptEvent {
        p.on_key(KeyEvent::new(code, mods))
    }

    #[test]
    fn edits_at_the_cursor() {
        let mut p = Prompt::new("helo");
        assert_eq!(p.cursor(), 4);
        assert_eq!(
            press(&mut p, KeyCode::Left, KeyModifiers::NONE),
            PromptEvent::None
        );
        assert_eq!(
            press(&mut p, KeyCode::Char('l'), KeyModifiers::NONE),
            PromptEvent::Changed
        );
        assert_eq!((p.text(), p.cursor()), ("hello", 4));
        press(&mut p, KeyCode::Home, KeyModifiers::NONE);
        press(&mut p, KeyCode::Delete, KeyModifiers::NONE);
        assert_eq!(p.text(), "ello");
        press(&mut p, KeyCode::End, KeyModifiers::NONE);
        press(&mut p, KeyCode::Char(' '), KeyModifiers::NONE);
        press(&mut p, KeyCode::Char('W'), KeyModifiers::SHIFT);
        assert_eq!(p.text(), "ello W");
        press(&mut p, KeyCode::Char('w'), KeyModifiers::CONTROL);
        assert_eq!(p.text(), "ello ");
        press(&mut p, KeyCode::Char('u'), KeyModifiers::CONTROL);
        assert_eq!(p.text(), "");
        assert_eq!(
            press(&mut p, KeyCode::Enter, KeyModifiers::NONE),
            PromptEvent::Submit
        );
        assert_eq!(
            press(&mut p, KeyCode::Esc, KeyModifiers::NONE),
            PromptEvent::Cancel
        );
    }
}
