//! Stress tests — LEX-001: Lexer and spans.

use astra_core::lexer::{Keyword, Punctuation, TokenKind, lex};

fn kinds(source: &str) -> Vec<TokenKind> {
    lex(source).tokens.into_iter().map(|t| t.kind).collect()
}
fn has_error(source: &str, code: &str) -> bool {
    lex(source).diagnostics.iter().any(|d| d.code == code)
}
fn error_codes(source: &str) -> Vec<String> {
    lex(source)
        .diagnostics
        .iter()
        .map(|d| d.code.clone())
        .collect()
}

// ─── Unicode identifiers ──────────────────────────────────────────────────────

#[test]
fn unicode_greek_identifiers_lex() {
    let result = lex("α β γ δ τ ω π Ω Δ Σ");
    assert!(!result.has_errors());
    let ids = result
        .tokens
        .iter()
        .filter(|t| t.kind == TokenKind::Identifier)
        .count();
    assert_eq!(ids, 10);
}

#[test]
fn underscore_alone_is_wildcard() {
    let result = lex("_");
    assert!(!result.has_errors());
    assert_eq!(
        result.tokens[0].kind,
        TokenKind::Punct(Punctuation::Underscore)
    );
}

#[test]
fn underscore_prefix_is_identifier() {
    let result = lex("_foo _bar _1");
    assert!(!result.has_errors());
    let ids = result
        .tokens
        .iter()
        .filter(|t| t.kind == TokenKind::Identifier)
        .count();
    assert_eq!(ids, 3);
}

#[test]
fn bom_stripped_span_starts_after_bom() {
    let source = "\u{feff}fn main() {}";
    let result = lex(source);
    assert!(!result.has_errors());
    assert_eq!(result.tokens[0].kind, TokenKind::Keyword(Keyword::Fn));
    assert_eq!(result.tokens[0].span.start, 3); // BOM is 3 bytes
}

// ─── Keyword aliases all canonical ───────────────────────────────────────────

#[test]
fn all_aliases_produce_same_kind_as_canonical() {
    let pairs = [
        ("fn", "function"),
        ("let", "var"),
        ("const", "constant"),
        ("type", "typedef"),
        ("struct", "record"),
        ("mut", "mutable"),
        ("pub", "public"),
    ];
    for (canonical, alias) in &pairs {
        let k1 = kinds(canonical);
        let k2 = kinds(alias);
        assert_eq!(k1[0], k2[0], "{canonical} and {alias} should be same kind");
    }
}

// ─── Integer literal forms ────────────────────────────────────────────────────

#[test]
fn integer_decimal_hex_binary_underscore() {
    for case in &[
        "0",
        "42",
        "1_000_000",
        "0xFF",
        "0XFF",
        "0b1010",
        "0B0101",
        "0xDEAD_BEEF",
        "255u8",
        "0xFFu8",
    ] {
        let result = lex(case);
        assert!(
            !result.has_errors(),
            "'{case}' should be valid: {:?}",
            result.diagnostics
        );
        assert!(
            result.tokens.iter().any(|t| t.kind == TokenKind::Integer),
            "'{case}' should be Integer"
        );
    }
}

#[test]
fn hex_no_digits_is_ns1005() {
    assert!(has_error("0x", "NS1005"));
    assert!(has_error("0b", "NS1005"));
}

// ─── Float literal forms ──────────────────────────────────────────────────────

#[test]
fn float_all_forms() {
    for case in &["1.0", "1e9", "1E9", "1.5e-3", "1.5e+3", "0.0", "3.14"] {
        let result = lex(case);
        assert!(!result.has_errors(), "'{case}': {:?}", result.diagnostics);
        assert!(result.tokens.iter().any(|t| t.kind == TokenKind::Float));
    }
}

#[test]
fn dot_without_following_digit_is_not_float() {
    let result = lex("1.");
    assert!(!result.has_errors());
    let k: Vec<_> = result.tokens.iter().map(|t| t.kind).collect();
    assert!(k.contains(&TokenKind::Integer));
    assert!(k.contains(&TokenKind::Punct(Punctuation::Dot)));
}

// ─── String escapes ───────────────────────────────────────────────────────────

#[test]
fn string_all_valid_escapes() {
    for case in &[
        r#""\n""#,
        r#""\r""#,
        r#""\t""#,
        r#""\0""#,
        r#""\\""#,
        r#""\"""#,
        r#""\u{0}""#,
        r#""\u{41}""#,
        r#""\u{1F600}""#,
    ] {
        assert!(!lex(case).has_errors(), "'{case}' should be valid");
    }
}

#[test]
fn string_invalid_escapes_are_ns1002() {
    for case in &[r#""\q""#, r#""\x41""#, r#""\u{110000}""#, r#""\u{}""#] {
        assert!(has_error(case, "NS1002"), "'{case}' should be NS1002");
    }
}

#[test]
fn unterminated_strings_are_ns1003() {
    assert!(has_error(r#""hello"#, "NS1003"));
    assert!(has_error("\"hello\nworld", "NS1003"));
}

#[test]
fn byte_string_rejects_non_byte_codepoints() {
    assert!(has_error(r#"b"hello😀""#, "NS1002"));
}

// ─── Comment forms ────────────────────────────────────────────────────────────

#[test]
fn valid_line_comments_collected() {
    let result = lex("// hello  \n42");
    assert!(!result.has_errors());
    assert_eq!(result.comments[0].text, "// hello");
}

#[test]
fn doc_comment_triple_slash_is_valid() {
    let result = lex("/// docs\nfn f() {}");
    assert!(!result.has_errors());
    assert!(result.comments[0].text.starts_with("///"));
}

#[test]
fn all_invalid_comment_forms_are_ns1001() {
    assert!(has_error("/* block */", "NS1001"));
    assert!(has_error("# hash", "NS1001"));
    assert!(has_error("-- dash", "NS1001"));
}

#[test]
fn three_invalid_comments_all_reported() {
    let codes = error_codes("/* a */\n# b\n-- c\n42");
    assert_eq!(codes.len(), 3);
    assert!(codes.iter().all(|c| c == "NS1001"));
}

// ─── Operators ────────────────────────────────────────────────────────────────

#[test]
fn wrapping_operators_all_lex() {
    for op in &["+%", "-%", "*%", "/%"] {
        assert!(!lex(&format!("a {op} b")).has_errors(), "'{op}' should lex");
    }
}

#[test]
fn arrow_and_fat_arrow_lex() {
    let result = lex("-> =>");
    assert!(!result.has_errors());
    let k: Vec<_> = result.tokens.iter().map(|t| t.kind).collect();
    assert!(k.contains(&TokenKind::Punct(Punctuation::Arrow)));
    assert!(k.contains(&TokenKind::Punct(Punctuation::FatArrow)));
}

#[test]
fn single_ampersand_is_ns1005() {
    assert!(has_error("a & b", "NS1005"));
}

// ─── Span correctness ────────────────────────────────────────────────────────

#[test]
fn multibyte_span_is_byte_offset() {
    // "café" = 5 bytes
    let result = lex("café");
    assert!(!result.has_errors());
    assert_eq!(result.tokens[0].span.end, 5);
}

#[test]
fn spans_sequential_non_overlapping() {
    let result = lex("let x = 42 + 1");
    let tokens = &result.tokens;
    for w in tokens.windows(2) {
        if w[0].kind != TokenKind::Eof {
            assert!(
                w[0].span.end <= w[1].span.start,
                "overlap: {:?} {:?}",
                w[0],
                w[1]
            );
        }
    }
}

// ─── Newlines ────────────────────────────────────────────────────────────────

#[test]
fn newlines_produce_newline_tokens() {
    let result = lex("a\nb\nc");
    let nl = result
        .tokens
        .iter()
        .filter(|t| t.kind == TokenKind::Newline)
        .count();
    assert_eq!(nl, 2);
}

#[test]
fn crlf_is_single_newline() {
    let result = lex("a\r\nb");
    let nl = result
        .tokens
        .iter()
        .filter(|t| t.kind == TokenKind::Newline)
        .count();
    assert_eq!(nl, 1);
}

// ─── Large source ─────────────────────────────────────────────────────────────

#[test]
fn lexes_1000_functions_without_panic() {
    let source: String = (0..1000)
        .map(|i| format!("fn f{i}(x: i32) -> i32 {{ return x + {i} }}\n"))
        .collect();
    let result = lex(&source);
    assert!(!result.has_errors());
    let fns = result
        .tokens
        .iter()
        .filter(|t| t.kind == TokenKind::Keyword(Keyword::Fn))
        .count();
    assert_eq!(fns, 1000);
}

#[test]
fn all_tokens_have_nonempty_text() {
    let source = "+ - * / % +% -% *% /% == != < <= > >= && || ! = . , : ; -> => ( ) [ ] { } @ | _";
    let result = lex(source);
    for t in &result.tokens {
        if t.kind != TokenKind::Eof {
            assert!(!t.text.is_empty(), "token {:?} has empty text", t.kind);
        }
    }
}
