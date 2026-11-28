//! Output-contract tests for the CLI renderer.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_project(files: &[(&str, &str)]) -> PathBuf {
    let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir =
        std::env::temp_dir().join(format!("astra-cli-output-{}-{unique}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    for (relative, contents) in files {
        let path = dir.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }
    dir
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_astra-native"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn failing_check_has_empty_stdout() {
    let dir = temp_project(&[
        ("core.json", "{ \"package\": { \"name\": \"demo\" } }"),
        (
            "src/main.ns",
            "pub fn main() -> i32 {\n    return \"x\"\n}\n",
        ),
    ]);
    let output = run(&["check", dir.to_str().unwrap()]);
    let _ = fs::remove_dir_all(&dir);

    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error[NS"), "{stderr}");
    assert!(stderr.contains("aborting"), "{stderr}");
    assert!(!stderr.contains("Manifest:"), "{stderr}");
}

#[test]
fn successful_check_prints_manifest_without_format() {
    let dir = temp_project(&[
        ("core.json", "{ \"package\": { \"name\": \"demo\" } }"),
        ("src/main.ns", "pub fn main() -> i32 {\n    return 0\n}\n"),
    ]);
    let output = run(&["check", dir.to_str().unwrap()]);
    let _ = fs::remove_dir_all(&dir);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Manifest: "), "{stdout}");
    assert!(stdout.contains("Package: demo"), "{stdout}");
    assert!(!stdout.contains("Format: "), "{stdout}");
}

#[test]
fn explain_code_reports_missing_corpus() {
    let output = run(&["explain", "NS4001"]);

    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("explanations are not bundled"), "{stderr}");
    assert!(stderr.contains("spec-v0.1.md"), "{stderr}");
}
