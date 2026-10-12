macro_rules! builtins {
    ($($name:literal),* $(,)?) => {
        &[$(($name, include_str!(concat!("../../themes/", $name, ".toml")))),*]
    };
}

/// Built-in themes: name and TOML source.
pub(super) const BUILTINS: &[(&str, &str)] = builtins![
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
pub(super) const ALIASES: &[(&str, &str)] = &[
    ("tokyonight", "tokyonight-night"),
    ("kanagawa", "kanagawa-wave"),
    ("catppuccin", "catppuccin-mocha"),
    ("gruvbox", "gruvbox-dark"),
    ("everforest", "everforest-dark"),
    ("solarized", "solarized-dark"),
];

/// Names of the built-in themes, in display order.
pub fn builtin_names() -> impl Iterator<Item = &'static str> {
    BUILTINS.iter().map(|(name, _)| *name)
}

/// The built-in theme `name` stands for, following aliases.
pub(super) fn alias(name: &str) -> &str {
    ALIASES
        .iter()
        .find(|(alias, _)| *alias == name)
        .map_or(name, |(_, target)| target)
}

pub(super) fn builtin(name: &str) -> Option<&'static str> {
    let name = alias(name);
    BUILTINS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, src)| *src)
}
