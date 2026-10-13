use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::{Result, anyhow, bail};
use ratatui::style::Color;

use super::file::ThemeFile;
use crate::render::theme::Theme;

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

pub(super) fn color(s: &str, section: &str, key: &str, origin: &str) -> Result<Color> {
    let (r, g, b) = rgb(s).ok_or_else(|| {
        anyhow!("theme {origin}: `{section}.{key}` = {s:?} is not a #rrggbb color")
    })?;
    Ok(Color::Rgb(r, g, b))
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

    let to_syntect = |s: &str, key: &str| {
        let (r, g, b) = rgb(s)
            .ok_or_else(|| anyhow!("theme {origin}: `{key}` = {s:?} is not a #rrggbb color"))?;
        Ok::<_, anyhow::Error>(syntect::highlighting::Color { r, g, b, a: 0xff })
    };
    let mut theme = syntect::highlighting::Theme::default();
    theme.settings.foreground = Some(to_syntect(text, "ui.text")?);
    for (key, scopes) in SCOPES {
        theme.scopes.push(ThemeItem {
            scope: ScopeSelectors::from_str(scopes).expect("valid scope selector"),
            style: StyleModifier {
                foreground: Some(to_syntect(&colors[*key], &format!("syntax.{key}"))?),
                background: None,
                font_style: None,
            },
        });
    }
    Ok(theme)
}
