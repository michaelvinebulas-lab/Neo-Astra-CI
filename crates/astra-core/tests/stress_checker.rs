//! Stress tests — TYPE-002: Type checker, TYPE-003: Cross-module type surfaces.

use astra_core::checker::{CheckOptions, check_resolved, check_source};
use astra_core::mode::Mode;
use astra_core::parser::parse;
use astra_core::resolver::resolve_module;
use astra_core::type_surface::{
    ModuleTypeSurface, ProjectModule, check_module_graph, module_type_surface,
};
use std::collections::BTreeMap;

fn ok(source: &str) {
    check_source(source, CheckOptions::default())
        .unwrap_or_else(|e| panic!("check failed: {e:?}\nsource:\n{source}"));
}

fn ok_mode(source: &str, mode: Mode) {
    check_source(source, CheckOptions::new(mode))
        .unwrap_or_else(|e| panic!("check failed with mode {mode:?}: {e:?}"));
}

fn err(source: &str, code: &str) {
    let e = check_source(source, CheckOptions::default())
        .expect_err(&format!("should have failed:\n{source}"));
    assert!(
        e.iter().any(|d| d.code == code),
        "expected {code} in {e:?}\nsource:\n{source}"
    );
}

// ─── Primitive types ──────────────────────────────────────────────────────────

#[test]
fn all_primitive_types_check() {
    ok("fn f(a: i8, b: i16, c: i32, d: i64, e: u8, f: u16, g: u32, h: u64) {}");
    ok("fn f(a: usize, b: isize, c: f32, d: f64, e: bool, f: str, g: bytes) {}");
    ok("fn f() -> () {}");
}

#[test]
fn no_implicit_numeric_coercion() {
    err("fn f() -> i64 { let x: i32 = 1\nreturn x }", "NS4001");
    err("fn f() -> f64 { let x: f32 = 1.0\nreturn x }", "NS4001");
}

// ─── Inference ────────────────────────────────────────────────────────────────

#[test]
fn local_inference_works() {
    ok("fn f() {\nlet x = 42\nlet y = x + 1\n}");
    ok("fn f() {\nlet xs = [1, 2, 3]\nlet n = xs[0]\n}");
}

#[test]
fn generic_inference_works() {
    ok("fn identity<T>(value: T) -> T { return value }\nfn f() -> i32 { return identity(42) }");
}

// ─── Composites ───────────────────────────────────────────────────────────────

#[test]
fn struct_construction_and_field_access_check() {
    ok(
        "struct Point { x: i32\ny: i32 }\nfn f() -> i32 { let p = Point { x: 1, y: 2 }\nreturn p.x }",
    );
}

#[test]
fn tuple_struct_construction_checks() {
    ok("struct Wrap(i32)\nfn f() -> i32 { let w = Wrap(42)\nreturn w.0 }");
}

#[test]
fn enum_variant_construction_checks() {
    ok("enum E { A, B(i32) }\nfn f() -> E { return E.A }");
    ok("enum E { A, B(i32) }\nfn f() -> E { return E.B(1) }");
}

#[test]
fn missing_struct_field_is_ns4016() {
    err(
        "struct S { x: i32\ny: i32 }\nfn f() { let value = S { x: 1 } }",
        "NS4016",
    );
}

#[test]
fn wrong_struct_field_type_is_ns4001() {
    err(
        "struct S { x: i32 }\nfn f() { let value = S { x: \"wrong\" } }",
        "NS4001",
    );
}

// ─── Exhaustive matching ──────────────────────────────────────────────────────

#[test]
fn exhaustive_option_match_ok() {
    ok("fn f(x: Option<i32>) -> str { return match x { Some(n) => \"s\"\nNone => \"n\" } }");
}

#[test]
fn non_exhaustive_option_match_is_ns4010() {
    err(
        "fn f(x: Option<i32>) -> str { return match x { Some(n) => \"s\" } }",
        "NS4010",
    );
}

#[test]
fn exhaustive_bool_match_ok() {
    ok("fn f(b: bool) -> str { return match b { true => \"t\"\nfalse => \"f\" } }");
}

#[test]
fn exhaustive_enum_match_ok() {
    ok("enum E { A, B, C }\nfn f(e: E) -> i32 { return match e { E.A => 1\nE.B => 2\nE.C => 3 } }");
}

#[test]
fn non_exhaustive_enum_match_is_ns4010() {
    err(
        "enum E { A, B, C }\nfn f(e: E) -> i32 { return match e { E.A => 1\nE.B => 2 } }",
        "NS4010",
    );
}

// ─── Error effects ────────────────────────────────────────────────────────────

#[test]
fn error_propagation_ok() {
    ok(
        "struct E(str)\nfn parse(s: str) -> i32 ! E { return 0 }\nfn f(s: str) -> i32 ! E { return parse(s)! }",
    );
}

#[test]
fn unhandled_error_is_ns4011() {
    err(
        "struct E(str)\nfn parse(s: str) -> i32 ! E { return 0 }\nfn f(s: str) -> i32 { return parse(s) }",
        "NS4011",
    );
}

#[test]
fn undeclared_error_in_set_is_ns4012() {
    err(
        "struct A(str)\nstruct B(str)\nfn f() -> i32 ! A { return !B(\"x\") }",
        "NS4012",
    );
}

#[test]
fn error_union_all_variants_ok() {
    ok(
        "struct A(str)\nstruct B(str)\nfn f() -> i32 ! A | B { return !A(\"x\") }\nfn g() -> i32 ! A | B { return !B(\"y\") }",
    );
}

// ─── Mutability ───────────────────────────────────────────────────────────────

#[test]
fn assign_to_immutable_binding_is_ns4009() {
    err("fn f() { let x = 1\nx = 2 }", "NS4009");
}

#[test]
fn assign_to_mutable_binding_ok() {
    ok("fn f() { let mut x = 1\nx = 2 }");
}

// ─── Return paths ─────────────────────────────────────────────────────────────

#[test]
fn missing_return_on_path_is_ns4007() {
    err("fn f() -> i32 { let x = 1 }", "NS4007");
}

#[test]
fn all_paths_return_ok() {
    ok("fn f(b: bool) -> i32 { if b { return 1 } else { return 2 } }");
}

// ─── Mode and capability checks ───────────────────────────────────────────────

#[test]
fn requires_system_in_core_mode_is_ns5001() {
    err("@requires(system)\nfn f() {}", "NS5001");
}

#[test]
fn requires_system_in_system_mode_ok() {
    ok_mode("@requires(system)\nfn f() {}", Mode::System);
}

#[test]
fn foreign_module_in_core_mode_is_ns5001() {
    err("foreign module m { fn g(x: i32) -> str }", "NS5001");
}

#[test]
fn mode_widening_is_ns5002() {
    err(
        "{@mode:system;\nfn f() {}\n}\n{@mode:web;\nfn g() {}\n}",
        "NS5002",
    );
}

#[test]
fn memory_manual_in_core_mode_is_ns5004() {
    err("fn f() { {@memory:manual;\nlet x = 1\n} }", "NS5004");
}

#[test]
fn capability_tag_fs_needs_system_mode() {
    err("@tags(fs)\nfn f() {}", "NS5003");
}

// ─── TYPE-003: Cross-module type surfaces ─────────────────────────────────────

fn checked(source: &str) -> astra_core::checker::CheckResult {
    let m = parse(source).expect("parse");
    let r = resolve_module(&m).expect("resolve");
    check_resolved(&m, &r, CheckOptions::default()).expect("check")
}

fn checked_with(
    source: &str,
    surfaces: BTreeMap<String, ModuleTypeSurface>,
) -> Result<astra_core::checker::CheckResult, Vec<astra_core::diagnostic::Diagnostic>> {
    let m = parse(source).expect("parse");
    let r = resolve_module(&m).expect("resolve");
    check_resolved(
        &m,
        &r,
        CheckOptions {
            module_surfaces: surfaces,
            ..CheckOptions::default()
        },
    )
}

#[test]
fn surface_captures_exported_function_type() {
    let result = checked("pub fn add(a: i32, b: i32) -> i32 { return a + b }");
    let surface = module_type_surface(&result);
    assert!(surface.get_value("add").is_some());
    assert!(!surface.values.is_empty());
}

#[test]
fn debug_tuple_struct_error_type_surface() {
    let result = checked("pub struct E(str)\npub fn parse() -> i32 ! E { return !E(\"bad\") }");
    let surface = module_type_surface(&result);
    assert!(
        surface.get_value("E").is_some(),
        "E should be a value (constructor)"
    );
    assert!(
        surface.get_type_def("E").is_some(),
        "E should be a type def"
    );
}

#[test]
fn surface_omits_unexported_declarations() {
    let result = checked("fn hidden() -> i32 { return 1 }\npub fn public_fn() {}");
    let surface = module_type_surface(&result);
    assert!(surface.get_value("hidden").is_none());
    assert!(surface.get_value("public_fn").is_some());
}

#[test]
fn cross_module_correct_call_passes() {
    let lib = checked("pub fn double(n: i32) -> i32 { return n * 2 }");
    let lib_surface = module_type_surface(&lib);
    let mut surfaces = BTreeMap::new();
    surfaces.insert("lib".to_owned(), lib_surface);
    let result = checked_with(
        "import { double } from \"lib\"\nfn f() -> i32 { return double(21) }",
        surfaces,
    );
    assert!(
        result.is_ok(),
        "cross-module correct call should pass: {:?}",
        result.err()
    );
}

#[test]
fn cross_module_type_mismatch_caught() {
    let lib = checked("pub fn greet(name: str) -> str { return name }");
    let lib_surface = module_type_surface(&lib);
    let mut surfaces = BTreeMap::new();
    surfaces.insert("lib".to_owned(), lib_surface);
    let result = checked_with(
        "import { greet } from \"lib\"\nfn f() -> str { return greet(42) }",
        surfaces,
    );
    assert!(result.is_err());
    assert!(result.unwrap_err().iter().any(|d| d.code == "NS4001"));
}

#[test]
fn cross_module_wrong_arg_count_caught() {
    let lib = checked("pub fn f(a: i32, b: i32) -> i32 { return a + b }");
    let lib_surface = module_type_surface(&lib);
    let mut surfaces = BTreeMap::new();
    surfaces.insert("lib".to_owned(), lib_surface);
    let result = checked_with(
        "import { f } from \"lib\"\nfn g() -> i32 { return f(1) }",
        surfaces,
    );
    assert!(result.is_err());
    assert!(result.unwrap_err().iter().any(|d| d.code == "NS4004"));
}

#[test]
fn graph_checker_three_modules_all_pass() {
    let modules = vec![
        ProjectModule {
            key: "a".to_owned(),
            source: parse("pub fn fa() -> i32 { return 1 }").unwrap(),
        },
        ProjectModule {
            key: "b".to_owned(),
            source: parse("import { fa } from \"a\"\npub fn fb() -> i32 { return fa() + 1 }")
                .unwrap(),
        },
        ProjectModule {
            key: "c".to_owned(),
            source: parse("import { fb } from \"b\"\npub fn main() -> i32 { return fb() }")
                .unwrap(),
        },
    ];
    let result = check_module_graph(&modules, CheckOptions::default());
    assert!(
        result.is_ok(),
        "3-module chain should pass: {:?}",
        result.all_diagnostics()
    );
}

#[test]
fn graph_checker_catches_error_in_leaf() {
    let modules = vec![
        ProjectModule {
            key: "lib".to_owned(),
            source: parse("pub fn sq(n: i32) -> i32 { return n * n }").unwrap(),
        },
        ProjectModule {
            key: "main".to_owned(),
            source: parse(
                "import { sq } from \"lib\"\nfn run() -> i32 { return sq(\"not a number\") }",
            )
            .unwrap(),
        },
    ];
    let result = check_module_graph(&modules, CheckOptions::default());
    assert!(!result.is_ok());
    assert!(result.all_diagnostics().iter().any(|d| d.code == "NS4001"));
}

// ─── @script block — checker conformance ─────────────────────────────────────

fn ok_target(source: &str, target: &str) {
    check_source(source, CheckOptions::new(Mode::Core).with_target(target))
        .unwrap_or_else(|e| panic!("check failed (target={target}): {e:?}\nsource:\n{source}"));
}

fn err_target(source: &str, target: &str, code: &str) {
    let e = check_source(source, CheckOptions::new(Mode::Core).with_target(target))
        .expect_err(&format!("should have failed (target={target}):\n{source}"));
    assert!(
        e.iter().any(|d| d.code == code),
        "expected {code} in {e:?}\nsource:\n{source}"
    );
}

const JS_SCRIPT: &str = r#"{@script:js as math;
    @mode(core)
    @exports {
        fn add(a: i32, b: i32) -> i32
    }

    @impl
    export function add(a, b) { return a + b }
@end
}"#;

const C_SCRIPT: &str = r#"{@script:c as native_math;
    @mode(system)
    @exports {
        fn mul(a: i32, b: i32) -> i32
    }

    @impl
    int mul(int a, int b) { return a * b; }
@end
}"#;

#[test]
fn script_block_checks_without_target() {
    // Without a target, NS7002 is not emitted — it can't be validated.
    ok(JS_SCRIPT);
}

#[test]
fn js_script_passes_on_js_target() {
    ok_target(JS_SCRIPT, "js");
}

#[test]
fn js_script_passes_on_ts_target() {
    ok_target(JS_SCRIPT, "ts");
}

#[test]
fn js_script_rejected_on_native_target_ns7002() {
    err_target(JS_SCRIPT, "native", "NS7002");
}

#[test]
fn c_script_passes_on_native_target() {
    check_source(
        C_SCRIPT,
        CheckOptions::new(Mode::System).with_target("native"),
    )
    .unwrap_or_else(|e| panic!("check failed: {e:?}"));
}

#[test]
fn c_script_rejected_on_js_target_ns7002() {
    let e = check_source(C_SCRIPT, CheckOptions::new(Mode::System).with_target("js"))
        .expect_err("should have failed with NS7002");
    assert!(e.iter().any(|d| d.code == "NS7002"), "got: {e:?}");
}

#[test]
fn script_namespace_exposed_to_caller() {
    // A function calling math.add must check without error when types match.
    let src = format!("{JS_SCRIPT}\nfn use_math() -> i32 {{ return math.add(1, 2) }}\n");
    ok(&src);
}

#[test]
fn script_namespace_collision_is_ns7006() {
    // Two @script blocks with the same namespace name → NS7006.
    let src = format!("{JS_SCRIPT}\n{JS_SCRIPT}\n");
    err(&src, "NS7006");
}

#[test]
fn script_export_non_boundary_safe_type_is_ns7005() {
    // A generic export is not allowed (NS7005).
    // We simulate a non-boundary-safe type with a closure / fn type.
    let src = r#"{@script:js as bad;
    @mode(core)
    @exports {
        fn apply(f: fn(i32) -> i32, x: i32) -> i32
    }

    @impl
    export function apply(f, x) { return f(x) }
@end
}"#;
    err(src, "NS7005");
}
