//! Source-aware canonical syntax lint.
//!
//! This is a deny lint, not a rewrite pass. It reports deprecated spellings in
//! edition `0.2`; edition `0.1` is unchanged. Migration is the only rewrite
//! path (Phase 5), so this pass must never produce edited source.

use crate::diagnostic::Diagnostic;
use crate::edition::Edition;
use crate::lexer::{Keyword, LexOptions, Punctuation, TokenKind, lex_with_options};

const DEPRECATED_ALIAS: &str = "NS2009";
const DEPRECATED_SPAWN: &str = "NS2012";
const DEPRECATED_DIVISION: &str = "NS2013";
const DEPRECATED_SPELLING: &str = "NS2015";

const ALIASES: [(&str, &str); 7] = [
    ("function", "fn"),
    ("var", "let"),
    ("constant", "const"),
    ("typedef", "type"),
    ("record", "struct"),
    ("mutable", "mut"),
    ("public", "pub"),
];

/// Reports every deprecated spelling in `source` under edition `0.2`.
///
/// Edition `0.1` returns no diagnostics; the legacy spellings are still valid
/// there. Parse errors are deliberately ignored: the normal check command
/// reports those, and the lint is only about canonical spelling.
#[must_use]
pub fn check_canonical(source: &str, edition: Edition) -> Vec<Diagnostic> {
    if edition != Edition::V0_2 {
        return Vec::new();
    }

    let tokens = lex_with_options(source, LexOptions { edition }).tokens;
    let mut diagnostics = Vec::new();

    for (index, token) in tokens.iter().enumerate() {
        if let TokenKind::Keyword(_) = token.kind
            && let Some((_, canonical)) = ALIASES
                .iter()
                .find(|(alias, _)| *alias == token.text.as_str())
        {
            diagnostics.push(
                Diagnostic::error(
                    DEPRECATED_ALIAS,
                    format!("Deprecated alias '{}'.", token.text),
                )
                .with_span(token.span)
                .with_hint(format!("Use '{canonical}' instead.")),
            );
            continue;
        }

        if token.kind == TokenKind::Punct(Punctuation::SlashWrap) {
            diagnostics.push(
                Diagnostic::error(
                    DEPRECATED_DIVISION,
                    "Wrapping division '/%' is not canonical.",
                )
                .with_span(token.span)
                .with_hint("Keep '/%'; migration never rewrites it to '/'."),
            );
            continue;
        }

        if token.kind == TokenKind::Keyword(Keyword::Spawn)
            && tokens
                .get(index + 1)
                .is_some_and(|next| next.kind == TokenKind::Punct(Punctuation::PipePipe))
        {
            diagnostics.push(
                Diagnostic::error(DEPRECATED_SPAWN, "Deprecated 'spawn ||' form.")
                    .with_span(token.span)
                    .with_hint("Use 'spawn { ... }'."),
            );
            continue;
        }

        if token.kind == TokenKind::Punct(Punctuation::LBrace)
            && tokens
                .get(index + 1)
                .is_some_and(|next| next.kind == TokenKind::Punct(Punctuation::At))
            && tokens
                .get(index + 2)
                .is_some_and(|next| next.kind == TokenKind::Identifier)
            && tokens
                .get(index + 3)
                .is_some_and(|next| next.kind == TokenKind::Punct(Punctuation::Colon))
        {
            diagnostics.push(
                Diagnostic::error(
                    DEPRECATED_SPELLING,
                    "The old scope-block spelling is deprecated in edition 0.2.",
                )
                .with_span(token.span)
                .with_hint("Use @mode(name) { ... } or @memory(name) { ... }."),
            );
        }
    }

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::check_canonical;
    use crate::edition::Edition;

    #[test]
    fn edition_0_1_has_no_canonical_diagnostics() {
        let source = "public function f() { var x = 1 /% 2 }";
        assert!(check_canonical(source, Edition::V0_1).is_empty());
    }

    #[test]
    fn reports_all_deprecated_spellings_in_edition_0_2() {
        let source = r#"
public function f() {
    var x = 1 /% 2
    spawn || { return }
    {@mode:core; let y = 1}
}
"#;
        let diagnostics = check_canonical(source, Edition::V0_2);
        let codes = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<Vec<_>>();
        assert!(codes.contains(&"NS2009"), "{codes:?}");
        assert!(codes.contains(&"NS2012"), "{codes:?}");
        assert!(codes.contains(&"NS2013"), "{codes:?}");
        assert!(codes.contains(&"NS2015"), "{codes:?}");
    }

    #[test]
    fn canonical_source_has_no_diagnostics() {
        let source = r#"
pub fn f() {
    let x = 1 /% 2
    spawn { return }
    @mode(core) {
        let y = 1
    }
}
"#;
        let diagnostics = check_canonical(source, Edition::V0_2);
        let codes = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<Vec<_>>();
        assert_eq!(codes, vec!["NS2013"], "{codes:?}");
    }
}
