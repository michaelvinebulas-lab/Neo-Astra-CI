//! Conformance check for the C backend.
//!
//! Every shared case must lower to C with no rejection and compile as C17:
//!
//! * the generated translation unit must compile freestanding
//!   (`clang -std=c17 -ffreestanding -Wall -c`), and
//! * when a hosted-capable compiler is available (libc headers and a linker),
//!   the case is linked against `astra_host.c`, run, and its stdout plus
//!   `RETURN=<code>` compared with the corpus `.expected` file.
//!
//! A case the backend cannot lower yet is listed in [`KNOWN_GAPS`] with an
//! owning task, and the test fails when a listed case starts emitting so the
//! ledger cannot rot into permanent skips.

use std::path::{Path, PathBuf};
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
const KNOWN_GAPS: &[(&str, &str)] = &[(
    "channels",
    "CONC-001: `Channel` is a generic builtin; the C first slice rejects spawn and channels",
)];

fn cases_directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/cases")
}

fn known_gap(name: &str) -> Option<&'static str> {
    KNOWN_GAPS
        .iter()
        .find(|(case, _)| *case == name)
        .map(|(_, owner)| *owner)
}

fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n").trim_end().to_string()
}

/// Renders the runner's view of a program: stdout followed by the final
/// `RETURN=<code>` line, matching the JavaScript and Nova runners.
fn render(stdout: &str, code: i32) -> String {
    let mut rendered = stdout.trim_end_matches('\n').to_string();
    if !rendered.is_empty() {
        rendered.push('\n');
    }
    rendered.push_str(&format!("RETURN={code}"));
    rendered
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

fn tool_path(candidate: &str) -> Option<PathBuf> {
    let path = PathBuf::from(candidate);
    if path.exists() {
        return Some(path);
    }
    if let Ok(output) = Command::new(candidate).arg("--version").output()
        && output.status.success()
    {
        return Some(PathBuf::from(candidate));
    }
    None
}

fn clang_path() -> Option<PathBuf> {
    [
        "C:\\Program Files\\LLVM\\bin\\clang.exe",
        "clang",
        "clang-19",
        "clang-18",
    ]
    .iter()
    .find_map(|candidate| tool_path(candidate))
}

/// Returns a compiler that can build a *hosted* program, or None. A freestanding
/// `-c` compile is not enough: clang on a Windows host without an SDK cannot
/// find `<stdio.h>`, while gcc from MSYS2 can. The probe is a real compile.
fn hosted_compiler() -> Option<PathBuf> {
    for candidate in [
        "C:\\Program Files\\LLVM\\bin\\clang.exe",
        "clang",
        "gcc",
        "cc",
    ] {
        if let Some(path) = tool_path(candidate)
            && hosted_probe(&path)
        {
            return Some(path);
        }
    }
    None
}

fn hosted_probe(compiler: &Path) -> bool {
    let Ok(dir) = tempfile::tempdir() else {
        return false;
    };
    let probe = dir.path().join("probe.c");
    let obj = dir.path().join("probe.o");
    if std::fs::write(&probe, "#include <stdio.h>\nint main(void) { return 0; }\n").is_err() {
        return false;
    }
    Command::new(compiler)
        .args(["-std=c17", "-c", "-o"])
        .arg(&obj)
        .arg(&probe)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
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

/// Links the generated translation unit with the hosted runtime shim, runs the
/// produced executable, and returns (stdout, exit code).
fn hosted_run(source: &str, compiler: &Path) -> Result<(String, i32), String> {
    let dir = tempfile::tempdir().map_err(|error| format!("temp dir: {error}"))?;
    let src_path = dir.path().join("module.c");
    let host_path = dir.path().join("astra_host.c");
    let exe_path = dir
        .path()
        .join(if cfg!(windows) { "prog.exe" } else { "prog" });
    std::fs::write(&src_path, source).map_err(|error| format!("write module.c: {error}"))?;
    std::fs::write(&host_path, astra_codegen_c::runtime::host_shim())
        .map_err(|error| format!("write astra_host.c: {error}"))?;
    let build = Command::new(compiler)
        .args(["-std=c17", "-Wall", "-Wno-unused-variable", "-o"])
        .arg(&exe_path)
        .arg(&src_path)
        .arg(&host_path)
        .output()
        .map_err(|error| format!("start hosted compiler: {error}"))?;
    if !build.status.success() {
        return Err(String::from_utf8_lossy(&build.stderr).into_owned());
    }
    let run = Command::new(&exe_path)
        .output()
        .map_err(|error| format!("run executable: {error}"))?;
    Ok((
        String::from_utf8_lossy(&run.stdout).into_owned(),
        run.status.code().unwrap_or(-1),
    ))
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

    let hosted = hosted_compiler();
    let mut emitted = 0;
    let mut executed = 0;
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
        let c_source = match result {
            Ok(c_source) => c_source,
            Err(error) => {
                failures.push(format!("{name}: C emission failed: {error}"));
                continue;
            }
        };

        if let Some(Err(stderr)) = clang_check(&c_source) {
            failures.push(format!("{name}: clang rejected the generated C:\n{stderr}"));
        }

        let Some(compiler) = hosted.as_ref() else {
            continue;
        };
        let expected_path = path.with_extension("expected");
        let expected = normalize(
            &std::fs::read_to_string(&expected_path)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", expected_path.display())),
        );
        match hosted_run(&c_source, compiler) {
            Ok((stdout, code)) => {
                executed += 1;
                let actual = normalize(&render(&stdout, code));
                if actual != expected {
                    failures.push(format!(
                        "{name}: runtime output did not match {}\n--- expected ---\n{expected}\n--- actual ---\n{actual}",
                        expected_path.display()
                    ));
                }
            }
            Err(error) => failures.push(format!(
                "{name}: hosted link/run failed ({}):\n{error}",
                compiler.display()
            )),
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

    if hosted.is_some() {
        assert_eq!(
            executed, emitted,
            "every emitted case must run when a hosted compiler is available"
        );
    }

    assert!(
        failures.is_empty(),
        "{} conformance problem(s):\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}
