use super::Reader;
use crate::render::Rendered;

impl Reader {
    pub(super) fn set_query(&mut self, query: &str) {
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
    use crate::doc::parse;
    use crate::reader::Reader;
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
