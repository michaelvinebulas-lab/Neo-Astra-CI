//! Stress tests — TYPE-001: Resolver.

use astra_core::parser::parse;
use astra_core::resolver::{ModuleResolver, resolve_module};

fn ok(source: &str) {
    let m = parse(source).expect("parse");
    resolve_module(&m).unwrap_or_else(|e| panic!("resolve failed: {e:?}\nsource:\n{source}"));
}

fn err(source: &str, code: &str) {
    let m = parse(source).expect("parse");
    let e = resolve_module(&m).expect_err("should fail");
    assert!(
        e.iter().any(|d| d.code == code),
        "expected {code} in {e:?}\nsource:\n{source}"
    );
}

fn ok_graph(modules: Vec<(&str, &str)>) {
    let parsed: Vec<_> = modules
        .iter()
        .map(|(key, src)| {
            let m = parse(src).expect("parse");
            (key.to_string(), m)
        })
        .collect();
    let resolver = ModuleResolver::from_modules(parsed.clone());
    for (key, _) in &parsed {
        resolver
            .resolve_named(key)
            .unwrap_or_else(|e| panic!("resolve_named({key}) failed: {e:?}"));
    }
}

// ─── Local declarations ───────────────────────────────────────────────────────

#[test]
fn all_declaration_kinds_resolve() {
    ok("fn f() {}");
    ok("struct S { x: i32 }");
    ok("enum E { A, B(i32) }");
    ok("const C: i32 = 1");
    ok("type T = i32");
    ok("foreign module m { fn g(x: i32) -> str }");
}

#[test]
fn scoped_items_visible_at_module_scope() {
    ok("{@mode:core;\nfn f() {}\n}\nfn g() { f() }");
}

// ─── Export surfaces ──────────────────────────────────────────────────────────

#[test]
fn pub_declarations_in_export_surface() {
    let m = parse("pub fn f() {}\nfn g() {}").unwrap();
    let resolved = resolve_module(&m).unwrap();
    assert!(
        resolved.exports.named.contains_key("f"),
        "f should be exported"
    );
    assert!(
        !resolved.exports.named.contains_key("g"),
        "g should not be exported"
    );
}

#[test]
fn export_list_promotes_normal_declarations() {
    let m = parse("fn f() {}\nexport { f }").unwrap();
    let resolved = resolve_module(&m).unwrap();
    assert!(resolved.exports.named.contains_key("f"));
}

#[test]
fn export_with_rename() {
    let m = parse("fn f() {}\nexport { f as g }").unwrap();
    let resolved = resolve_module(&m).unwrap();
    assert!(resolved.exports.named.contains_key("g"));
    assert!(!resolved.exports.named.contains_key("f"));
}

#[test]
fn default_export_registered() {
    let m = parse("fn f() {}\nexport default f").unwrap();
    let resolved = resolve_module(&m).unwrap();
    assert!(resolved.exports.default.is_some());
}

#[test]
fn export_default_fn_registered() {
    let m = parse("export default fn f() {}").unwrap();
    let resolved = resolve_module(&m).unwrap();
    assert!(resolved.exports.default.is_some());
}

// ─── Visibility errors ────────────────────────────────────────────────────────

#[test]
fn private_export_is_ns3001() {
    err("private fn f() {}\nexport { f }", "NS3001");
}

#[test]
fn duplicate_export_name_is_ns3003() {
    err("pub fn f() {}\nfn g() {}\nexport { g as f }", "NS3003");
}

#[test]
fn duplicate_default_export_is_ns3004() {
    err(
        "fn f() {}\nfn g() {}\nexport default f\nexport default g",
        "NS3004",
    );
}

#[test]
fn duplicate_declaration_is_ns3011() {
    err("fn f() {}\nfn f() {}", "NS3011");
}

#[test]
fn private_type_leak_through_public_fn_is_ns3010() {
    err("private struct S { x: i32 }\npub fn f(s: S) {}", "NS3010");
}

// ─── Import forms ─────────────────────────────────────────────────────────────

#[test]
fn path_import_resolves_in_graph() {
    ok_graph(vec![
        ("std.io", "pub fn read() {}"),
        ("main", "import std.io"),
    ]);
}

#[test]
fn named_import_resolves_in_graph() {
    ok_graph(vec![
        ("lib", "pub fn add(a: i32, b: i32) -> i32 { return a + b }"),
        (
            "main",
            "import { add } from \"lib\"\nfn f() -> i32 { return add(1, 2) }",
        ),
    ]);
}

#[test]
fn default_import_resolves_in_graph() {
    ok_graph(vec![
        ("lib", "fn helper() {}\nexport default helper"),
        ("main", "import h from \"lib\"\nfn f() { h() }"),
    ]);
}

#[test]
fn star_reexport_resolves_in_graph() {
    ok_graph(vec![
        ("a", "pub fn x() {}"),
        ("b", "export * from \"a\""),
        ("main", "import { x } from \"b\"\nfn f() { x() }"),
    ]);
}

#[test]
fn unresolved_module_in_strict_mode_is_ns3012() {
    let m = parse("import { x } from \"missing\"").unwrap();
    let resolver = ModuleResolver::from_modules(vec![]);
    let e = resolver.resolve(&m).expect_err("should fail");
    assert!(
        e.iter().any(|d| d.code == "NS3012"),
        "expected NS3012 in {e:?}"
    );
}

#[test]
fn cyclic_named_import_is_ns3013() {
    let a = parse("import { x } from \"b\"\nexport { x }").unwrap();
    let b = parse("import { x } from \"a\"\nexport { x }").unwrap();
    let resolver = ModuleResolver::from_modules(vec![("a".to_owned(), a), ("b".to_owned(), b)]);
    let e = resolver.resolve_named("a").expect_err("should fail");
    assert!(
        e.iter().any(|d| d.code == "NS3013"),
        "expected NS3013 in {e:?}"
    );
}

// ─── Symbol tables ────────────────────────────────────────────────────────────

#[test]
fn imported_namespace_in_symbol_table() {
    ok_graph(vec![
        ("math", "pub fn double(n: i32) -> i32 { return n * 2 }"),
        (
            "main",
            "import math\nfn f() -> i32 { return math.double(21) }",
        ),
    ]);
}

#[test]
fn symbols_contain_all_local_declarations() {
    let m = parse("fn f() {}\nstruct S { x: i32 }\nenum E { A }\nconst C: i32 = 1").unwrap();
    let resolved = resolve_module(&m).unwrap();
    assert!(resolved.symbols.values.contains_key("f"));
    assert!(resolved.symbols.values.contains_key("S"));
    assert!(resolved.symbols.values.contains_key("E"));
    assert!(resolved.symbols.values.contains_key("C"));
    assert!(resolved.symbols.types.contains_key("S"));
    assert!(resolved.symbols.types.contains_key("E"));
}
