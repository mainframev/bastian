use std::sync::LazyLock;

use ratatui::style::{Color, Style};
use ratatui::text::Span;
use syntect::easy::HighlightLines;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

use super::theme::Theme;

static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);

/// The syntax token for a file extension, if it names a language the
/// highlighter knows (e.g. `rs` → `rs`).
pub fn known_extension(ext: &str) -> bool {
    SYNTAXES.find_syntax_by_extension(ext).is_some()
}

/// Syntax-highlight `src` into one span list per source line, each tab
/// as `tab_width` spaces.
pub fn highlight(
    src: &str,
    lang: Option<&str>,
    theme: &Theme,
    tab_width: u16,
) -> Vec<Vec<Span<'static>>> {
    let src = src.replace('\t', &" ".repeat(usize::from(tab_width)));
    let plain = || {
        src.lines()
            .map(|l| vec![Span::styled(l.to_string(), Style::default().fg(theme.text))])
            .collect()
    };

    let ss = &*SYNTAXES;
    let Some(syntax) = lang.and_then(|l| ss.find_syntax_by_token(l)) else {
        return plain();
    };
    let mut h = HighlightLines::new(syntax, &theme.syntax);
    let mut out = Vec::new();
    for line in LinesWithEndings::from(&src) {
        let Ok(regions) = h.highlight_line(line, ss) else {
            return plain();
        };
        out.push(
            regions
                .into_iter()
                .map(|(s, text)| {
                    let fg = Color::Rgb(s.foreground.r, s.foreground.g, s.foreground.b);
                    Span::styled(
                        text.trim_end_matches(['\n', '\r']).to_string(),
                        Style::default().fg(fg),
                    )
                })
                .filter(|span| !span.content.is_empty())
                .collect(),
        );
    }
    out
}
