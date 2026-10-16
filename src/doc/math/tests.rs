use super::{inline, to_unicode};

#[test]
fn greek_letters_and_symbols() {
    assert_eq!(to_unicode(r"\alpha + \beta \leq \Omega"), "α + β ≤ Ω");
    assert_eq!(
        to_unicode(r"\forall x \in \mathbb{R}, \exists y \neq x"),
        "∀ x ∈ ℝ, ∃ y ≠ x"
    );
    assert_eq!(
        to_unicode(r"a \cdot b \times c \pm d \approx e"),
        "a · b × c ± d ≈ e"
    );
    assert_eq!(to_unicode(r"n \to \infty"), "n → ∞");
}

#[test]
fn scripts_use_unicode_glyphs_where_they_exist() {
    assert_eq!(to_unicode("x^2"), "x²");
    assert_eq!(to_unicode("x^{n+1} + a_{i-1}"), "xⁿ⁺¹ + aᵢ₋₁");
    assert_eq!(to_unicode(r"\sum_{i=1}^{n} i"), "∑ᵢ₌₁ⁿ i");
    assert_eq!(to_unicode(r"90^\circ"), "90°");
    assert_eq!(to_unicode(r"e^{i\pi}"), "e^(iπ)");
}

#[test]
fn scripts_without_glyphs_fall_back_to_marks() {
    assert_eq!(to_unicode("x^q"), "x^q");
    assert_eq!(to_unicode("x_{b,c}"), "x_(b,c)");
}

#[test]
fn fractions_parenthesise_compound_parts() {
    assert_eq!(to_unicode(r"\frac{1}{2}"), "1/2");
    assert_eq!(to_unicode(r"\frac{a+b}{2}"), "(a+b)/2");
    assert_eq!(to_unicode(r"\frac{x^2}{2y}"), "x²/(2y)");
    assert_eq!(to_unicode(r"\frac12"), "1/2");
    assert_eq!(to_unicode(r"\frac{(a+b)}{10}"), "(a+b)/10");
    assert_eq!(to_unicode(r"\frac{(a)+(b)}{c}"), "((a)+(b))/c");
}

#[test]
fn roots() {
    assert_eq!(to_unicode(r"\sqrt{x}"), "√x");
    assert_eq!(to_unicode(r"\sqrt{x^2 + y^2}"), "√(x² + y²)");
    assert_eq!(to_unicode(r"\sqrt[3]{8}"), "∛8");
    assert_eq!(to_unicode(r"\sqrt[n]{a}"), "ⁿ√a");
}

#[test]
fn layout_and_font_commands_are_dropped() {
    assert_eq!(to_unicode(r"\left( \frac{a}{b} \right)"), "( a/b )");
    assert_eq!(to_unicode(r"\left. f \right|_0^1"), "f |₀¹");
    assert_eq!(to_unicode(r"\mathrm{d}x"), "dx");
    assert_eq!(to_unicode(r"x \text{ if } x > 0"), "x if x > 0");
    assert_eq!(to_unicode(r"\operatorname{sgn}(x)"), "sgn(x)");
    assert_eq!(to_unicode(r"\sin x + \log_2 n"), "sin x + log₂ n");
    assert_eq!(to_unicode(r"\displaystyle\int_0^1 f\,dx"), "∫₀¹ f dx");
}

#[test]
fn rows_become_lines() {
    assert_eq!(
        to_unicode("\\begin{aligned}\na &= b \\\\\nc &= d\n\\end{aligned}"),
        "a = b\nc = d"
    );
    assert_eq!(inline(r"a \\ b"), "a b");
}

#[test]
fn unknown_commands_stay_as_source() {
    assert_eq!(to_unicode(r"\foo{x^2} + \bar"), "\\foo{x²} + \\bar");
    assert_eq!(to_unicode(r"\binom{n}{k}"), "\\binom{n}{k}");
    assert_eq!(to_unicode("\\"), "\\");
}

#[test]
fn not_negates_a_relation() {
    assert_eq!(to_unicode(r"a \not= b \not\in S"), "a ≠ b ∉ S");
}

#[test]
fn accents_combine_with_their_character() {
    assert_eq!(to_unicode(r"\hat{x}"), "x\u{302}");
    assert_eq!(to_unicode(r"\frac{\vec{v}}{2}"), "v\u{20D7}/2");
}

#[test]
fn deep_nesting_stays_as_source_without_overflowing() {
    let groups = format!("{}x{}", "{".repeat(10_000), "}".repeat(10_000));
    assert!(to_unicode(&groups).contains('x'));
    let roots = format!("{}x", r"\sqrt".repeat(10_000));
    assert!(to_unicode(&roots).contains(r"\sqrtx"));
}
