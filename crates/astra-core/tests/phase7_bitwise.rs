//! Phase 7 slice 3: edition-0.2 bitwise operators.

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
fn all_bitwise_operators_typecheck() {
    let diagnostics = check_v02(
        r#"
module app.main

pub fn main() -> i32 {
    let and = 0b1100 & 0b1010
    let or = 0b1000 | 0b0010
    let xor = 0b1010 ^ 0b1111
    let left = 1 << 4
    let right = 256 >> 4
    println(and)
    println(or)
    println(xor)
    println(left)
    println(right)
    return 0
}
"#,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn error_set_pipe_is_not_bitwise_or() {
    let diagnostics = check_v02(
        r#"
module app.main

struct Foo(i32)
struct Bar(i32)

fn explode() -> i32 ! Foo | Bar {
    return 0
}

pub fn main() -> i32 {
    let value = 1 | 2
    return value
}
"#,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn non_integer_bitwise_operands_are_rejected() {
    let diagnostics = check_v02(
        r#"
module app.main

pub fn main() -> i32 {
    let value = 1.0 & 2.0
    return 0
}
"#,
    );
    assert!(
        diagnostics.contains(&"NS4006".to_owned()),
        "{diagnostics:?}"
    );
}

#[test]
fn edition_0_1_rejects_bitwise_syntax() {
    let diagnostics = match check_source("let x = 1 & 2\n", CheckOptions::default()) {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.code)
            .collect(),
    };
    assert!(
        diagnostics.contains(&"NS1005".to_owned()),
        "{diagnostics:?}"
    );
}

#[test]
fn formatter_preserves_bitwise_precedence() {
    let formatted = format_v02(
        r#"
module app.main

fn mask(value: i32) -> i32 {
    let result = 1 | 2 & 3 << 1
    return result
}
"#,
    );
    assert!(
        formatted.contains("let result = 1 | 2 & 3 << 1"),
        "{formatted}"
    );
    assert_eq!(format_v02(&formatted), formatted);
}
