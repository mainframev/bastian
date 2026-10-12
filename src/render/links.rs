use ratatui::style::{Modifier, Style};
use ratatui::text::Line;

use super::LinkSpan;
use super::theme::Theme;

/// Spans drawn in the link style.
pub(super) fn is_link_style(style: Style, t: &Theme) -> bool {
    style.fg == Some(t.link) && style.add_modifier.contains(Modifier::UNDERLINED)
}

/// Match link-styled spans, in order, to the link runs the renderer styled
/// (`(url, non-space characters)`), and merge each run's spans on a line.
/// Wrapping only drops whitespace, so counting the other characters ties
/// every span back to its run, even across line breaks.
pub(super) fn locate_links(lines: &[Line], runs: &[(String, usize)], t: &Theme) -> Vec<LinkSpan> {
    let mut out: Vec<LinkSpan> = Vec::new();
    let mut run = 0;
    let mut left = runs.first().map_or(0, |r| r.1);
    for (i, line) in lines.iter().enumerate() {
        let mut col = 0;
        for span in &line.spans {
            let w = span.width();
            if is_link_style(span.style, t) {
                let used = span.content.chars().filter(|c| !c.is_whitespace()).count();
                // Move on once the current run's text is used up; spaces
                // still belong to the run they follow.
                while used > 0 && left == 0 && run + 1 < runs.len() {
                    run += 1;
                    left = runs[run].1;
                }
                let Some((url, _)) = runs.get(run) else {
                    return out;
                };
                match out.last_mut() {
                    Some(l) if l.line == i && l.col + l.width == col && l.url == *url => {
                        l.width += w;
                    }
                    _ => out.push(LinkSpan {
                        line: i,
                        col,
                        width: w,
                        url: url.clone(),
                    }),
                }
                left = left.saturating_sub(used);
            }
            col += w;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::parse;
    use crate::render::*;

    #[test]
    fn links_are_located_across_wraps_and_prefixes() {
        let src = "# Top [x](https://h1.dev)\n\nsee [the long guide](docs/guide.md) and [b](https://b.dev)\n\n- item [c](#c)\n";
        let r = render(&parse(src), 16, &Theme::dark());
        let text = |l: &LinkSpan| -> String {
            let line = plain(&r.lines[l.line]);
            line.chars().skip(l.col).take(l.width).collect()
        };
        let found: Vec<(String, String)> =
            r.links.iter().map(|l| (l.url.clone(), text(l))).collect();
        assert_eq!(
            found,
            [
                // The H1 band shows its text plainly, so only these count.
                ("docs/guide.md".to_string(), "the long".to_string()),
                ("docs/guide.md".to_string(), "guide".to_string()),
                ("https://b.dev".to_string(), "b".to_string()),
                ("#c".to_string(), "c".to_string()),
            ],
            "{:?}",
            to_plain(&r.lines)
        );
    }
}
