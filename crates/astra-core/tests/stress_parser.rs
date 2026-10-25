//! Stress tests — PAR-001: Parser and AST.

use astra_core::parser::parse;

fn ok(source: &str) {
    parse(source).unwrap_or_else(|e| panic!("parse failed for:\n{source}\nerrors: {e:?}"));
}

fn err(source: &str, code: &str) {
    let e = parse(source).expect_err(&format!("should have failed: {source}"));
    assert!(
        e.iter().any(|d| d.code == code),
        "expected {code} in {e:?}\nsource: {source}"
    );
}

// ─── Declarations ─────────────────────────────────────────────────────────────

#[test]
fn all_declaration_kinds_parse() {
    ok("fn f() {}");
    ok("struct S { x: i32 }");
    ok("struct T(i32, str)");
    ok("enum E { A, B(i32), C(x: str) }");
    ok("const MAX: u32 = 42");
    ok("type Alias = i32");
    ok("foreign module m { fn g(x: i32) -> str }");
}

#[test]
fn visibility_modifiers_parse() {
    ok("pub fn f() {}");
    ok("public fn f() {}");
    ok("export fn f() {}");
    ok("private fn f() {}");
    ok("pub struct S {}");
    ok("private enum E { A }");
}

#[test]
fn generic_declarations_parse() {
    ok("fn identity<T>(value: T) -> T { return value }");
    ok("struct Box<T> { value: T }");
    ok("enum Result<T, E> { Ok(T), Err(E) }");
    ok("fn map<A, B>(a: A) -> B { return a }");
}

// ─── Types ────────────────────────────────────────────────────────────────────

#[test]
fn composite_types_parse() {
    ok("fn f(x: [i32]) {}");
    ok("fn f(x: [i32; 10]) {}");
    ok("fn f(x: {str: i32}) {}");
    ok("fn f(x: (i32, str)) {}");
    ok("fn f(x: Option<i32>) {}");
    ok("fn f(x: Result<i32, str>) {}");
    ok("fn f(x: fn(i32) -> str) {}");
}

#[test]
fn error_sets_parse() {
    ok("fn f() -> i32 ! Err {}");
    ok("fn f() -> i32 ! A | B | C {}");
    ok("foreign module m { fn g() -> i32 ! E }");
}

// ─── Imports / exports ────────────────────────────────────────────────────────

#[test]
fn all_import_forms_parse() {
    ok("import std.io");
    ok("import std.io as io");
    ok(r#"import "specifier""#);
    ok(r#"import * as ns from "specifier""#);
    ok(r#"import { a } from "specifier""#);
    ok(r#"import { a as b } from "specifier""#);
    ok(r#"import def from "specifier""#);
    ok(r#"import def, { a } from "specifier""#);
}

#[test]
fn all_export_forms_parse() {
    ok("export { f }");
    ok("export { f as g }");
    ok(r#"export { f } from "specifier""#);
    ok(r#"export * from "specifier""#);
    ok(r#"export * as ns from "specifier""#);
    ok("export default fn f() {}");
    ok("fn x() {}\nexport default x");
}

// ─── Statements ───────────────────────────────────────────────────────────────

#[test]
fn all_statement_kinds_parse() {
    ok("fn f() { let x = 1 }");
    ok("fn f() { let mut x = 1 }");
    ok("fn f() { var mut x = 1 }");
    ok("fn f() { return 42 }");
    ok("fn f() { return }");
    ok("fn f() { while true {} }");
    ok("fn f() { for x in xs {} }");
    ok("fn f() { defer print(1) }");
    ok("fn f() { break }");
    ok("fn f() { continue }");
}

#[test]
fn same_line_semicolons_parse() {
    ok("fn f() { let x = 1; let y = 2 }");
    ok("fn f() { let x = 1; let y = 2; let z = 3 }");
    ok("fn a() {}\nfn b() {}");
}

// ─── Expressions ──────────────────────────────────────────────────────────────

#[test]
fn all_expression_forms_parse() {
    ok("fn f() { let value = 1 + 2 }");
    ok("fn f() { let value = a * b + c * d }");
    ok("fn f(x: i32) { let value = -x }");
    ok("fn f() { let value = not true }");
    ok("fn f() { let value = [1, 2, 3] }");
    ok("fn f() { let value = {\"a\": 1, \"b\": 2} }");
    ok("fn f() { let value = (1, \"two\", 3.0) }");
    ok("fn f() { let value = f(1, 2) }");
    ok("fn f() { let value = x.field }");
    ok("fn f() { let value = xs[0] }");
    ok("fn f() { let value = pair.0 }");
    ok("fn f() { spawn || { let value = 1 } }");
}

#[test]
fn wrapping_arithmetic_operators_parse() {
    ok("fn f(a: i32, b: i32) -> i32 { return a +% b }");
    ok("fn f(a: i32, b: i32) -> i32 { return a -% b }");
    ok("fn f(a: i32, b: i32) -> i32 { return a *% b }");
    ok("fn f(a: i32, b: i32) -> i32 { return a /% b }");
}

#[test]
fn error_propagation_forms_parse() {
    ok("fn f() -> i32 ! E { return g()! }");
    ok("fn f() -> i32 ! E { return !E(\"msg\") }");
    ok("fn f() -> i32 ! E { return !E { msg: \"x\" } }");
}

// ─── Match ────────────────────────────────────────────────────────────────────

#[test]
fn match_all_pattern_forms_parse() {
    ok("fn f(x: Option<i32>) -> str { match x { Some(n) => \"some\", None => \"none\" } }");
    ok("fn f(x: bool) { match x { true => {}, false => {} } }");
    ok("fn f(x: i32) { match x { 0 => {}, _ => {} } }");
    ok("fn f(x: (i32, str)) { match x { (1, s) => {}, _ => {} } }");
    ok("fn f() { match x { E.A => {}, E.B(v) => {} } }");
}

// ─── Scoped blocks ────────────────────────────────────────────────────────────

#[test]
fn scoped_mode_and_memory_blocks_parse() {
    ok("{@mode:core;\nfn f() {}\n}");
    ok("{@mode:system;\nfn f() {}\n}");
    ok("fn f() { {@memory:manual;\nlet x = 1\n} }");
    ok("{@mode:web;\n{@memory:auto;\nfn f() {}\n}\n}");
}

// ─── Error recovery ───────────────────────────────────────────────────────────

#[test]
fn missing_closing_paren_is_ns2001() {
    err("fn f(x: i32 {}", "NS2001");
}

#[test]
fn duplicate_visibility_modifier_is_ns3002() {
    err("pub public fn f() {}", "NS3002");
}

#[test]
fn private_with_export_is_ns3002() {
    err("private export fn f() {}", "NS3002");
}

// ─── Complex programs parse ───────────────────────────────────────────────────

#[test]
fn full_module_with_all_constructs_parses() {
    ok(r#"
module app.main

import std.io as io
import { read } from "std:fs"

const MAX: u32 = 1024
type UserId = u64

struct User {
    id: UserId
    name: str
}

enum Status {
    Active
    Inactive(reason: str)
}

struct ParseError(str)

fn parse(input: str) -> i32 ! ParseError {
    if input.is_empty() {
        return !ParseError("empty")
    }
    return input.to_int()!
}

pub fn main() {
    let user = User { id: 1, name: "Astra" }
    let mut count = 0
    for item in [1, 2, 3] {
        count = count + 1
    }
    match Status.Active {
        Status.Active => println("active")
        Status.Inactive(reason) => println(reason)
    }
    let task = spawn || {
        println("task")
    }
    task.join()
}
"#);
}

#[test]
fn parser_handles_1000_fn_declarations() {
    let source: String = (0..1000)
        .map(|i| format!("fn f{i}(x: i32) -> i32 {{ return x + {i} }}\n"))
        .collect();
    ok(&source);
}
