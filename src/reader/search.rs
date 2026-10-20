use super::Reader;
use crate::render::Rendered;

impl Reader {
    pub(super) fn set_query(&mut self, query: &str) {
        if query.is_empty() {
            self.search = None;
            self.return_to_search_origin();
            return;
        }
        let matches = find(&self.rendered, query);
        // Start from the first hit at or below where the search started,
        // not where the last keystroke's match moved the view.
        let current = matches
            .iter()
            .position(|m| m.line >= self.search_origin)
            .unwrap_or(0);
        if let Some(m) = matches.get(current) {
            let line = m.line;
            self.scroll = self.search_origin;
            self.reveal(line);
        } else {
            self.return_to_search_origin();
        }
        self.search = Some(Search {
            query: query.to_string(),
            matches,
            current,
        });
    }

    /// Back to where the search prompt opened, as if it never moved.
    pub(super) fn return_to_search_origin(&mut self) {
        self.scroll = self.search_origin.min(self.max_scroll());
    }

    pub(super) fn step_match(&mut self, forward: bool) {
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
    let mut hay = Vec::new();
    for (i, line) in rendered.lines.iter().enumerate() {
        hay.clear();
        hay.extend(line.spans.iter().flat_map(|s| s.content.chars()).map(fold));
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
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use crate::doc::parse;
    use crate::reader::{Options, Reader, Source};
    use crate::render::theme::Theme;

    fn rendered(src: &str) -> Rendered {
        crate::render::render(&parse(src), 40, &Theme::dark())
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
    fn matches_in_inline_code_count_its_padding() {
        // "x  code  y": the code sits inside a space of padding.
        let r = rendered("x `code` y\n");
        let m = find(&r, "code");
        assert_eq!(
            m,
            [Match {
                line: 0,
                start: 3,
                end: 7
            }]
        );
        let text: String = crate::render::plain(&r.lines[0])
            .chars()
            .skip(3)
            .take(4)
            .collect();
        assert_eq!(text, "code");
    }

    #[test]
    fn search_jumps_and_cycles() {
        let src: String = (0..100).map(|i| format!("line {i}\n\n")).collect();
        let mut r = Reader::new(Source::in_memory("t", parse(&src)), &Options::default());
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

    fn press(r: &mut Reader, code: KeyCode) {
        r.on_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    /// 100 paragraphs, `line {i}` on row `2 * i`, at the top of a short view.
    fn lines() -> Reader {
        let src: String = (0..100).map(|i| format!("line {i}\n\n")).collect();
        let mut r = Reader::new(Source::in_memory("t", parse(&src)), &Options::default());
        r.layout(80, 10);
        r
    }

    fn find_typed(r: &mut Reader, text: &str) {
        press(r, KeyCode::Char('/'));
        for c in text.chars() {
            press(r, KeyCode::Char(c));
        }
    }

    fn current_line(r: &Reader) -> usize {
        let s = r.search.as_ref().expect("searching");
        s.matches[s.current].line
    }

    #[test]
    fn cancel_returns_to_where_the_search_started() {
        let mut r = lines();
        find_typed(&mut r, "line 50");
        assert!(r.scroll > 0);
        press(&mut r, KeyCode::Esc);
        assert_eq!(r.scroll, 0);
        assert!(r.search.is_none());
    }

    #[test]
    fn emptying_the_query_returns_to_where_the_search_started() {
        let mut r = lines();
        find_typed(&mut r, "line 50");
        for _ in 0.."line 50".len() {
            press(&mut r, KeyCode::Backspace);
        }
        assert_eq!(r.scroll, 0);
        assert!(r.search.is_none());
    }

    #[test]
    fn a_query_with_no_matches_returns_to_where_the_search_started() {
        let mut r = lines();
        find_typed(&mut r, "line 50x");
        assert_eq!(r.scroll, 0);
    }

    #[test]
    fn a_refined_query_matches_from_where_the_search_started() {
        let mut r = lines();
        find_typed(&mut r, "line 50");
        press(&mut r, KeyCode::Backspace);
        // "line 5", on row 10, not "line 50" where the view had moved.
        assert_eq!(current_line(&r), 10);
    }

    #[test]
    fn submit_keeps_the_view_on_the_match() {
        let mut r = lines();
        find_typed(&mut r, "line 50");
        press(&mut r, KeyCode::Enter);
        assert!((r.scroll..r.scroll + 10).contains(&current_line(&r)));
        assert!(r.scroll > 0);
    }
}
