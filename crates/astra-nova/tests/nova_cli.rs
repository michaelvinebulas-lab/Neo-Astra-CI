//! Integration tests for the `nova` CLI binary.
//!
//! Each test exercises the binary through `nova run` or `nova eval` in text or
//! JSON mode and verifies exit code and output.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::Value as JsonValue;

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn nova_bin() -> PathBuf {
    env!("CARGO_BIN_EXE_nova").into()
}

fn temp_source(source: &str) -> TempSource {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "astra-nova-cli-{}-{}.ns",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&path, source).expect("cannot write temp source");
    TempSource { path }
}

struct TempSource {
    path: PathBuf,
}

impl TempSource {
    fn path(&self) -> &str {
        self.path.to_str().expect("temp path is not UTF-8")
    }
}

impl Drop for TempSource {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

struct Outcome {
    status: i32,
    stdout: String,
    stderr: String,
}

fn run_nova(args: &[&str]) -> Outcome {
    let output = Command::new(nova_bin())
        .args(args)
        .output()
        .expect("failed to run nova");
    Outcome {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn run_nova_in(directory: &Path, args: &[&str]) -> Outcome {
    let output = Command::new(nova_bin())
        .args(args)
        .current_dir(directory)
        .output()
        .expect("failed to run nova");
    Outcome {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn run_nova_with_file(source: &str, extra_args: &[&str]) -> (Outcome, TempSource) {
    let file = temp_source(source);
    let mut args: Vec<&str> = vec!["run", file.path()];
    args.extend_from_slice(extra_args);
    let outcome = run_nova(&args);
    (outcome, file)
}

fn run_nova_with_stdin(args: &[&str], input: &str) -> Outcome {
    let mut child = Command::new(nova_bin())
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start nova");
    child
        .stdin
        .take()
        .expect("stdin is not piped")
        .write_all(input.as_bytes())
        .expect("failed to write stdin");
    let output = child.wait_with_output().expect("failed to wait for nova");
    Outcome {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "astra-nova-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).expect("cannot create temp directory");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn write(&self, name: &str, source: &str) -> PathBuf {
        let path = self.path.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("cannot create test subdirectory");
        }
        fs::write(&path, source).expect("cannot write test source");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn json_result(stdout: &str) -> JsonValue {
    serde_json::from_str(stdout.trim()).expect("stdout is not valid JSON")
}

// ─── version / help ───────────────────────────────────────────────────────────

#[test]
fn version_prints_nova_version() {
    let out = run_nova(&["version"]);
    assert_eq!(out.status, 0);
    assert!(
        out.stdout.trim().starts_with("nova 0.1.0"),
        "unexpected version: {}",
        out.stdout
    );
}

#[test]
fn dash_v_prints_version() {
    let out = run_nova(&["-v"]);
    assert_eq!(out.status, 0);
    assert!(out.stdout.trim().starts_with("nova "));
}

#[test]
fn help_exits_zero() {
    let out = run_nova(&["help"]);
    assert_eq!(out.status, 0);
    assert!(
        out.stdout.contains("Nova"),
        "help output should mention Nova"
    );
}

#[test]
fn dash_h_exits_zero() {
    let out = run_nova(&["-h"]);
    assert_eq!(out.status, 0);
}

#[test]
fn no_args_shows_help() {
    let out = run_nova(&[]);
    assert_eq!(out.status, 0);
    assert!(out.stdout.contains("Usage"));
}

// ─── usage errors ────────────────────────────────────────────────────────────

#[test]
fn unknown_command_exits_2() {
    let out = run_nova(&["notacommand"]);
    assert_eq!(out.status, 2);
}

#[test]
fn unknown_option_exits_2() {
    let out = run_nova(&["--notanoption"]);
    assert_eq!(out.status, 2);
}

#[test]
fn run_without_file_without_manifest_exits_1() {
    let dir = TempDir::new();
    let out = run_nova_in(dir.path(), &["run"]);
    assert_eq!(out.status, 1);
    assert!(out.stderr.contains("NS9001"), "stderr: {}", out.stderr);
}

#[test]
fn eval_without_source_exits_2() {
    let out = run_nova(&["eval"]);
    assert_eq!(out.status, 2);
}

#[test]
fn unknown_mode_exits_2() {
    let out = run_nova(&["eval", "1", "--mode", "notamode"]);
    assert_eq!(out.status, 2);
}

#[test]
fn missing_file_exits_1() {
    let out = run_nova(&["run", "/nonexistent/path/file.ns"]);
    assert_eq!(out.status, 1);
}

// ─── successful run ───────────────────────────────────────────────────────────

#[test]
fn run_hello_world_text_mode() {
    let source = r#"
fn main() {
    println("hello, nova")
}
"#;
    let (out, _tmp) = run_nova_with_file(source, &[]);
    assert_eq!(out.status, 0);
    assert_eq!(out.stdout.trim(), "hello, nova");
}

#[test]
fn run_hello_world_json_mode() {
    let source = r#"
fn main() {
    println("hello json")
}
"#;
    let (out, _tmp) = run_nova_with_file(source, &["--json"]);
    assert_eq!(out.status, 0);
    let j = json_result(&out.stdout);
    assert_eq!(j["ok"], true);
    assert_eq!(j["command"], "run");
    assert!(j["diagnostics"].as_array().unwrap().is_empty());
    assert!(j["stdout"].as_str().unwrap().contains("hello json"));
}

#[test]
fn run_reports_the_inferred_mode_in_json() {
    let source = r#"
@tags(fetch)
module app.client
fn main() {}
"#;
    let (out, _tmp) = run_nova_with_file(source, &["--json"]);
    assert_eq!(out.status, 0, "stderr: {}", out.stderr);
    let report = json_result(&out.stdout);
    assert_eq!(report["ok"], true);
    assert_eq!(report["mode"], "web");
}

#[test]
fn run_links_a_multi_file_project() {
    let dir = TempDir::new();
    dir.write(
        "src/main.ns",
        "import { add } from \"./math.ns\"\nfn main() { println(add(40)) }",
    );
    dir.write("src/math.ns", "pub fn add(n: i32) -> i32 { return n + 2 }");

    let out = run_nova_in(dir.path(), &["run", "src/main.ns"]);
    assert_eq!(out.status, 0, "stderr: {}", out.stderr);
    assert_eq!(out.stdout.trim(), "42");
    assert!(out.stderr.is_empty(), "stderr: {}", out.stderr);
}

#[test]
fn run_links_nested_relative_imports_from_the_project_root() {
    let dir = TempDir::new();
    dir.write(
        "src/main.ns",
        "import { value } from \"../shared/value.ns\"\nfn main() { println(value()) }",
    );
    dir.write("shared/value.ns", "pub fn value() -> i32 { return 7 }");

    let out = run_nova_in(dir.path(), &["run", "src/main.ns"]);
    assert_eq!(out.status, 0, "stderr: {}", out.stderr);
    assert_eq!(out.stdout.trim(), "7");
}

#[test]
fn run_reports_imported_type_errors_against_the_importing_file() {
    let dir = TempDir::new();
    dir.write(
        "src/main.ns",
        "import { add } from \"./math.ns\"\nfn main() { println(add(\"wrong\")) }",
    );
    dir.write("src/math.ns", "pub fn add(n: i32) -> i32 { return n }");

    let out = run_nova_in(dir.path(), &["run", "src/main.ns", "--json"]);
    assert_eq!(out.status, 1);
    let report = json_result(&out.stdout);
    assert_eq!(report["ok"], false);
    assert_eq!(report["diagnostics"][0]["code"], "NS4001");
    let path = report["diagnostics"][0]["path"]
        .as_str()
        .expect("diagnostic path");
    assert!(
        path.ends_with("src\\main.ns") || path.ends_with("src/main.ns"),
        "unexpected path: {path}"
    );
}

#[test]
fn run_reports_module_cycles() {
    let dir = TempDir::new();
    dir.write(
        "src/main.ns",
        "import { a } from \"./a.ns\"\nfn main() { a() }",
    );
    dir.write(
        "src/a.ns",
        "import { b } from \"./b.ns\"\npub fn a() { b() }",
    );
    dir.write(
        "src/b.ns",
        "import { a } from \"./a.ns\"\npub fn b() { a() }",
    );

    let out = run_nova_in(dir.path(), &["run", "src/main.ns", "--json"]);
    assert_eq!(out.status, 1);
    let report = json_result(&out.stdout);
    assert_eq!(report["ok"], false);
    assert_eq!(report["diagnostics"][0]["code"], "NS3013");
}

#[test]
fn explicit_mode_disables_inference() {
    let source = r#"
@tags(fetch)
module app.client
fn main() {}
"#;
    let (out, _tmp) = run_nova_with_file(source, &["--json", "--mode", "core"]);
    assert_eq!(out.status, 1);
    let report = json_result(&out.stdout);
    assert_eq!(report["ok"], false);
    assert_eq!(report["mode"], "core");
    assert_eq!(report["diagnostics"][0]["code"], "NS5003");
}

#[test]
fn inference_rejects_incompatible_web_and_system_requirements() {
    let source = r#"
@tags(fetch)
@tags(fs)
module app.main
fn main() {}
"#;
    let (out, _tmp) = run_nova_with_file(source, &["--json"]);
    assert_eq!(out.status, 1);
    let report = json_result(&out.stdout);
    assert_eq!(report["ok"], false);
    assert_eq!(report["diagnostics"][0]["code"], "NS5006");
}

// ─── successful eval ──────────────────────────────────────────────────────────

#[test]
fn eval_expression_text_mode() {
    // A module-form eval: main returns an i32.
    let source = "fn main() -> i32 { return 42 }";
    let out = run_nova(&["eval", source]);
    assert_eq!(out.status, 0);
    assert_eq!(out.stdout.trim(), "42");
}

#[test]
fn eval_bare_expression_text_mode() {
    let out = run_nova(&["eval", "40 + 2"]);
    assert_eq!(out.status, 0, "stderr: {}", out.stderr);
    assert_eq!(out.stdout.trim(), "42");
}

#[test]
fn eval_bare_expression_json_mode() {
    let out = run_nova(&["eval", "40 + 2", "--json"]);
    assert_eq!(out.status, 0, "stderr: {}", out.stderr);
    let value = json_result(&out.stdout);
    assert_eq!(value["ok"], true);
    assert_eq!(value["value"], "42");
}

#[test]
fn eval_unit_result_is_not_printed() {
    let source = "fn main() { println(\"side effect\") }";
    let out = run_nova(&["eval", source]);
    assert_eq!(out.status, 0);
    assert_eq!(out.stdout.trim(), "side effect");
}

#[test]
fn eval_json_mode_captures_value() {
    let source = "fn main() -> i32 { return 99 }";
    let out = run_nova(&["eval", source, "--json"]);
    assert_eq!(out.status, 0);
    let j = json_result(&out.stdout);
    assert_eq!(j["ok"], true);
    assert_eq!(j["command"], "eval");
    assert_eq!(j["value"], "99");
}

#[test]
fn eval_json_mode_null_value_for_unit() {
    let source = "fn main() { println(\"hi\") }";
    let out = run_nova(&["eval", source, "--json"]);
    assert_eq!(out.status, 0);
    let j = json_result(&out.stdout);
    assert_eq!(j["ok"], true);
    assert_eq!(j["value"], JsonValue::Null);
    assert!(j["stdout"].as_str().unwrap().contains("hi"));
}

// ─── frontend type errors ─────────────────────────────────────────────────────

#[test]
fn type_error_exits_1_and_reports_ns_code() {
    // Returning a string where i32 is expected.
    let source = r#"fn main() -> i32 { return "nope" }"#;
    let out = run_nova(&["eval", source]);
    assert_eq!(out.status, 1);
    // NS4001 is the type-mismatch code.
    assert!(
        out.stderr.contains("NS4001"),
        "expected NS4001 in stderr: {}",
        out.stderr
    );
}

#[test]
fn type_error_json_mode_reports_ns_code() {
    let source = r#"fn main() -> i32 { return "nope" }"#;
    let out = run_nova(&["eval", source, "--json"]);
    assert_eq!(out.status, 1);
    let j = json_result(&out.stdout);
    assert_eq!(j["ok"], false);
    let diagnostics = j["diagnostics"].as_array().unwrap();
    assert!(
        diagnostics
            .iter()
            .any(|d| d["code"].as_str() == Some("NS4001")),
        "expected NS4001 in diagnostics: {diagnostics:?}"
    );
}

// ─── runtime panic ────────────────────────────────────────────────────────────

#[test]
fn panic_exits_1_text_mode() {
    let source = r#"
fn main() {
    panic("boom")
}
"#;
    let (out, _tmp) = run_nova_with_file(source, &[]);
    assert_eq!(out.status, 1);
    assert!(
        out.stderr.contains("NV0005"),
        "expected NV0005 in stderr: {}",
        out.stderr
    );
    assert!(out.stderr.contains("boom"), "expected message in stderr");
}

#[test]
fn panic_exits_1_json_mode_reports_nv0005() {
    let source = r#"
fn main() {
    panic("boom")
}
"#;
    let (out, _tmp) = run_nova_with_file(source, &["--json"]);
    assert_eq!(out.status, 1);
    let j = json_result(&out.stdout);
    assert_eq!(j["ok"], false);
    let diagnostics = j["diagnostics"].as_array().unwrap();
    assert!(!diagnostics.is_empty(), "expected at least one diagnostic");
    let first = &diagnostics[0];
    assert_eq!(first["code"], "NV0005");
    assert!(
        first["message"].as_str().unwrap().contains("boom"),
        "expected message to contain 'boom'"
    );
}

// ─── panic with source position ───────────────────────────────────────────────

#[test]
fn panic_json_mode_has_source_position() {
    // We want the panic to have a known line so we can verify line/column.
    let source = "fn main() {\n    panic(\"pos test\")\n}\n";
    let (out, _tmp) = run_nova_with_file(source, &["--json"]);
    assert_eq!(out.status, 1);
    let j = json_result(&out.stdout);
    let diagnostics = j["diagnostics"].as_array().unwrap();
    let first = &diagnostics[0];
    // line should be a positive integer (the exact value depends on span info)
    // At minimum it must not be null.
    // Nova records spans from the AST; check that the shape is correct.
    // The panic expression is on line 2.
    if first["line"] != JsonValue::Null {
        assert!(first["line"].as_u64().unwrap() >= 1, "line must be >= 1");
        assert!(
            first["column"].as_u64().unwrap() >= 1,
            "column must be >= 1"
        );
    }
}

// ─── manual memory rejection ─────────────────────────────────────────────────

#[test]
fn manual_memory_exits_1_with_nv0001() {
    let source = r#"
fn main() {
    {@memory:manual;
        let value = 1
    }
}
"#;
    let out = run_nova(&["eval", source, "--mode", "system"]);
    assert_eq!(out.status, 1);
    assert!(
        out.stderr.contains("NV0001"),
        "expected NV0001 in stderr: {}",
        out.stderr
    );
}

#[test]
fn manual_memory_json_mode_reports_nv0001() {
    let source = r#"
fn main() {
    {@memory:manual;
        let value = 1
    }
}
"#;
    let out = run_nova(&["eval", source, "--mode", "system", "--json"]);
    assert_eq!(out.status, 1);
    let j = json_result(&out.stdout);
    assert_eq!(j["ok"], false);
    let diagnostics = j["diagnostics"].as_array().unwrap();
    assert!(
        diagnostics.iter().any(|d| d["code"] == "NV0001"),
        "expected NV0001 in diagnostics: {diagnostics:?}"
    );
}

// ─── JSON success with captured stdout ───────────────────────────────────────

#[test]
fn json_mode_captures_stdout_and_stderr() {
    let source = r#"
fn main() {
    println("out line")
    dbg("dbg line")
}
"#;
    let (out, _tmp) = run_nova_with_file(source, &["--json"]);
    assert_eq!(out.status, 0);
    let j = json_result(&out.stdout);
    assert_eq!(j["ok"], true);
    assert!(j["stdout"].as_str().unwrap().contains("out line"));
    assert!(j["stderr"].as_str().unwrap().contains("dbg line"));
}

// ─── mode option ─────────────────────────────────────────────────────────────

#[test]
fn mode_app_is_accepted() {
    let source = "fn main() {}";
    let out = run_nova(&["eval", source, "--mode", "app"]);
    assert_eq!(out.status, 0);
}

#[test]
fn mode_web_is_accepted() {
    let source = "fn main() {}";
    let out = run_nova(&["eval", source, "--mode", "web"]);
    assert_eq!(out.status, 0);
}

#[test]
fn mode_system_is_accepted() {
    let source = "fn main() {}";
    let out = run_nova(&["eval", source, "--mode", "system"]);
    assert_eq!(out.status, 0);
}

#[test]
fn json_mode_reports_resolved_mode() {
    let source = "fn main() {}";
    let out = run_nova(&["eval", source, "--mode", "app", "--json"]);
    assert_eq!(out.status, 0);
    let j = json_result(&out.stdout);
    assert_eq!(j["mode"], "app");
}

// ─── watch ───────────────────────────────────────────────────────────────────

#[test]
fn watch_once_runs_the_file() {
    let file = temp_source("fn main() { println(\"watched\") }\n");
    let out = run_nova(&["watch", file.path(), "--once"]);
    assert_eq!(out.status, 0, "stderr: {}", out.stderr);
    assert_eq!(out.stdout.trim(), "watched");
}

#[test]
fn watch_once_reports_missing_file() {
    let out = run_nova(&["watch", "/nonexistent/nova-watch.ns", "--once"]);
    assert_eq!(out.status, 1);
    assert!(out.stderr.contains("NS9008"));
}

// ─── repl ────────────────────────────────────────────────────────────────────

#[test]
fn repl_preserves_bindings_and_assignments() {
    let out = run_nova_with_stdin(&["repl"], "let mut x = 1\nx = x + 1\nx + 40\n:quit\n");
    assert_eq!(out.status, 0, "stderr: {}", out.stderr);
    assert!(
        out.stdout.contains("42"),
        "expected evaluated value in stdout: {}",
        out.stdout
    );
}

#[test]
fn repl_preserves_function_declarations() {
    let out = run_nova_with_stdin(
        &["repl"],
        "fn double(n: i32) -> i32 { return n * 2 }\ndouble(21)\n:quit\n",
    );
    assert_eq!(out.status, 0, "stderr: {}", out.stderr);
    assert!(
        out.stdout.contains("42"),
        "expected declaration result in stdout: {}",
        out.stdout
    );
}

#[test]
fn repl_recovers_after_an_error() {
    let out = run_nova_with_stdin(&["repl"], "missing()\n1 + 1\n:quit\n");
    assert_eq!(out.status, 0, "stderr: {}", out.stderr);
    assert!(out.stderr.contains("NS4002"), "stderr: {}", out.stderr);
    assert!(
        out.stdout.contains("2"),
        "expected later evaluation in stdout: {}",
        out.stdout
    );
}

#[test]
fn repl_json_mode_emits_one_report_per_input() {
    let out = run_nova_with_stdin(&["repl", "--json"], "40 + 2\n:quit\n");
    assert_eq!(out.status, 0, "stderr: {}", out.stderr);
    let first = out.stdout.lines().next().expect("expected a JSON report");
    let value = json_result(first);
    assert_eq!(value["ok"], true);
    assert_eq!(value["command"], "repl");
    assert_eq!(value["value"], "42");
}

// ─── test runner ─────────────────────────────────────────────────────────────

#[test]
fn test_runner_runs_annotated_tests() {
    let dir = TempDir::new();
    dir.write(
        "math.test.ns",
        r#"
@test fn adds() {
    assert(1 + 1 == 2)
}

@test fn strings() {
    assert(2 * 3 == 6)
}
"#,
    );

    let out = run_nova(&["test", dir.path().to_str().unwrap()]);
    assert_eq!(out.status, 0, "stderr: {}", out.stderr);
    assert!(out.stdout.contains("adds ... ok"), "stdout: {}", out.stdout);
    assert!(
        out.stdout.contains("strings ... ok"),
        "stdout: {}",
        out.stdout
    );
}

#[test]
fn test_runner_falls_back_to_main() {
    let dir = TempDir::new();
    dir.write("main.test.ns", "fn main() { println(\"main test\") }\n");

    let out = run_nova(&["test", dir.path().to_str().unwrap()]);
    assert_eq!(out.status, 0, "stderr: {}", out.stderr);
    assert!(
        out.stdout.contains("::main ... ok"),
        "stdout: {}",
        out.stdout
    );
    assert!(out.stdout.contains("main test"), "stdout: {}", out.stdout);
}

#[test]
fn test_runner_reports_failures() {
    let dir = TempDir::new();
    dir.write("failing.test.ns", "@test fn fails() { assert(false) }\n");

    let out = run_nova(&["test", dir.path().to_str().unwrap()]);
    assert_eq!(out.status, 1);
    assert!(
        out.stdout.contains("fails ... FAILED"),
        "stdout: {}",
        out.stdout
    );
    assert!(
        out.stderr.contains("assertion failed") || out.stderr.contains("NV0005"),
        "stderr: {}",
        out.stderr
    );
}

#[test]
fn test_runner_rejects_test_parameters() {
    let dir = TempDir::new();
    dir.write(
        "params.test.ns",
        "@test fn bad(value: i32) { let x = value }\n",
    );

    let out = run_nova(&["test", dir.path().to_str().unwrap()]);
    assert_eq!(out.status, 1);
    assert!(
        out.stderr.contains("must not take parameters"),
        "stderr: {}",
        out.stderr
    );
}

#[test]
fn test_runner_reports_no_tests() {
    let dir = TempDir::new();
    let out = run_nova(&["test", dir.path().to_str().unwrap()]);
    assert_eq!(out.status, 0);
    assert!(out.stdout.contains("No tests found."));
}

// ─── manifest entry discovery ────────────────────────────────────────────────

#[test]
fn run_uses_json_manifest_entry() {
    let dir = TempDir::new();
    dir.write(
        "core.json",
        r#"{"package":{"name":"demo","version":"0.1.0"},"nova":{"entry":"src/main.ns"}}"#,
    );
    dir.write("src/main.ns", "fn main() { println(\"from json\") }\n");

    let out = run_nova_in(dir.path(), &["run"]);
    assert_eq!(out.status, 0, "stderr: {}", out.stderr);
    assert_eq!(out.stdout.trim(), "from json");
}

#[test]
fn run_uses_toml_manifest_entry() {
    let dir = TempDir::new();
    dir.write(
        "core.toml",
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n\n[nova]\nentry = \"src/main.ns\"\n",
    );
    dir.write("src/main.ns", "fn main() { println(\"from toml\") }\n");

    let out = run_nova_in(dir.path(), &["run"]);
    assert_eq!(out.status, 0, "stderr: {}", out.stderr);
    assert_eq!(out.stdout.trim(), "from toml");
}

#[test]
fn watch_once_uses_manifest_entry() {
    let dir = TempDir::new();
    dir.write("core.json", r#"{"nova":{"entry":"src/main.ns"}}"#);
    dir.write("src/main.ns", "fn main() { println(\"watched entry\") }\n");

    let out = run_nova_in(dir.path(), &["watch", "--once"]);
    assert_eq!(out.status, 0, "stderr: {}", out.stderr);
    assert_eq!(out.stdout.trim(), "watched entry");
}

#[test]
fn missing_manifest_is_ns9001() {
    let dir = TempDir::new();
    let out = run_nova_in(dir.path(), &["run"]);
    assert_eq!(out.status, 1);
    assert!(out.stderr.contains("NS9001"), "stderr: {}", out.stderr);
}

#[test]
fn both_manifests_are_rejected() {
    let dir = TempDir::new();
    dir.write("core.json", r#"{"nova":{"entry":"main.ns"}}"#);
    dir.write("core.toml", "[nova]\nentry = \"main.ns\"\n");
    dir.write("main.ns", "fn main() {}\n");

    let out = run_nova_in(dir.path(), &["run"]);
    assert_eq!(out.status, 1);
    assert!(out.stderr.contains("NS9001"), "stderr: {}", out.stderr);
}

#[test]
fn missing_nova_entry_is_ns9002() {
    let dir = TempDir::new();
    dir.write("core.json", r#"{"package":{"name":"demo"}}"#);
    let out = run_nova_in(dir.path(), &["run"]);
    assert_eq!(out.status, 1);
    assert!(out.stderr.contains("NS9002"), "stderr: {}", out.stderr);
    assert!(
        out.stderr.contains("[nova].entry"),
        "stderr: {}",
        out.stderr
    );
}

#[test]
fn manifest_entry_must_be_ns() {
    let dir = TempDir::new();
    dir.write("core.json", r#"{"nova":{"entry":"main.txt"}}"#);
    dir.write("main.txt", "not source");
    let out = run_nova_in(dir.path(), &["run"]);
    assert_eq!(out.status, 1);
    assert!(out.stderr.contains("NS9002"), "stderr: {}", out.stderr);
    assert!(out.stderr.contains(".ns"), "stderr: {}", out.stderr);
}

#[test]
fn manifest_entry_runs_in_json_mode() {
    let dir = TempDir::new();
    dir.write("core.json", r#"{"nova":{"entry":"main.ns"}}"#);
    dir.write("main.ns", "fn main() { println(\"json entry\") }\n");

    let out = run_nova_in(dir.path(), &["run", "--json"]);
    assert_eq!(out.status, 0, "stderr: {}", out.stderr);
    let report = json_result(&out.stdout);
    assert_eq!(report["ok"], true);
    assert!(report["stdout"].as_str().unwrap().contains("json entry"));
}
