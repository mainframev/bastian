//! Named color themes. Built-ins are TOML files embedded from `themes/`;
//! each defines the document colors (`[ui]`) and the code colors
//! (`[syntax]`) the highlighter is built from.

mod build;
mod builtin;
mod file;
mod list;
mod theme;

pub use build::from_syntect;
pub use builtin::builtin_names;
pub use list::{Listing, format_list, list};
pub use theme::Theme;

use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;

use build::build;
use builtin::{alias, builtin};
use file::{ThemeFile, user_file};

/// The default `dark_theme`.
pub const DEFAULT_DARK: &str = "rose-pine";
/// The default `light_theme`.
pub const DEFAULT_LIGHT: &str = "rose-pine-dawn";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    Dark,
    Light,
    /// Follows the terminal's own colors, so suits either background.
    Any,
}

/// Whether `name` is a theme bastian can load: a file in `dir` (the user's
/// themes directory) or a built-in.
pub fn exists(name: &str, dir: Option<&Path>) -> bool {
    user_file(name, dir).is_some() || builtin(name).is_some()
}

/// The theme `name` loads: itself when `dir` has a file of that name, which
/// wins as in `load`, else the built-in an alias stands for.
pub fn canonical<'a>(name: &'a str, dir: Option<&Path>) -> &'a str {
    if user_file(name, dir).is_some() {
        name
    } else {
        alias(name)
    }
}

/// Load the theme called `name`. A file in `dir` wins over a built-in of the
/// same name, so a built-in can be tweaked by extending it under its own name.
pub fn load(name: &str, dir: Option<&Path>) -> Result<Theme> {
    let mut chain = Vec::new();
    let (file, origin) = read(name, dir, &mut chain)?;
    build(&file, &origin)
}

/// The merged file for `name` and where it came from. `chain` holds the
/// names being resolved, to catch `extends` cycles.
fn read(name: &str, dir: Option<&Path>, chain: &mut Vec<String>) -> Result<(ThemeFile, String)> {
    // Extending your own name means the built-in underneath.
    let own = chain.last().is_some_and(|n| n == name);
    let path = if own { None } else { user_file(name, dir) };
    if !own && chain.iter().any(|n| n == name) {
        chain.push(name.to_string());
        bail!("theme `extends` loops: {}", chain.join(" → "));
    }
    chain.push(name.to_string());
    let (file, origin): (ThemeFile, String) = if let Some(path) = path {
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let file =
            toml::from_str(&text).with_context(|| format!("parsing theme {}", path.display()))?;
        (file, path.display().to_string())
    } else {
        let src = builtin(name)
            .ok_or_else(|| anyhow!("unknown theme {name:?}; run `bastian themes` to list them"))?;
        let file = toml::from_str(src).with_context(|| format!("parsing built-in theme {name}"))?;
        (file, alias(name).to_string())
    };
    match file.extends.clone() {
        Some(base) => {
            let (base, _) = read(&base, dir, chain)
                .with_context(|| format!("loading `extends = {base:?}` of {origin}"))?;
            Ok((file.over(base), origin))
        }
        None => Ok((file, origin)),
    }
}

#[cfg(test)]
mod tests {
    use super::build::rgb;
    use super::builtin::{ALIASES, BUILTINS};
    use super::*;
    use ratatui::style::Color;

    #[test]
    fn every_builtin_loads() {
        for name in builtin_names() {
            let t = load(name, None).unwrap_or_else(|e| panic!("{name}: {e:#}"));
            let expect = if name == "terminal" {
                Appearance::Any
            } else if name.ends_with("-day")
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
        assert_eq!(BUILTINS.len(), 26);
    }

    #[test]
    fn a_user_file_shadows_an_alias() {
        let dir = theme_dir(&[("tokyonight", "extends = \"nord\"\n")]);
        assert_eq!(canonical("tokyonight", None), "tokyonight-night");
        assert_eq!(canonical("tokyonight", Some(dir.path())), "tokyonight");
        assert_eq!(canonical("kanagawa", Some(dir.path())), "kanagawa-wave");
    }

    #[test]
    fn aliases_resolve() {
        for (alias, target) in ALIASES {
            assert!(exists(alias, None), "{alias}");
            assert!(exists(target, None), "{target}");
        }
        assert!(!exists("nope", None));
        let err = load("nope", None).unwrap_err().to_string();
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
        let err = build(&file(&with_line("link", "")), "t")
            .unwrap_err()
            .to_string();
        assert!(err.contains("missing ui.link"), "{err}");

        let src = BUILTINS[0]
            .1
            .replace("[syntax]", "[syntax]\nbogus = \"#000000\"");
        let err = build(&file(&src), "t").unwrap_err().to_string();
        assert!(err.contains("unknown key `syntax.bogus`"), "{err}");

        let src = with_line("accent", "accent = \"purple\"");
        let err = build(&file(&src), "t").unwrap_err().to_string();
        assert!(err.contains("`ui.accent` = \"purple\""), "{err}");
    }

    #[test]
    fn code_takes_the_palette_colors() {
        let theme = load("kanagawa-wave", None).unwrap();
        let palette: ThemeFile = toml::from_str(
            BUILTINS
                .iter()
                .find(|(n, _)| *n == "kanagawa-wave")
                .unwrap()
                .1,
        )
        .unwrap();
        let want = |key: &str| {
            let (red, green, blue) = rgb(&palette.syntax[key]).unwrap();
            Color::Rgb(red, green, blue)
        };
        let lines = crate::render::code::highlight(
            "fn main() { let s = \"hi\"; struct Point; }\n",
            Some("rust"),
            &theme,
            4,
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

    #[test]
    fn ansi_names_follow_the_terminal_palette() {
        let t = load("terminal", None).unwrap();
        assert_eq!(t.text, Color::Reset);
        assert_eq!(t.muted, Color::DarkGray);
        assert_eq!(t.accent, Color::Blue);
        let lines =
            crate::render::code::highlight("fn main() { let s = \"hi\"; }\n", Some("rust"), &t, 4);
        let color_of = |text: &str| {
            lines[0]
                .iter()
                .find(|s| s.content.contains(text))
                .unwrap_or_else(|| panic!("no span with {text}: {:?}", lines[0]))
                .style
                .fg
        };
        assert_eq!(color_of("fn"), Some(Color::Magenta));
        assert_eq!(color_of("hi"), Some(Color::Green));
        assert_eq!(color_of("main"), Some(Color::Blue));
        assert_eq!(color_of("{"), Some(Color::Reset));

        let src = with_line("accent", "accent = \"bright-white\"");
        assert_eq!(build(&file(&src), "t").unwrap().accent, Color::White);
    }

    #[test]
    fn optional_colors_fall_back() {
        let src = "appearance = \"dark\"\n[ui]\n".to_string()
            + &build::UI_KEYS
                .iter()
                .map(|k| format!("{k} = \"#101010\"\n"))
                .collect::<String>()
            + "heading = \"#202020\"\nmuted = \"#303030\"\n[syntax]\n"
            + &build::SCOPES
                .iter()
                .map(|(k, _)| format!("{k} = \"#101010\"\n"))
                .collect::<String>();
        // TOML rejects the repeated keys, so drop the first `heading` and `muted`.
        let src =
            src.replacen("heading = \"#101010\"\n", "", 1)
                .replacen("muted = \"#101010\"\n", "", 1);
        let t = build(&file(&src), "t").unwrap();
        let heading = Color::Rgb(0x20, 0x20, 0x20);
        for level in 2..=6 {
            assert_eq!(t.heading_color(level), heading, "level {level}");
        }
        assert_eq!(t.heading_underline, Color::Rgb(0x30, 0x30, 0x30));
        assert_eq!(t.bullets, [t.accent; 3]);

        let src = src.replace(
            "[syntax]",
            "heading_3 = \"red\"\nheading_5 = \"#505050\"\nheading_underline = \"default\"\n[syntax]",
        );
        let t = build(&file(&src), "t").unwrap();
        assert_eq!(t.heading_color(2), heading);
        assert_eq!(t.heading_color(3), Color::Red);
        assert_eq!(t.heading_color(4), heading);
        assert_eq!(t.heading_color(5), Color::Rgb(0x50, 0x50, 0x50));
        assert_eq!(t.heading_color(6), Color::Rgb(0x50, 0x50, 0x50));
        assert_eq!(t.heading_underline, Color::Reset);

        let src = src.replace("[syntax]", "bullet_2 = \"green\"\n[syntax]");
        let t = build(&file(&src), "t").unwrap();
        assert_eq!(t.bullets, [t.accent, Color::Green, t.accent]);

        let bad = src.replace("heading_3 = \"red\"", "heading_3 = \"nope\"");
        let err = build(&file(&bad), "t").unwrap_err().to_string();
        assert!(err.contains("`ui.heading_3` = \"nope\""), "{err}");
    }

    fn theme_dir(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (name, src) in files {
            std::fs::write(dir.path().join(format!("{name}.toml")), src).unwrap();
        }
        dir
    }

    #[test]
    fn user_themes_extend_and_override() {
        let dir = theme_dir(&[
            ("mine", "extends = \"nord\"\n[ui]\naccent = \"#ff0000\"\n"),
            // Same name as a built-in: extends the built-in underneath.
            (
                "dracula",
                "extends = \"dracula\"\n[syntax]\nstring = \"#00ff00\"\n",
            ),
        ]);
        let d = Some(dir.path());
        let nord = load("nord", None).unwrap();
        let mine = load("mine", d).unwrap();
        assert_eq!(mine.accent, Color::Rgb(255, 0, 0));
        assert_eq!(mine.text, nord.text);
        assert_eq!(mine.appearance, Appearance::Dark);
        assert!(exists("mine", d) && !exists("mine", None));

        let tweaked = load("dracula", d).unwrap();
        assert_eq!(tweaked.accent, load("dracula", None).unwrap().accent);

        let rows = list(d);
        assert_eq!(rows.len(), BUILTINS.len() + 1);
        let row = |n: &str| rows.iter().find(|r| r.name == n).unwrap();
        assert!(row("dracula").overrides && row("dracula").path.is_some());
        assert!(!row("mine").overrides && row("mine").theme.is_ok());
        assert!(row("nord").path.is_none());
    }

    #[test]
    fn broken_user_themes_say_why() {
        let dir = theme_dir(&[
            ("a", "extends = \"b\"\n"),
            ("b", "extends = \"a\"\n"),
            (
                "partial",
                "appearance = \"dark\"\n[ui]\ntext = \"#ffffff\"\n",
            ),
            ("typo", "extend = \"nord\"\n"),
            ("orphan", "extends = \"missing\"\n"),
        ]);
        let d = Some(dir.path());
        let err = |n: &str| format!("{:#}", load(n, d).unwrap_err());
        assert!(err("a").contains("loops: a → b → a"), "{}", err("a"));
        assert!(
            err("partial").contains("is missing ui.muted"),
            "{}",
            err("partial")
        );
        assert!(
            err("typo").contains("unknown field `extend`"),
            "{}",
            err("typo")
        );
        assert!(
            err("orphan").contains("unknown theme \"missing\""),
            "{}",
            err("orphan")
        );
        // Listing still shows them, with the error.
        assert!(
            list(d)
                .iter()
                .find(|r| r.name == "a")
                .unwrap()
                .theme
                .is_err()
        );
        // Names that aren't plain file names never reach the filesystem.
        assert!(!exists("../a", d) && !exists(".a", d));
    }
}
