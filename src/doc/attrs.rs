use std::time::Duration;

/// Options from a fence info string, e.g. ```` ```sh {name=deploy cwd=./infra confirm} ````.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Attrs {
    pub name: Option<String>,
    /// Working directory, relative to the document's directory.
    pub cwd: Option<String>,
    pub env: Vec<(String, String)>,
    /// Always ask before running, even with `--yes`.
    pub confirm: bool,
    /// Never run this block.
    pub norun: bool,
    pub timeout: Option<Duration>,
    /// Keys bastian doesn't know, kept so they can be reported.
    pub unknown: Vec<String>,
}

impl Attrs {
    /// Parse the `{...}` part of an info string. Missing braces mean no attributes.
    pub fn parse(info: &str) -> Self {
        let mut attrs = Self::default();
        let (Some(open), Some(close)) = (info.find('{'), info.rfind('}')) else {
            return attrs;
        };
        if close <= open {
            return attrs;
        }
        for token in tokens(&info[open + 1..close]) {
            let (key, value) = match token.split_once('=') {
                Some((k, v)) => (k.trim(), Some(v.to_string())),
                None => (token.trim(), None),
            };
            match (key, value) {
                ("name", Some(v)) => attrs.name = Some(v),
                ("cwd", Some(v)) => attrs.cwd = Some(v),
                ("env", Some(v)) => match v.split_once('=') {
                    Some((k, v)) => attrs.env.push((k.to_string(), v.to_string())),
                    None => attrs.unknown.push(format!("env={v}")),
                },
                ("timeout", Some(v)) => match parse_duration(&v) {
                    Some(d) => attrs.timeout = Some(d),
                    None => attrs.unknown.push(format!("timeout={v}")),
                },
                ("confirm", None) => attrs.confirm = true,
                ("norun", None) => attrs.norun = true,
                ("confirm" | "norun", Some(v)) => {
                    let on = matches!(v.as_str(), "true" | "yes" | "1");
                    if key == "confirm" {
                        attrs.confirm = on;
                    } else {
                        attrs.norun = on;
                    }
                }
                (k, _) => attrs.unknown.push(k.to_string()),
            }
        }
        attrs
    }
}

/// Split on whitespace and commas, keeping double-quoted values together
/// and removing the quotes: `name="two words"` → `name=two words`.
fn tokens(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in s.chars() {
        match c {
            '"' => quoted = !quoted,
            c if !quoted && (c.is_whitespace() || c == ',') => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// `30s`, `5m`, `1h`, `500ms`, or bare seconds.
fn parse_duration(s: &str) -> Option<Duration> {
    let s = s.trim();
    let split = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    let (num, unit) = s.split_at(split);
    let n: u64 = num.parse().ok()?;
    Some(match unit {
        "" | "s" => Duration::from_secs(n),
        "ms" => Duration::from_millis(n),
        "m" => Duration::from_secs(n * 60),
        "h" => Duration::from_secs(n * 3600),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_braces() {
        assert_eq!(Attrs::parse("sh"), Attrs::default());
    }

    #[test]
    fn all_keys() {
        let a = Attrs::parse(
            r#"sh {name="deploy app" cwd=./infra confirm env=A=1 env=B=x=y timeout=2m}"#,
        );
        assert_eq!(a.name.as_deref(), Some("deploy app"));
        assert_eq!(a.cwd.as_deref(), Some("./infra"));
        assert!(a.confirm);
        assert!(!a.norun);
        assert_eq!(
            a.env,
            [("A".into(), "1".into()), ("B".into(), "x=y".into())]
        );
        assert_eq!(a.timeout, Some(Duration::from_secs(120)));
        assert!(a.unknown.is_empty());
    }

    #[test]
    fn commas_flags_and_unknown() {
        let a = Attrs::parse("sh {norun, interactive=false, timeout=soon}");
        assert!(a.norun);
        assert_eq!(a.unknown, ["interactive", "timeout=soon"]);
    }

    #[test]
    fn explicit_bool_values() {
        assert!(!Attrs::parse("sh {confirm=false}").confirm);
        assert!(Attrs::parse("sh {norun=true}").norun);
    }

    #[test]
    fn durations() {
        assert_eq!(parse_duration("500ms"), Some(Duration::from_millis(500)));
        assert_eq!(parse_duration("10"), Some(Duration::from_secs(10)));
        assert_eq!(parse_duration("1h"), Some(Duration::from_secs(3600)));
        assert_eq!(parse_duration("x"), None);
    }
}
