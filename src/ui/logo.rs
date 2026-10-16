use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use crate::render::theme::Theme;

/// Rows the logo takes up.
pub const HEIGHT: u16 = 3;

/// "bastian" in sextant block glyphs.
const ROWS: [&str; HEIGHT as usize] = [
    "🬨▌  🬞🬭🬏 🬞🬭🬭 🬞🬻🬭 🬞🬰  🬞🬭🬏 🬭🬭🬏",
    "▐🬕🬨🬓🬵🬋█ 🬊🬋🬱  █🬞  █  🬵🬋█ █ █",
    "🬂🬁🬂 🬁🬂🬁🬀🬂🬂🬀  🬁🬀 🬁🬂🬀 🬁🬂🬁🬀🬂 🬂",
];

/// The logo, shaded from the theme's heading color on the left to its
/// accent color on the right.
pub fn draw(frame: &mut Frame, area: Rect, t: &Theme) {
    // The gradient spans the columns that fit, up to the last drawn glyph.
    let width = (0..area.width)
        .rev()
        .find(|&x| {
            ROWS.iter()
                .any(|r| r.chars().nth(x.into()).is_some_and(|c| c != ' '))
        })
        .map_or(1, |x| x + 1);
    let buf = frame.buffer_mut();
    for (y, row) in (area.y..area.y + area.height.min(HEIGHT)).zip(ROWS) {
        for (x, ch) in (area.x..area.right()).zip(row.chars()) {
            if ch != ' ' {
                let color = mix(
                    t.heading,
                    t.accent,
                    f32::from(x - area.x) / f32::from(width),
                );
                buf[(x, y)]
                    .set_char(ch)
                    .set_style(Style::default().fg(color));
            }
        }
    }
}

/// Linear blend of two RGB colors; non-RGB colors fall back to `a`.
fn mix(a: Color, b: Color, k: f32) -> Color {
    match (a, b) {
        (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => {
            let ch =
                |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * k).round() as u8;
            Color::Rgb(ch(r1, r2), ch(g1, g2), ch(b1, b2))
        }
        _ => a,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn mixes_endpoints() {
        let a = Color::Rgb(0, 0, 0);
        let b = Color::Rgb(200, 100, 50);
        assert_eq!(mix(a, b, 0.0), a);
        assert_eq!(mix(a, b, 1.0), b);
        assert_eq!(mix(a, b, 0.5), Color::Rgb(100, 50, 25));
    }

    #[test]
    fn draws_gradient_logo_in_three_rows() {
        let t = Theme::dark();
        let mut term = Terminal::new(TestBackend::new(40, 5)).unwrap();
        term.draw(|f| draw(f, Rect::new(0, 0, 40, 5), &t)).unwrap();
        let buf = term.backend().buffer();
        // Nothing drawn below the logo's rows.
        assert!((0..40).all(|x| buf[(x, 3)].symbol() == " "));
        let colored: Vec<Color> = (0..40)
            .filter_map(|x| {
                (0..3)
                    .find(|&y| buf[(x, y)].symbol() != " ")
                    .map(|y| buf[(x, y)].fg)
            })
            .collect();
        assert_eq!(colored.first(), Some(&t.heading));
        assert_ne!(colored.first(), colored.last(), "expected a gradient");
    }
}
