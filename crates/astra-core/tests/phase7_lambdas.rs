//! Phase 7 slice 5: edition-0.2 immediate lambdas.

use astra_core::checker::{CheckOptions, check_source};
use astra_core::edition::Edition;
use astra_core::formatter::{FormatOptions, format_source_with_options};

fn check_v02(source: &str) -> Vec<String> {
    match check_source(source, CheckOptions::default().with_edition(Edition::V0_2)) {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.code)
            .collect(),
    }
}

fn format_v02(source: &str) -> String {
    format_source_with_options(
        source,
        FormatOptions {
            edition: Edition::V0_2,
        },
    )
    .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"))
}

#[test]
fn inference_annotations_and_captures_typecheck() {
    let diagnostics = check_v02(
        r#"
module app.main

pub fn main() -> i32 {
    let offset = 10
    let mut total = 0
    let doubled = (fn(value) => value * 2)(21)
    let typed = (fn(x: i32, y: i32) => x + y)(1, 2)
    let captured = (fn(value) => value + offset)(5)
    let block = (fn(value) => {
        println(value)
        value + 1
    })(8)
    total = (fn(value) => {
        total = total + value
        total
    })(3)
    println(doubled)
    println(typed)
    println(captured)
    println(block)
    println(total)
    return 0
}
"#,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn bare_lambda_is_rejected() {
    let diagnostics = check_v02(
        r#"
module app.main

pub fn main() -> i32 {
    let f = fn(x) => x + 1
    return 0
}
"#,
    );
    assert!(
        diagnostics.contains(&"NS2001".to_owned()),
        "{diagnostics:?}"
    );
}

#[test]
fn arity_mismatch_is_rejected() {
    let diagnostics = check_v02(
        r#"
module app.main

pub fn main() -> i32 {
    let value = (fn(x, y) => x + y)(1)
    return 0
}
"#,
    );
    assert!(
        diagnostics.contains(&"NS2001".to_owned()),
        "{diagnostics:?}"
    );
}

#[test]
fn edition_0_1_rejects_lambda_syntax() {
    let diagnostics =
        match check_source("let x = (fn(value) => value)(1)\n", CheckOptions::default()) {
            Ok(_) => Vec::new(),
            Err(diagnostics) => diagnostics
                .into_iter()
                .map(|diagnostic| diagnostic.code)
                .collect(),
        };
    assert!(!diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn formatter_canonicalizes_lambda_calls_to_blocks() {
    let formatted = format_v02(
        r#"
module app.main

fn value() -> i32 {
    return (fn(x) => x + 1)(2)
}
"#,
    );
    assert!(formatted.contains("let x = 2"), "{formatted}");
    assert!(!formatted.contains("=>"), "{formatted}");
    assert_eq!(format_v02(&formatted), formatted);
}
