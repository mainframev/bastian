use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Whether ctrl is held.
pub fn ctrl(key: &KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL)
}

/// Single-line text input shown in the status bar.
pub struct Prompt {
    pub text: String,
}

#[derive(Debug, PartialEq)]
pub enum PromptEvent {
    None,
    Changed,
    Submit,
    Cancel,
}

impl Prompt {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) -> PromptEvent {
        let ctrl = ctrl(&key);
        match key.code {
            KeyCode::Enter => PromptEvent::Submit,
            KeyCode::Esc => PromptEvent::Cancel,
            KeyCode::Char('u') if ctrl => {
                self.text.clear();
                PromptEvent::Changed
            }
            KeyCode::Char('w') if ctrl => {
                let trimmed = self.text.trim_end();
                let cut = trimmed.rfind(char::is_whitespace).map_or(0, |i| i + 1);
                self.text.truncate(cut);
                PromptEvent::Changed
            }
            KeyCode::Backspace => {
                self.text.pop();
                PromptEvent::Changed
            }
            KeyCode::Char(c) if !ctrl => {
                self.text.push(c);
                PromptEvent::Changed
            }
            _ => PromptEvent::None,
        }
    }
}
