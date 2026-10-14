pub mod browser;
mod logo;
mod pane;
pub mod picker;
pub mod reader;
mod status;
pub(crate) mod text;

use ratatui::layout::Rect;

/// Columns of space on each side of the content.
pub const MARGIN: u16 = 2;

/// `area` with `MARGIN` taken off the left and right.
pub fn inset(area: Rect) -> Rect {
    Rect {
        x: area.x + MARGIN,
        width: area.width.saturating_sub(MARGIN * 2),
        ..area
    }
}
