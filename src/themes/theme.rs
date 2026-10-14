use std::sync::Arc;

use ratatui::style::Color;

use super::Appearance;

/// Colors for rendered Markdown. The page background is left to the terminal.
/// Built from a theme file by `load`.
#[derive(Debug, Clone)]
pub struct Theme {
    pub text: Color,
    pub muted: Color,
    pub subtle: Color,
    pub surface: Color,
    pub accent: Color,
    pub h1_fg: Color,
    pub h1_bg: Color,
    /// Headings below the title, unless a theme sets a color per level;
    /// `heading_5` colors levels 5 and 6.
    pub heading: Color,
    pub heading_2: Color,
    pub heading_3: Color,
    pub heading_4: Color,
    pub heading_5: Color,
    /// The rule under level-2 headings.
    pub heading_underline: Color,
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
    /// Text color of a level-`level` heading below the title.
    pub fn heading_color(&self, level: u8) -> Color {
        match level {
            2 => self.heading_2,
            3 => self.heading_3,
            4 => self.heading_4,
            5.. => self.heading_5,
            _ => self.heading,
        }
    }

    /// The default dark theme, Rosé Pine.
    pub fn dark() -> Self {
        super::load(super::DEFAULT_DARK, None).expect("built-in theme")
    }

    /// The default light theme, Rosé Pine Dawn.
    pub fn light() -> Self {
        super::load(super::DEFAULT_LIGHT, None).expect("built-in theme")
    }
}
