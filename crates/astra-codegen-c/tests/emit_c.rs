//! Emission tests for the C17 backend.
//!
//! Each test:
//!  1. Parses + resolves + type-checks a minimal `.ns` snippet through the
//!     shared `astra-core` frontend (never re-parses in the backend itself).
//!  2. Calls `astra_codegen_c::generate` to produce a C translation unit.
//!  3. Asserts structural properties of the output (presence of expected
//!     identifiers, types, and patterns).
//!  4. Where `clang` is available on the machine, compiles the output with
//!     `-std=c17 -ffreestanding -Wall -c` to confirm it is valid C17 (no
//!     linker or libc needed).
//!
//! The clang path is probed once; if absent the compile-check is skipped and
//! the test still passes (the structural check alone guards correctness).

use std::path::PathBuf;

use astra_capabilities::Mode;
use astra_codegen_c::{CModule, EmitKind, IntegerPolicy, ProjectEntry, generate};
use astra_core::checker::{CheckOptions, check_resolved};
use astra_core::parser::parse;
use astra_core::resolver::resolve_module;
use astra_core::type_surface::{ProjectModule, check_module_graph};

// ─── helpers ─────────────────────────────────────────────────────────────────

fn compile_and_emit(source: &str) -> String {
    let module = parse(source).expect("parse failed");
    let resolved = resolve_module(&module).expect("resolve failed");
    let checked = check_resolved(&module, &resolved, CheckOptions::new(Mode::Core))
        .expect("type check failed");
    let c_module = CModule {
        name: checked.module.name.clone(),
        ast: module,
        hir: checked.module,
        annotations: checked.annotations,
        manual_memory_scopes: checked.manual_memory_scopes,
        integer_policy: IntegerPolicy::Wrapping,
    };
    let generated = generate(&c_module).expect("C generation failed");
    generated.source
}

/// Returns the path to clang if available, or None.
fn clang_path() -> Option<PathBuf> {
    // Try the known Windows LLVM install first, then PATH.
    resolve_tool([
        "C:\\Program Files\\LLVM\\bin\\clang.exe",
        "clang",
        "clang-19",
        "clang-18",
    ])
}

fn tool_path(candidate: &str) -> Option<PathBuf> {
    let path = PathBuf::from(candidate);
    if path.exists() {
        return Some(path);
    }
    if let Ok(output) = std::process::Command::new(candidate)
        .arg("--version")
        .output()
        && output.status.success()
    {
        return Some(PathBuf::from(candidate));
    }
    None
}

fn resolve_tool<const N: usize>(candidates: [&str; N]) -> Option<PathBuf> {
    candidates.iter().find_map(|candidate| tool_path(candidate))
}

/// Returns a compiler that can build a *hosted* program (libc headers and a
/// linker), or None. clang on a Windows host without an SDK cannot do this even
/// though it can compile freestanding objects, so the probe is a real compile.
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

fn hosted_probe(compiler: &std::path::Path) -> bool {
    let Ok(dir) = tempfile::tempdir() else {
        return false;
    };
    let probe = dir.path().join("probe.c");
    let obj = dir.path().join("probe.o");
    if std::fs::write(&probe, "#include <stdio.h>\nint main(void) { return 0; }\n").is_err() {
        return false;
    }
    std::process::Command::new(compiler)
        .args(["-std=c17", "-c", "-o"])
        .arg(&obj)
        .arg(&probe)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Writes the generated C + runtime header to a temp dir, compiles it
/// freestanding (no libc), and returns the clang exit status + stderr.
/// Returns `None` if clang is not available (test skips the check).
fn clang_check(c_source: &str) -> Option<(bool, String)> {
    let clang = clang_path()?;
    let dir = tempfile::tempdir().expect("tempdir");
    let src_path = dir.path().join("module.c");
    std::fs::write(&src_path, c_source).expect("write");
    let output = std::process::Command::new(&clang)
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
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    Some((output.status.success(), stderr))
}

fn assert_clang_ok(c_source: &str) {
    if let Some((ok, stderr)) = clang_check(c_source) {
        assert!(
            ok,
            "clang rejected the generated C:\n{stderr}\n\nSource:\n{c_source}"
        );
    }
}

/// Compiles the generated C + host shim into an executable, runs it, and
/// returns (success, stdout, stderr).
/// Returns `None` when no hosted-capable compiler is available.
fn hosted_build_and_run(c_source: &str) -> Option<(bool, String, String)> {
    let compiler = hosted_compiler()?;
    let dir = tempfile::tempdir().expect("tempdir");
    let src_path = dir.path().join("module.c");
    let host_path = dir.path().join("astra_host.c");
    let exe_path = dir.path().join("prog.exe");
    std::fs::write(&src_path, c_source).expect("write module.c");
    std::fs::write(&host_path, astra_codegen_c::runtime::host_shim()).expect("write astra_host.c");
    let output = std::process::Command::new(&compiler)
        .args(["-std=c17", "-Wall", "-Wno-unused-variable", "-o"])
        .arg(&exe_path)
        .arg(&src_path)
        .arg(&host_path)
        .output()
        .expect("hosted compiler failed to link");
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Some((false, String::new(), stderr));
    }
    let run_output = std::process::Command::new(&exe_path)
        .output()
        .expect("failed to run compiled executable");
    let stdout = String::from_utf8_lossy(&run_output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&run_output.stderr).into_owned();
    Some((run_output.status.success(), stdout, stderr))
}

/// Compiles a multi-module project (key → source, in any order) through the
/// shared graph frontend and emits one translation unit per module.
///
/// Returns the emitted sources keyed by module, plus the header for each.
fn compile_project_and_emit(modules: &[(&str, &str)]) -> Vec<(String, String, Option<String>)> {
    try_compile_project(modules).expect("project generation failed")
}

/// Like [`compile_project_and_emit`] but surfaces the backend error instead of
/// panicking, so rejection tests can assert on the exact diagnostic code.
fn try_compile_project(
    modules: &[(&str, &str)],
) -> Result<Vec<(String, String, Option<String>)>, astra_codegen_c::BackendError> {
    try_compile_project_kind(modules, EmitKind::Executable)
}

/// Project emission for a specific artifact kind.
fn try_compile_project_kind(
    modules: &[(&str, &str)],
    kind: EmitKind,
) -> Result<Vec<(String, String, Option<String>)>, astra_codegen_c::BackendError> {
    let project: Vec<ProjectModule> = modules
        .iter()
        .map(|(key, source)| ProjectModule {
            key: (*key).to_string(),
            source: parse(source).expect("parse failed"),
        })
        .collect();
    let graph = check_module_graph(&project, CheckOptions::new(Mode::Core));
    assert!(
        graph.is_ok(),
        "project type check failed: {:?}",
        graph.all_diagnostics()
    );

    let entries: Vec<ProjectEntry> = project
        .iter()
        .map(|pm| {
            let checked = graph.get(&pm.key).expect("module missing from graph");
            let dep_keys: Vec<String> = checked
                .module
                .imports
                .iter()
                .map(|imp| imp.module.clone())
                .collect();
            ProjectEntry {
                module: CModule {
                    name: checked.module.name.clone(),
                    ast: pm.source.clone(),
                    hir: checked.module.clone(),
                    annotations: checked.annotations.clone(),
                    manual_memory_scopes: checked.manual_memory_scopes.clone(),
                    integer_policy: IntegerPolicy::Wrapping,
                },
                key: pm.key.clone(),
                dep_keys,
            }
        })
        .collect();

    let generated = astra_codegen_c::generate_project_with_kind(&entries, kind)?;
    Ok(entries
        .iter()
        .zip(generated.iter())
        .map(|(entry, out)| (entry.key.clone(), out.source.clone(), out.header.clone()))
        .collect())
}

/// Returns the emitted translation unit for module `key`.
fn project_source<'a>(emitted: &'a [(String, String, Option<String>)], key: &str) -> &'a str {
    &emitted
        .iter()
        .find(|(k, _, _)| k == key)
        .unwrap_or_else(|| panic!("module '{key}' not emitted"))
        .1
}

/// Returns the emitted public header for module `key`.
fn project_header<'a>(emitted: &'a [(String, String, Option<String>)], key: &str) -> &'a str {
    emitted
        .iter()
        .find(|(k, _, _)| k == key)
        .unwrap_or_else(|| panic!("module '{key}' not emitted"))
        .2
        .as_deref()
        .expect("module has no header")
}

/// Writes every emitted module and header, then compiles each module as its own
/// translation unit with clang. Pins header/runtime typedef ordering, which a
/// single-file check cannot catch (a dependency header re-includes the runtime
/// header).
fn assert_project_compiles(emitted: &[(String, String, Option<String>)]) {
    let Some(clang) = clang_path() else {
        return;
    };
    let dir = tempfile::tempdir().expect("tempdir");
    let stem_of = |key: &str| {
        key.strip_suffix(".ns")
            .unwrap_or(key)
            .replace(['/', '.', '-'], "_")
    };
    for (key, source, header) in emitted {
        let stem = stem_of(key);
        if let Some(header) = header {
            std::fs::write(dir.path().join(format!("{stem}.h")), header).expect("write header");
        }
        std::fs::write(dir.path().join(format!("{stem}.c")), source).expect("write source");
    }
    for (key, _, _) in emitted {
        let stem = stem_of(key);
        let output = std::process::Command::new(&clang)
            .args([
                "-std=c17",
                "-ffreestanding",
                "-Wall",
                "-Wno-unused-variable",
                "-I",
            ])
            .arg(dir.path())
            .arg("-c")
            .arg(dir.path().join(format!("{stem}.c")))
            .arg("-o")
            .arg(dir.path().join(format!("{stem}.o")))
            .output()
            .expect("clang failed to start");
        assert!(
            output.status.success(),
            "clang rejected module '{key}':\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

// ─── tests ────────────────────────────────────────────────────────────────────

#[test]
fn empty_module_emits_valid_header() {
    let out = compile_and_emit("module test.empty\n");
    assert!(out.contains("stdint.h"), "missing stdint include");
    assert!(out.contains("stdbool.h"), "missing stdbool include");
    assert!(out.contains("NS_Unit"), "missing NS_Unit typedef");
    assert_clang_ok(&out);
}

#[test]
fn simple_fn_emits_prototype_and_definition() {
    let src = "module test.funcs\npub fn add(a: i32, b: i32) -> i32 { return a + b }\n";
    let out = compile_and_emit(src);
    assert!(
        out.contains("static int32_t ns_add(int32_t v_a, int32_t v_b)"),
        "{out}"
    );
    // Wrapping policy: signed add must use the helper, not raw `+`.
    assert!(out.contains("astra_add_i32"), "{out}");
    assert_clang_ok(&out);
}

#[test]
fn void_fn_emits_void_return() {
    let src = "pub fn say_hi() { println(\"hi\") }\n";
    let out = compile_and_emit(src);
    assert!(out.contains("static void ns_say_hi(void)"), "{out}");
    assert!(out.contains("astra_println_str"), "{out}");
    assert_clang_ok(&out);
}

#[test]
fn main_fn_emits_c_main() {
    let src = "pub fn main() { println(\"hello\") }\n";
    let out = compile_and_emit(src);
    assert!(out.contains("int main(void)"), "{out}");
    assert_clang_ok(&out);
}

#[test]
fn integer_literals_strip_underscores() {
    let src = "pub fn big() -> i32 { return 1_000_000 }\n";
    let out = compile_and_emit(src);
    assert!(out.contains("1000000"), "{out}");
    assert!(!out.contains("1_000"), "{out}");
    assert_clang_ok(&out);
}

#[test]
fn bool_literals_emit_c_booleans() {
    let src = "pub fn yes() -> bool { return true }\n";
    let out = compile_and_emit(src);
    assert!(out.contains("return true"), "{out}");
    assert_clang_ok(&out);
}

#[test]
fn string_literal_emits_ns_str() {
    let src = r#"pub fn greet() -> str { return "hello" }"#;
    let out = compile_and_emit(&format!("{src}\n"));
    assert!(out.contains("NS_Str"), "{out}");
    assert!(out.contains("\"hello\""), "{out}");
    assert_clang_ok(&out);
}

#[test]
fn constant_emits_static_const() {
    let src = "const ANSWER: i32 = 42\npub fn get() -> i32 { return ANSWER }\n";
    let out = compile_and_emit(src);
    assert!(
        out.contains("static const int32_t const_ANSWER = 42"),
        "{out}"
    );
    assert!(out.contains("const_ANSWER"), "{out}");
    assert_clang_ok(&out);
}

#[test]
fn let_binding_emits_local_var() {
    let src = "pub fn calc() -> i32 {\n  let x: i32 = 10\n  return x\n}\n";
    let out = compile_and_emit(src);
    assert!(out.contains("int32_t v_x = 10"), "{out}");
    assert_clang_ok(&out);
}

#[test]
fn if_else_emits_conditional() {
    let src = "pub fn abs(x: i32) -> i32 {\n  if x < 0 { return -x } else { return x }\n}\n";
    let out = compile_and_emit(src);
    assert!(out.contains("if ("), "{out}");
    assert_clang_ok(&out);
}
#[test]
fn while_loop_emits_while() {
    let src = "pub fn countdown(n: i32) {\n  let mut count: i32 = n\n  while count > 0 { count = count - 1 }\n}\n";
    let out = compile_and_emit(src);
    assert!(out.contains("while ("), "{out}");
    assert_clang_ok(&out);
}

#[test]
fn named_struct_emits_typedef_and_init() {
    let src = r#"
struct Point { x: i32, y: i32 }
pub fn make() -> Point { return Point { x: 1, y: 2 } }
"#;
    let out = compile_and_emit(src);
    assert!(out.contains("typedef struct"), "{out}");
    assert!(out.contains("int32_t x;"), "{out}");
    assert!(out.contains("NS_Point"), "{out}");
    assert!(out.contains(".x = 1"), "{out}");
    assert_clang_ok(&out);
}

#[test]
fn enum_emits_tag_enum_and_union() {
    let src = r#"
enum Color { Red, Green, Blue }
pub fn red() -> Color { return Color.Red }
"#;
    let out = compile_and_emit(src);
    assert!(out.contains("typedef enum"), "{out}");
    assert!(out.contains("NS_Color_TAG_Red"), "{out}");
    assert!(out.contains("typedef struct"), "{out}");
    assert!(out.contains("union"), "{out}");
    assert_clang_ok(&out);
}

#[test]
fn option_some_none_emit_tagged_struct() {
    let src = "pub fn wrap(x: i32) -> Option<i32> { return Some(x) }\n";
    let out = compile_and_emit(src);
    assert!(out.contains("NS_Option_i32"), "{out}");
    assert!(out.contains(".some = true"), "{out}");
    assert_clang_ok(&out);
}

#[test]
fn result_ok_err_emit_tagged_struct() {
    let src = r#"
struct ParseError { msg: str }
pub fn parse(s: str) -> i32 ! ParseError {
  if s.is_empty() { return !ParseError { msg: "empty" } }
  return 0
}
"#;
    let out = compile_and_emit(src);
    assert!(out.contains("NS_Result_"), "{out}");
    assert!(out.contains(".ok = true"), "{out}");
    assert!(out.contains(".ok = false"), "{out}");
    assert_clang_ok(&out);
}

#[test]
fn error_propagation_emits_early_return() {
    let src = r#"
struct E { msg: str }
fn inner() -> i32 ! E { return 1 }
pub fn outer() -> i32 ! E {
  let v = inner()!
  return v
}
"#;
    let out = compile_and_emit(src);
    // The `!` propagator must emit an early-return on failure.
    assert!(out.contains("if (!"), "{out}");
    assert!(out.contains("return"), "{out}");
    assert_clang_ok(&out);
}

#[test]
fn match_on_bool_emits_if_chain() {
    let src = "pub fn describe(b: bool) -> i32 {\n  return match b {\n    true => 1\n    _ => 0\n  }\n}\n";
    let out = compile_and_emit(src);
    assert!(out.contains("if ("), "{out}");
    assert_clang_ok(&out);
}

#[test]
fn match_on_option_emits_if_some() {
    let src = r#"
pub fn unwrap_or(x: Option<i32>) -> i32 {
  return match x {
    Some(v) => v
    _ => 0
  }
}
"#;
    let out = compile_and_emit(src);
    assert!(out.contains("if ("), "{out}");
    assert!(out.contains(".some"), "{out}");
    assert_clang_ok(&out);
}

#[test]
fn match_on_result_emits_if_ok() {
    let src = r#"
struct E { code: i32 }
pub fn try_val(r: Result<i32, E>) -> i32 {
  return match r {
    Ok(v) => v
    Err(_) => -1
  }
}
"#;
    let out = compile_and_emit(src);
    assert!(out.contains(".ok"), "{out}");
    assert_clang_ok(&out);
}

#[test]
fn match_on_enum_emits_switch() {
    let src = r#"
enum Dir { North, South, East, West }
pub fn is_north(d: Dir) -> bool {
  return match d {
    Dir.North => true
    _ => false
  }
}
"#;
    let out = compile_and_emit(src);
    assert!(out.contains("switch ("), "{out}");
    assert!(out.contains("NS_Dir_TAG_North"), "{out}");
    assert_clang_ok(&out);
}

#[test]
fn array_literal_emits_ns_array() {
    let src = "pub fn nums() { let a = [1, 2, 3] }\n";
    let out = compile_and_emit(src);
    assert!(out.contains("astra_array_new_lit"), "{out}");
    assert_clang_ok(&out);
}

#[test]
fn print_int_emits_typed_helper() {
    let src = "pub fn show(x: i32) { print(x) }\n";
    let out = compile_and_emit(src);
    assert!(out.contains("astra_print_i32"), "{out}");
    assert_clang_ok(&out);
}

#[test]
fn println_str_emits_println_str() {
    let src = "pub fn greet(name: str) { println(name) }\n";
    let out = compile_and_emit(src);
    assert!(out.contains("astra_println_str"), "{out}");
    assert_clang_ok(&out);
}

#[test]
fn panic_emits_astra_panic() {
    let src = "pub fn boom() { panic(\"oops\") }\n";
    let out = compile_and_emit(src);
    assert!(out.contains("astra_panic"), "{out}");
    assert_clang_ok(&out);
}

#[test]
fn assert_emits_astra_assert() {
    let src = "pub fn check(x: bool) { assert(x) }\n";
    let out = compile_and_emit(src);
    assert!(out.contains("astra_assert"), "{out}");
    assert_clang_ok(&out);
}

#[test]
fn wrapping_add_uses_helper() {
    // The `+%` explicit-wrap operator must always use the helper regardless of policy.
    let src = "pub fn wrap(a: i32, b: i32) -> i32 { return a +% b }\n";
    let out = compile_and_emit(src);
    assert!(out.contains("astra_add_i32"), "{out}");
    assert_clang_ok(&out);
}

#[test]
fn unsigned_add_uses_plain_plus() {
    // Unsigned integer overflow is well-defined in C, so no helper needed.
    let src = "pub fn uadd(a: u32, b: u32) -> u32 { return a + b }\n";
    let out = compile_and_emit(src);
    // No astra_add helper for unsigned.
    assert!(!out.contains("astra_add_u32"), "{out}");
    assert!(out.contains("v_a + v_b"), "{out}");
    assert_clang_ok(&out);
}

#[test]
fn float_arithmetic_uses_plain_operators() {
    let src = "pub fn sum(a: f64, b: f64) -> f64 { return a + b }\n";
    let out = compile_and_emit(src);
    assert!(out.contains("double"), "{out}");
    let fn_body = &out[out.find("ns_sum").unwrap()..];
    assert!(!fn_body.contains("astra_add"), "{out}");
    assert!(fn_body.contains("v_a + v_b"), "{out}");
    assert_clang_ok(&out);
}

#[test]
fn manual_memory_is_rejected() {
    use astra_capabilities::Mode;
    use astra_core::checker::CheckOptions;
    use astra_core::checker::check_source;
    // system mode allows manual memory scopes
    let src = "{@memory:manual;\npub fn alloc() {}\n}\n";
    let checked = match check_source(src, CheckOptions::new(Mode::System)) {
        Ok(r) => r,
        Err(_diags) => {
            // Checker may also reject it with NS5xxx; either way the backend never sees it.
            return;
        }
    };
    let c_module = CModule {
        name: checked.module.name.clone(),
        ast: astra_core::parser::parse(src).unwrap(),
        hir: checked.module,
        annotations: checked.annotations,
        manual_memory_scopes: checked.manual_memory_scopes,
        integer_policy: IntegerPolicy::Wrapping,
    };
    let result = generate(&c_module);
    assert!(
        result.is_err(),
        "C backend should reject manual-memory scopes"
    );
    let err = result.unwrap_err();
    assert_eq!(err.code, "NS8012", "wrong rejection code: {}", err.code);
}

#[test]
fn unsupported_target_map_emits_ns_map() {
    // Maps are now supported — the backend must emit NS_Map via astra_map_new.
    use astra_core::checker::{CheckOptions, check_source};
    let src = "pub fn mm() { let m = {\"a\": 1} }\n";
    let checked = match check_source(src, CheckOptions::new(Mode::Core)) {
        Ok(r) => r,
        Err(_) => return, // if checker rejects it, backend never sees it
    };
    let c_module = CModule {
        name: checked.module.name.clone(),
        ast: astra_core::parser::parse(src).unwrap(),
        hir: checked.module,
        annotations: checked.annotations,
        manual_memory_scopes: checked.manual_memory_scopes,
        integer_policy: IntegerPolicy::Wrapping,
    };
    let result = generate(&c_module);
    assert!(
        result.is_ok(),
        "C backend should emit map literals: {:?}",
        result
    );
    let out = result.unwrap().source;
    assert!(
        out.contains("astra_map_new"),
        "expected astra_map_new:\n{out}"
    );
}

#[test]
fn tuple_struct_emits_positional_fields() {
    let src = r#"
struct Pair(i32, i32)
pub fn make() -> Pair { return Pair(1, 2) }
"#;
    let out = compile_and_emit(src);
    assert!(out.contains("NS_Pair"), "{out}");
    assert_clang_ok(&out);
}

#[test]
fn enum_with_payload_emits_union() {
    let src = r#"
enum Shape { Circle(f64), Square(f64) }
pub fn area(s: Shape) -> f64 {
  return match s {
    Shape.Circle(r) => r
    Shape.Square(side) => side
  }
}
"#;
    let out = compile_and_emit(src);
    assert!(out.contains("union"), "{out}");
    assert!(out.contains("NS_Shape_TAG_Circle"), "{out}");
    assert_clang_ok(&out);
}

#[test]
fn compiler_api_c_target_returns_artifacts() {
    use astra_compiler_api::API_VERSION;
    use astra_compiler_api::{CompileRequest, FrontendCompiler, IntegerPolicy, SourceUnit};

    let request = CompileRequest {
        api_version: API_VERSION.to_owned(),
        source: SourceUnit {
            path: "src/main.ns".to_owned(),
            text: "pub fn add(a: i32, b: i32) -> i32 { return a + b }\n".to_owned(),
        },
        target: "c".to_owned(),
        mode: None,
        integer_policy: IntegerPolicy::Wrapping,
    };

    let response = FrontendCompiler.compile(&request);
    assert!(
        response.program.is_some(),
        "expected program: {:?}",
        response.diagnostics
    );
    assert!(
        response.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        response.diagnostics
    );
    assert!(
        response.artifacts.iter().any(|a| a["type"] == "c-source"),
        "missing c-source artifact: {:?}",
        response.artifacts
    );
    assert!(
        response
            .artifacts
            .iter()
            .any(|a| a["type"] == "c-host-shim"),
        "missing c-host-shim artifact: {:?}",
        response.artifacts
    );
    // Verify the C source artifact contains the generated function.
    let c_src = response
        .artifacts
        .iter()
        .find(|a| a["type"] == "c-source")
        .and_then(|a| a["content"].as_str())
        .unwrap_or("");
    assert!(
        c_src.contains("ns_add"),
        "expected ns_add in C source: {c_src}"
    );
}

#[test]
fn compiler_api_rejects_unsupported_target() {
    use astra_compiler_api::{
        API_VERSION, CompileRequest, FrontendCompiler, IntegerPolicy, SourceUnit,
    };
    let request = CompileRequest {
        api_version: API_VERSION.to_owned(),
        source: SourceUnit {
            path: "src/main.ns".to_owned(),
            text: "fn main() {}\n".to_owned(),
        },
        target: "native".to_owned(),
        mode: None,
        integer_policy: IntegerPolicy::Wrapping,
    };
    let response = FrontendCompiler.compile(&request);
    assert!(response.program.is_none(), "expected failure");
    assert!(
        response.diagnostics.iter().any(|d| d.code == "NS8001"),
        "expected NS8001: {:?}",
        response.diagnostics
    );
}

#[test]
fn host_shim_contains_astra_panic() {
    use astra_codegen_c::runtime::HOST_SHIM;
    assert!(
        HOST_SHIM.contains("astra_panic"),
        "host shim missing astra_panic"
    );
    assert!(
        HOST_SHIM.contains("astra_println_i32"),
        "host shim missing astra_println_i32"
    );
    assert!(
        HOST_SHIM.contains("ASTRA_WRAP"),
        "host shim missing wrapping macros"
    );
}

/// Regression: `astra_host.c` is its own translation unit, so it must carry the
/// shared runtime declarations and every libc header it uses. It previously
/// omitted `NS_Bytes`/`NS_Map` (unknown type names) and `<inttypes.h>`/
/// `<math.h>` (`PRId8`, `INFINITY`), so every hosted native build failed.
#[test]
fn host_shim_is_a_complete_hosted_translation_unit() {
    let shim = astra_codegen_c::runtime::host_shim();
    for needle in [
        "NS_Bytes",
        "NS_Map",
        "NS_SliceError",
        "#include <inttypes.h>",
        "#include <math.h>",
        "#include <stdio.h>",
    ] {
        assert!(shim.contains(needle), "host shim missing {needle}");
    }
    assert!(
        shim.contains("typedef struct { const uint8_t* data; size_t len; } NS_Bytes;"),
        "host shim must use the runtime header's NS_Bytes typedef"
    );

    let Some(compiler) = hosted_compiler() else {
        return;
    };
    let dir = tempfile::tempdir().expect("tempdir");
    let src_path = dir.path().join("astra_host.c");
    let obj_path = dir.path().join("astra_host.o");
    std::fs::write(&src_path, &shim).expect("write astra_host.c");
    let output = std::process::Command::new(&compiler)
        .args(["-std=c17", "-Wall", "-c", "-o"])
        .arg(&obj_path)
        .arg(&src_path)
        .output()
        .expect("hosted compiler failed to start");
    assert!(
        output.status.success(),
        "astra_host.c did not compile:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn defer_runs_in_lifo_order() {
    let src = r#"
module test.defer_order

pub fn main() {
    println("start")
    defer println("defer-1")
    defer println("defer-2")
    println("end")
}
"#;
    let out = compile_and_emit(src);
    assert_clang_ok(&out);
    let pos_start = out.find("\"start\"").expect("start present");
    let pos_end = out.find("\"end\"").expect("end present");
    let pos_def2 = out.find("\"defer-2\"").expect("defer-2 present");
    let pos_def1 = out.find("\"defer-1\"").expect("defer-1 present");
    assert!(
        pos_start < pos_end && pos_end < pos_def2 && pos_def2 < pos_def1,
        "expected LIFO defer ordering in generated C"
    );
    if let Some((ok, stdout, stderr)) = hosted_build_and_run(&out) {
        assert!(ok, "execution failed:\nstderr:\n{stderr}");
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines, vec!["start", "end", "defer-2", "defer-1"]);
    }
}

#[test]
fn defer_runs_on_early_return() {
    let src = r#"
module test.defer_return

fn check(x: i32) -> i32 {
    defer println("cleanup")
    if x > 0 {
        return x * 2
    }
    return 0
}

pub fn main() {
    let v = check(5)
    println(v)
}
"#;
    let out = compile_and_emit(src);
    assert_clang_ok(&out);
    let fn_body =
        &out[out.find("ns_check").unwrap()..out.find("int main(void)").unwrap_or(out.len())];
    let pos_cleanup = fn_body.find("\"cleanup\"").expect("cleanup present");
    let pos_ret = fn_body
        .find("return v_x * 2")
        .or_else(|| fn_body.find("return astra_mul"))
        .expect("return present");
    assert!(
        pos_cleanup < pos_ret,
        "expected cleanup before return in generated C"
    );
    if let Some((ok, stdout, stderr)) = hosted_build_and_run(&out) {
        assert!(ok, "execution failed:\nstderr:\n{stderr}");
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines, vec!["cleanup", "10"]);
    }
}

#[test]
fn end_to_end_native_execution() {
    let src = r#"
module test.e2e

struct Point { x: i32, y: i32 }

fn add_points(a: Point, b: Point) -> Point {
    return Point { x: a.x + b.x, y: a.y + b.y }
}

pub fn main() {
    let p1 = Point { x: 10, y: 20 }
    let p2 = Point { x: 30, y: 40 }
    let p3 = add_points(p1, p2)
    assert(p3.x == 40)
    assert(p3.y == 60)
    println(p3.x)
    println(p3.y)
}
"#;
    let out = compile_and_emit(src);
    assert!(out.contains("NS_Point"), "{out}");
    assert!(out.contains("ns_add_points"), "{out}");
    assert!(out.contains("int main(void)"), "{out}");
    assert_clang_ok(&out);
    if let Some((ok, stdout, stderr)) = hosted_build_and_run(&out) {
        assert!(ok, "execution failed:\nstderr:\n{stderr}");
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines, vec!["40", "60"]);
    }
}

/// Regression: arm bodies are emitted through the `pre` side buffer. A pending
/// `pre` flush used to escape the arm it belonged to, so `99 => { break }` ran
/// in the wildcard branch and the accumulator never updated (the `early_exit`
/// conformance case printed `-1` instead of `3`).
#[test]
fn match_arm_statements_stay_inside_their_branch() {
    let src = r#"
fn summarize(values: [i32]) -> i32 {
    let mut total = 0
    for value in values {
        if value < 0 {
            continue
        }
        match value {
            99 => { break }
            _ => { total = total + value }
        }
    }
    if total == 0 {
        return -1
    }
    return total
}

pub fn main() -> i32 {
    println(summarize([-5, 1, 2, 99, 3]))
    println(summarize([0]))
    println(summarize([]))
    return 0
}
"#;
    let out = compile_and_emit(src);
    assert_clang_ok(&out);

    let if_pos = out
        .find("if (v_value == 99)")
        .unwrap_or_else(|| panic!("literal arm missing:\n{out}"));
    let else_pos = if_pos
        + out[if_pos..]
            .find("} else {")
            .unwrap_or_else(|| panic!("wildcard arm missing:\n{out}"));
    let break_pos = if_pos
        + out[if_pos..]
            .find("break;")
            .unwrap_or_else(|| panic!("break missing:\n{out}"));
    let add_pos = else_pos
        + out[else_pos..]
            .find("v_total = astra_add_i32")
            .unwrap_or_else(|| panic!("accumulator update missing:\n{out}"));

    assert!(
        if_pos < break_pos && break_pos < else_pos,
        "`break` must stay inside the literal arm:\n{out}"
    );
    assert!(
        else_pos < add_pos,
        "accumulator update must stay inside the wildcard arm:\n{out}"
    );

    if let Some((ok, stdout, stderr)) = hosted_build_and_run(&out) {
        assert!(ok, "execution failed:\nstderr:\n{stderr}");
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines, vec!["3", "-1", "-1"]);
    }
}

/// `Array<T>.get(i)` was implemented by the checker but not by the C emitter.
/// It must return `Option<T>` and stay in bounds instead of using the panicking
/// index path.
#[test]
fn array_get_returns_bounded_option() {
    let src = r#"
pub fn main() -> i32 {
    let xs = [10, 20, 30]
    println(xs.get(1).unwrap_or(0))
    println(xs.get(9).unwrap_or(-1))
    println(xs.get(0).is_some())
    return 0
}
"#;
    let out = compile_and_emit(src);
    assert!(
        out.contains("astra_array_index"),
        "expected the array runtime helper:\n{out}"
    );
    assert!(
        out.contains("NS_Option_i32"),
        "expected an Option<i32> result:\n{out}"
    );
    assert_clang_ok(&out);
    if let Some((ok, stdout, stderr)) = hosted_build_and_run(&out) {
        assert!(ok, "execution failed:\nstderr:\n{stderr}");
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines, vec!["20", "-1", "true"]);
    }
}

#[test]
fn library_emission_omits_main_and_exports_public_functions() {
    let modules = &[(
        "src/lib.ns",
        "module demo.lib\n\npub fn add(a: i32, b: i32) -> i32 { return a + b }\n\npub fn main() -> i32 { return add(20, 22) }\n",
    )];
    let executable =
        try_compile_project_kind(modules, EmitKind::Executable).expect("executable emission");
    let library = try_compile_project_kind(modules, EmitKind::Library).expect("library emission");

    let exe_src = project_source(&executable, "src/lib.ns");
    let lib_src = project_source(&library, "src/lib.ns");
    assert!(
        exe_src.contains("int main(void)"),
        "executable emission must keep the entry wrapper:\n{exe_src}"
    );
    assert!(
        !lib_src.contains("int main(void)"),
        "library emission must not emit an entry wrapper:\n{lib_src}"
    );
    assert!(
        lib_src.contains("ns_src_lib_add"),
        "library function must stay external:\n{lib_src}"
    );
    assert!(
        project_header(&library, "src/lib.ns").contains("ns_src_lib_add"),
        "library header must expose the public symbol"
    );
    assert_clang_ok(lib_src);
}

/// CONC-001 first slice: bounded single-threaded channels and eager `spawn`.
#[test]
fn channels_and_eager_spawn_run_the_corpus_semantics() {
    let src = r#"
fn produce(channel: Channel<str>) -> () ! ChannelError {
    channel.send("hello")!
    channel.send("world")!
}

pub fn main() -> i32 {
    let channel = Channel.create<str>()
    spawn || {
        match produce(channel) {
            Ok(value) => println("sent"),
            Err(error) => println("send failed"),
        }
    }
    match channel.receive() {
        Ok(message) => println(message),
        Err(error) => println("closed"),
    }
    match channel.receive() {
        Ok(message) => println(message),
        Err(error) => println("closed"),
    }
    channel.close()
    match channel.send("late") {
        Ok(value) => println("sent late"),
        Err(error) => println("closed send"),
    }
    return 0
}
"#;
    let out = compile_and_emit(src);
    for needle in [
        "astra_channel_new",
        "astra_channel_send",
        "astra_channel_receive",
        "astra_channel_close",
        "NS_ChannelError",
        "NS_Task_Unit",
    ] {
        assert!(out.contains(needle), "missing {needle}:\n{out}");
    }
    assert_clang_ok(&out);
    if let Some((ok, stdout, stderr)) = hosted_build_and_run(&out) {
        assert!(ok, "execution failed:\nstderr:\n{stderr}");
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines, vec!["sent", "hello", "world", "closed send"]);
    }
}

#[test]
fn spawn_returns_an_eager_task_that_joins() {
    let src = r#"
pub fn main() {
    let task = spawn || { 21 + 21 }
    println(task.join())
}
"#;
    let out = compile_and_emit(src);
    assert!(out.contains("NS_Task_i32"), "{out}");
    assert!(out.contains(".value"), "{out}");
    assert_clang_ok(&out);
    if let Some((ok, stdout, stderr)) = hosted_build_and_run(&out) {
        assert!(ok, "execution failed:\nstderr:\n{stderr}");
        assert_eq!(stdout.lines().collect::<Vec<_>>(), vec!["42"]);
    }
}

// ─── Map literal emission ─────────────────────────────────────────────────────

#[test]
fn test_map_literal_emits_map_new_and_set() {
    let src = r#"
module test.map_lit

fn get_len(m: {str: i32}) -> usize {
    return m.len()
}
"#;
    let out = compile_and_emit(src);
    // Map len call must appear.
    assert!(
        out.contains("astra_map_len"),
        "expected astra_map_len:\n{out}"
    );
    assert_clang_ok(&out);
}

#[test]
fn test_map_literal_construction() {
    let src = r#"
module test.map_construct

pub fn main() {
    let m: {str: i32} = { "alice": 10, "bob": 20 }
    let n = m.len()
}
"#;
    let out = compile_and_emit(src);
    assert!(
        out.contains("astra_map_new"),
        "expected astra_map_new:\n{out}"
    );
    assert!(
        out.contains("astra_map_set"),
        "expected astra_map_set:\n{out}"
    );
    assert!(
        out.contains("astra_map_len"),
        "expected astra_map_len:\n{out}"
    );
    assert_clang_ok(&out);
}

#[test]
fn test_map_contains_method() {
    let src = r#"
module test.map_contains

fn check(m: {str: i32}) -> bool {
    return m.contains("hello")
}
"#;
    let out = compile_and_emit(src);
    assert!(
        out.contains("astra_map_contains"),
        "expected astra_map_contains:\n{out}"
    );
    assert_clang_ok(&out);
}

#[test]
fn test_map_get_returns_option() {
    let src = r#"
module test.map_get

fn lookup(m: {str: i32}) -> Option<i32> {
    return m.get("x")
}
"#;
    let out = compile_and_emit(src);
    assert!(
        out.contains("astra_map_get"),
        "expected astra_map_get:\n{out}"
    );
    // The result should be wrapped in an Option typedef.
    assert!(
        out.contains("NS_Option_i32"),
        "expected NS_Option_i32:\n{out}"
    );
    assert_clang_ok(&out);
}

#[test]
fn test_map_insert_method() {
    let src = r#"
module test.map_insert

fn add_or_update() -> Option<i32> {
    let mut m: {str: i32} = { "initial": 1 }
    return m.insert("count", 99)
}
"#;
    let out = compile_and_emit(src);
    assert!(
        out.contains("astra_map_set"),
        "expected astra_map_set:\n{out}"
    );
    assert!(
        out.contains("astra_map_get"),
        "expected astra_map_get:\n{out}"
    );
    assert!(
        out.contains("NS_Option_i32"),
        "expected NS_Option_i32:\n{out}"
    );
    assert_clang_ok(&out);
}

#[test]
fn test_for_over_map() {
    let src = r#"
module test.for_map

fn iterate_map() {
    let m: {str: i32} = { "a": 1, "b": 2 }
    for k in m {
        let copy = k
    }
}
"#;
    let out = compile_and_emit(src);
    assert!(
        out.contains("astra_map_key_at"),
        "expected astra_map_key_at:\n{out}"
    );
    assert_clang_ok(&out);
}

#[test]
fn test_for_over_array() {
    let src = r#"
module test.for_array

fn iterate_array() {
    let arr = [10, 20, 30]
    for x in arr {
        let copy = x
    }
}
"#;
    let out = compile_and_emit(src);
    assert!(
        out.contains("astra_array_index"),
        "expected astra_array_index:\n{out}"
    );
    assert_clang_ok(&out);
}

// ─── Byte literals ────────────────────────────────────────────────────────────

#[test]
fn test_bytes_literal_emits_ns_bytes() {
    // NS byte literals don't support \x escapes; use plain ASCII bytes.
    let src = r#"
module test.bytes_lit

fn data() -> bytes {
    return b"abc"
}
"#;
    let out = compile_and_emit(src);
    assert!(out.contains("NS_Bytes"), "expected NS_Bytes:\n{out}");
    // The hex-encoded byte values must appear (a=0x61, b=0x62, c=0x63).
    assert!(
        out.contains("0x61"),
        "expected 0x61 in bytes literal:\n{out}"
    );
    assert!(
        out.contains("0x62"),
        "expected 0x62 in bytes literal:\n{out}"
    );
    assert!(
        out.contains("0x63"),
        "expected 0x63 in bytes literal:\n{out}"
    );
    assert_clang_ok(&out);
}

#[test]
fn test_bytes_empty_literal() {
    let src = r#"
module test.bytes_empty

fn empty() -> bytes {
    return b""
}
"#;
    let out = compile_and_emit(src);
    assert!(out.contains("NS_Bytes"), "expected NS_Bytes:\n{out}");
    // Empty bytes uses the zero-length sentinel.
    assert!(out.contains(".len = 0"), "expected .len = 0:\n{out}");
    assert_clang_ok(&out);
}

#[test]
fn test_bytes_indexing_and_methods() {
    let src = r#"
module test.bytes_ops

fn process(b: bytes) -> u8 {
    let l = b.len()
    let empty = b.is_empty()
    let sub = b.slice(1, 3)
    return b[0]
}

fn write() -> u8 {
    let mut buffer = bytes.new(4)
    buffer[0] = 255
    return buffer[0]
}
"#;
    let out = compile_and_emit(src);
    assert!(
        out.contains("astra_bytes_get"),
        "expected astra_bytes_get:\n{out}"
    );
    // `slice(start, end)` lowers to the end-offset runtime helper, not a
    // length: `1, 3` selects bytes [1, 3).
    assert!(
        out.contains("astra_bytes_slice(v_b, (size_t)(1), (size_t)(3))"),
        "expected end-offset astra_bytes_slice:\n{out}"
    );
    // Writes are bounds-checked through the runtime setter, and `bytes.new`
    // allocates a zero-filled buffer.
    assert!(
        out.contains("astra_bytes_set(&(v_buffer), 0, (uint8_t)(255))"),
        "expected a bounds-checked byte write:\n{out}"
    );
    assert!(
        out.contains("astra_bytes_new((size_t)(4))"),
        "expected the bytes allocation helper:\n{out}"
    );
    assert_clang_ok(&out);
}

#[test]
fn str_slice_lowers_to_result_with_bounds_check() {
    // `str.slice` is fallible (spec §6.2): it lowers to the bounds-checking
    // runtime helper and a `Result<str, SliceError>` built at the call site —
    // never a bare substring and never a rejected `SliceError` type.
    let src = r#"
fn take(s: str) -> bool {
    return match s.slice(1, 3) {
        Ok(piece) => piece == "é",
        Err(_) => false,
    }
}

fn failure(s: str) -> bool {
    return match s.slice(0, 99) {
        Ok(_) => false,
        Err(error) => true,
    }
}
"#;
    let out = compile_and_emit(src);
    assert!(
        out.contains("NS_Result_Str_SliceError"),
        "expected the shared Result typedef name:\n{out}"
    );
    assert!(
        out.contains("astra_str_slice_try(v_s, (size_t)(1), (size_t)(3)"),
        "expected the bounds-checking helper with end offsets:\n{out}"
    );
    assert!(
        out.contains(".ok = true, .value ="),
        "expected a constructed ok Result:\n{out}"
    );
    assert!(
        out.contains("(NS_SliceError)"),
        "expected the shared SliceError payload:\n{out}"
    );
    assert!(
        !out.contains("astra_str_slice("),
        "the old infallible helper must be gone:\n{out}"
    );
    // The canonical consumption is a Result match: the scrutinee is the
    // constructed Result and the arms dispatch on its `ok` tag.
    assert!(
        out.contains(".ok) {"),
        "expected a Result match over the slice result:\n{out}"
    );
    assert_clang_ok(&out);
}

#[test]
fn propagate_materializes_fallible_builtin_results() {
    // `expr!` on a fallible builtin call: the checker annotates the call with
    // its base type, so propagation must materialize `Result<base, E>` before
    // reading `.ok`/`.error` — including when the enclosing function's own
    // Result wraps a different base type.
    let src = r#"
fn count(s: str) -> i32 ! SliceError {
    let part = s.slice(0, 1)!
    return 1
}
"#;
    let out = compile_and_emit(src);
    assert!(
        out.contains("NS_Result_Str_SliceError"),
        "expected the inner Result materialized with the slice error:\n{out}"
    );
    assert!(
        out.contains("if (!"),
        "expected a propagation early return:\n{out}"
    );
    assert_clang_ok(&out);
}

#[test]
fn str_to_int_and_integer_checked_arithmetic_lower_to_results() {
    // Spec §6.2 `to_int` and §6.1 `checked_*`: fallible builtins lower to a
    // Result at the call site (strict decimal parse / overflow verdicts), and
    // wrapping arithmetic stays infallible.
    let src = r#"
fn parse(s: str) -> i32 {
    return match s.to_int() {
        Ok(value) => value,
        Err(_) => 0,
    }
}

fn add(a: i32, b: i32) -> i32 {
    return match a.checked_add(b) {
        Ok(value) => value,
        Err(_) => 0,
    }
}

fn divide(a: i32, b: i32) -> i32 {
    return match a.checked_div(b) {
        Ok(value) => value,
        Err(_) => 0,
    }
}

fn wrap(a: i8, b: i8) -> i8 {
    return a.wrapping_add(b)
}
"#;
    let out = compile_and_emit(src);
    assert!(
        out.contains("astra_str_to_int_try"),
        "expected the strict decimal parser:\n{out}"
    );
    assert!(
        out.contains("NS_Result_i32_ParseIntError"),
        "expected the ParseIntError Result:\n{out}"
    );
    assert!(
        out.contains("NS_Result_i32_OverflowError"),
        "expected the OverflowError Result:\n{out}"
    );
    assert!(
        out.contains("__builtin_add_overflow"),
        "expected the checked addition builtin:\n{out}"
    );
    assert!(
        out.contains("INT32_MIN") && out.contains("division by zero"),
        "expected the division guards:\n{out}"
    );
    assert!(
        out.contains("astra_add_i8"),
        "expected the signed wrap helper:\n{out}"
    );
    assert_clang_ok(&out);
}

#[test]
fn str_scalar_and_sequence_methods_lower_to_runtime_helpers() {
    // Spec §6.2: byte/scalar indexing, scalar counting, and the byte/char
    // sequence views all lower to the UTF-8-aware runtime helpers.
    let src = r#"
fn inspect(text: str) -> bool {
    let first = match text.byte_at(1) {
        Some(byte) => byte == 195,
        None => false,
    }
    let second = match text.char_at(1) {
        Some(character) => character == "é",
        None => false,
    }
    let count = text.char_count()
    let raw = text.bytes()
    let parts = text.chars()
    return first && second && count == 5 && raw.len() == 6 && parts.len() == 5
}

fn missing(text: str) -> i32 {
    return match text.byte_at(9) {
        Some(_) => 1,
        None => 0,
    }
}

fn fresh(n: usize) -> bytes {
    return bytes.new(n)
}
"#;
    let out = compile_and_emit(src);
    assert!(
        out.contains("astra_str_char_at"),
        "expected the scalar indexing helper:\n{out}"
    );
    assert!(
        out.contains("astra_str_char_count"),
        "expected the scalar counting helper:\n{out}"
    );
    assert!(
        out.contains("astra_str_bytes"),
        "expected the byte-view helper:\n{out}"
    );
    assert!(
        out.contains("astra_str_chars"),
        "expected the char-view helper:\n{out}"
    );
    assert!(
        out.contains("NS_Option_u8"),
        "expected the byte Option type:\n{out}"
    );
    assert!(
        out.contains("NS_Option_Str"),
        "expected the scalar Option type:\n{out}"
    );
    assert!(
        out.contains("astra_bytes_new"),
        "expected the bytes allocation helper:\n{out}"
    );
    assert_clang_ok(&out);
}

#[test]
fn option_and_result_accessors_lower_to_checked_reads() {
    // `unwrap`/`unwrap_or`/`error` evaluate the payload once and panic on the
    // wrong shape; `unwrap_or` keeps its fallback in the branch so it is only
    // evaluated when needed.
    let src = r#"
fn unwrap_option(maybe: Option<i32>) -> i32 {
    return maybe.unwrap()
}

fn or_default(maybe: Option<i32>) -> i32 {
    return maybe.unwrap_or(7)
}

fn unwrap_result(r: Result<i32, i32>) -> i32 {
    return r.unwrap()
}

fn result_error(r: Result<i32, i32>) -> i32 {
    return r.error()
}

fn result_or(r: Result<i32, i32>) -> i32 {
    return r.unwrap_or(3)
}

fn flags(r: Result<i32, i32>) -> bool {
    return r.is_ok() && not r.is_err()
}
"#;
    let out = compile_and_emit(src);
    assert!(
        out.contains("called unwrap() on None"),
        "expected the Option unwrap guard:\n{out}"
    );
    assert!(
        out.contains("called unwrap() on Err"),
        "expected the Result unwrap guard:\n{out}"
    );
    assert!(
        out.contains("called error() on Ok"),
        "expected the Result error guard:\n{out}"
    );
    assert!(
        out.contains(".some ? ") && out.contains(".ok ? "),
        "expected the conditional fallbacks:\n{out}"
    );
    assert_clang_ok(&out);
}

#[test]
fn generic_functions_monomorphize_per_instantiation() {
    // One specialized copy per distinct type-argument set, with the mangled
    // symbol and substituted signature; call sites refer to the copy.
    let src = r#"
fn identity<T>(value: T) -> T {
    return value
}

fn forward<T>(value: T) -> T {
    return identity(value)
}

pub fn main() -> i32 {
    println(identity(7))
    println(identity("ok"))
    println(identity(true))
    println(forward(9))
    return 0
}
"#;
    let out = compile_and_emit(src);
    assert!(
        out.contains("ns_identity_i32"),
        "expected the i32 instantiation:\n{out}"
    );
    assert!(
        out.contains("ns_identity_Str"),
        "expected the str instantiation:\n{out}"
    );
    assert!(
        out.contains("ns_identity_bool"),
        "expected the bool instantiation:\n{out}"
    );
    // A generic-to-generic call instantiates the callee with the caller's
    // concrete substitution.
    assert!(
        out.contains("ns_forward_i32") && out.contains("ns_identity_i32(v_value)"),
        "expected the forwarded instantiation to call the i32 copy:\n{out}"
    );
    // Specialized bodies (not just prototypes) must be emitted.
    assert!(
        out.contains("ns_identity_i32(int32_t v_value) {"),
        "expected the i32 body:\n{out}"
    );
    assert_clang_ok(&out);
}

#[test]
fn test_bytes_and_str_comparison() {
    let src = r#"
module test.comparisons

fn cmp_bytes(a: bytes, b: bytes) -> bool {
    let eq = a == b
    let ne = a != b
    let lt = a < b
    return eq && not ne && lt
}

fn cmp_str(a: str, b: str) -> bool {
    let eq = a == b
    let gt = a > b
    return eq || gt
}
"#;
    let out = compile_and_emit(src);
    assert!(
        out.contains("astra_bytes_eq"),
        "expected astra_bytes_eq:\n{out}"
    );
    assert!(
        out.contains("astra_bytes_cmp"),
        "expected astra_bytes_cmp:\n{out}"
    );
    assert!(
        out.contains("astra_str_eq"),
        "expected astra_str_eq:\n{out}"
    );
    assert!(
        out.contains("astra_str_cmp"),
        "expected astra_str_cmp:\n{out}"
    );
    assert_clang_ok(&out);
}

#[test]
fn test_for_over_bytes() {
    let src = r#"
module test.for_bytes

fn count_bytes(b: bytes) {
    for byte in b {
        let copy = byte
    }
}
"#;
    let out = compile_and_emit(src);
    assert!(
        out.contains("astra_bytes_get"),
        "expected astra_bytes_get in loop:\n{out}"
    );
    assert_clang_ok(&out);
}

#[test]
fn test_match_on_str() {
    let src = r#"
module test.match_str

fn evaluate(s: str) -> i32 {
    return match s {
        "apple" => 1,
        "banana" => 2,
        other => 0,
    }
}
"#;
    let out = compile_and_emit(src);
    assert!(
        out.contains("astra_str_eq"),
        "expected astra_str_eq in match:\n{out}"
    );
    assert_clang_ok(&out);
}

#[test]
fn test_match_on_bytes() {
    let src = r#"
module test.match_bytes

fn evaluate_bytes(b: bytes) -> i32 {
    return match b {
        b"ok" => 200,
        b"err" => 500,
        _ => 0,
    }
}
"#;
    let out = compile_and_emit(src);
    assert!(
        out.contains("astra_bytes_eq"),
        "expected astra_bytes_eq in match:\n{out}"
    );
    assert_clang_ok(&out);
}

#[test]
fn test_script_c_emission() {
    let src = r#"
module test.script_c

{@script:c as math;
    @mode(system)
    @exports {
        fn mul(a: i32, b: i32) -> i32
    }

    @impl
    int32_t mul(int32_t a, int32_t b) {
        return a * b;
    }
@end
}

fn calculate(x: i32, y: i32) -> i32 {
    return math.mul(x, y)
}
"#;
    let module = parse(src).expect("parse failed");
    let resolved = resolve_module(&module).expect("resolve failed");
    let checked = check_resolved(&module, &resolved, CheckOptions::new(Mode::System))
        .expect("type check failed");
    let c_module = CModule {
        name: checked.module.name.clone(),
        ast: module,
        hir: checked.module,
        annotations: checked.annotations,
        manual_memory_scopes: checked.manual_memory_scopes,
        integer_policy: IntegerPolicy::Wrapping,
    };
    let generated = generate(&c_module).expect("C generation failed");
    assert!(
        generated
            .source
            .contains("int32_t mul(int32_t a, int32_t b);"),
        "missing proto in:\n{}",
        generated.source
    );
    assert!(
        generated.source.contains("mul(v_x, v_y)"),
        "missing call in:\n{}",
        generated.source
    );
    assert_eq!(generated.script_sources.len(), 1);
    let (filename, content) = &generated.script_sources[0];
    assert_eq!(filename, "script_math.c");
    assert!(content.contains("int32_t mul(int32_t a, int32_t b)"));
    assert_clang_ok(&generated.source);
}

#[test]
fn test_script_cpp_emission() {
    let src = r#"
module test.script_cpp

{@script:cpp as native_calc;
    @mode(system)
    @exports {
        fn add(a: i32, b: i32) -> i32
    }

    @impl
    int32_t add(int32_t a, int32_t b) {
        return a + b;
    }
@end
}

fn calculate(x: i32, y: i32) -> i32 {
    return native_calc.add(x, y)
}
"#;
    let module = parse(src).expect("parse failed");
    let resolved = resolve_module(&module).expect("resolve failed");
    let checked = check_resolved(&module, &resolved, CheckOptions::new(Mode::System))
        .expect("type check failed");
    let c_module = CModule {
        name: checked.module.name.clone(),
        ast: module,
        hir: checked.module,
        annotations: checked.annotations,
        manual_memory_scopes: checked.manual_memory_scopes,
        integer_policy: IntegerPolicy::Wrapping,
    };
    let generated = generate(&c_module).expect("C generation failed");
    assert!(
        generated
            .source
            .contains("int32_t add(int32_t a, int32_t b);"),
        "missing proto in:\n{}",
        generated.source
    );
    assert!(
        generated.source.contains("add(v_x, v_y)"),
        "missing call in:\n{}",
        generated.source
    );
    assert_eq!(generated.script_sources.len(), 1);
    let (filename, content) = &generated.script_sources[0];
    assert_eq!(filename, "script_native_calc.cpp");
    assert!(content.contains("extern \"C\""));
    assert_clang_ok(&generated.source);
}

#[test]
fn test_script_js_rejected_on_c_backend() {
    let src = r#"
module test.script_js

{@script:js as math;
    @mode(core)
    @exports {
        fn add(a: i32, b: i32) -> i32
    }

    @impl
    export function add(a, b) { return a + b; }
@end
}
"#;
    let module = parse(src).expect("parse failed");
    let resolved = resolve_module(&module).expect("resolve failed");
    let checked = check_resolved(&module, &resolved, CheckOptions::new(Mode::Core))
        .expect("type check failed");
    let c_module = CModule {
        name: checked.module.name.clone(),
        ast: module,
        hir: checked.module,
        annotations: checked.annotations,
        manual_memory_scopes: checked.manual_memory_scopes,
        integer_policy: IntegerPolicy::Wrapping,
    };
    let err = generate(&c_module).expect_err("should reject js script on c target");
    assert_eq!(err.code, "NS7002");
}

// ─── multi-module (linked) project emission ──────────────────────────────────

#[test]
fn project_resolves_cross_module_call_to_dependency_symbol() {
    let emitted = compile_project_and_emit(&[
        (
            "lib",
            "pub fn add(a: i32, b: i32) -> i32 { return a + b }\n",
        ),
        (
            "app/main",
            "import { add } from \"lib\"\npub fn main() -> i32 { return add(1, 2) }\n",
        ),
    ]);
    let main = project_source(&emitted, "app/main");
    // The call must target the dependency's qualified symbol, not a local one.
    assert!(
        main.contains("ns_lib_add(v_1, v_2)") || main.contains("ns_lib_add("),
        "cross-module call not resolved in:\n{main}"
    );
    assert!(
        !main.contains("ns_app_main_add"),
        "call resolved to the wrong module symbol in:\n{main}"
    );
    // The dependency header must be included so the prototype is visible.
    assert!(
        main.contains("#include \"lib.h\""),
        "missing dep header:\n{main}"
    );
    assert_project_compiles(&emitted);
}

#[test]
fn project_module_qualifies_symbols_to_avoid_collision() {
    // Two modules each independently declare `helper`. Without module
    // qualification both would emit `ns_helper` with external linkage and
    // collide at link time. `app/main` imports a *different* function (`add`)
    // to establish the link dependency while defining its own `helper`.
    let emitted = compile_project_and_emit(&[
        (
            "lib",
            "pub fn helper() -> i32 { return 1 }\npub fn add(a: i32, b: i32) -> i32 { return a + b }\n",
        ),
        (
            "app/main",
            "import { add } from \"lib\"\nfn helper() -> i32 { return 2 }\npub fn main() -> i32 { return helper() + add(1, 2) }\n",
        ),
    ]);
    let lib = project_source(&emitted, "lib");
    let main = project_source(&emitted, "app/main");
    assert!(
        lib.contains("ns_lib_helper"),
        "lib symbol not qualified:\n{lib}"
    );
    // The local `helper` in main is qualified with main's stem, and the call
    // to the imported `add` targets lib's symbol.
    assert!(
        main.contains("ns_app_main_helper"),
        "main local symbol not qualified:\n{main}"
    );
    assert!(main.contains("ns_lib_add("), "call not qualified:\n{main}");
    // Neither module emits an unqualified `ns_helper`.
    assert!(
        !lib.contains("ns_helper("),
        "lib emitted bare symbol:\n{lib}"
    );
    assert!(
        !main.contains("ns_helper("),
        "main emitted bare symbol:\n{main}"
    );
}

#[test]
fn project_header_exports_qualified_prototypes() {
    let emitted = compile_project_and_emit(&[
        (
            "lib",
            "pub fn add(a: i32, b: i32) -> i32 { return a + b }\n",
        ),
        (
            "app/main",
            "import { add } from \"lib\"\npub fn main() -> i32 { return add(1, 2) }\n",
        ),
    ]);
    let lib_header = &emitted
        .iter()
        .find(|(k, _, _)| k == "lib")
        .expect("lib missing")
        .2
        .clone()
        .expect("lib header missing");
    assert!(
        lib_header.contains("ns_lib_add(int32_t v_a, int32_t v_b)"),
        "header prototype not qualified:\n{lib_header}"
    );
}

#[test]
fn project_emits_main_wrapper_only_for_entry_module() {
    let emitted = compile_project_and_emit(&[
        ("lib", "pub fn main() -> i32 { return 1 }\n"),
        (
            "app/main",
            "import { main as lib_main } from \"lib\"\npub fn main() -> i32 { return lib_main() }\n",
        ),
    ]);
    let lib = project_source(&emitted, "lib");
    let main = project_source(&emitted, "app/main");
    // Only the entry module gets the `int main(void)` wrapper.
    assert!(
        !lib.contains("int main(void)"),
        "lib should not emit main:\n{lib}"
    );
    assert!(
        main.contains("int main(void)"),
        "entry missing main:\n{main}"
    );
    assert!(
        main.contains("return (int)ns_app_main_main();"),
        "main wrapper should call the qualified symbol:\n{main}"
    );
}

#[test]
fn project_resolves_namespace_import_call() {
    let emitted = compile_project_and_emit(&[
        ("lib", "pub fn triple(n: i32) -> i32 { return n * 3 }\n"),
        (
            "app/main",
            "import * as lib from \"lib\"\npub fn main() -> i32 { return lib.triple(4) }\n",
        ),
    ]);
    let main = project_source(&emitted, "app/main");
    assert!(
        main.contains("ns_lib_triple("),
        "namespace call not resolved in:\n{main}"
    );
}

#[test]
fn project_rejects_cross_module_value_reference() {
    // A cross-module constant used as a bare value cannot link in this slice;
    // it must be rejected with a clear diagnostic, not mis-emitted.
    let err = try_compile_project(&[
        ("lib", "pub const MAX: i32 = 10\n"),
        (
            "app/main",
            "import { MAX } from \"lib\"\npub fn main() -> i32 { return MAX }\n",
        ),
    ])
    .expect_err("cross-module value reference should be rejected");
    assert_eq!(err.code, "NS8019");
    assert!(
        err.message.contains("MAX"),
        "diagnostic should name the symbol: {}",
        err.message
    );
}

#[test]
fn project_emits_cross_module_struct_types() {
    // A struct imported from a dependency lowers to the dependency's qualified
    // typedef (`NS_lib_Point`). The typedef stays in the dependency — its
    // translation unit and header — and this module includes that header.
    let emitted = compile_project_and_emit(&[
        ("lib", "pub struct Point { x: i32, y: i32 }\n"),
        (
            "app/main",
            "import { Point } from \"lib\"\npub fn take(p: Point) -> i32 { return p.x }\npub fn origin() -> Point { return Point { x: 0, y: 0 } }\n",
        ),
    ]);
    let main = project_source(&emitted, "app/main");
    let lib = project_source(&emitted, "lib");
    assert!(
        lib.contains("typedef struct { int32_t x; int32_t y; } NS_lib_Point;"),
        "dependency declares its own typedef:\n{lib}"
    );
    assert!(
        main.contains("#include \"lib.h\""),
        "missing dep header:\n{main}"
    );
    assert!(
        main.contains("NS_lib_Point v_p"),
        "signature not lowered to the dependency's typedef:\n{main}"
    );
    assert!(
        main.contains("(NS_lib_Point){ .x = 0, .y = 0"),
        "literal not constructed with the dependency's type:\n{main}"
    );
    assert!(
        !main.contains("typedef struct { int32_t x; int32_t y; } NS_lib_Point"),
        "the dependency's typedef must not be re-emitted:\n{main}"
    );
    let header = project_header(&emitted, "app/main");
    assert!(
        header.contains("#include \"lib.h\""),
        "header must include dependencies so its signatures resolve:\n{header}"
    );
}

#[test]
fn project_emits_cross_module_enum_construction_and_match() {
    // Constructing and matching an imported enum must use the dependency's tag
    // constants (`NS_lib_Status_TAG_*`) — the exact names its own translation
    // unit declares — and its qualified enum type.
    let emitted = compile_project_and_emit(&[
        ("lib", "pub enum Status { Active, Idle }\n"),
        (
            "app/main",
            "import { Status } from \"lib\"\npub fn main() -> i32 {\n    let s = Status.Active\n    return match s {\n        Status.Active => 1\n        _ => 2\n    }\n}\n",
        ),
    ]);
    let main = project_source(&emitted, "app/main");
    let lib = project_source(&emitted, "lib");
    assert!(
        lib.contains("NS_lib_Status_TAG_Active = 0"),
        "dependency declares tags with its own stem:\n{lib}"
    );
    assert!(
        main.contains("case NS_lib_Status_TAG_Active:"),
        "match case must use the dependency's tag:\n{main}"
    );
    assert!(
        main.contains("(NS_lib_Status){ .tag = NS_lib_Status_TAG_Active }"),
        "unit construction must use the dependency's type and tag:\n{main}"
    );
}

#[test]
fn project_emits_cross_module_tuple_struct_types() {
    let emitted = compile_project_and_emit(&[
        ("lib", "pub struct Pair(i32, i32)\n"),
        (
            "app/main",
            "import { Pair } from \"lib\"\npub fn make() -> Pair { return Pair(1, 2) }\n",
        ),
    ]);
    let main = project_source(&emitted, "app/main");
    assert!(
        main.contains("(NS_lib_Pair){ 1, 2 }"),
        "tuple construction not lowered to the dependency's type:\n{main}"
    );
    assert!(
        main.contains("NS_lib_Pair ns_app_main_make"),
        "signature not lowered to the dependency's typedef:\n{main}"
    );
}

#[test]
fn project_rejects_default_import_call() {
    let err = try_compile_project(&[
        (
            "lib",
            "export default fn add(a: i32, b: i32) -> i32 { return a + b }\n",
        ),
        (
            "app/main",
            "import add from \"lib\"\npub fn main() -> i32 { return add(1, 2) }\n",
        ),
    ])
    .expect_err("default import call should be rejected");
    assert_eq!(err.code, "NS8019");
    assert!(
        err.message.contains("add"),
        "diagnostic should name the symbol: {}",
        err.message
    );
}
