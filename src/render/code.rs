use ratatui::style::{Color, Style};
use ratatui::text::Span;
use syntect::easy::HighlightLines;
use syntect::util::LinesWithEndings;

use super::theme::Theme;

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

    let ss = crate::syntax::set();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calls_macros_and_self_are_colored() {
        let theme = crate::themes::load("kanagawa-wave", None).unwrap();
        let text = Some(theme.text);
        let lines = highlight("self.go(1);\nprintln!(\"x\");\n", Some("rust"), &theme, 4);
        let color_of = |line: usize, text: &str| {
            lines[line]
                .iter()
                .find(|s| s.content.contains(text))
                .unwrap_or_else(|| panic!("no span with {text}: {:?}", lines[line]))
                .style
                .fg
        };
        assert_ne!(color_of(0, "self"), text);
        assert_ne!(color_of(0, "go"), text);
        assert_ne!(color_of(1, "println!"), text);
    }

    #[test]
    fn common_languages_beyond_the_syntect_defaults_highlight() {
        let theme = crate::themes::load("kanagawa-wave", None).unwrap();
        for (lang, src) in [
            ("ts", "const x: number = 1;\n"),
            ("tsx", "const x = <div />;\n"),
            ("toml", "a = \"b\"\n"),
            ("dockerfile", "FROM alpine\n"),
            ("zsh", "echo hi\n"),
        ] {
            let lines = highlight(src, Some(lang), &theme, 4);
            assert!(
                lines[0].iter().any(|s| s.style.fg != Some(theme.text)),
                "{lang} is not highlighted: {:?}",
                lines[0]
            );
        }
    }
}
