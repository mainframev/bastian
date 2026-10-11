use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use tui_big_text::{BigText, PixelSize};

use crate::render::theme::Theme;

/// Rows the logo takes up.
pub const HEIGHT: u16 = 3;

/// "bastian" in sextant big text, shaded from the theme's heading color on
/// the left to its accent color on the right.
pub fn draw(frame: &mut Frame, area: Rect, t: &Theme) {
    let area = Rect {
        height: area.height.min(HEIGHT),
        ..area
    };
    let text = BigText::builder()
        .pixel_size(PixelSize::Sextant)
        .lines(vec!["bastian".into()])
        .build();
    frame.render_widget(text, area);

    // Recolor the drawn cells column by column to get a horizontal gradient.
    let buf = frame.buffer_mut();
    let width = drawn_width(buf, area).max(1);
    for x in area.x..area.x + width {
        let color = mix(
            t.heading,
            t.accent,
            f32::from(x - area.x) / f32::from(width),
        );
        for y in area.y..area.y + area.height {
            let cell = &mut buf[(x, y)];
            if cell.symbol() != " " {
                cell.set_style(Style::default().fg(color));
            }
        }
    }
}

/// Columns up to and including the last non-blank cell.
fn drawn_width(buf: &ratatui::buffer::Buffer, area: Rect) -> u16 {
    (area.x..area.x + area.width)
        .rev()
        .find(|&x| (area.y..area.y + area.height).any(|y| buf[(x, y)].symbol() != " "))
        .map_or(0, |x| x - area.x + 1)
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
