use std::path::Path;

use super::value::Value;
use super::{Resolved, Setting, Source};

/// The resolved settings as TOML, each value annotated with where it came
/// from, for `--print-config`.
pub fn describe(r: &Resolved, file: Option<&Path>) -> String {
    const ALIGN_MAX: usize = 40;
    fn src(s: Source) -> &'static str {
        match s {
            Source::Default => "default",
            Source::File => "config file",
            Source::Env => "environment",
            Source::Flag => "flag",
        }
    }
    fn row<T: Value>(key: &str, s: &Setting<T>) -> (String, Source) {
        (format!("{key} = {}", s.value.toml()), s.source)
    }
    let mut out = String::new();
    if let Some(f) = file {
        out.push_str(&format!("# config file: {}\n", f.display()));
    }
    let rows = [
        row("theme", &r.theme),
        row("dark_theme", &r.dark_theme),
        row("light_theme", &r.light_theme),
        row("width", &r.width),
        row("confirm", &r.confirm),
        row("line_numbers", &r.line_numbers),
        row("color", &r.color),
        row("pager", &r.pager),
        row("all", &r.all),
        row("mouse", &r.mouse),
        row("logo", &r.logo),
        row("images", &r.images),
        row("selector", &r.selector),
        row("list_layout", &r.list_layout),
        row("sort", &r.sort),
        row("dates", &r.dates),
        row("date_format", &r.date_format),
        row("extensions", &r.extensions),
        row("ignore", &r.ignore),
        row("image_height", &r.image_height),
        row("tab_width", &r.tab_width),
        row("hyperlinks", &r.hyperlinks),
        row("scrollbar", &r.scrollbar),
        row("remember_position", &r.remember_position),
        row("editor", &r.editor),
    ];
    // Line the source comments up, at least two spaces after the value;
    // a long list doesn't push every other comment out.
    let w = rows
        .iter()
        .map(|(text, _)| text.len())
        .filter(|&len| len <= ALIGN_MAX)
        .max()
        .unwrap_or(0)
        + 2;
    for (text, source) in rows {
        let pad = w.saturating_sub(text.len()).max(2);
        out.push_str(&format!("{text}{:pad$}# {}\n", "", src(source)));
    }
    if !r.interpreters.is_empty() {
        out.push_str("\n[interpreters]\n");
        let mut langs: Vec<_> = r.interpreters.iter().collect();
        langs.sort();
        for (lang, cmd) in langs {
            out.push_str(&format!("{lang} = {cmd:?}\n"));
        }
    }
    out
}
