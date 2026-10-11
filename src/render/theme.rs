use ratatui::style::Color;

/// Colors for rendered Markdown. The page background is left to the terminal.
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
    /// syntect theme name used for code highlighting.
    pub syntax: &'static str,
}

const fn hex(rgb: u32) -> Color {
    Color::Rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

impl Theme {
    /// Rosé Pine (main).
    pub fn dark() -> Self {
        Self {
            text: hex(0xe0def4),
            muted: hex(0x6e6a86),
            subtle: hex(0x908caa),
            surface: hex(0x1f1d2e),
            accent: hex(0xc4a7e7),
            h1_fg: hex(0x191724),
            h1_bg: hex(0xc4a7e7),
            heading: hex(0xebbcba),
            link: hex(0x9ccfd8),
            code: hex(0xebbcba),
            match_bg: hex(0x524f67),
            current_bg: hex(0xf6c177),
            err: hex(0xeb6f92),
            ok: hex(0x9ccfd8),
            warn: hex(0xf6c177),
            syntax: "base16-ocean.dark",
        }
    }

    /// Rosé Pine Dawn.
    pub fn light() -> Self {
        Self {
            text: hex(0x575279),
            muted: hex(0x9893a5),
            subtle: hex(0x797593),
            surface: hex(0xf2e9e1),
            accent: hex(0x907aa9),
            h1_fg: hex(0xfaf4ed),
            h1_bg: hex(0x907aa9),
            heading: hex(0xd7827e),
            link: hex(0x56949f),
            code: hex(0xd7827e),
            match_bg: hex(0xcecacd),
            current_bg: hex(0xea9d34),
            err: hex(0xb4637a),
            ok: hex(0x286983),
            warn: hex(0xea9d34),
            syntax: "InspiredGitHub",
        }
    }
}
