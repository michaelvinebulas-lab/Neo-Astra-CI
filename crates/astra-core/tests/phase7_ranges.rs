//! Phase 7 slice 2: edition-0.2 ranges in `for` loops and slicing sugar.

use astra_core::checker::{CheckOptions, check_source};
use astra_core::edition::Edition;

fn check_v02(source: &str) -> Vec<String> {
    match check_source(source, CheckOptions::default().with_edition(Edition::V0_2)) {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.code)
            .collect(),
    }
}

#[test]
fn exclusive_range_for_typechecks() {
    let diagnostics = check_v02(
        r#"
module app.main

pub fn main() -> i32 {
    let mut total = 0
    for value in 0..3 {
        total = total + value
    }
    return total
}
"#,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn inclusive_range_for_typechecks() {
    let diagnostics = check_v02(
        r#"
module app.main

pub fn main() -> i32 {
    let mut total = 0
    for value in 0..=3 {
        total = total + value
    }
    return total
}
"#,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn string_range_slice_typechecks_with_slice_error() {
    let diagnostics = check_v02(
        r#"
module app.main

fn part(text: str) -> str ! SliceError {
    return text[0..2]?
}

pub fn main() -> i32 {
    return 0
}
"#,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn inclusive_range_slice_adds_one_to_end() {
    let diagnostics = check_v02(
        r#"
module app.main

fn part(text: str) -> str ! SliceError {
    return text[0..=1]?
}

pub fn main() -> i32 {
    return 0
}
"#,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn array_range_slice_typechecks() {
    let diagnostics = check_v02(
        r#"
module app.main

pub fn main() -> i32 {
    let values = [10, 20, 30]
    let part = values[0..2]
    let tail = values[1..=2]
    println(part.len())
    println(tail.len())
    return 0
}
"#,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn non_integer_range_bounds_are_rejected() {
    let diagnostics = check_v02(
        r#"
module app.main

pub fn main() -> i32 {
    for value in "a".."z" {
        println(value)
    }
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
fn edition_0_1_rejects_range_syntax() {
    let source = r#"
module app.main

pub fn main() -> i32 {
    for value in 0..3 {
        return value
    }
    return 0
}
"#;
    assert!(check_source(source, CheckOptions::default()).is_err());
}
