use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Stands for a space that is part of the word next to it, like the
/// padding around inline code: it is never broken at or dropped, and comes
/// out as a plain space. A Unicode noncharacter, so documents don't hold it.
pub const PAD: char = '\u{FDD0}';

/// Word-wrap styled text into lines of at most `width` columns.
/// `\n` forces a break; words longer than a line are split. Words are found
/// across style changes, so a styled part of a word (or of CJK text with
/// its punctuation) stays with the rest.
pub fn wrap(runs: &[(String, Style)], width: usize) -> Vec<Line<'static>> {
    let width = width.max(1);
    let text: String = runs.iter().map(|(t, _)| t.as_str()).collect();
    let mut ends = Vec::with_capacity(runs.len());
    let mut end = 0;
    for (t, style) in runs {
        end += t.len();
        ends.push((end, *style));
    }
    // The styled pieces of `text[start..end]`.
    let pieces = |start: usize, end: usize| -> Vec<(&str, Style)> {
        let mut out = Vec::new();
        let mut at = start;
        let mut i = ends.partition_point(|(e, _)| *e <= start);
        while at < end && i < ends.len() {
            let (run_end, style) = ends[i];
            let to = run_end.min(end);
            if to > at {
                out.push((&text[at..to], style));
            }
            at = to;
            i += 1;
        }
        out
    };

    let mut w = Wrapper::new(width);
    let mut space: Option<Style> = None;
    let mut offset = 0;
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            w.newline();
            space = None;
        }
        for token in tokens(line) {
            let start = offset + (token.as_ptr() as usize - line.as_ptr() as usize);
            let parts = pieces(start, start + token.len());
            if token.starts_with(char::is_whitespace) {
                space = parts.first().map(|p| p.1);
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
                w.push_split(&parts, true);
            } else {
                for (piece, style) in parts {
                    w.push(piece.to_string(), style, piece.width());
                }
            }
        }
        offset += line.len() + 1;
    }
    w.finish()
}

/// Split spans at `width` columns without regard for words. Used for code.
pub fn hard_wrap(spans: &[Span], width: usize) -> Vec<Vec<Span<'static>>> {
    let mut w = Wrapper::new(width);
    for span in spans {
        w.push_split(&[(&span.content, span.style)], false);
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

    fn push(&mut self, mut text: String, style: Style, w: usize) {
        if text.contains(PAD) {
            text = text.replace(PAD, " ");
        }
        self.cur.push(Span::styled(text, style));
        self.cur_w += w;
    }

    /// Push styled `pieces` character by character, breaking lines where
    /// they're full. With `kinsoku`, a break moves back a character or two
    /// rather than start a line with closing punctuation or end one with
    /// opening punctuation.
    fn push_split(&mut self, pieces: &[(&str, Style)], kinsoku: bool) {
        // Each char remembers which piece it came from, so pieces that look
        // alike (two adjacent links) still come out as separate spans.
        let mut chunk: Vec<(char, Style, usize)> = Vec::new();
        let mut chunk_w = 0;
        for (piece, &(text, style)) in pieces.iter().enumerate() {
            for c in text.chars() {
                let cw = c.width().unwrap_or(0);
                if self.cur_w + chunk_w + cw > self.width && self.cur_w + chunk_w > 0 {
                    let mut carry = Vec::new();
                    if kinsoku {
                        // What moves down must still fit with `c` on the
                        // next line; the width wins over punctuation rules.
                        let mut next = c;
                        let mut moved_w = cw;
                        while chunk.len() > 1
                            && let Some(&(last, ..)) = chunk.last()
                            && (no_break_before(next) || no_break_after(last))
                            && moved_w + last.width().unwrap_or(0) <= self.width
                        {
                            carry.push(chunk.pop().expect("chunk has more than one char"));
                            moved_w += last.width().unwrap_or(0);
                            next = last;
                        }
                        carry.reverse();
                    }
                    self.push_chars(&chunk);
                    self.newline();
                    chunk = carry;
                    chunk_w = chunk.iter().map(|(c, ..)| c.width().unwrap_or(0)).sum();
                }
                chunk.push((c, style, piece));
                chunk_w += cw;
            }
        }
        self.push_chars(&chunk);
    }

    /// Push characters as spans, one per run from the same piece.
    fn push_chars(&mut self, chars: &[(char, Style, usize)]) {
        for group in chars.chunk_by(|a, b| a.2 == b.2) {
            let text: String = group.iter().map(|(c, ..)| c).collect();
            let w = text.width();
            self.push(text, group[0].1, w);
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
        | '\u{FF00}'..='\u{FF9F}' // Fullwidth forms, halfwidth Katakana
        | '\u{FFE0}'..='\u{FFE6}'
        | '\u{20000}'..='\u{3FFFF}' // CJK extensions B and later
    )
}

/// Characters a line must not start with. A chip's padding stays with
/// its code on both sides.
fn no_break_before(c: char) -> bool {
    c == PAD
        || "、。，．：；！？）］｝」』】〕〉》〙〗〟’”ー々ゝゞヽヾぁぃぅぇぉっゃゅょゎゕゖァィゥェォッャュョヮヵヶ・…‥｡｣､･ｧｨｩｪｫｬｭｮｯｰﾞﾟ,.:;!?)]}%"
        .contains(c)
}

/// Characters a line must not end with.
fn no_break_after(c: char) -> bool {
    c == PAD || "（［｛「『【〔〈《〘〖〝‘“｢([{".contains(c)
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
    fn cjk_punctuation_holds_across_styles() {
        // "「" ends a plain run and "本" is bold: the break still can't
        // fall between them.
        let runs = vec![
            ("これは「".to_string(), Style::default()),
            ("本".to_string(), Style::default().bold()),
            ("」です".to_string(), Style::default()),
        ];
        let lines = wrap(&runs, 8);
        assert_eq!(text(&lines), ["これは", "「本」で", "す"]);
        assert!(
            lines[1]
                .spans
                .iter()
                .any(|s| s.content == "本" && s.style == Style::default().bold())
        );
    }

    #[test]
    fn oversized_tokens_keep_punctuation_attached() {
        assert_eq!(text(&wrap(&run("「ab」"), 4)), ["「a", "b」"]);
        // Code wraps exactly at the width.
        let code = hard_wrap(&[Span::raw("「ab」")], 4);
        assert_eq!(
            code[0]
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>(),
            "「ab"
        );
    }

    #[test]
    fn kinsoku_never_overflows_the_line() {
        // From 2: a double-width character never fits in 1.
        for width in 2..8 {
            for text in ["Rust」）》", "「「「ab」」」", "ab」」」」」"] {
                for line in wrap(&run(text), width) {
                    assert!(line.width() <= width, "{text:?} at {width}: {line:?}");
                }
            }
        }
    }

    #[test]
    fn split_words_keep_their_pieces_apart() {
        // Two adjacent links in one word too wide for the line: same look,
        // still two spans, so each keeps its own target.
        let link = Style::default().underlined();
        let runs = vec![("abcdef".to_string(), link), ("ghij".to_string(), link)];
        let lines = wrap(&runs, 8);
        let spans: Vec<Vec<&str>> = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        assert_eq!(spans, [vec!["abcdef", "gh"], vec!["ij"]]);
    }

    #[test]
    fn halfwidth_katakana_breaks_between_characters() {
        assert_eq!(text(&wrap(&run("ｱｲｳｴｵｶｷｸ"), 4)), ["ｱｲｳｴ", "ｵｶｷｸ"]);
        assert_eq!(text(&wrap(&run("ｱｲｳ｡"), 3)), ["ｱｲ", "ｳ｡"]);
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

    #[test]
    fn chip_padding_stays_with_cjk_code() {
        let pad = (PAD.to_string(), Style::default());
        let runs = vec![
            ("aaaaaaaa ".to_string(), Style::default()),
            pad.clone(),
            ("中".to_string(), Style::default()),
            pad,
        ];
        let lines = text(&wrap(&runs, 10));
        assert_eq!(lines[0].trim_end(), "aaaaaaaa", "{lines:?}");
        assert_eq!(lines[1], " 中 ", "{lines:?}");
    }
}
