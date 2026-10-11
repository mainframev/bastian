//! Named color themes. Built-ins are TOML files embedded from `themes/`;
//! each defines the document colors (`[ui]`) and the code colors
//! (`[syntax]`) the highlighter is built from.

use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow, bail};
use ratatui::style::Color;
use serde::Deserialize;

use crate::render::theme::Theme;

macro_rules! builtins {
    ($($name:literal),* $(,)?) => {
        &[$(($name, include_str!(concat!("../themes/", $name, ".toml")))),*]
    };
}

/// Built-in themes: name and TOML source.
const BUILTINS: &[(&str, &str)] = builtins![
    "tokyonight-night",
    "tokyonight-storm",
    "tokyonight-moon",
    "tokyonight-day",
    "rose-pine",
    "rose-pine-moon",
    "rose-pine-dawn",
    "kanagawa-wave",
    "kanagawa-dragon",
    "kanagawa-lotus",
    "catppuccin-mocha",
    "catppuccin-macchiato",
    "catppuccin-frappe",
    "catppuccin-latte",
    "gruvbox-dark",
    "gruvbox-light",
    "nord",
    "dracula",
    "everforest-dark",
    "everforest-light",
    "solarized-dark",
    "solarized-light",
];

/// Short names for a family's default flavor.
const ALIASES: &[(&str, &str)] = &[
    ("tokyonight", "tokyonight-night"),
    ("kanagawa", "kanagawa-wave"),
    ("catppuccin", "catppuccin-mocha"),
    ("gruvbox", "gruvbox-dark"),
    ("everforest", "everforest-dark"),
    ("solarized", "solarized-dark"),
];

/// Keys every theme defines, by section.
const UI_KEYS: &[&str] = &[
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
const SYNTAX_KEYS: &[&str] = &[
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

/// TextMate scopes each `[syntax]` color applies to.
const SCOPES: &[(&str, &str)] = &[
    ("comment", "comment, punctuation.definition.comment"),
    // Grammars scope `fn`, `let`, `class`, `func` etc. as storage.type.
    ("keyword", "keyword, storage"),
    ("string", "string, punctuation.definition.string"),
    ("number", "constant.numeric"),
    (
        "constant",
        "constant.language, constant.character, support.constant",
    ),
    ("function", "entity.name.function, support.function"),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    Dark,
    Light,
}

/// A theme file as written: every section is a table of color strings.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeFile {
    appearance: Option<Appearance>,
    #[serde(default)]
    ui: BTreeMap<String, String>,
    #[serde(default)]
    syntax: BTreeMap<String, String>,
}

/// Names of the built-in themes, in display order.
pub fn builtin_names() -> impl Iterator<Item = &'static str> {
    BUILTINS.iter().map(|(name, _)| *name)
}

/// The theme `name` stands for, following aliases.
fn canonical(name: &str) -> &str {
    ALIASES
        .iter()
        .find(|(alias, _)| *alias == name)
        .map_or(name, |(_, target)| target)
}

/// Whether `name` is a theme bastian can load.
pub fn exists(name: &str) -> bool {
    let name = canonical(name);
    BUILTINS.iter().any(|(n, _)| *n == name)
}

/// Load the theme called `name` (or an alias of one).
pub fn load(name: &str) -> Result<Theme> {
    let canonical = canonical(name);
    let (_, src) = BUILTINS
        .iter()
        .find(|(n, _)| *n == canonical)
        .ok_or_else(|| anyhow!("unknown theme {name:?}; run `bastian themes` to list them"))?;
    let file: ThemeFile =
        toml::from_str(src).with_context(|| format!("parsing built-in theme {canonical}"))?;
    build(file, canonical)
}

/// Check a parsed file is complete and turn it into a `Theme`.
fn build(file: ThemeFile, origin: &str) -> Result<Theme> {
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
fn check_keys(
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
fn rgb(s: &str) -> Option<(u8, u8, u8)> {
    let hex = s.strip_prefix('#').filter(|h| h.len() == 6)?;
    let n = u32::from_str_radix(hex, 16).ok()?;
    Some(((n >> 16) as u8, (n >> 8) as u8, n as u8))
}

fn color(s: &str, section: &str, key: &str, origin: &str) -> Result<Color> {
    let (r, g, b) = rgb(s).ok_or_else(|| {
        anyhow!("theme {origin}: `{section}.{key}` = {s:?} is not a #rrggbb color")
    })?;
    Ok(Color::Rgb(r, g, b))
}

/// A highlighter theme mapping the `[syntax]` colors onto TextMate scopes.
/// Anything unmatched uses the document text color.
fn syntax_theme(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_loads() {
        for name in builtin_names() {
            let t = load(name).unwrap_or_else(|e| panic!("{name}: {e:#}"));
            let expect = if name.ends_with("-day")
                || name.ends_with("-dawn")
                || name.ends_with("-lotus")
                || name.ends_with("-latte")
                || name.ends_with("-light")
            {
                Appearance::Light
            } else {
                Appearance::Dark
            };
            assert_eq!(t.appearance, expect, "{name}");
        }
        assert_eq!(BUILTINS.len(), 22);
    }

    #[test]
    fn aliases_resolve() {
        for (alias, target) in ALIASES {
            assert!(exists(alias), "{alias}");
            assert!(exists(target), "{target}");
        }
        assert!(!exists("nope"));
        let err = load("nope").unwrap_err().to_string();
        assert!(err.contains("unknown theme \"nope\""), "{err}");
    }

    fn file(src: &str) -> ThemeFile {
        toml::from_str(src).unwrap()
    }

    /// A built-in's source with the line setting `key` replaced.
    fn with_line(key: &str, replacement: &str) -> String {
        BUILTINS[0]
            .1
            .lines()
            .map(|l| {
                if l.split('=').next().unwrap().trim() == key {
                    replacement
                } else {
                    l
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn incomplete_or_bad_themes_are_errors() {
        let err = build(file(&with_line("link", "")), "t")
            .unwrap_err()
            .to_string();
        assert!(err.contains("missing ui.link"), "{err}");

        let src = BUILTINS[0]
            .1
            .replace("[syntax]", "[syntax]\nbogus = \"#000000\"");
        let err = build(file(&src), "t").unwrap_err().to_string();
        assert!(err.contains("unknown key `syntax.bogus`"), "{err}");

        let src = with_line("accent", "accent = \"purple\"");
        let err = build(file(&src), "t").unwrap_err().to_string();
        assert!(err.contains("`ui.accent` = \"purple\""), "{err}");
    }

    #[test]
    fn code_takes_the_palette_colors() {
        let t = load("kanagawa-wave").unwrap();
        let f: ThemeFile = toml::from_str(
            BUILTINS
                .iter()
                .find(|(n, _)| *n == "kanagawa-wave")
                .unwrap()
                .1,
        )
        .unwrap();
        let want = |key: &str| {
            let (r, g, b) = rgb(&f.syntax[key]).unwrap();
            Color::Rgb(r, g, b)
        };
        let lines = crate::render::code::highlight(
            "fn main() { let s = \"hi\"; struct Point; }\n",
            Some("rust"),
            &t,
        );
        let color_of = |text: &str| {
            lines[0]
                .iter()
                .find(|s| s.content.contains(text))
                .unwrap_or_else(|| panic!("no span with {text}: {:?}", lines[0]))
                .style
                .fg
        };
        assert_eq!(color_of("fn"), Some(want("keyword")));
        assert_eq!(color_of("hi"), Some(want("string")));
        assert_eq!(color_of("main"), Some(want("function")));
        assert_eq!(color_of("Point"), Some(want("type")));
    }
}
