//! TeX math to readable Unicode: `\alpha^2 \leq \frac{a+b}{2}` reads as
//! `α² ≤ (a+b)/2`. A small table and a tiny recursive parser, not a
//! typesetter: it knows the common symbols, scripts, fractions and roots,
//! drops layout-only commands like `\left` and `\mathrm`, and keeps any
//! command it doesn't know as source text, so nothing is lost.

/// Deepest nesting of groups and command arguments converted. Past it the
/// rest of the source stays as written, so pathological input like
/// `\sqrt\sqrt\sqrt…` can't exhaust the stack.
const MAX_DEPTH: usize = 32;

/// `tex` as Unicode text: one line per `\\` row, runs of spaces made one.
pub(crate) fn to_unicode(tex: &str) -> String {
    let mut p = Parser {
        chars: tex.chars().collect(),
        pos: 0,
        depth: 0,
    };
    let out = p.seq();
    out.split('\n')
        .map(|row| row.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|row| !row.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// `tex` as Unicode text on one line, for math inside a paragraph.
pub(crate) fn inline(tex: &str) -> String {
    to_unicode(tex).replace('\n', " ")
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
    /// Groups and command arguments open around the current position.
    depth: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    /// Atoms up to the `}` closing the current group (consumed) or the end.
    fn seq(&mut self) -> String {
        let mut out = String::new();
        while let Some(c) = self.peek() {
            if c == '}' && self.depth > 0 {
                self.pos += 1;
                break;
            }
            // `&` aligns columns and `~` is a fixed space: both read as one.
            if c.is_whitespace() || c == '&' || c == '~' {
                self.pos += 1;
                push_space(&mut out);
                continue;
            }
            if c == '^' || c == '_' {
                self.pos += 1;
                let arg = self.arg();
                out.push_str(&script(&arg, c == '^'));
                continue;
            }
            let atom = self.atom();
            out.push_str(&atom);
        }
        out
    }

    /// One atom: a group, a command or a character.
    fn atom(&mut self) -> String {
        let Some(c) = self.peek() else {
            return String::new();
        };
        self.pos += 1;
        match c {
            '{' => self.group(),
            '\\' => self.command(),
            '}' => "}".into(),
            '\'' => "′".into(),
            c => c.to_string(),
        }
    }

    /// A group's converted content; the `{` is already consumed.
    fn group(&mut self) -> String {
        if self.depth >= MAX_DEPTH {
            return format!("{{{}", self.rest());
        }
        self.depth += 1;
        let inner = self.seq();
        self.depth -= 1;
        inner
    }

    /// A command's argument: the next atom, past any spaces.
    fn arg(&mut self) -> String {
        self.skip_spaces();
        match self.peek() {
            None | Some('}') => String::new(),
            Some(_) if self.depth >= MAX_DEPTH => self.rest(),
            Some(_) => {
                self.depth += 1;
                let arg = self.atom();
                self.depth -= 1;
                arg
            }
        }
    }

    /// The source from here to the end, as written.
    fn rest(&mut self) -> String {
        let rest = self.chars[self.pos..].iter().collect();
        self.pos = self.chars.len();
        rest
    }

    /// A text argument, `{...}` taken as written apart from nested braces.
    fn text_arg(&mut self) -> String {
        self.skip_spaces();
        if self.peek() != Some('{') {
            return self.arg();
        }
        self.pos += 1;
        let mut out = String::new();
        let mut open = 1usize;
        while let Some(c) = self.peek() {
            self.pos += 1;
            match c {
                '{' => open += 1,
                '}' => {
                    open -= 1;
                    if open == 0 {
                        break;
                    }
                }
                '\\' => {
                    if let Some(next) = self.peek() {
                        self.pos += 1;
                        out.push(next);
                    }
                }
                c => out.push(c),
            }
        }
        out
    }

    /// An optional `[...]` argument, as written.
    fn optional(&mut self) -> Option<String> {
        self.skip_spaces();
        if self.peek() != Some('[') {
            return None;
        }
        let close = self.chars[self.pos..].iter().position(|&c| c == ']')?;
        let inner = self.chars[self.pos + 1..self.pos + close].iter().collect();
        self.pos += close + 1;
        Some(inner)
    }

    fn skip_spaces(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.pos += 1;
        }
    }

    /// A command after its `\`: a run of letters, or one other character.
    fn command(&mut self) -> String {
        let start = self.pos;
        while self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
            self.pos += 1;
        }
        if self.pos == start {
            let Some(c) = self.peek() else {
                return "\\".into();
            };
            self.pos += 1;
            return match c {
                '\\' => "\n".into(),
                ',' | ':' | ';' | ' ' | '>' => " ".into(),
                '!' => String::new(),
                '|' => "‖".into(),
                c => c.to_string(),
            };
        }
        let name: String = self.chars[start..self.pos].iter().collect();
        if let Some(s) = symbol(&name) {
            return s.into();
        }
        match name.as_str() {
            "frac" | "dfrac" | "tfrac" | "cfrac" => {
                let num = self.arg();
                let den = self.arg();
                format!("{}/{}", wrap(&num), wrap(&den))
            }
            "sqrt" => {
                let index = self.optional();
                let body = self.arg();
                let root = match index.as_deref().map(str::trim) {
                    None | Some("" | "2") => "√".to_string(),
                    Some("3") => "∛".to_string(),
                    Some("4") => "∜".to_string(),
                    Some(n) => format!("{}√", script(n, true)),
                };
                format!("{root}{}", wrap(&body))
            }
            "mathbb" => self.arg().chars().map(double_struck).collect(),
            "not" => {
                let next = self.arg();
                match next.as_str() {
                    "=" => "≠".into(),
                    "∈" => "∉".into(),
                    "⊂" => "⊄".into(),
                    "⊆" => "⊈".into(),
                    "∃" => "∄".into(),
                    _ => format!("{next}\u{338}"),
                }
            }
            "text" | "textrm" | "textit" | "textbf" | "textsf" | "texttt" | "textnormal"
            | "mbox" | "hbox" | "operatorname" => self.text_arg(),
            "mathrm" | "mathit" | "mathbf" | "mathsf" | "mathtt" | "mathcal" | "mathfrak"
            | "mathscr" | "mathnormal" | "boldsymbol" | "bm" => self.arg(),
            "left" | "right" | "middle" | "big" | "Big" | "bigg" | "Bigg" | "bigl" | "bigr"
            | "Bigl" | "Bigr" | "biggl" | "biggr" | "Biggl" | "Biggr" => self.delimiter(),
            "displaystyle" | "textstyle" | "scriptstyle" | "limits" | "nolimits" | "nonumber"
            | "notag" => String::new(),
            "begin" => {
                // `\begin{array}{cc}` names its columns too.
                if matches!(self.text_arg().as_str(), "array" | "tabular") {
                    self.text_arg();
                }
                String::new()
            }
            "end" | "label" => {
                self.text_arg();
                String::new()
            }
            "quad" | "qquad" => " ".into(),
            "hat" | "widehat" | "bar" | "overline" | "tilde" | "widetilde" | "vec" | "dot"
            | "ddot" | "underline" => match self.arg() {
                body if body.is_empty() => format!("\\{name}"),
                body => accent(&name, &body),
            },
            _ if is_function(&name) => name,
            _ => self.unknown(&name),
        }
    }

    /// The delimiter after `\left`, `\big` and friends, which only size
    /// it; `.` is none.
    fn delimiter(&mut self) -> String {
        self.skip_spaces();
        match self.peek() {
            Some('.') => {
                self.pos += 1;
                String::new()
            }
            Some('{' | '}') | None => String::new(),
            Some(_) => self.atom(),
        }
    }

    /// A command bastian doesn't know, as written: `\name` and its brace
    /// groups, converted inside.
    fn unknown(&mut self, name: &str) -> String {
        let mut out = format!("\\{name}");
        while self.peek() == Some('{') {
            self.pos += 1;
            let inner = self.group();
            out.push('{');
            out.push_str(&inner);
            out.push('}');
        }
        out
    }
}

/// Add one space unless `out` already ends with one (or a row break).
fn push_space(out: &mut String) {
    if !out.is_empty() && !out.ends_with([' ', '\n']) {
        out.push(' ');
    }
}

/// `s` in parentheses when it is more than one term: `2`, `x` and `x²`
/// stand alone; `a+b` and `2x` become `(a+b)` and `(2x)`.
fn wrap(s: &str) -> String {
    let s = s.trim();
    // Scripts and accents belong to the character before them.
    let core: String = s
        .chars()
        .filter(|&c| !is_script_glyph(c) && !is_combining(c))
        .collect();
    let simple = core.chars().count() <= 1
        || core.chars().all(|c| c.is_ascii_digit() || c == '.')
        || (s.starts_with('(') && s.ends_with(')') && balanced_once(s));
    if simple {
        s.to_string()
    } else {
        format!("({s})")
    }
}

/// Whether `s`, which starts with `(` and ends with `)`, is one
/// parenthesized whole rather than `(a)+(b)`.
fn balanced_once(s: &str) -> bool {
    let mut depth = 0i32;
    let last = s.chars().count() - 1;
    for (i, c) in s.chars().enumerate() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        if depth == 0 && i < last {
            return false;
        }
    }
    depth == 0
}

/// `s` as a superscript (`up`) or subscript: Unicode script glyphs when
/// every character has one, else `^x` or `^(…)` as written.
fn script(s: &str, up: bool) -> String {
    let s = s.trim();
    if s.is_empty() {
        return String::new();
    }
    let map = if up { sup } else { sub };
    let glyphs: Option<String> = s.chars().filter(|c| !c.is_whitespace()).map(map).collect();
    if let Some(g) = glyphs {
        return g;
    }
    let mark = if up { '^' } else { '_' };
    if s.chars().count() == 1 {
        format!("{mark}{s}")
    } else {
        format!("{mark}({s})")
    }
}

/// A combining mark, as `accent` adds.
fn is_combining(c: char) -> bool {
    matches!(c, '\u{300}'..='\u{36F}' | '\u{20D0}'..='\u{20FF}')
}

/// A superscript or subscript glyph `sup` or `sub` makes.
fn is_script_glyph(c: char) -> bool {
    const GLYPHS: &str = "⁰¹²³⁴⁵⁶⁷⁸⁹⁺⁻⁼⁽⁾ᵃᵇᶜᵈᵉᶠᵍʰⁱʲᵏˡᵐⁿᵒᵖʳˢᵗᵘᵛʷˣʸᶻᴬᴮᴰᴱᴳᴴᴵᴶᴷᴸᴹᴺᴼᴾᴿᵀᵁⱽᵂᵅᵝᵞᵟᶿᵠᵡ°′\
                          ₀₁₂₃₄₅₆₇₈₉₊₋₌₍₎ₐₑₕᵢⱼₖₗₘₙₒₚᵣₛₜᵤᵥₓᵦᵧᵨᵩᵪ";
    GLYPHS.contains(c)
}

/// The superscript glyph for `c`, if Unicode has one.
fn sup(c: char) -> Option<char> {
    Some(match c {
        '0' => '⁰',
        '1' => '¹',
        '2' => '²',
        '3' => '³',
        '4' => '⁴',
        '5' => '⁵',
        '6' => '⁶',
        '7' => '⁷',
        '8' => '⁸',
        '9' => '⁹',
        '+' => '⁺',
        '-' | '−' => '⁻',
        '=' => '⁼',
        '(' => '⁽',
        ')' => '⁾',
        'a' => 'ᵃ',
        'b' => 'ᵇ',
        'c' => 'ᶜ',
        'd' => 'ᵈ',
        'e' => 'ᵉ',
        'f' => 'ᶠ',
        'g' => 'ᵍ',
        'h' => 'ʰ',
        'i' => 'ⁱ',
        'j' => 'ʲ',
        'k' => 'ᵏ',
        'l' => 'ˡ',
        'm' => 'ᵐ',
        'n' => 'ⁿ',
        'o' => 'ᵒ',
        'p' => 'ᵖ',
        'r' => 'ʳ',
        's' => 'ˢ',
        't' => 'ᵗ',
        'u' => 'ᵘ',
        'v' => 'ᵛ',
        'w' => 'ʷ',
        'x' => 'ˣ',
        'y' => 'ʸ',
        'z' => 'ᶻ',
        'A' => 'ᴬ',
        'B' => 'ᴮ',
        'D' => 'ᴰ',
        'E' => 'ᴱ',
        'G' => 'ᴳ',
        'H' => 'ᴴ',
        'I' => 'ᴵ',
        'J' => 'ᴶ',
        'K' => 'ᴷ',
        'L' => 'ᴸ',
        'M' => 'ᴹ',
        'N' => 'ᴺ',
        'O' => 'ᴼ',
        'P' => 'ᴾ',
        'R' => 'ᴿ',
        'T' => 'ᵀ',
        'U' => 'ᵁ',
        'V' => 'ⱽ',
        'W' => 'ᵂ',
        'α' => 'ᵅ',
        'β' => 'ᵝ',
        'γ' => 'ᵞ',
        'δ' => 'ᵟ',
        'θ' => 'ᶿ',
        'φ' | 'ϕ' => 'ᵠ',
        'χ' => 'ᵡ',
        '∘' => '°',
        '′' => '′',
        _ => return None,
    })
}

/// The subscript glyph for `c`, if Unicode has one.
fn sub(c: char) -> Option<char> {
    Some(match c {
        '0' => '₀',
        '1' => '₁',
        '2' => '₂',
        '3' => '₃',
        '4' => '₄',
        '5' => '₅',
        '6' => '₆',
        '7' => '₇',
        '8' => '₈',
        '9' => '₉',
        '+' => '₊',
        '-' | '−' => '₋',
        '=' => '₌',
        '(' => '₍',
        ')' => '₎',
        'a' => 'ₐ',
        'e' => 'ₑ',
        'h' => 'ₕ',
        'i' => 'ᵢ',
        'j' => 'ⱼ',
        'k' => 'ₖ',
        'l' => 'ₗ',
        'm' => 'ₘ',
        'n' => 'ₙ',
        'o' => 'ₒ',
        'p' => 'ₚ',
        'r' => 'ᵣ',
        's' => 'ₛ',
        't' => 'ₜ',
        'u' => 'ᵤ',
        'v' => 'ᵥ',
        'x' => 'ₓ',
        'β' => 'ᵦ',
        'γ' => 'ᵧ',
        'ρ' => 'ᵨ',
        'φ' | 'ϕ' => 'ᵩ',
        'χ' => 'ᵪ',
        _ => return None,
    })
}

/// `c` in double-struck type (`\mathbb{R}` is `ℝ`); other characters as
/// they are.
fn double_struck(c: char) -> char {
    match c {
        'C' => 'ℂ',
        'H' => 'ℍ',
        'N' => 'ℕ',
        'P' => 'ℙ',
        'Q' => 'ℚ',
        'R' => 'ℝ',
        'Z' => 'ℤ',
        'A'..='Z' => char::from_u32(0x1D538 + (c as u32 - 'A' as u32)).unwrap_or(c),
        '0'..='9' => char::from_u32(0x1D7D8 + (c as u32 - '0' as u32)).unwrap_or(c),
        _ => c,
    }
}

/// `body` with the combining mark of accent `name` after each character.
fn accent(name: &str, body: &str) -> String {
    let mark = match name {
        "hat" | "widehat" => '\u{302}',
        "bar" | "overline" => '\u{305}',
        "tilde" | "widetilde" => '\u{303}',
        "vec" => '\u{20D7}',
        "dot" => '\u{307}',
        "ddot" => '\u{308}',
        _ => '\u{332}',
    };
    let mut out = String::new();
    for c in body.chars() {
        out.push(c);
        if !c.is_whitespace() {
            out.push(mark);
        }
    }
    out
}

/// Operator names set upright as words: `\sin x` is `sin x`.
fn is_function(name: &str) -> bool {
    matches!(
        name,
        "sin"
            | "cos"
            | "tan"
            | "cot"
            | "sec"
            | "csc"
            | "arcsin"
            | "arccos"
            | "arctan"
            | "sinh"
            | "cosh"
            | "tanh"
            | "coth"
            | "log"
            | "ln"
            | "lg"
            | "exp"
            | "lim"
            | "liminf"
            | "limsup"
            | "max"
            | "min"
            | "sup"
            | "inf"
            | "det"
            | "dim"
            | "ker"
            | "deg"
            | "gcd"
            | "hom"
            | "arg"
            | "Pr"
            | "mod"
            | "bmod"
    )
}

/// The Unicode character a symbol command stands for.
fn symbol(name: &str) -> Option<&'static str> {
    Some(match name {
        // Greek, lower case.
        "alpha" => "α",
        "beta" => "β",
        "gamma" => "γ",
        "delta" => "δ",
        "epsilon" => "ϵ",
        "varepsilon" => "ε",
        "zeta" => "ζ",
        "eta" => "η",
        "theta" => "θ",
        "vartheta" => "ϑ",
        "iota" => "ι",
        "kappa" => "κ",
        "lambda" => "λ",
        "mu" => "μ",
        "nu" => "ν",
        "xi" => "ξ",
        "omicron" => "ο",
        "pi" => "π",
        "varpi" => "ϖ",
        "rho" => "ρ",
        "varrho" => "ϱ",
        "sigma" => "σ",
        "varsigma" => "ς",
        "tau" => "τ",
        "upsilon" => "υ",
        "phi" => "ϕ",
        "varphi" => "φ",
        "chi" => "χ",
        "psi" => "ψ",
        "omega" => "ω",
        // Greek, upper case (the rest look like Latin letters).
        "Gamma" => "Γ",
        "Delta" => "Δ",
        "Theta" => "Θ",
        "Lambda" => "Λ",
        "Xi" => "Ξ",
        "Pi" => "Π",
        "Sigma" => "Σ",
        "Upsilon" => "Υ",
        "Phi" => "Φ",
        "Psi" => "Ψ",
        "Omega" => "Ω",
        // Big operators.
        "sum" => "∑",
        "prod" => "∏",
        "coprod" => "∐",
        "int" => "∫",
        "iint" => "∬",
        "iiint" => "∭",
        "oint" => "∮",
        "bigcup" => "⋃",
        "bigcap" => "⋂",
        "bigoplus" => "⨁",
        "bigotimes" => "⨂",
        "bigvee" => "⋁",
        "bigwedge" => "⋀",
        // Binary operators.
        "pm" => "±",
        "mp" => "∓",
        "times" => "×",
        "div" => "÷",
        "cdot" => "·",
        "ast" => "∗",
        "star" => "⋆",
        "circ" => "∘",
        "bullet" => "•",
        "oplus" => "⊕",
        "ominus" => "⊖",
        "otimes" => "⊗",
        "odot" => "⊙",
        "cap" => "∩",
        "cup" => "∪",
        "sqcap" => "⊓",
        "sqcup" => "⊔",
        "setminus" => "∖",
        "wedge" | "land" => "∧",
        "vee" | "lor" => "∨",
        "neg" | "lnot" => "¬",
        // Relations.
        "leq" | "le" => "≤",
        "geq" | "ge" => "≥",
        "neq" | "ne" => "≠",
        "leqslant" => "⩽",
        "geqslant" => "⩾",
        "approx" => "≈",
        "equiv" => "≡",
        "sim" => "∼",
        "simeq" => "≃",
        "cong" => "≅",
        "propto" => "∝",
        "ll" => "≪",
        "gg" => "≫",
        "prec" => "≺",
        "succ" => "≻",
        "in" => "∈",
        "notin" => "∉",
        "ni" => "∋",
        "subset" => "⊂",
        "supset" => "⊃",
        "subseteq" => "⊆",
        "supseteq" => "⊇",
        "perp" | "bot" => "⊥",
        "parallel" => "∥",
        "mid" => "∣",
        "models" => "⊨",
        "vdash" => "⊢",
        "coloneqq" => "≔",
        "doteq" => "≐",
        // Arrows.
        "to" | "rightarrow" => "→",
        "leftarrow" | "gets" => "←",
        "leftrightarrow" => "↔",
        "Rightarrow" => "⇒",
        "Leftarrow" => "⇐",
        "Leftrightarrow" => "⇔",
        "implies" | "Longrightarrow" => "⟹",
        "impliedby" | "Longleftarrow" => "⟸",
        "iff" | "Longleftrightarrow" => "⟺",
        "longrightarrow" => "⟶",
        "longleftarrow" => "⟵",
        "mapsto" => "↦",
        "hookrightarrow" => "↪",
        "uparrow" => "↑",
        "downarrow" => "↓",
        // Logic, sets and other symbols.
        "forall" => "∀",
        "exists" => "∃",
        "nexists" => "∄",
        "emptyset" | "varnothing" => "∅",
        "infty" => "∞",
        "partial" => "∂",
        "nabla" => "∇",
        "angle" => "∠",
        "triangle" => "△",
        "prime" => "′",
        "ell" => "ℓ",
        "hbar" => "ℏ",
        "Re" => "ℜ",
        "Im" => "ℑ",
        "aleph" => "ℵ",
        "wp" => "℘",
        "top" => "⊤",
        "therefore" => "∴",
        "because" => "∵",
        "cdots" => "⋯",
        "ldots" | "dots" => "…",
        "vdots" => "⋮",
        "ddots" => "⋱",
        "langle" => "⟨",
        "rangle" => "⟩",
        "lceil" => "⌈",
        "rceil" => "⌉",
        "lfloor" => "⌊",
        "rfloor" => "⌋",
        "vert" | "lvert" | "rvert" => "|",
        "Vert" | "lVert" | "rVert" => "‖",
        "lbrace" => "{",
        "rbrace" => "}",
        "backslash" => "\\",
        "colon" => ":",
        "surd" => "√",
        "dagger" => "†",
        "ddagger" => "‡",
        "square" | "Box" => "□",
        "blacksquare" => "■",
        "diamond" => "⋄",
        "checkmark" => "✓",
        "degree" => "°",
        _ => return None,
    })
}

#[cfg(test)]
mod tests;
