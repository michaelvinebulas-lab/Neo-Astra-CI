//! Negative conformance: programs that must be rejected by the frontend.
//!
//! Each case in `conformance/rejects` is a `.ns` file that must fail to parse,
//! resolve, or type-check, plus a `.codes` file listing the stable diagnostic
//! codes the compiler must report. Codes are compared as a subset so a richer
//! diagnostic set is allowed, but every listed code must appear.
//!
//! This runner is backend-neutral: rejection happens before code generation, so
//! it pins the frontend contract for every target at once.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use astra_core::checker::{CheckOptions, check_resolved};
use astra_core::diagnostic::Diagnostic;
use astra_core::parser::parse;
use astra_core::resolver::resolve_module;

fn rejects_directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/rejects")
}

fn codes(diagnostics: Vec<Diagnostic>) -> BTreeSet<String> {
    diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

/// Runs the frontend pipeline and returns the reported codes, or `None` when
/// the program was accepted.
fn frontend_codes(source: &str) -> Option<BTreeSet<String>> {
    let module = match parse(source) {
        Ok(module) => module,
        Err(diagnostics) => return Some(codes(diagnostics)),
    };

    let resolved = match resolve_module(&module) {
        Ok(resolved) => resolved,
        Err(diagnostics) => return Some(codes(diagnostics)),
    };

    match check_resolved(&module, &resolved, CheckOptions::default()) {
        Ok(_) => None,
        Err(diagnostics) => Some(codes(diagnostics)),
    }
}

fn read_expected(path: &PathBuf) -> BTreeSet<String> {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(ToOwned::to_owned)
        .collect()
}

#[test]
fn frontend_rejects_the_negative_conformance_corpus() {
    let directory = rejects_directory();
    let mut cases = fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", directory.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "ns"))
        .collect::<Vec<_>>();
    cases.sort();

    assert!(
        !cases.is_empty(),
        "no negative conformance cases found in {}",
        directory.display()
    );

    let mut failures = Vec::new();

    for path in cases {
        let name = path.file_stem().map_or_else(
            || "<unknown>".to_string(),
            |stem| stem.to_string_lossy().into_owned(),
        );
        let codes_path = path.with_extension("codes");
        assert!(
            codes_path.exists(),
            "{name}: missing {} for the negative case",
            codes_path.display()
        );

        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        let expected = read_expected(&codes_path);
        assert!(
            !expected.is_empty(),
            "{name}: {} must list at least one diagnostic code",
            codes_path.display()
        );

        match frontend_codes(&source) {
            None => failures.push(format!(
                "{name}: expected rejection with {expected:?} but the frontend accepted it"
            )),
            Some(actual) => {
                let missing = expected.difference(&actual).collect::<Vec<_>>();
                if !missing.is_empty() {
                    failures.push(format!(
                        "{name}: missing diagnostic code(s) {missing:?}\n--- expected (subset) ---\n{expected:?}\n--- actual ---\n{actual:?}"
                    ));
                }
            }
        }
    }

    for entry in fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", directory.display()))
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "codes")
            && !path.with_extension("ns").exists()
        {
            failures.push(format!(
                "{}: .codes file has no matching .ns case",
                path.display()
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{} negative conformance problem(s):\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}
