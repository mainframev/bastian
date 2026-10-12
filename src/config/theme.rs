use std::path::Path;

use anyhow::{Context, Result, anyhow};

use super::{Config, Resolved, Source, THEME_MODES, env_var};
use crate::paths;

/// Write `theme = "<name>"` into the config file at `path`, replacing a
/// top-level `theme` line if there is one and keeping everything else
/// (comments, other keys, tables) as it is. Creates the file if needed.
pub fn save_theme(path: &Path, name: &str) -> Result<()> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    };
    let updated = with_theme(&text, name);
    // Never write something that no longer parses.
    toml::from_str::<Config>(&updated)
        .with_context(|| format!("updating {} would break it", path.display()))?;
    paths::write_creating_parent(path, updated)
}

/// `text` with its top-level `theme` set to `name`.
pub(super) fn with_theme(text: &str, name: &str) -> String {
    let line = format!("theme = {name:?}");
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    // Top-level keys come before the first `[table]`.
    let top = lines
        .iter()
        .position(|l| l.trim_start().starts_with('['))
        .unwrap_or(lines.len());
    let existing = lines[..top].iter().position(|l| {
        let l = l.trim_start();
        l.strip_prefix("theme")
            .is_some_and(|rest| rest.trim_start().starts_with('='))
    });
    if let Some(i) = existing {
        // Keep a trailing comment on the line.
        let comment = lines[i]
            .find('#')
            .map(|h| lines[i][lines[i][..h].trim_end().len()..].to_string());
        lines[i] = format!("{line}{}", comment.unwrap_or_default());
    } else {
        // After any leading comment block, before the first key or table.
        let at = lines[..top]
            .iter()
            .position(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty())
            .unwrap_or(top);
        lines.insert(at, line);
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// Name of the theme to use. `auto` follows the terminal background
/// (`light_background`, unknown counts as dark); `dark` and `light` pick
/// `dark_theme` and `light_theme`.
pub fn theme_name(r: &Resolved, light_background: bool) -> &str {
    match r.theme.value.as_str() {
        "auto" if light_background => &r.light_theme.value,
        "auto" | "dark" => &r.dark_theme.value,
        "light" => &r.light_theme.value,
        name => name,
    }
}

/// Every theme the settings name exists; the error says where a bad name
/// came from.
pub fn check_themes(r: &Resolved, exists: impl Fn(&str) -> bool) -> Result<()> {
    let settings = [
        ("theme", &r.theme),
        ("dark_theme", &r.dark_theme),
        ("light_theme", &r.light_theme),
    ];
    for (key, s) in settings {
        if (key == "theme" && THEME_MODES.contains(&s.value.as_str())) || exists(&s.value) {
            continue;
        }
        let from = match s.source {
            Source::Flag => "--theme".to_string(),
            Source::Env => env_var(key),
            Source::File => format!("`{key}` in the config file"),
            Source::Default => "the default".to_string(),
        };
        return Err(anyhow!(
            "unknown theme {:?} (from {from}); run `bastian themes` to list them",
            s.value
        ));
    }
    Ok(())
}
