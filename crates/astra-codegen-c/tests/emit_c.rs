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

use astra_codegen_c::{CModule, IntegerPolicy, generate};
use astra_core::checker::{CheckOptions, check_resolved};
use astra_core::parser::parse;
use astra_core::resolver::resolve_module;
use astra_capabilities::Mode;

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
    let candidates = [
        "C:\\Program Files\\LLVM\\bin\\clang.exe",
        "clang",
        "clang-19",
        "clang-18",
    ];
    for c in &candidates {
        let path = PathBuf::from(c);
        if path.exists() {
            return Some(path);
        }
        if let Ok(output) = std::process::Command::new(c).arg("--version").output() {
            if output.status.success() {
                return Some(PathBuf::from(c));
            }
        }
    }
    None
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
        .args(["-std=c17", "-ffreestanding", "-Wall", "-Wno-unused-variable",
               "-c", "-o"])
        .arg(dir.path().join("out.o"))
        .arg(&src_path)
        .output()
        .expect("clang failed to start");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    Some((output.status.success(), stderr))
}

fn assert_clang_ok(c_source: &str) {
    if let Some((ok, stderr)) = clang_check(c_source) {
        assert!(ok, "clang rejected the generated C:\n{stderr}\n\nSource:\n{c_source}");
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
    assert!(out.contains("static int32_t ns_add(int32_t v_a, int32_t v_b)"), "{out}");
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
    assert!(out.contains("static const int32_t const_ANSWER = 42"), "{out}");
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
    assert!(!out.contains("astra_add"), "{out}");
    assert_clang_ok(&out);
}

#[test]
fn manual_memory_is_rejected() {
    use astra_core::checker::check_source;
    use astra_core::checker::CheckOptions;
    use astra_capabilities::Mode;
    // system mode allows manual memory scopes
    let src = "{@memory:manual;\npub fn alloc() {}\n}\n";
    let checked = match check_source(src, CheckOptions::new(Mode::System)) {
        Ok(r) => r,
        Err(diags) => {
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
    assert!(result.is_err(), "C backend should reject manual-memory scopes");
    let err = result.unwrap_err();
    assert_eq!(err.code, "NS8012", "wrong rejection code: {}", err.code);
}

#[test]
fn unsupported_target_map_is_rejected() {
    // Maps are not supported in the first slice; the backend must reject them.
    use astra_core::checker::{CheckOptions, check_source};
    let src = "pub fn mm() { let m = {\"a\": 1} }\n";
    // This source passes the type checker (maps are valid in core mode).
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
    assert!(result.is_err(), "C backend should reject map literals");
    let err = result.unwrap_err();
    assert_eq!(err.code, "NS8013", "wrong rejection code: {}", err.code);
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
    use astra_compiler_api::{CompileRequest, FrontendCompiler, IntegerPolicy, SourceUnit};
    use astra_compiler_api::API_VERSION;

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
    assert!(response.program.is_some(), "expected program: {:?}", response.diagnostics);
    assert!(response.diagnostics.is_empty(), "unexpected diagnostics: {:?}", response.diagnostics);
    assert!(
        response.artifacts.iter().any(|a| a["type"] == "c-source"),
        "missing c-source artifact: {:?}", response.artifacts
    );
    assert!(
        response.artifacts.iter().any(|a| a["type"] == "c-host-shim"),
        "missing c-host-shim artifact: {:?}", response.artifacts
    );
    // Verify the C source artifact contains the generated function.
    let c_src = response.artifacts.iter()
        .find(|a| a["type"] == "c-source")
        .and_then(|a| a["content"].as_str())
        .unwrap_or("");
    assert!(c_src.contains("ns_add"), "expected ns_add in C source: {c_src}");
}

#[test]
fn compiler_api_rejects_unsupported_target() {
    use astra_compiler_api::{CompileRequest, FrontendCompiler, IntegerPolicy, SourceUnit, API_VERSION};
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
        "expected NS8001: {:?}", response.diagnostics
    );
}

#[test]
fn host_shim_contains_astra_panic() {
    use astra_codegen_c::runtime::HOST_SHIM;
    assert!(HOST_SHIM.contains("astra_panic"), "host shim missing astra_panic");
    assert!(HOST_SHIM.contains("astra_println_i32"), "host shim missing astra_println_i32");
    assert!(HOST_SHIM.contains("ASTRA_WRAP"), "host shim missing wrapping macros");
}
