use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::{Result, anyhow, bail};
use ratatui::style::Color;

use super::Theme;
use super::file::ThemeFile;

/// Keys every theme defines, by section.
pub(super) const UI_KEYS: &[&str] = &[
    "text",
    "muted",
    "subtle",
    "surface",
    "accent",
    "h1_fg",
    "h1_bg",
    "heading",
    "link",
    "code",
    "match_bg",
    "current_bg",
    "err",
    "ok",
    "warn",
];

pub(super) const SYNTAX_KEYS: &[&str] = &[
    "comment",
    "keyword",
    "string",
    "number",
    "constant",
    "function",
    "type",
    "operator",
    "punctuation",
    "tag",
    "attribute",
    "inserted",
    "deleted",
];

/// `TextMate` scopes each `[syntax]` color applies to.
pub(super) const SCOPES: &[(&str, &str)] = &[
    ("comment", "comment, punctuation.definition.comment"),
    // Grammars scope `fn`, `let`, `class`, `func` etc. as storage.type.
    ("keyword", "keyword, storage"),
    ("string", "string, punctuation.definition.string"),
    ("number", "constant.numeric"),
    (
        "constant",
        "constant.language, constant.character, constant.other, support.constant, \
         variable.language",
    ),
    // Calls are variable.function in most grammars; Rust macros are support.macro.
    (
        "function",
        "entity.name.function, support.function, variable.function, support.macro",
    ),
    (
        "type",
        "entity.name.type, entity.name.class, entity.name.struct, entity.name.enum, \
         entity.name.trait, entity.name.impl, support.type, support.class",
    ),
    ("operator", "keyword.operator"),
    ("punctuation", "punctuation"),
    ("tag", "entity.name.tag"),
    ("attribute", "entity.other.attribute-name"),
    ("inserted", "markup.inserted"),
    ("deleted", "markup.deleted"),
];

/// Check a parsed file is complete and turn it into a `Theme`.
pub(super) fn build(file: &ThemeFile, origin: &str) -> Result<Theme> {
    check_keys(&file.ui, "ui", UI_KEYS, origin)?;
    check_keys(&file.syntax, "syntax", SYNTAX_KEYS, origin)?;
    let appearance = file
        .appearance
        .ok_or_else(|| anyhow!("theme {origin} is missing `appearance`"))?;
    let ui = |key: &str| color(&file.ui[key], "ui", key, origin);
    Ok(Theme {
        text: ui("text")?,
        muted: ui("muted")?,
        subtle: ui("subtle")?,
        surface: ui("surface")?,
        accent: ui("accent")?,
        h1_fg: ui("h1_fg")?,
        h1_bg: ui("h1_bg")?,
        heading: ui("heading")?,
        link: ui("link")?,
        code: ui("code")?,
        match_bg: ui("match_bg")?,
        current_bg: ui("current_bg")?,
        err: ui("err")?,
        ok: ui("ok")?,
        warn: ui("warn")?,
        appearance,
        syntax: Arc::new(syntax_theme(&file.ui["text"], &file.syntax, origin)?),
    })
}

/// Every key in `section` is known and every known key is present.
pub(super) fn check_keys(
    section: &BTreeMap<String, String>,
    name: &str,
    keys: &[&str],
    origin: &str,
) -> Result<()> {
    if let Some(unknown) = section.keys().find(|k| !keys.contains(&k.as_str())) {
        bail!("theme {origin}: unknown key `{name}.{unknown}`");
    }
    let missing: Vec<_> = keys
        .iter()
        .filter(|k| !section.contains_key(**k))
        .map(|k| format!("{name}.{k}"))
        .collect();
    if !missing.is_empty() {
        bail!("theme {origin} is missing {}", missing.join(", "));
    }
    Ok(())
}

/// Parse `#rrggbb`.
pub(super) fn rgb(s: &str) -> Option<(u8, u8, u8)> {
    let hex = s.strip_prefix('#').filter(|h| h.len() == 6)?;
    let n = u32::from_str_radix(hex, 16).ok()?;
    Some(((n >> 16) as u8, (n >> 8) as u8, n as u8))
}

/// The 16 ANSI colors by name, in palette order. They show in whatever
/// colors the terminal's own palette gives them.
const ANSI: [(&str, Color); 16] = [
    ("black", Color::Black),
    ("red", Color::Red),
    ("green", Color::Green),
    ("yellow", Color::Yellow),
    ("blue", Color::Blue),
    ("magenta", Color::Magenta),
    ("cyan", Color::Cyan),
    ("white", Color::Gray),
    ("bright-black", Color::DarkGray),
    ("bright-red", Color::LightRed),
    ("bright-green", Color::LightGreen),
    ("bright-yellow", Color::LightYellow),
    ("bright-blue", Color::LightBlue),
    ("bright-magenta", Color::LightMagenta),
    ("bright-cyan", Color::LightCyan),
    ("bright-white", Color::White),
];

/// `default` is the terminal's own foreground or background.
const DEFAULT: &str = "default";

/// Parse `#rrggbb`, an ANSI color name or `default`.
pub(super) fn color(s: &str, section: &str, key: &str, origin: &str) -> Result<Color> {
    if s == DEFAULT {
        return Ok(Color::Reset);
    }
    if let Some((_, c)) = ANSI.iter().find(|(name, _)| *name == s) {
        return Ok(*c);
    }
    let (r, g, b) = rgb(s).ok_or_else(|| {
        anyhow!(
            "theme {origin}: `{section}.{key}` = {s:?} is not a #rrggbb color, \
             an ANSI color name or \"default\""
        )
    })?;
    Ok(Color::Rgb(r, g, b))
}

/// `s` as a highlighter color. Highlighter colors are RGBA, so like bat an
/// alpha of 0 marks an ANSI color (its index in `r`) and 1 the default.
fn to_syntect(
    s: &str,
    section: &str,
    key: &str,
    origin: &str,
) -> Result<syntect::highlighting::Color> {
    use syntect::highlighting::Color as C;
    let c = match color(s, section, key, origin)? {
        Color::Rgb(r, g, b) => C { r, g, b, a: 0xff },
        Color::Reset => C {
            r: 0,
            g: 0,
            b: 0,
            a: 1,
        },
        ansi => C {
            r: ansi_index(ansi).expect("an ANSI color"),
            g: 0,
            b: 0,
            a: 0,
        },
    };
    Ok(c)
}

/// Palette index (0–15) of an ANSI color.
pub(super) fn ansi_index(c: Color) -> Option<u8> {
    ANSI.iter().position(|(_, a)| *a == c).map(|i| i as u8)
}

/// The terminal color a highlighter color stands for; see `to_syntect`.
pub fn from_syntect(c: syntect::highlighting::Color) -> Color {
    match c.a {
        0 => ANSI.get(usize::from(c.r)).map_or(Color::Reset, |(_, c)| *c),
        1 => Color::Reset,
        _ => Color::Rgb(c.r, c.g, c.b),
    }
}

/// A highlighter theme mapping the `[syntax]` colors onto `TextMate` scopes.
/// Anything unmatched uses the document text color.
pub(super) fn syntax_theme(
    text: &str,
    colors: &BTreeMap<String, String>,
    origin: &str,
) -> Result<syntect::highlighting::Theme> {
    use std::str::FromStr;
    use syntect::highlighting::{ScopeSelectors, StyleModifier, ThemeItem};

    let mut theme = syntect::highlighting::Theme::default();
    theme.settings.foreground = Some(to_syntect(text, "ui", "text", origin)?);
    for (key, scopes) in SCOPES {
        theme.scopes.push(ThemeItem {
            scope: ScopeSelectors::from_str(scopes).expect("valid scope selector"),
            style: StyleModifier {
                foreground: Some(to_syntect(&colors[*key], "syntax", key, origin)?),
                background: None,
                font_style: None,
            },
        });
    }
    Ok(theme)
}
