pub mod browser;
mod logo;
mod pane;
pub mod picker;
pub mod reader;
mod status;
pub(crate) mod text;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::themes::Theme;

/// Columns of space on each side of the content.
pub const MARGIN: u16 = 2;

/// The text of `buf`, row by row.
#[cfg(test)]
pub(crate) fn rows(buf: &ratatui::buffer::Buffer) -> Vec<String> {
    (0..buf.area.height)
        .map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect())
        .collect()
}

/// Paint the theme's page background over the whole frame, under
/// everything drawn after; `Reset` leaves the terminal's.
fn paint_page(frame: &mut Frame, t: &Theme) {
    let area = frame.area();
    frame
        .buffer_mut()
        .set_style(area, Style::default().bg(t.background));
}

/// `area` with `MARGIN` taken off the left and right.
pub fn inset(area: Rect) -> Rect {
    Rect {
        x: area.x + MARGIN,
        width: area.width.saturating_sub(MARGIN * 2),
        ..area
    }
}
