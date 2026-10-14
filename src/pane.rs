//! Width rules shared by the panes docked beside the document: the table
//! of contents on the left and the theme picker on the right.

/// The narrowest a pane gets.
pub const MIN_WIDTH: u16 = 16;
/// Columns `<` and `>` take off or add.
pub const STEP: u16 = 4;
/// Screens narrower than this leave a pane out, so the document keeps
/// room to read.
pub const MIN_SCREEN: u16 = 40;

/// A pane's width on a screen `screen` columns wide when asked for
/// `width`: kept to at least `MIN_WIDTH` and at most half the screen, or 0
/// when the screen is too narrow for it.
pub fn fit(width: u16, screen: u16) -> u16 {
    if screen < MIN_SCREEN {
        return 0;
    }
    width.clamp(MIN_WIDTH, max_width(screen))
}

/// The widest a pane gets on a screen `screen` columns wide.
pub fn max_width(screen: u16) -> u16 {
    (screen / 2).max(MIN_WIDTH)
}

/// `width` a `STEP` narrower or wider, within what fits on a screen
/// `screen` columns wide. Starts from what fits, so a pane asked wider
/// than a small screen allows narrows at once.
pub fn resize(width: u16, screen: u16, wider: bool) -> u16 {
    let max = max_width(screen);
    let width = width.min(max);
    let width = if wider {
        width.saturating_add(STEP)
    } else {
        width.saturating_sub(STEP)
    };
    width.clamp(MIN_WIDTH, max)
}

/// Columns of the `<` and `>` markers on the header row of a pane `width`
/// wide, from its left edge: before a space and the pane's last column.
pub fn markers(width: u16) -> (u16, u16) {
    (width.saturating_sub(5), width.saturating_sub(3))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_keeps_the_pane_within_the_screen() {
        assert_eq!(fit(30, 100), 30);
        assert_eq!(fit(4, 100), MIN_WIDTH);
        assert_eq!(fit(90, 100), 50);
        assert_eq!(fit(30, MIN_SCREEN - 1), 0);
        assert_eq!(fit(30, MIN_SCREEN), 20);
    }

    #[test]
    fn resize_steps_within_the_screen() {
        assert_eq!(resize(30, 100, true), 30 + STEP);
        assert_eq!(resize(48, 100, true), 50);
        assert_eq!(resize(18, 100, false), MIN_WIDTH);
        // On a smaller screen, narrowing starts from what fits.
        assert_eq!(resize(50, 60, false), 30 - STEP);
    }
}
