use ratatui::style::{Modifier, Style};
use ratatui::text::Line;

use super::LinkSpan;
use super::theme::Theme;

/// Spans drawn in the link style.
pub(super) fn is_link_style(style: Style, t: &Theme) -> bool {
    style.fg == Some(t.link) && style.add_modifier.contains(Modifier::UNDERLINED)
}

/// Text the renderer drew in the link style, in the order it will appear
/// on screen.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct LinkRun {
    pub url: String,
    /// Non-space characters of the text.
    pub chars: usize,
    /// The link occurrence it belongs to (`InlineStyle::link_id`).
    pub id: usize,
}

/// Match link-styled spans, in order, to the link runs the renderer
/// styled, and merge each link's spans on a line. Wrapping only drops
/// whitespace, so counting the other characters ties every span back to
/// its run, even across line breaks.
pub(super) fn locate_links(lines: &[Line], runs: &[LinkRun], t: &Theme) -> Vec<LinkSpan> {
    let mut out: Vec<LinkSpan> = Vec::new();
    walk(lines, runs, t, |i, col, w, _, run| match out.last_mut() {
        Some(l) if l.line == i && l.col + l.width == col && l.id == run.id => l.width += w,
        _ => out.push(LinkSpan {
            line: i,
            col,
            width: w,
            url: run.url.clone(),
            id: run.id,
        }),
    });
    out
}

/// `runs` cut at the ends of `lines`: what each line holds of them, so a
/// table can list its cells' runs in screen order.
pub(super) fn runs_by_line(lines: &[Line], runs: &[LinkRun], t: &Theme) -> Vec<Vec<LinkRun>> {
    let mut out: Vec<Vec<LinkRun>> = vec![Vec::new(); lines.len()];
    walk(lines, runs, t, |i, _, _, used, run| {
        match out[i].last_mut() {
            Some(last) if last.id == run.id => last.chars += used,
            _ => out[i].push(LinkRun {
                chars: used,
                ..run.clone()
            }),
        }
    });
    out
}

/// Call `f(line, col, width, non-space chars, run)` for each link-styled
/// span, with the run it belongs to.
fn walk(
    lines: &[Line],
    runs: &[LinkRun],
    t: &Theme,
    mut f: impl FnMut(usize, usize, usize, usize, &LinkRun),
) {
    let mut run = 0;
    let mut left = runs.first().map_or(0, |r| r.chars);
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
                    left = runs[run].chars;
                }
                let Some(r) = runs.get(run) else {
                    return;
                };
                f(i, col, w, used, r);
                left = left.saturating_sub(used);
            }
            col += w;
        }
    }
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
