//! Stress tests — FMT-001 Canonical AST formatter, FMT-002 Comment preservation.

use astra_core::formatter::format_source;

fn fmt(source: &str) -> String {
    format_source(source).unwrap_or_else(|e| panic!("format failed: {e:?}\nsource:\n{source}"))
}

fn idempotent(source: &str) {
    let first = fmt(source);
    let second = fmt(&first);
    assert_eq!(first, second, "formatter is not idempotent on:\n{source}");
}

// ─── Keyword alias normalization ──────────────────────────────────────────────

#[test]
fn aliases_normalized_to_canonical() {
    let formatted = fmt("public function f() {}\n");
    assert!(formatted.contains("pub fn f()"), "got: {formatted}");
    assert!(!formatted.contains("public function"), "got: {formatted}");
}

#[test]
fn var_normalized_to_let() {
    let formatted = fmt("fn f() { var mut x = 1 }\n");
    assert!(formatted.contains("let mut x"), "got: {formatted}");
}

#[test]
fn constant_normalized_to_const() {
    let formatted = fmt("constant X: i32 = 42\n");
    assert!(formatted.contains("const X"), "got: {formatted}");
}

#[test]
fn typedef_normalized_to_type() {
    let formatted = fmt("typedef Alias = i32\n");
    assert!(formatted.contains("type Alias"), "got: {formatted}");
}

#[test]
fn record_normalized_to_struct() {
    let formatted = fmt("record S { x: i32 }\n");
    assert!(formatted.contains("struct S"), "got: {formatted}");
}

// ─── Idempotence ──────────────────────────────────────────────────────────────

#[test]
fn idempotent_simple_function() {
    idempotent("fn f(x: i32) -> i32 {\n    return x + 1\n}\n");
}

#[test]
fn idempotent_struct_and_enum() {
    idempotent(
        "struct User {\n    id: u64\n    name: str\n}\n\nenum Status {\n    Active\n    Inactive(reason: str)\n}\n",
    );
}

#[test]
fn idempotent_imports_and_exports() {
    idempotent("import std.io\nimport { read } from \"std:fs\"\nexport { read }\n");
}

#[test]
fn idempotent_match_expression() {
    idempotent(
        "fn f(x: Option<i32>) -> str {\n    return match x {\n        Some(n) => \"some\"\n        None => \"none\"\n    }\n}\n",
    );
}

#[test]
fn idempotent_scoped_blocks() {
    idempotent("{@mode:core;\nfn f() {}\n}\n");
}

#[test]
fn idempotent_generic_function() {
    idempotent("fn identity<T>(value: T) -> T {\n    return value\n}\n");
}

#[test]
fn idempotent_error_effects() {
    idempotent("fn parse(input: str) -> i32 ! ParseError {\n    return input.to_int()!\n}\n");
}

#[test]
fn idempotent_full_module() {
    idempotent(
        r#"module app.main

import std.io as io

const MAX: u32 = 1024

struct User {
    id: u64
    name: str
}

pub fn main() {
    let user = User { id: 1, name: "Astra" }
    println(user.name)
}
"#,
    );
}

// ─── Comment preservation ─────────────────────────────────────────────────────

#[test]
fn leading_comment_preserved() {
    let formatted = fmt("// top comment\nfn f() {}\n");
    assert!(formatted.contains("// top comment"), "got: {formatted}");
}

#[test]
fn trailing_comment_preserved() {
    let formatted = fmt("fn f() {} // trailing\n");
    assert!(formatted.contains("// trailing"), "got: {formatted}");
}

#[test]
fn comment_inside_function_preserved() {
    let formatted = fmt("fn f() {\n    // body comment\n    let x = 1\n}\n");
    assert!(formatted.contains("// body comment"), "got: {formatted}");
}

#[test]
fn field_comments_preserved() {
    let formatted = fmt("struct S {\n    // field comment\n    x: i32\n}\n");
    assert!(formatted.contains("// field comment"), "got: {formatted}");
}

#[test]
fn variant_comments_preserved() {
    let formatted = fmt("enum E {\n    // variant comment\n    A\n    B\n}\n");
    assert!(formatted.contains("// variant comment"), "got: {formatted}");
}

#[test]
fn eof_comment_preserved() {
    let formatted = fmt("fn f() {}\n// end of file\n");
    assert!(formatted.contains("// end of file"), "got: {formatted}");
}

#[test]
fn comment_idempotent_with_various_positions() {
    idempotent(
        "// header\nmodule app\n\n// before fn\nfn f() {\n    // inside\n    let x = 1 // inline\n}\n// after fn\n",
    );
}

#[test]
fn doc_comments_preserved() {
    let formatted = fmt("/// Public API\npub fn f() {}\n");
    assert!(formatted.contains("/// Public API"), "got: {formatted}");
}

// ─── Spacing and indentation ──────────────────────────────────────────────────

#[test]
fn four_space_indentation() {
    let formatted = fmt("fn f() {\nlet x = 1\n}\n");
    assert!(formatted.contains("    let x"), "got: {formatted}");
}

#[test]
fn nested_blocks_use_increasing_indentation() {
    let formatted = fmt("fn f() {\nif true {\nlet x = 1\n}\n}\n");
    assert!(formatted.contains("        let x"), "got: {formatted}");
}

#[test]
fn trailing_newline_added() {
    let formatted = fmt("fn f() {}");
    assert!(formatted.ends_with('\n'), "should end with newline");
}

// ─── Large source idempotence ─────────────────────────────────────────────────

#[test]
fn idempotent_on_100_functions() {
    let source: String = (0..100)
        .map(|i| format!("fn func_{i}(x: i32) -> i32 {{\n    return x + {i}\n}}\n\n"))
        .collect();
    idempotent(&source);
}

#[test]
fn formatter_does_not_panic_on_complex_source() {
    let source = r#"
module stress.test

import { a } from "a"
import { b, c as d } from "b"

struct Point<T> {
    x: T
    y: T
}

enum Shape {
    Circle(radius: f64)
    Rect(w: f64, h: f64)
}

fn area(shape: Shape) -> f64 {
    return match shape {
        Shape.Circle(radius) => 3.14 * radius * radius
        Shape.Rect(w, h) => w * h
    }
}

pub fn main() {
    let p = Point { x: 1, y: 2 }
    let shapes = [Shape.Circle(1.0), Shape.Rect(2.0, 3.0)]
    let mut total = 0.0
    for shape in shapes {
        total = total + area(shape)
    }
    println(total)
}
"#;
    let first = fmt(source);
    let second = fmt(&first);
    assert_eq!(first, second);
}

// ─── @script block formatting ─────────────────────────────────────────────────

/// The formatter must emit the canonical layout:
///
/// ```
/// {@script:<lang> as <name>;
///     @mode(<mode>)
///     @exports {
///         fn <name>(<params>) -> <ret>
///     }
///
///     @impl
///     <raw body — byte for byte>
/// @end
/// }
/// ```
#[test]
fn script_block_formats_to_canonical_layout() {
    let src = r#"{@script:js as math;
    @mode(core)
    @exports {
        fn add(a: i32, b: i32) -> i32
    }

    @impl
    export function add(a, b) { return a + b }
@end
}
"#;
    let out = fmt(src);
    assert!(
        out.contains("{@script:js as math;"),
        "header missing: {out}"
    );
    assert!(out.contains("@mode(core)"), "@mode missing: {out}");
    assert!(out.contains("@exports {"), "@exports missing: {out}");
    assert!(
        out.contains("fn add(a: i32, b: i32) -> i32"),
        "export sig missing: {out}"
    );
    assert!(out.contains("@impl"), "@impl missing: {out}");
    assert!(
        out.contains("export function add(a, b) { return a + b }"),
        "raw body missing: {out}"
    );
    assert!(out.contains("@end"), "@end missing: {out}");
}

#[test]
fn script_block_raw_body_preserved_verbatim() {
    // The body has JS object literals, multi-line code, and unusual indentation.
    // None of that should be touched.
    let raw_body =
        "    export function make() {\n        return JSON.stringify({ x: 1, y: { z: 2 } })\n    }";
    let src = format!(
        "{{@script:js as obj;\n    @mode(core)\n    @exports {{\n        fn make() -> str\n    }}\n\n    @impl\n{raw_body}\n@end\n}}\n"
    );
    let out = fmt(&src);
    assert!(
        out.contains("return JSON.stringify({ x: 1, y: { z: 2 } })"),
        "raw body mutated: {out}"
    );
}

#[test]
fn script_block_formatter_is_idempotent() {
    let src = "{@script:js as math;\n    @mode(core)\n    @exports {\n        fn add(a: i32, b: i32) -> i32\n    }\n\n    @impl\n    export function add(a, b) { return a + b }\n@end\n}\n";
    idempotent(src);
}

#[test]
fn script_block_multiple_exports_formatter_idempotent() {
    let src = "{@script:js as utils;\n    @mode(core)\n    @exports {\n        fn add(a: i32, b: i32) -> i32\n        fn sub(a: i32, b: i32) -> i32\n    }\n\n    @impl\n    export function add(a, b) { return a + b }\n    export function sub(a, b) { return a - b }\n@end\n}\n";
    idempotent(src);
}

#[test]
fn script_block_mixed_with_functions_is_idempotent() {
    let src = "fn before(x: i32) -> i32 {\n    return x\n}\n\n{@script:js as ext;\n    @mode(core)\n    @exports {\n        fn double(x: i32) -> i32\n    }\n\n    @impl\n    export function double(x) { return x * 2 }\n@end\n}\n\nfn after() {}\n";
    idempotent(src);
}
