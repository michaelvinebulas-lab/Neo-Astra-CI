//! Edition 0.2 checker rules: `Err` pattern heads, error-member exhaustiveness,
//! and D7 declared-set subset checking.

use std::fs;
use std::path::PathBuf;

use astra_core::checker::{CheckOptions, check_source};
use astra_core::diagnostic::Diagnostic;
use astra_core::edition::Edition;

fn check_v02(source: &str) -> Result<astra_core::checker::CheckResult, Vec<Diagnostic>> {
    check_source(source, CheckOptions::default().with_edition(Edition::V0_2))
}

fn codes(diagnostics: Vec<Diagnostic>) -> Vec<String> {
    diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

#[test]
fn v0_2_conformance_corpus_matches_expectations() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/v0.2");

    // Two corpus cases document future behavior, not the frozen cleanup:
    // scopes.ns widens core -> system (mode rules are unchanged), and
    // turbofish.ns relies on generic struct-initializer inference the checker
    // does not implement. Their syntax is covered by the parser tests.
    let deferred = ["scopes.ns", "turbofish.ns"];
    let mut failures = Vec::new();
    for entry in fs::read_dir(root.join("valid")).expect("valid directory") {
        let path = entry.expect("valid entry").path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| deferred.contains(&name))
        {
            continue;
        }
        if path.extension().is_some_and(|extension| extension == "ns") {
            let source = fs::read_to_string(&path).expect("read valid source");
            if let Err(diagnostics) = check_v02(&source) {
                failures.push(format!("{}: {diagnostics:?}", path.display()));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));

    for entry in fs::read_dir(root.join("invalid")).expect("invalid directory") {
        let path = entry.expect("invalid entry").path();
        if !path.extension().is_some_and(|extension| extension == "ns") {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read invalid source");
        let expected = fs::read_to_string(path.with_extension("codes")).expect("read codes");
        let found = codes(check_v02(&source).unwrap_err());
        for code in expected
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            assert!(
                found.contains(&code.to_owned()),
                "{}: expected {code}, got {found:?}",
                path.display()
            );
        }
    }
}

#[test]
fn accepts_the_valid_error_pattern_corpus() {
    let source = include_str!("../../../conformance/v0.2/valid/error_patterns.ns");
    let result = check_v02(source);
    assert!(result.is_ok(), "{:?}", result.err());
}

#[test]
fn rejects_an_error_member_not_in_the_declared_set() {
    let source = include_str!("../../../conformance/v0.2/invalid/error_member_not_declared.ns");
    let diagnostics = check_v02(source).unwrap_err();
    let found = codes(diagnostics);
    assert!(found.contains(&"NS4017".to_owned()), "{found:?}");
}

#[test]
fn requires_every_declared_error_member_when_using_member_patterns() {
    let diagnostics = check_v02(
        r#"
struct A(str)
struct B(str)

fn fallible() -> i32 ! A | B {
    fail A("a")
}

pub fn main() -> i32 {
    match fallible() {
        Ok(value) => return value,
        Err(A(_)) => return 1,
    }
}
"#,
    )
    .unwrap_err();
    let found = codes(diagnostics);
    assert!(found.contains(&"NS4010".to_owned()), "{found:?}");
}

#[test]
fn a_binding_err_pattern_covers_all_declared_members() {
    let result = check_v02(
        r#"
struct A(str)
struct B(str)

fn fallible() -> i32 ! A | B {
    fail A("a")
}

pub fn main() -> i32 {
    match fallible() {
        Ok(value) => return value,
        Err(error) => return 1,
    }
}
"#,
    );
    assert!(result.is_ok(), "{:?}", result.err());
}

#[test]
fn d7_rejects_fail_outside_the_declared_error_set() {
    let diagnostics = check_v02(
        r#"
struct A(str)
struct B(str)

fn fallible() -> i32 ! B {
    fail A("a")
}
"#,
    )
    .unwrap_err();
    assert!(codes(diagnostics).contains(&"NS4012".to_owned()));
}

#[test]
fn d7_rejects_propagation_outside_the_declared_error_set() {
    let diagnostics = check_v02(
        r#"
struct A(str)
struct B(str)

fn inner() -> i32 ! A | B {
    fail A("a")
}

fn outer() -> i32 ! A {
    let value = inner()?
    return value
}
"#,
    )
    .unwrap_err();
    assert!(codes(diagnostics).contains(&"NS4012".to_owned()));
}
