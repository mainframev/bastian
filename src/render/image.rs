use ratatui::style::Style;
use ratatui::text::Line;

use super::wrap::wrap;
use super::{ImageSlot, Out, Renderer};

impl Renderer<'_> {
    /// A picture: blank rows to draw it over, then its alt text as a
    /// caption. Until it's loaded (or where it can't be shown), just the
    /// alt text.
    pub(super) fn image(&self, alt: &str, url: &str, width: usize) -> Out {
        let caption = Style::default().fg(self.theme.muted);
        let Some(&(w, h)) = self.images.get(url) else {
            return Out::from(wrap(
                &[(crate::doc::image_placeholder(alt), caption)],
                width,
            ));
        };
        let (cols, rows) = fit_image(w, h, width as u16, self.opts.image_rows);
        let mut lines = vec![Line::default(); usize::from(rows)];
        if !alt.is_empty() {
            lines.extend(wrap(&[(alt.to_string(), caption)], width));
        }
        Out {
            images: vec![ImageSlot {
                url: url.to_string(),
                line: 0,
                col: 0,
                cols,
                rows,
            }],
            ..Out::from(lines)
        }
    }
}

/// Scale a `w`×`h` cell picture to at most `width` columns and
/// `max_rows` rows, keeping its shape.
pub(super) fn fit_image(w: u16, h: u16, width: u16, max_rows: u16) -> (u16, u16) {
    let (w, h) = (f32::from(w.max(1)), f32::from(h.max(1)));
    let scale = (f32::from(width.max(1)) / w)
        .min(f32::from(max_rows.max(1)) / h)
        .min(1.0);
    (
        ((w * scale).round() as u16).max(1),
        ((h * scale).round() as u16).max(1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::parse;
    use crate::render::*;

    #[test]
    fn images_reserve_rows_once_loaded() {
        let doc = parse("- item\n\n  ![A cat](cat.png)\n");
        let placeholder = render(&doc, 40, &Theme::dark());
        assert!(placeholder.images.is_empty(), "{:?}", placeholder.images);
        assert!(to_plain(&placeholder.lines).contains("[image: A cat]"));

        let sizes = ImageSizes::from([("cat.png".to_string(), (60, 40))]);
        let (runs, interpreters, highlights) = Default::default();
        let cx = Context {
            runs: &runs,
            interpreters: &interpreters,
            highlights: &highlights,
            images: &sizes,
            opts: Options::default(),
        };
        let r = render_with_runs(&doc, 40, &Theme::dark(), cx);
        let [slot] = r.images.as_slice() else {
            panic!("expected one slot: {:?}", r.images);
        };
        // Scaled to the 38 columns inside the list indent, keeping its shape.
        assert_eq!((slot.col, slot.cols, slot.rows), (2, 36, 24));
        assert_eq!(fit_image(60, 40, 38, 24), (36, 24));
        assert_eq!(fit_image(10, 5, 38, 24), (10, 5));
        assert_eq!(fit_image(60, 40, 38, 12), (18, 12));
        let caption = plain(&r.lines[slot.line + usize::from(slot.rows)]);
        assert_eq!(caption.trim(), "A cat");
    }
}
