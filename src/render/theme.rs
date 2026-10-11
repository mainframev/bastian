use std::sync::Arc;

use ratatui::style::Color;

pub use crate::themes::Appearance;

/// Colors for rendered Markdown. The page background is left to the terminal.
/// Built from a theme file; see `crate::themes`.
#[derive(Debug, Clone)]
pub struct Theme {
    pub text: Color,
    pub muted: Color,
    pub subtle: Color,
    pub surface: Color,
    pub accent: Color,
    pub h1_fg: Color,
    pub h1_bg: Color,
    pub heading: Color,
    pub link: Color,
    pub code: Color,
    /// Background of search matches, and of the current match.
    pub match_bg: Color,
    pub current_bg: Color,
    pub err: Color,
    pub ok: Color,
    pub warn: Color,
    pub appearance: Appearance,
    /// Highlighter theme for code blocks, from the theme's `[syntax]` colors.
    pub syntax: Arc<syntect::highlighting::Theme>,
}

impl Theme {
    /// The default dark theme, Rosé Pine.
    pub fn dark() -> Self {
        crate::themes::load("rose-pine", None).expect("built-in theme")
    }

    /// The default light theme, Rosé Pine Dawn.
    pub fn light() -> Self {
        crate::themes::load("rose-pine-dawn", None).expect("built-in theme")
    }
}
