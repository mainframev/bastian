use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::Appearance;

/// A theme file as written: every section is a table of color strings.
/// With `extends`, keys left out come from that theme.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ThemeFile {
    pub(super) extends: Option<String>,
    pub(super) appearance: Option<Appearance>,
    #[serde(default)]
    pub(super) ui: BTreeMap<String, String>,
    #[serde(default)]
    pub(super) syntax: BTreeMap<String, String>,
}

impl ThemeFile {
    /// `self` on top of `base`.
    pub(super) fn over(self, base: ThemeFile) -> ThemeFile {
        let mut ui = base.ui;
        ui.extend(self.ui);
        let mut syntax = base.syntax;
        syntax.extend(self.syntax);
        ThemeFile {
            extends: None,
            appearance: self.appearance.or(base.appearance),
            ui,
            syntax,
        }
    }
}

/// `<dir>/<name>.toml`, for names that are plain file names.
pub(super) fn user_file(name: &str, dir: Option<&Path>) -> Option<PathBuf> {
    let plain = !name.is_empty()
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    let path = dir?.join(format!("{name}.toml"));
    (plain && path.is_file()).then_some(path)
}
