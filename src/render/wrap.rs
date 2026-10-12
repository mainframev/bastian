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

/// Words of `s` as `wrap` sees them: runs of non-whitespace, with CJK text
/// split further so lines can break between its characters.
pub fn words(s: &str) -> impl Iterator<Item = &str> {
    tokens(s).filter(|t| !t.starts_with(char::is_whitespace))
}

/// Alternating runs of whitespace and non-whitespace, the latter split
/// wherever a CJK line break is allowed.
fn tokens(s: &str) -> impl Iterator<Item = &str> {
    let mut rest = s;
    std::iter::from_fn(move || {
        let first = rest.chars().next()?;
        let ws = first.is_whitespace();
        let mut prev = first;
        let end = rest
            .char_indices()
            .skip(1)
            .find(|&(_, c)| {
                let split = c.is_whitespace() != ws || (!ws && cjk_break(prev, c));
                prev = c;
                split
            })
            .map_or(rest.len(), |(i, _)| i);
        let (tok, tail) = rest.split_at(end);
        rest = tail;
        Some(tok)
    })
}

/// Whether a line may break between `prev` and `next` inside a word: next
/// to a Chinese or Japanese character, but not before closing punctuation
/// or after opening punctuation. Korean is left alone, as it spaces words.
fn cjk_break(prev: char, next: char) -> bool {
    (is_cjk(prev) || is_cjk(next)) && !no_break_before(next) && !no_break_after(prev)
}

fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{3000}'..='\u{303F}'   // CJK symbols and punctuation
        | '\u{3040}'..='\u{30FF}' // Hiragana, Katakana
        | '\u{31F0}'..='\u{31FF}' // Katakana phonetic extensions
        | '\u{3400}'..='\u{4DBF}' // CJK extension A
        | '\u{4E00}'..='\u{9FFF}' // CJK unified ideographs
        | '\u{F900}'..='\u{FAFF}' // CJK compatibility ideographs
        | '\u{FF00}'..='\u{FF60}' // Fullwidth forms
        | '\u{FFE0}'..='\u{FFE6}'
        | '\u{20000}'..='\u{3FFFF}' // CJK extensions B and later
    )
}

/// Characters a line must not start with.
fn no_break_before(c: char) -> bool {
    "、。，．：；！？）］｝」』】〕〉》〙〗〟’”ー々ゝゞヽヾぁぃぅぇぉっゃゅょゎゕゖァィゥェォッャュョヮヵヶ・…‥,.:;!?)]}%"
        .contains(c)
}

/// Characters a line must not end with.
fn no_break_after(c: char) -> bool {
    "（［｛「『【〔〈《〘〖〝‘“([{".contains(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(lines: &[Line]) -> Vec<String> {
        lines.iter().map(crate::render::plain).collect()
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
    fn breaks_between_cjk_characters() {
        let lines = wrap(&run("日本語の文章を折り返す"), 8);
        assert_eq!(text(&lines), ["日本語の", "文章を折", "り返す"]);
    }

    #[test]
    fn cjk_punctuation_stays_attached() {
        // "。" can't start a line and "「" can't end one.
        let lines = wrap(&run("これは「本」です。"), 8);
        assert_eq!(text(&lines), ["これは", "「本」で", "す。"]);
        let lines = wrap(&run("中文中文。"), 8);
        assert_eq!(text(&lines), ["中文中", "文。"]);
    }

    #[test]
    fn cjk_fills_the_line_before_breaking() {
        // A short CJK run continues the line instead of moving down whole.
        let lines = wrap(&run("ab 中文中文"), 8);
        assert_eq!(text(&lines), ["ab 中文", "中文"]);
    }

    #[test]
    fn latin_words_inside_cjk_stay_whole() {
        let lines = wrap(&run("这是一个很长的Rust句子"), 16);
        assert_eq!(text(&lines), ["这是一个很长的", "Rust句子"]);
    }

    #[test]
    fn cjk_mixed_with_latin() {
        let lines = wrap(&run("使用Rust编写的工具"), 10);
        assert_eq!(text(&lines), ["使用Rust编", "写的工具"]);
        assert_eq!(
            words("用Rust写, ok 한국어").collect::<Vec<_>>(),
            ["用", "Rust", "写,", "ok", "한국어"]
        );
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
