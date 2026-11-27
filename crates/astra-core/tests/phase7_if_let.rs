//! Phase 7 slice 1: `if let` / `while let` are edition-0.2 syntax that
//! desugars to the existing match/while forms, so the checker must accept them
//! without new diagnostics while edition 0.1 keeps rejecting them.

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
fn if_let_typechecks() {
    let diagnostics = check_v02(
        r#"
module app.main

pub fn main() -> i32 {
    let value: Option<i32> = Some(7)
    if let Some(inner) = value {
        return inner
    } else {
        return 0
    }
}
"#,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn nested_if_let_chain_typechecks() {
    let diagnostics = check_v02(
        r#"
module app.main

pub fn main() -> i32 {
    let first: Option<i32> = Some(1)
    let second: Option<i32> = Some(2)
    if let Some(value) = first {
        return value
    } else if let Some(other) = second {
        return other
    } else {
        return 0
    }
}
"#,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn while_let_typechecks() {
    let diagnostics = check_v02(
        r#"
module app.main

pub fn main() -> i32 {
    let value: Option<i32> = Some(7)
    while let Some(inner) = value {
        return inner
    }
    return 0
}
"#,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn edition_0_1_rejects_if_let() {
    let source = r#"
module app.main

pub fn main() -> i32 {
    let value: Option<i32> = Some(7)
    if let Some(inner) = value {
        return inner
    }
    return 0
}
"#;
    let result = check_source(source, CheckOptions::default());
    assert!(result.is_err());
}
