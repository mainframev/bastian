use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::doc::Document;
use crate::prompt::{Prompt, PromptEvent};
use crate::render::theme::Theme;
use crate::render::{self, Rendered};

pub enum Action {
    None,
    /// Go back to the browser, or quit when there is none.
    Back,
    Quit,
}

/// A search hit in rendered text, in char offsets within the line.
#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    pub line: usize,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Default)]
pub struct Search {
    pub query: String,
    pub matches: Vec<Match>,
    pub current: usize,
}

/// State of the document reader view.
pub struct Reader {
    pub name: String,
    pub doc: Document,
    pub theme: Theme,
    pub max_width: u16,
    pub rendered: Rendered,
    pub scroll: usize,
    pub search: Option<Search>,
    pub prompt: Option<Prompt>,
    width: u16,
    height: usize,
}

impl Reader {
    pub fn new(name: String, doc: Document, theme: Theme, max_width: u16) -> Self {
        Self {
            name,
            doc,
            theme,
            max_width,
            rendered: Rendered::default(),
            scroll: 0,
            search: None,
            prompt: None,
            width: 0,
            height: 0,
        }
    }

    /// Re-render when the content width changes and keep scroll in range.
    pub fn layout(&mut self, width: u16, height: u16) {
        if width != self.width {
            self.rendered = render::render(&self.doc, width, &self.theme);
            self.width = width;
            if let Some(s) = &mut self.search {
                s.matches = find(&self.rendered, &s.query);
                s.current = s.current.min(s.matches.len().saturating_sub(1));
            }
        }
        self.height = usize::from(height);
        self.scroll = self.scroll.min(self.max_scroll());
    }

    pub fn max_scroll(&self) -> usize {
        self.rendered.lines.len().saturating_sub(self.height)
    }

    fn scroll_by(&mut self, delta: isize) {
        self.scroll = self
            .scroll
            .saturating_add_signed(delta)
            .min(self.max_scroll());
    }

    /// Scroll so `line` is visible, placing it a third of the way down.
    fn reveal(&mut self, line: usize) {
        let visible = self.scroll..self.scroll + self.height;
        if !visible.contains(&line) {
            self.scroll = line.saturating_sub(self.height / 3).min(self.max_scroll());
        }
    }

    fn set_query(&mut self, query: &str) {
        if query.is_empty() {
            self.search = None;
            return;
        }
        let matches = find(&self.rendered, query);
        // Start from the first hit at or below the current view.
        let current = matches
            .iter()
            .position(|m| m.line >= self.scroll)
            .unwrap_or(0);
        if let Some(m) = matches.get(current) {
            let line = m.line;
            self.reveal(line);
        }
        self.search = Some(Search {
            query: query.to_string(),
            matches,
            current,
        });
    }

    fn step_match(&mut self, forward: bool) {
        let Some(s) = &mut self.search else { return };
        let n = s.matches.len();
        if n == 0 {
            return;
        }
        s.current = if forward {
            (s.current + 1) % n
        } else {
            (s.current + n - 1) % n
        };
        let line = s.matches[s.current].line;
        self.reveal(line);
    }

    pub fn on_key(&mut self, key: KeyEvent) -> Action {
        if let Some(prompt) = &mut self.prompt {
            match prompt.on_key(key) {
                PromptEvent::Changed => {
                    let q = prompt.text.clone();
                    self.set_query(&q);
                }
                PromptEvent::Submit => self.prompt = None,
                PromptEvent::Cancel => {
                    self.prompt = None;
                    self.search = None;
                }
                PromptEvent::None => {}
            }
            return Action::None;
        }

        let page = self.height.max(1) as isize;
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('q') => return Action::Quit,
            KeyCode::Esc if self.search.is_some() => self.search = None,
            KeyCode::Esc | KeyCode::Char('h') | KeyCode::Left => return Action::Back,
            KeyCode::Char('d') if ctrl => self.scroll_by(page / 2),
            KeyCode::Char('u') if ctrl => self.scroll_by(-page / 2),
            KeyCode::Char('j') | KeyCode::Down => self.scroll_by(1),
            KeyCode::Char('k') | KeyCode::Up => self.scroll_by(-1),
            KeyCode::Char(' ' | 'f') | KeyCode::PageDown => self.scroll_by(page),
            KeyCode::Char('b') | KeyCode::PageUp => self.scroll_by(-page),
            KeyCode::Char('g') | KeyCode::Home => self.scroll = 0,
            KeyCode::Char('G') | KeyCode::End => self.scroll = self.max_scroll(),
            KeyCode::Char('/') => self.prompt = Some(Prompt::new("/", "")),
            KeyCode::Char('n') => self.step_match(true),
            KeyCode::Char('N') => self.step_match(false),
            _ => {}
        }
        Action::None
    }
}

/// Find `query` in the rendered lines. Smart case: a lowercase query matches
/// case-insensitively. Matches don't cross wrapped line boundaries.
pub fn find(rendered: &Rendered, query: &str) -> Vec<Match> {
    let insensitive = !query.chars().any(char::is_uppercase);
    let fold = |c: char| {
        if insensitive {
            c.to_lowercase().next().unwrap_or(c)
        } else {
            c
        }
    };
    let needle: Vec<char> = query.chars().map(fold).collect();
    if needle.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();
    for (i, line) in rendered.lines.iter().enumerate() {
        let hay: Vec<char> = line
            .spans
            .iter()
            .flat_map(|s| s.content.chars())
            .map(fold)
            .collect();
        let mut start = 0;
        while start + needle.len() <= hay.len() {
            if hay[start..start + needle.len()] == needle[..] {
                out.push(Match {
                    line: i,
                    start,
                    end: start + needle.len(),
                });
                start += needle.len();
            } else {
                start += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::parse;

    fn rendered(src: &str) -> Rendered {
        render::render(&parse(src), 40, &Theme::dark())
    }

    #[test]
    fn smart_case() {
        let r = rendered("Foo foo FOO\n");
        assert_eq!(find(&r, "foo").len(), 3);
        assert_eq!(find(&r, "Foo").len(), 1);
    }

    #[test]
    fn match_positions_are_chars() {
        let r = rendered("héllo wörld\n");
        assert_eq!(
            find(&r, "wör"),
            [Match {
                line: 0,
                start: 6,
                end: 9
            }]
        );
    }

    #[test]
    fn search_jumps_and_cycles() {
        let src: String = (0..100).map(|i| format!("line {i}\n\n")).collect();
        let mut r = Reader::new("t".into(), parse(&src), Theme::dark(), 80);
        r.layout(80, 10);
        r.set_query("line 50");
        assert!(
            r.scroll <= 100 && r.scroll + 10 > 100,
            "scroll {}",
            r.scroll
        );
        r.set_query("line 1");
        let n = r.search.as_ref().unwrap().matches.len();
        // "line 1" plus "line 10".."line 19"
        assert_eq!(n, 11);
        r.step_match(false);
        assert_eq!(r.search.as_ref().unwrap().current, n - 1);
    }
}
