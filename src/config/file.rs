use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

use super::Config;
use crate::paths;

pub fn path() -> Option<PathBuf> {
    let base = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => PathBuf::from(std::env::var_os("HOME")?).join(".config"),
    };
    Some(base.join("bastian").join("config.toml"))
}

/// Which config file to use: `--config`, else `BASTIAN_CONFIG`, else the
/// default location. Returns the path and whether it was given explicitly
/// (an explicit file must exist). `~` and `$VAR` are expanded in both.
pub fn locate(
    flag: Option<&str>,
    env_path: Option<&str>,
    lookup: &dyn Fn(&str) -> Option<String>,
) -> Option<(PathBuf, bool)> {
    match flag.or(env_path.filter(|p| !p.is_empty())) {
        Some(p) => Some((paths::expand(p, lookup), true)),
        None => path().map(|p| (p, false)),
    }
}

/// Load the located config file. A missing default file is an empty
/// config; a missing explicit one is an error.
pub fn load_at(path: &Path, explicit: bool) -> Result<Config> {
    if path.exists() {
        load_from(path)
    } else if explicit {
        Err(anyhow!("config file {} does not exist", path.display()))
    } else {
        Ok(Config::default())
    }
}

pub fn load_from(path: &Path) -> Result<Config> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// Commented starting point written by `bastian config`.
pub const TEMPLATE: &str = r#"# bastian configuration. Every key is optional; command-line flags and
# BASTIAN_* environment variables take precedence over this file.

# theme = "auto"        # "auto" (match the terminal), "dark", "light" or a
#                       # theme name; `bastian themes` lists them, and your
#                       # own go in themes/<name>.toml next to this file
# dark_theme = "rose-pine"        # used by "auto" and "dark"
# light_theme = "rose-pine-dawn"  # used by "auto" and "light"
# width = 100           # maximum document width
# confirm = "always"    # "never" runs code blocks without asking, like --yes
# line_numbers = false  # show line numbers in the reader
# color = "auto"        # printed output: "auto", "always" or "never"
# pager = false         # show documents in $PAGER instead of the TUI
# all = false           # also list hidden and gitignored files
# mouse = false         # scroll with the mouse wheel
# logo = true           # show the logo at the top of the file browser
# images = true         # show pictures for images in documents (`i` toggles)
# selector = "bar"      # marks the selected file: "bar", "thick", "block",
#                       # "arrow" or "highlight"
# list_layout = "cards"  # "cards" (title, path, date) or "compact" (one row)
# sort = "modified"     # browser order: "modified", "created" or "name"
# dates = true          # show file dates in the browser
# date_format = "%d %b %Y %H:%M %Z"  # strftime format of those dates
# extensions = ["md", "markdown", "mdx", "mdown", "mkdn", "mkd"]
# ignore = ["node_modules", "CHANGELOG.md"]  # gitignore-style globs to leave out
# image_height = 24     # rows a picture takes up at most
# tab_width = 4         # columns per tab in code
# hyperlinks = true     # clickable links (turn off if your terminal shows
#                       # escape codes instead)
# scrollbar = true      # show scrollbars
# remember_position = true  # reopen files where you left off
# editor = "nvim"       # used by `e`/`ctrl-e`, before $VISUAL and $EDITOR

# [interpreters]
# The block source is appended as the last argument.
# ruby = ["ruby", "-e"]
# fish = []             # an empty list stops a language from running
"#;
