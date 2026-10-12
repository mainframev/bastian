use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Word-wrap styled text into lines of at most `width` columns.
/// `\n` forces a break; words longer than a line are split.
pub fn wrap(runs: &[(String, Style)], width: usize) -> Vec<Line<'static>> {
    let width = width.max(1);
    let mut w = Wrapper::new(width);
    let mut space: Option<Style> = None;

    for (text, style) in runs {
        for (i, segment) in text.split('\n').enumerate() {
            if i > 0 {
                w.newline();
                space = None;
            }
            for token in tokens(segment) {
                if token.starts_with(char::is_whitespace) {
                    space = Some(*style);
                    continue;
                }
                let tw = token.width();
                let gap = usize::from(space.is_some() && w.cur_w > 0);
                if w.cur_w > 0 && w.cur_w + gap + tw > width {
                    w.newline();
                } else if gap == 1 {
                    w.push(" ".to_string(), space.unwrap_or_default(), 1);
                }
                space = None;
                if tw > width {
                    w.push_split(token, *style);
                } else {
                    w.push(token.to_string(), *style, tw);
                }
            }
        }
    }
    w.finish()
}

/// Split spans at `width` columns without regard for words. Used for code.
pub fn hard_wrap(spans: &[Span], width: usize) -> Vec<Vec<Span<'static>>> {
    let mut w = Wrapper::new(width);
    for span in spans {
        w.push_split(&span.content, span.style);
    }
    w.finish().into_iter().map(|l| l.spans).collect()
}

struct Wrapper {
    width: usize,
    lines: Vec<Line<'static>>,
    cur: Vec<Span<'static>>,
    cur_w: usize,
}

impl Wrapper {
    fn new(width: usize) -> Self {
        Self {
            width: width.max(1),
            lines: Vec::new(),
            cur: Vec::new(),
            cur_w: 0,
        }
    }

    /// Close the last line; always yields at least one.
    fn finish(mut self) -> Vec<Line<'static>> {
        if !self.cur.is_empty() || self.lines.is_empty() {
            self.newline();
        }
        self.lines
    }

    fn newline(&mut self) {
        self.lines.push(Line::from(std::mem::take(&mut self.cur)));
        self.cur_w = 0;
    }

    fn push(&mut self, text: String, style: Style, w: usize) {
        self.cur.push(Span::styled(text, style));
        self.cur_w += w;
    }

    fn push_split(&mut self, text: &str, style: Style) {
        let mut chunk = String::new();
        let mut chunk_w = 0;
        for c in text.chars() {
            let cw = c.width().unwrap_or(0);
            if self.cur_w + chunk_w + cw > self.width && self.cur_w + chunk_w > 0 {
                if !chunk.is_empty() {
                    self.push(std::mem::take(&mut chunk), style, chunk_w);
                    chunk_w = 0;
                }
                self.newline();
            }
            chunk.push(c);
            chunk_w += cw;
        }
        if !chunk.is_empty() {
            self.push(chunk, style, chunk_w);
        }
    }
}

/// Alternating runs of whitespace and non-whitespace.
fn tokens(s: &str) -> impl Iterator<Item = &str> {
    let mut rest = s;
    std::iter::from_fn(move || {
        let first = rest.chars().next()?;
        let ws = first.is_whitespace();
        let end = rest
            .char_indices()
            .find(|(_, c)| c.is_whitespace() != ws)
            .map_or(rest.len(), |(i, _)| i);
        let (tok, tail) = rest.split_at(end);
        rest = tail;
        Some(tok)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(lines: &[Line]) -> Vec<String> {
        lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    fn run(s: &str) -> Vec<(String, Style)> {
        vec![(s.to_string(), Style::default())]
    }

    #[test]
    fn wraps_on_words() {
        let lines = wrap(&run("the quick brown fox jumps"), 10);
        assert_eq!(text(&lines), ["the quick", "brown fox", "jumps"]);
    }

    #[test]
    fn splits_long_words() {
        let lines = wrap(&run("abcdefghijkl"), 5);
        assert_eq!(text(&lines), ["abcde", "fghij", "kl"]);
    }

    #[test]
    fn hard_breaks() {
        let lines = wrap(&run("a\nb c"), 10);
        assert_eq!(text(&lines), ["a", "b c"]);
    }

    #[test]
    fn keeps_space_between_styled_runs() {
        let runs = vec![
            ("hello ".to_string(), Style::default()),
            ("world".to_string(), Style::default().bold()),
        ];
        assert_eq!(text(&wrap(&runs, 20)), ["hello world"]);
    }

    #[test]
    fn empty_input_is_one_empty_line() {
        assert_eq!(text(&wrap(&run(""), 10)), [""]);
    }

    #[test]
    fn hard_wrap_code() {
        let lines = hard_wrap(&[Span::raw("0123456789")], 4);
        let t: Vec<String> = lines
            .iter()
            .map(|l| l.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        assert_eq!(t, ["0123", "4567", "89"]);
    }
}
