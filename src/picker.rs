//! Theme picker: a list of themes in a pane on the right, previewed live
//! as the selection moves.

use std::path::Path;

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::nav;
use crate::pane;
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

/// Columns the pane starts with: room for the longest built-in name and
/// its dark or light.
pub const DEFAULT_WIDTH: u16 = 34;

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
    /// Columns asked for the pane; `<` and `>` change it.
    pub width: u16,
    /// Width of the screen at the last draw, which the pane is sized
    /// against.
    screen: u16,
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
    /// Every theme from `dir` and the built-ins, starting on `current`,
    /// in a pane asked `width` columns wide.
    pub fn new(dir: Option<&Path>, current: &str, width: u16) -> Self {
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
            width,
            screen: 0,
        }
    }

    /// Columns the pane takes on a screen `screen` columns wide, as the
    /// table of contents would; all of them on a screen too narrow to
    /// keep the document beside it. Remembers the screen width, which `<`
    /// and `>` size the pane against.
    pub fn fit(&mut self, screen: u16) -> u16 {
        self.screen = screen;
        match pane::fit(self.width, screen) {
            0 => screen,
            width => width,
        }
    }

    /// A left click at `column`, `row` of the screen: the `<` and `>`
    /// markers on the pane's header resize it. Returns whether it hit one.
    pub fn on_click(&mut self, column: u16, row: u16) -> bool {
        let width = self.fit(self.screen);
        let Some(col) = column.checked_sub(self.screen - width) else {
            return false;
        };
        let (narrower, wider) = pane::markers(width);
        if width == 0 || row != 0 || (col != narrower && col != wider) {
            return false;
        }
        self.width = pane::resize(self.width, self.screen, col == wider);
        true
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
            KeyCode::Char(c @ ('<' | '>')) => {
                self.width = pane::resize(self.width, self.screen, c == '>');
                Event::None
            }
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
        let p = ThemePicker::new(Some(dir.path()), "tokyonight", DEFAULT_WIDTH);
        let current = p.current.map(|i| p.entries[i].name.as_str());
        assert_eq!(current, Some("tokyonight"));
    }

    #[test]
    fn moves_previews_and_picks() {
        let mut p = ThemePicker::new(None, "kanagawa", DEFAULT_WIDTH);
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
    fn angle_brackets_resize_the_pane_within_the_screen() {
        let mut p = ThemePicker::new(None, "nord", DEFAULT_WIDTH);
        assert_eq!(p.fit(100), DEFAULT_WIDTH);
        assert!(matches!(key(&mut p, KeyCode::Char('>')), Event::None));
        assert_eq!(p.width, DEFAULT_WIDTH + pane::STEP);
        for _ in 0..20 {
            key(&mut p, KeyCode::Char('>'));
        }
        assert_eq!(p.width, 50, "half of the 100-column screen");
        for _ in 0..20 {
            key(&mut p, KeyCode::Char('<'));
        }
        assert_eq!(p.width, pane::MIN_WIDTH);
        // Resizing moves nothing.
        assert_eq!(p.selected, p.current.unwrap());
    }

    #[test]
    fn clicks_on_the_markers_resize() {
        let mut p = ThemePicker::new(None, "nord", DEFAULT_WIDTH);
        let width = p.fit(100);
        let (narrower, wider) = pane::markers(width);
        let (narrower, wider) = (100 - width + narrower, 100 - width + wider);
        assert!(p.on_click(wider, 0));
        assert_eq!(p.width, width + pane::STEP);
        // The pane grows to the left: its markers stay at the screen's edge.
        p.fit(100);
        assert!(p.on_click(narrower, 0));
        assert_eq!(p.width, width);
        assert!(!p.on_click(narrower, 1), "only the header row");
        assert!(!p.on_click(narrower + 1, 0));
        assert!(!p.on_click(0, 0), "left of the pane");
        assert_eq!(p.width, width);
    }

    #[test]
    fn a_narrow_screen_gives_the_pane_all_of_it() {
        let mut p = ThemePicker::new(None, "nord", DEFAULT_WIDTH);
        assert_eq!(p.fit(60), 30);
        assert_eq!(p.fit(pane::MIN_SCREEN), 20);
        assert_eq!(p.fit(pane::MIN_SCREEN - 1), pane::MIN_SCREEN - 1);
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
