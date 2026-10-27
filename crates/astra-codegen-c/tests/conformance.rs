//! Static conformance check for the C backend.
//!
//! The native conformance runner (`conformance/README.md`) waits on a hosted
//! toolchain: this Windows host has no libc headers for `astra_host.c`. Every
//! shared case can already be *lowered* and compiled freestanding, though, so
//! this test keeps the corpus honest for C:
//!
//! * a case must generate C with no rejection, and that C must compile with
//!   `clang -std=c17 -ffreestanding -Wall -c`;
//! * a case the backend cannot lower yet is listed in [`KNOWN_GAPS`] with an
//!   owning task, and the test fails when a listed case starts emitting so the
//!   ledger cannot rot into permanent skips.

use std::path::PathBuf;
use std::process::Command;

use astra_capabilities::Mode;
use astra_codegen_c::{CModule, IntegerPolicy, generate};
use astra_core::checker::{CheckOptions, check_resolved};
use astra_core::parser::parse;
use astra_core::resolver::resolve_module;

/// Cases the C backend cannot lower yet. Each entry names the owning task.
///
/// When a case starts emitting, remove it from this table; the test fails on
/// stale entries so gaps stay visible instead of becoming permanent skips.
const KNOWN_GAPS: &[(&str, &str)] = &[
    (
        "channels",
        "CONC-001: `Channel` is a generic builtin; the C first slice rejects spawn and channels",
    ),
    (
        "generics",
        "C-001: generic functions need monomorphization, which the first slice does not implement",
    ),
];

fn cases_directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/cases")
}

fn known_gap(name: &str) -> Option<&'static str> {
    KNOWN_GAPS
        .iter()
        .find(|(case, _)| *case == name)
        .map(|(_, owner)| *owner)
}

fn emit_case(source: &str) -> Result<String, String> {
    let module = parse(source).map_err(|error| format!("parse failed: {error:?}"))?;
    let resolved = resolve_module(&module).map_err(|error| format!("resolve failed: {error:?}"))?;
    let checked = check_resolved(&module, &resolved, CheckOptions::new(Mode::Core))
        .map_err(|error| format!("type check failed: {error:?}"))?;
    let c_module = CModule {
        name: checked.module.name.clone(),
        ast: module,
        hir: checked.module,
        annotations: checked.annotations,
        manual_memory_scopes: checked.manual_memory_scopes,
        integer_policy: IntegerPolicy::Wrapping,
    };
    generate(&c_module)
        .map(|generated| generated.source)
        .map_err(|error| format!("{}: {}", error.code, error.message))
}

fn clang_path() -> Option<PathBuf> {
    let candidates = [
        "C:\\Program Files\\LLVM\\bin\\clang.exe",
        "clang",
        "clang-19",
        "clang-18",
    ];
    for candidate in candidates {
        let path = PathBuf::from(candidate);
        if path.exists() {
            return Some(path);
        }
        if let Ok(output) = Command::new(&path).arg("--version").output()
            && output.status.success()
        {
            return Some(path);
        }
    }
    None
}

/// Compiles the generated C freestanding. Returns `None` when clang is absent.
fn clang_check(source: &str) -> Option<Result<(), String>> {
    let clang = clang_path()?;
    let dir = tempfile::tempdir().expect("temp dir");
    let src_path = dir.path().join("module.c");
    std::fs::write(&src_path, source).expect("write");
    let output = Command::new(&clang)
        .args([
            "-std=c17",
            "-ffreestanding",
            "-Wall",
            "-Wno-unused-variable",
            "-c",
            "-o",
        ])
        .arg(dir.path().join("out.o"))
        .arg(&src_path)
        .output()
        .expect("clang failed to start");
    if output.status.success() {
        Some(Ok(()))
    } else {
        Some(Err(String::from_utf8_lossy(&output.stderr).into_owned()))
    }
}

#[test]
fn c_backend_emits_every_conformance_case() {
    let directory = cases_directory();
    let mut cases = std::fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", directory.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "ns"))
        .collect::<Vec<_>>();
    cases.sort();

    assert!(
        !cases.is_empty(),
        "no conformance cases found in {}",
        directory.display()
    );

    let mut emitted = 0;
    let mut failures = Vec::new();

    for path in &cases {
        let name = path.file_stem().map_or_else(
            || "<unknown>".to_string(),
            |stem| stem.to_string_lossy().into_owned(),
        );
        let source = std::fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));

        let result = emit_case(&source);

        if let Some(owner) = known_gap(&name) {
            if result.is_ok() {
                failures.push(format!(
                    "{name}: listed as a C gap but it now emits; remove it from KNOWN_GAPS ({owner})"
                ));
            }
            continue;
        }

        emitted += 1;
        match result {
            Ok(c_source) => {
                if let Some(Err(stderr)) = clang_check(&c_source) {
                    failures.push(format!("{name}: clang rejected the generated C:\n{stderr}"));
                }
            }
            Err(error) => failures.push(format!("{name}: C emission failed: {error}")),
        }
    }

    for (name, _) in KNOWN_GAPS {
        if !cases_directory().join(format!("{name}.ns")).exists() {
            failures.push(format!(
                "{name}: listed in KNOWN_GAPS but conformance/cases/{name}.ns does not exist"
            ));
        }
    }

    assert!(
        emitted > 0,
        "no conformance case was emitted; every case is in KNOWN_GAPS"
    );

    assert!(
        failures.is_empty(),
        "{} conformance problem(s):\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}
