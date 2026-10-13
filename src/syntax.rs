//! The highlighter's syntax set, shared by `doc` (is this extension a code
//! file?) and `render` (highlighting). Built once per process.

use std::sync::LazyLock;

use syntect::parsing::SyntaxSet;

// bat's syntax set: syntect's defaults lack TypeScript, TOML, Dockerfile
// and more, which then showed as plain text.
static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(two_face::syntax::extra_newlines);

/// The syntaxes bastian highlights with.
pub(crate) fn set() -> &'static SyntaxSet {
    &SYNTAXES
}

/// Whether `ext` names a language the highlighter knows (e.g. `rs`).
pub fn known_extension(ext: &str) -> bool {
    SYNTAXES.find_syntax_by_extension(ext).is_some()
}
