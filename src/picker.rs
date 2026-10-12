//! Theme picker: a list of themes previewed live as the selection moves.

use std::path::Path;

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::nav;
use crate::render::theme::Theme;
use crate::themes;

/// A document that shows off a theme, for `bastian themes`.
pub const SAMPLE: &str = r#"# Theme preview

Text with **bold**, *italic*, `inline code` and a [link](https://example.com).

## Code

```rust
/// Greet someone.
fn greet(name: &str) -> String {
    let count = 42;
    format!("hello {name}, {count} times")
}
```

```sh
for f in *.md; do echo "$f"; done
```

> A quote, with a list:
>
> - one
> - two

| Theme | Kind |
|:------|:----:|
| tokyonight | dark |
| rose-pine-dawn | light |
"#;

pub struct Entry {
    pub name: String,
    /// The loaded theme, or why it doesn't load.
    pub theme: Result<Theme, String>,
}

pub struct ThemePicker {
    pub entries: Vec<Entry>,
    pub selected: usize,
    /// Row of the theme in use when the picker opened.
    pub current: Option<usize>,
}

pub enum Event {
    None,
    /// Show this theme while the picker stays open.
    Preview(Theme),
    /// Use and save this theme.
    Pick {
        name: String,
        theme: Theme,
    },
    /// Closed without picking: go back to the theme from before.
    Cancel,
}

impl ThemePicker {
    /// Every theme from `dir` and the built-ins, starting on `current`.
    pub fn new(dir: Option<&Path>, current: &str) -> Self {
        let entries: Vec<Entry> = themes::list(dir)
            .into_iter()
            .map(|l| Entry {
                name: l.name,
                theme: l.theme.map_err(|e| format!("{e:#}")),
            })
            .collect();
        let current = entries
            .iter()
            .position(|e| e.name == themes::canonical(current, dir));
        Self {
            selected: current.unwrap_or(0),
            current,
            entries,
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) -> Event {
        if let Some(motion) = nav::list_motion(&key) {
            return self.select(nav::step(self.selected, self.entries.len(), motion));
        }
        match key.code {
            KeyCode::Enter => match &self.entries[self.selected] {
                Entry {
                    name,
                    theme: Ok(theme),
                } => Event::Pick {
                    name: name.clone(),
                    theme: theme.clone(),
                },
                Entry { theme: Err(_), .. } => Event::None,
            },
            KeyCode::Esc | KeyCode::Char('q' | 'T') => Event::Cancel,
            _ => Event::None,
        }
    }

    /// Move the selection to `to`, previewing the theme there.
    fn select(&mut self, to: usize) -> Event {
        if to == self.selected {
            return Event::None;
        }
        self.selected = to;
        match &self.entries[to].theme {
            Ok(theme) => Event::Preview(theme.clone()),
            Err(_) => Event::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyModifiers;

    fn key(p: &mut ThemePicker, code: KeyCode) -> Event {
        p.on_key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn a_user_theme_named_like_an_alias_is_current() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("tokyonight.toml");
        std::fs::write(file, "extends = \"nord\"\n").unwrap();
        let p = ThemePicker::new(Some(dir.path()), "tokyonight");
        let current = p.current.map(|i| p.entries[i].name.as_str());
        assert_eq!(current, Some("tokyonight"));
    }

    #[test]
    fn moves_previews_and_picks() {
        let mut p = ThemePicker::new(None, "kanagawa");
        let start = p.entries.iter().position(|e| e.name == "kanagawa-wave");
        assert_eq!((p.current, p.selected), (start, start.unwrap()));

        let Event::Preview(t) = key(&mut p, KeyCode::Char('j')) else {
            panic!("expected a preview");
        };
        assert_eq!(p.entries[p.selected].name, "kanagawa-dragon");
        assert_eq!(
            t.accent,
            themes::load("kanagawa-dragon", None).unwrap().accent
        );

        key(&mut p, KeyCode::Char('g'));
        assert_eq!(p.selected, 0);
        assert!(matches!(key(&mut p, KeyCode::Up), Event::None));
        match key(&mut p, KeyCode::Enter) {
            Event::Pick { name, .. } => assert_eq!(name, p.entries[0].name),
            _ => panic!("expected a pick"),
        }
        assert!(matches!(key(&mut p, KeyCode::Esc), Event::Cancel));
    }

    #[test]
    fn sample_parses_with_code_blocks() {
        let doc = crate::doc::parse(SAMPLE);
        let codes = doc
            .blocks
            .iter()
            .filter(|b| matches!(b, crate::doc::Block::Code(_)))
            .count();
        assert_eq!(codes, 2);
    }
}
