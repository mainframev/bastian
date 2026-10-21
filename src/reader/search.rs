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
        let first = matches
            .iter()
            .find(|m| m.line >= self.search_origin)
            .or(matches.first());
        let current = first.map_or(0, |m| m.hit);
        if let Some(m) = first {
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
        let n = s.hits();
        if n == 0 {
            return;
        }
        s.current = if forward {
            (s.current + 1) % n
        } else {
            (s.current + n - 1) % n
        };
        let line = s.current_line();
        self.reveal(line);
    }
}

/// Part of a search hit in rendered text, in char offsets within the line.
/// A hit that a soft wrap splits has a part on each line, with the same
/// `hit`.
#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    pub line: usize,
    pub start: usize,
    pub end: usize,
    /// Which hit this is part of, counting from 0.
    pub hit: usize,
}

#[derive(Debug, Default)]
pub struct Search {
    pub query: String,
    /// The parts of every hit, in line order.
    pub matches: Vec<Match>,
    /// The current hit.
    pub current: usize,
}

impl Search {
    /// How many hits there are.
    pub fn hits(&self) -> usize {
        self.matches.last().map_or(0, |m| m.hit + 1)
    }

    /// The line where the current hit starts.
    fn current_line(&self) -> usize {
        let i = self.matches.partition_point(|m| m.hit < self.current);
        self.matches[i].line
    }
}

/// Find `query` in the rendered lines. Smart case: a lowercase query matches
/// case-insensitively. A paragraph wrapped over several lines is searched
/// as one, so a hit can go on past the end of a line.
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

    let mut out: Vec<Match> = Vec::new();
    let mut hits = 0;
    // The text of a run of joined lines, each char with the line and
    // column it's drawn at: `None` for the space a soft break took.
    let mut hay: Vec<(char, Option<(usize, usize)>)> = Vec::new();
    let mut joins = rendered.joins.iter().peekable();
    let lines = &rendered.lines;
    let mut i = 0;
    while i < lines.len() {
        hay.clear();
        let push = |line: usize, skip: usize, hay: &mut Vec<_>| {
            let chars = lines[line].spans.iter().flat_map(|s| s.content.chars());
            hay.extend(
                chars
                    .enumerate()
                    .skip(skip)
                    .map(|(col, c)| (fold(c), Some((line, col)))),
            );
        };
        push(i, 0, &mut hay);
        let mut next = i + 1;
        while let Some(join) = joins.next_if(|j| j.line <= next) {
            if join.line < next {
                continue;
            }
            // Decoration at the end of the line isn't part of the text;
            // spaces that are (an inline code chip's padding) stay.
            hay.truncate(hay.len().saturating_sub(join.pad));
            if join.space {
                hay.push((' ', None));
            }
            push(next, join.skip, &mut hay);
            next += 1;
        }

        let mut start = 0;
        while start + needle.len() <= hay.len() {
            let found = &hay[start..start + needle.len()];
            if !found.iter().map(|h| h.0).eq(needle.iter().copied()) {
                start += 1;
                continue;
            }
            // One part per line the hit is drawn on.
            let before = out.len();
            for (line, col) in found.iter().filter_map(|h| h.1) {
                if out.len() > before
                    && let Some(m) = out.last_mut()
                    && m.line == line
                {
                    m.end = col + 1;
                } else {
                    out.push(Match {
                        line,
                        start: col,
                        end: col + 1,
                        hit: hits,
                    });
                }
            }
            // A query of just a space can land on a break, drawn nowhere.
            if out.len() > before {
                hits += 1;
            }
            start += needle.len();
        }
        i = next;
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
                end: 9,
                hit: 0
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
                end: 7,
                hit: 0
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
        let n = r.search.as_ref().unwrap().hits();
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
        r.search.as_ref().expect("searching").current_line()
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

    #[test]
    fn where_the_search_started_survives_a_new_width() {
        let para = "words that wrap ".repeat(12);
        let src: String = (0..40).map(|i| format!("{i} {para}\n\n")).collect();
        let mut r = Reader::new(Source::in_memory("t", parse(&src)), &Options::default());
        r.layout(80, 10);
        r.scroll = r.rendered.block_starts[10];
        find_typed(&mut r, "39 words");
        r.layout(40, 10);
        press(&mut r, KeyCode::Esc);
        assert_eq!(r.scroll, r.rendered.block_starts[10]);
    }

    /// Each hit's text as drawn, its parts on different lines joined by `|`.
    fn hits(r: &Rendered, query: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for m in find(r, query) {
            let line = crate::render::plain(&r.lines[m.line]);
            let part: String = line.chars().skip(m.start).take(m.end - m.start).collect();
            if m.hit < out.len() {
                out[m.hit] = format!("{}|{part}", out[m.hit]);
            } else {
                out.push(part);
            }
        }
        out
    }

    const LONG: &str = "The quick brown fox jumps over the lazy dog and keeps running";

    #[test]
    fn a_hit_goes_on_past_a_soft_wrap() {
        // At 40 columns "lazy" ends the first line.
        let r = rendered(&format!("{LONG}\n"));
        assert_eq!(hits(&r, "lazy dog and keeps"), ["lazy|dog and keeps"]);
        let parts = find(&r, "lazy dog");
        assert_eq!(parts.len(), 2);
        assert_eq!((parts[0].line, parts[1].line), (0, 1));
        assert!(parts.iter().all(|m| m.hit == 0));
    }

    #[test]
    fn a_hit_goes_on_past_a_quote_bar_and_a_list_indent() {
        let r = rendered(&format!("> {LONG}\n\n- {LONG}\n\n1. {LONG}\n"));
        assert_eq!(
            hits(&r, "keeps running").len(),
            3,
            "{:?}",
            hits(&r, "keeps running")
        );
        let found = hits(&r, "brown fox jumps over the lazy dog and");
        assert_eq!(found.len(), 3, "{found:?}");
        for h in found {
            assert!(h.contains('|'), "{h}");
            assert_eq!(h.replace('|', " "), "brown fox jumps over the lazy dog and");
        }
    }

    #[test]
    fn a_hit_goes_on_past_a_wrapped_heading() {
        let r = rendered(&format!("# {LONG}\n\n### {LONG}\n"));
        assert_eq!(hits(&r, "over the lazy dog and keeps").len(), 2);
    }

    #[test]
    fn a_word_split_by_the_wrap_is_still_found() {
        let word = "x".repeat(30) + "needle" + &"y".repeat(30);
        let r = rendered(&format!("{word}\n"));
        // The line breaks inside the run of y's, 40 chars in.
        let query = format!("needle{}", "y".repeat(10));
        assert_eq!(
            hits(&r, &query),
            [format!("needle{}|{}", "y".repeat(4), "y".repeat(6))]
        );
    }

    #[test]
    fn separate_blocks_and_hard_breaks_are_not_joined() {
        let r = rendered("one\n\ntwo\n\n- three\n- four\n\nfive  \nsix\n");
        for query in ["one two", "three four", "five six"] {
            let found = find(&r, query);
            assert!(found.is_empty(), "{query:?}: {found:?}");
        }
    }

    #[test]
    fn a_space_query_never_lands_only_on_a_break() {
        let r = rendered(&format!("{LONG}\n"));
        let n = hits(&r, " ").len();
        assert_eq!(
            n,
            LONG.matches(' ').count() - 1,
            "the break's space is drawn nowhere"
        );
        assert!(hits(&r, " ").iter().all(|h| h == " "));
    }

    #[test]
    fn stepping_counts_a_split_hit_once() {
        let src = format!("{LONG}\n\n{LONG}\n");
        let mut r = Reader::new(Source::in_memory("t", parse(&src)), &Options::default());
        r.layout(40, 10);
        // Long enough to cross the break wherever the reader's width puts it.
        r.set_query("over the lazy dog and keeps");
        let s = r.search.as_ref().unwrap();
        assert_eq!((s.hits(), s.matches.len()), (2, 4));
        r.step_match(true);
        assert_eq!(r.search.as_ref().unwrap().current, 1);
        r.step_match(true);
        assert_eq!(r.search.as_ref().unwrap().current, 0);
    }

    #[test]
    fn an_inline_code_chip_keeps_its_padding_at_a_wrap() {
        let doc = parse("Run `cargo build` now\n");
        for width in [17, 40] {
            let r = crate::render::render(&doc, width, &Theme::dark());
            assert_eq!(hits(&r, "cargo build  now").len(), 1, "width {width}");
        }
    }
}
