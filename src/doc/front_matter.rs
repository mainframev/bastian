//! Front matter: the YAML (`---`) or TOML (`+++`) block a document may
//! start with, shown as a two-column table of its keys and values. Both
//! front ends hand the block's text here, so they show it the same way.

use super::{Align, Block, Inline, InlineStyle};

/// Which syntax the front matter is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Yaml,
    Toml,
}

/// The block showing front matter `src` (its text between the fences): a
/// key/value table, or the text as a code block when it can't be read as
/// one. `None` when there is nothing in it.
pub(super) fn block(kind: Kind, src: &str) -> Option<Block> {
    if src.trim().is_empty() {
        return None;
    }
    let map = match kind {
        Kind::Yaml => yaml(src),
        Kind::Toml => toml(src),
    };
    let Some(map) = map else {
        let lang = match kind {
            Kind::Yaml => "yaml",
            Kind::Toml => "toml",
        };
        let mut src = src.to_string();
        if !src.ends_with('\n') {
            src.push('\n');
        }
        return Some(super::code_block(lang, src));
    };
    let mut rows = Vec::new();
    flatten("", map, &mut rows);
    if rows.is_empty() {
        return None;
    }
    let cell = |text: String| {
        if text.is_empty() {
            Vec::new()
        } else {
            vec![Inline {
                text,
                style: InlineStyle::default(),
            }]
        }
    };
    Some(Block::Table {
        align: vec![Align::None, Align::None],
        header: vec![cell("Key".into()), cell("Value".into())],
        rows: rows
            .into_iter()
            .map(|(k, v)| vec![cell(k), cell(v)])
            .collect(),
    })
}

/// A front matter value, as either syntax reads.
#[derive(Debug, PartialEq)]
enum Value {
    Text(String),
    List(Vec<Value>),
    Map(Vec<(String, Value)>),
}

/// One row per scalar, list or empty map, keys in source order: nested
/// maps give `key.sub` rows.
fn flatten(prefix: &str, map: Vec<(String, Value)>, out: &mut Vec<(String, String)>) {
    for (key, value) in map {
        let key = if prefix.is_empty() {
            key
        } else {
            format!("{prefix}.{key}")
        };
        match value {
            Value::Map(inner) if !inner.is_empty() => flatten(&key, inner, out),
            Value::List(items) => {
                let items: Vec<String> = items.iter().map(inline).collect();
                out.push((key, items.join(", ")));
            }
            other => out.push((key, inline(&other))),
        }
    }
}

/// A value on one line: a list in a list as `[a, b]`, a map in a list as
/// `{k: v}`.
fn inline(value: &Value) -> String {
    match value {
        Value::Text(t) => t.clone(),
        Value::List(items) => {
            let items: Vec<String> = items.iter().map(inline).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Map(entries) => {
            let entries: Vec<String> = entries
                .iter()
                .map(|(k, v)| format!("{k}: {}", inline(v)))
                .collect();
            format!("{{{}}}", entries.join(", "))
        }
    }
}

/// TOML front matter, keys in the order they're written.
fn toml(src: &str) -> Option<Vec<(String, Value)>> {
    use ::toml::de::{DeTable, DeValue};

    fn table(t: &DeTable) -> Vec<(String, Value)> {
        let mut entries: Vec<_> = t.iter().collect();
        entries.sort_by_key(|(k, _)| k.span().start);
        entries
            .into_iter()
            .map(|(k, v)| (k.get_ref().to_string(), value(v.get_ref())))
            .collect()
    }
    fn value(v: &DeValue) -> Value {
        match v {
            DeValue::String(s) => Value::Text(s.to_string()),
            DeValue::Integer(i) => Value::Text(i.to_string()),
            DeValue::Float(f) => Value::Text(f.to_string()),
            DeValue::Boolean(b) => Value::Text(b.to_string()),
            DeValue::Datetime(d) => Value::Text(d.to_string()),
            DeValue::Array(a) => Value::List(a.iter().map(|i| value(i.get_ref())).collect()),
            DeValue::Table(t) => Value::Map(table(t)),
        }
    }
    DeTable::parse(src).ok().map(|t| table(t.get_ref()))
}

/// YAML front matter, read by a small line-based reader: `key: value`
/// pairs, maps nested by indentation, `- item` and `[a, b]` lists, quoted
/// and block scalars. `None` for anything else, such as a list of maps.
fn yaml(src: &str) -> Option<Vec<(String, Value)>> {
    let mut lines = Vec::new();
    for raw in src.lines() {
        let text = raw.trim();
        if text.is_empty() || text.starts_with('#') {
            continue;
        }
        let lead = &raw[..raw.len() - raw.trim_start().len()];
        if lead.contains('\t') {
            return None;
        }
        lines.push(YamlLine {
            indent: lead.len(),
            text,
        });
    }
    let Some(indent) = lines.first().map(|l| l.indent) else {
        return Some(Vec::new());
    };
    let mut y = Yaml { lines, at: 0 };
    let map = y.map(indent)?;
    (y.at == y.lines.len()).then_some(map)
}

/// A YAML line that holds something, trimmed, with its indent.
#[derive(Clone, Copy)]
struct YamlLine<'a> {
    indent: usize,
    text: &'a str,
}

struct Yaml<'a> {
    lines: Vec<YamlLine<'a>>,
    /// The next line to read.
    at: usize,
}

impl<'a> Yaml<'a> {
    fn peek(&self) -> Option<YamlLine<'a>> {
        self.lines.get(self.at).copied()
    }

    /// The `key: value` pairs at `indent`, up to a line indented less.
    fn map(&mut self, indent: usize) -> Option<Vec<(String, Value)>> {
        let mut out = Vec::new();
        while let Some(line) = self.peek() {
            if line.indent < indent {
                break;
            }
            if line.indent > indent || is_item(line.text) {
                return None;
            }
            let (key, rest) = entry(line.text)?;
            self.at += 1;
            let value = if rest.is_empty() {
                self.nested(indent)?
            } else if is_block_scalar(rest) {
                self.block_scalar(indent)
            } else {
                scalar(rest)?
            };
            out.push((key, value));
        }
        Some(out)
    }

    /// What a key with nothing after its colon holds: a list (which YAML
    /// allows at the key's own indent), a map indented under it, or
    /// nothing.
    fn nested(&mut self, indent: usize) -> Option<Value> {
        match self.peek() {
            Some(l) if is_item(l.text) && l.indent >= indent => {
                let i = l.indent;
                self.list(i)
            }
            Some(l) if l.indent > indent => {
                let i = l.indent;
                self.map(i).map(Value::Map)
            }
            _ => Some(Value::Text(String::new())),
        }
    }

    /// `- item` lines at `indent`. Items must be scalars.
    fn list(&mut self, indent: usize) -> Option<Value> {
        let mut items = Vec::new();
        while let Some(l) = self.peek() {
            if l.indent != indent || !is_item(l.text) {
                break;
            }
            let item = l.text[1..].trim_start();
            if item.is_empty() || is_item(item) || entry(item).is_some() {
                return None;
            }
            items.push(scalar(item)?);
            self.at += 1;
        }
        Some(Value::List(items))
    }

    /// The lines of a `|` or `>` scalar, joined onto one line.
    fn block_scalar(&mut self, indent: usize) -> Value {
        let mut parts = Vec::new();
        while let Some(l) = self.peek()
            && l.indent > indent
        {
            parts.push(l.text);
            self.at += 1;
        }
        Value::Text(parts.join(" "))
    }
}

/// A list item line: `- x`, or a bare `-`.
fn is_item(text: &str) -> bool {
    text == "-" || text.starts_with("- ")
}

/// `|`, `>`, and their chomping and indentation indicators.
fn is_block_scalar(text: &str) -> bool {
    let mut chars = text.chars();
    matches!(chars.next(), Some('|' | '>'))
        && chars.all(|c| c == '+' || c == '-' || c.is_ascii_digit())
}

/// A `key: rest` line split at the colon, the key unquoted.
fn entry(text: &str) -> Option<(String, &str)> {
    if let Some(q @ ('"' | '\'')) = text.chars().next() {
        let close = text[1..].find(q)? + 1;
        let rest = text[close + 1..].strip_prefix(':')?;
        if !(rest.is_empty() || rest.starts_with(' ')) {
            return None;
        }
        return Some((text[1..close].to_string(), rest.trim()));
    }
    if text.starts_with(['[', '{', '-', '?', '&', '*', '!', '|', '>']) {
        return None;
    }
    let colon = text
        .match_indices(':')
        .map(|(i, _)| i)
        .find(|&i| matches!(text.as_bytes().get(i + 1), None | Some(b' ')))?;
    let key = text[..colon].trim();
    (!key.is_empty()).then(|| (key.to_string(), text[colon + 1..].trim()))
}

/// A value written on its key's line: quoted, a `[a, b]` list, or plain
/// up to a ` #` comment. `None` for a quote that doesn't close on the line.
fn scalar(text: &str) -> Option<Value> {
    if let Some(inner) = text.strip_prefix('"') {
        let close = closing_double_quote(inner)?;
        let s = inner[..close].replace("\\\"", "\"").replace("\\\\", "\\");
        return Some(Value::Text(s));
    }
    if let Some(inner) = text.strip_prefix('\'') {
        let close = closing_single_quote(inner)?;
        return Some(Value::Text(inner[..close].replace("''", "'")));
    }
    let text = text.find(" #").map_or(text, |i| &text[..i]).trim_end();
    if let Some(inner) = text.strip_prefix('[') {
        let inner = inner.strip_suffix(']')?;
        if inner.contains(['[', '{']) {
            return Some(Value::Text(text.to_string()));
        }
        let items = inner
            .split(',')
            .map(str::trim)
            .filter(|i| !i.is_empty())
            .map(scalar)
            .collect::<Option<_>>()?;
        return Some(Value::List(items));
    }
    Some(Value::Text(text.to_string()))
}

/// Where a `"`-quoted string closes, skipping escaped quotes.
fn closing_double_quote(s: &str) -> Option<usize> {
    let mut escaped = false;
    for (i, c) in s.char_indices() {
        match c {
            '\\' if !escaped => escaped = true,
            '"' if !escaped => return Some(i),
            _ => escaped = false,
        }
    }
    None
}

/// Where a `'`-quoted string closes; `''` is a quote inside it.
fn closing_single_quote(s: &str) -> Option<usize> {
    let mut chars = s.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c == '\'' {
            if chars.peek().is_some_and(|&(_, n)| n == '\'') {
                chars.next();
            } else {
                return Some(i);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::plain_text;

    /// The table's rows as `(key, value)` text.
    fn rows(kind: Kind, src: &str) -> Vec<(String, String)> {
        let Some(Block::Table { header, rows, .. }) = block(kind, src) else {
            panic!("expected a table for {src:?}");
        };
        assert_eq!(plain_text(&header[0]), "Key");
        rows.iter()
            .map(|r| (plain_text(&r[0]), plain_text(&r[1])))
            .collect()
    }

    fn pairs(p: &[(&str, &str)]) -> Vec<(String, String)> {
        p.iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn yaml_scalars_keep_their_order() {
        let src = "title: \"A: guide\"\nauthor: 'O''Brien'\ndraft: false\n\
                   url: https://x.dev/a # where\nempty:\n";
        assert_eq!(
            rows(Kind::Yaml, src),
            pairs(&[
                ("title", "A: guide"),
                ("author", "O'Brien"),
                ("draft", "false"),
                ("url", "https://x.dev/a"),
                ("empty", ""),
            ])
        );
    }

    #[test]
    fn yaml_lists_are_joined() {
        let src = "tags: [rust, \"cli\"]\nauthors:\n  - Ann\n  - Bob\nflat:\n- x\n- y\nlast: z\n";
        assert_eq!(
            rows(Kind::Yaml, src),
            pairs(&[
                ("tags", "rust, cli"),
                ("authors", "Ann, Bob"),
                ("flat", "x, y"),
                ("last", "z"),
            ])
        );
    }

    #[test]
    fn yaml_nested_maps_become_dotted_keys() {
        let src = "seo:\n  title: T\n  og:\n    image: a.png\n# note\n\nsummary: >\n  one\n  two\n";
        assert_eq!(
            rows(Kind::Yaml, src),
            pairs(&[
                ("seo.title", "T"),
                ("seo.og.image", "a.png"),
                ("summary", "one two"),
            ])
        );
    }

    #[test]
    fn toml_reads_in_source_order() {
        let src = "title = \"T\"\nweight = 3\ntags = [\"a\", \"b\"]\n\
                   date = 2026-10-16\n[extra]\nmath = true\nitems = [{ n = 1 }]\n";
        assert_eq!(
            rows(Kind::Toml, src),
            pairs(&[
                ("title", "T"),
                ("weight", "3"),
                ("tags", "a, b"),
                ("date", "2026-10-16"),
                ("extra.math", "true"),
                ("extra.items", "{n: 1}"),
            ])
        );
    }

    #[test]
    fn unreadable_front_matter_shows_as_code() {
        for (kind, src, lang) in [
            (Kind::Yaml, "authors:\n  - name: Ann\n    url: x\n", "yaml"),
            (Kind::Yaml, "title: \"open\n", "yaml"),
            (Kind::Yaml, "- just\n- a list\n", "yaml"),
            (Kind::Toml, "title = \n", "toml"),
        ] {
            let Some(Block::Code(c)) = block(kind, src) else {
                panic!("expected code for {src:?}");
            };
            assert_eq!(c.lang.as_deref(), Some(lang));
            assert_eq!(c.src, src);
        }
    }

    #[test]
    fn empty_front_matter_shows_nothing() {
        assert_eq!(block(Kind::Yaml, "\n"), None);
        assert_eq!(block(Kind::Yaml, "# only a comment\n"), None);
        assert_eq!(block(Kind::Toml, ""), None);
    }
}
