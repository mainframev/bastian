use clap::ValueEnum;

use super::{ColorMode, Confirm, ListLayout, PageBackground, Selector, SortBy};

/// A setting's value type: how it reads from the environment and prints
/// in `--print-config`.
pub(super) trait Value: Sized {
    fn parse_env(v: &str) -> Option<Self>;
    /// What a valid value looks like, for error messages.
    fn expected() -> String;
    fn toml(&self) -> String;
}

impl Value for bool {
    fn parse_env(v: &str) -> Option<Self> {
        match v.to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        }
    }
    fn expected() -> String {
        "true or false".into()
    }
    fn toml(&self) -> String {
        self.to_string()
    }
}

impl Value for String {
    fn parse_env(v: &str) -> Option<Self> {
        Some(v.to_string())
    }
    fn expected() -> String {
        "a theme name".into()
    }
    fn toml(&self) -> String {
        format!("{self:?}")
    }
}

/// Lists: comma-separated in the environment (`BASTIAN_EXTENSIONS=md,txt`).
impl Value for Vec<String> {
    fn parse_env(v: &str) -> Option<Self> {
        Some(
            v.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        )
    }
    fn expected() -> String {
        "a comma-separated list".into()
    }
    fn toml(&self) -> String {
        format!("{self:?}")
    }
}

impl Value for u16 {
    fn parse_env(v: &str) -> Option<Self> {
        v.parse().ok()
    }
    fn expected() -> String {
        "a whole number".into()
    }
    fn toml(&self) -> String {
        self.to_string()
    }
}

pub(super) fn enum_name<T: ValueEnum>(v: &T) -> String {
    v.to_possible_value()
        .map_or_else(String::new, |p| p.get_name().to_string())
}

macro_rules! enum_value {
    ($($t:ty),*) => {$(
        impl Value for $t {
            fn parse_env(v: &str) -> Option<Self> {
                <$t as ValueEnum>::from_str(v, true).ok()
            }
            fn expected() -> String {
                let names: Vec<String> = <$t as ValueEnum>::value_variants()
                    .iter()
                    .map(enum_name)
                    .collect();
                match names.split_last() {
                    Some((last, [])) => last.clone(),
                    Some((last, rest)) => format!("{} or {last}", rest.join(", ")),
                    None => String::new(),
                }
            }
            fn toml(&self) -> String {
                format!("{:?}", enum_name(self))
            }
        }
    )*};
}
enum_value!(
    PageBackground,
    Confirm,
    ColorMode,
    Selector,
    ListLayout,
    SortBy
);
