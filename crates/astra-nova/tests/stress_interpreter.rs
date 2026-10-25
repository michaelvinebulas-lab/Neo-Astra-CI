//! Stress tests — NOVA-001: Typed-HIR interpreter.

use astra_nova::{Host, IntegerPolicy, NovaError, NovaOptions, Value, build_interpreter};
use std::cell::RefCell;
use std::rc::Rc;

struct Cap {
    stdout: Rc<RefCell<String>>,
    stderr: Rc<RefCell<String>>,
}
impl Host for Cap {
    fn stdout(&mut self, t: &str) {
        self.stdout.borrow_mut().push_str(t);
    }
    fn stderr(&mut self, t: &str) {
        self.stderr.borrow_mut().push_str(t);
    }
}

fn run(source: &str) -> (Result<Value, NovaError>, String, String) {
    let stdout = Rc::new(RefCell::new(String::new()));
    let stderr = Rc::new(RefCell::new(String::new()));
    let host = Cap {
        stdout: stdout.clone(),
        stderr: stderr.clone(),
    };
    let result = match build_interpreter(source, NovaOptions::default(), Box::new(host)) {
        Ok(mut i) => i.run(),
        Err(e) => Err(e),
    };
    (result, stdout.borrow().clone(), stderr.borrow().clone())
}

fn run_mode(
    source: &str,
    mode: astra_core::mode::Mode,
) -> (Result<Value, NovaError>, String, String) {
    let stdout = Rc::new(RefCell::new(String::new()));
    let stderr = Rc::new(RefCell::new(String::new()));
    let host = Cap {
        stdout: stdout.clone(),
        stderr: stderr.clone(),
    };
    let opts = NovaOptions {
        mode,
        infer_mode: false,
        ..NovaOptions::default()
    };
    let result = match build_interpreter(source, opts, Box::new(host)) {
        Ok(mut i) => i.run(),
        Err(e) => Err(e),
    };
    (result, stdout.borrow().clone(), stderr.borrow().clone())
}

// ─── Primitive values ─────────────────────────────────────────────────────────

#[test]
fn integer_arithmetic() {
    let (result, out, _) = run(
        "fn main() {\nprintln(1 + 2)\nprintln(10 - 3)\nprintln(4 * 5)\nprintln(20 / 4)\nprintln(7 % 3)\n}",
    );
    assert!(result.is_ok());
    assert_eq!(out, "3\n7\n20\n5\n1\n");
}

#[test]
fn float_arithmetic() {
    let (result, out, _) = run("fn main() { println(1.5 + 2.5) }");
    assert!(result.is_ok());
    assert!(out.trim() == "4.0", "got: {out}");
}

#[test]
fn boolean_logic() {
    let (result, out, _) =
        run("fn main() {\nprintln(true && false)\nprintln(true || false)\nprintln(not true)\n}");
    assert!(result.is_ok());
    assert_eq!(out, "false\ntrue\nfalse\n");
}

#[test]
fn string_operations() {
    let (result, out, _) = run("fn main() { let s = \"hello, Nova\"\nprintln(s.len()) }");
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(out.trim(), "11");
}

// ─── Control flow ─────────────────────────────────────────────────────────────

#[test]
fn if_else_branch() {
    let (result, out, _) =
        run("fn main() {\nif 1 < 2 { println(\"yes\") } else { println(\"no\") }\n}");
    assert!(result.is_ok());
    assert_eq!(out.trim(), "yes");
}

#[test]
fn while_loop() {
    let (result, out, _) =
        run("fn main() {\nlet mut i = 0\nwhile i < 5 { i = i + 1 }\nprintln(i)\n}");
    assert!(result.is_ok());
    assert_eq!(out.trim(), "5");
}

#[test]
fn for_loop_over_array() {
    let (result, out, _) = run(
        "fn main() {\nlet mut sum = 0\nfor x in [1, 2, 3, 4, 5] { sum = sum + x }\nprintln(sum)\n}",
    );
    assert!(result.is_ok());
    assert_eq!(out.trim(), "15");
}

#[test]
fn break_and_continue() {
    let (result, out, _) = run(
        "fn main() {\nlet mut n = 0\nfor x in [1, 2, 3, 4, 5] {\nif x == 2 { continue }\nif x == 4 { break }\nn = n + x\n}\nprintln(n)\n}",
    );
    assert!(result.is_ok());
    assert_eq!(out.trim(), "4"); // 1 + 3 (skipped 2, stopped at 4)
}

#[test]
fn defer_runs_in_reverse_order() {
    let (result, out, _) = run(
        "fn main() {\ndefer println(\"1\")\ndefer println(\"2\")\ndefer println(\"3\")\nprintln(\"body\")\n}",
    );
    assert!(result.is_ok());
    assert_eq!(out, "body\n3\n2\n1\n");
}

// ─── Recursion ────────────────────────────────────────────────────────────────

#[test]
fn recursive_fibonacci() {
    let (result, out, _) = run(
        "fn fib(n: i32) -> i32 {\nif n <= 1 { return n }\nreturn fib(n - 1) + fib(n - 2)\n}\nfn main() { println(fib(10)) }",
    );
    assert!(result.is_ok());
    assert_eq!(out.trim(), "55");
}

#[test]
fn mutual_recursion() {
    let (result, out, _) = run(
        "fn is_even(n: i32) -> bool {\nif n == 0 { return true }\nreturn is_odd(n - 1)\n}\nfn is_odd(n: i32) -> bool {\nif n == 0 { return false }\nreturn is_even(n - 1)\n}\nfn main() {\nprintln(is_even(10))\nprintln(is_odd(7))\n}",
    );
    assert!(result.is_ok());
    assert_eq!(out, "true\ntrue\n");
}

// ─── Structs and enums ────────────────────────────────────────────────────────

#[test]
fn struct_field_mutation() {
    let (result, out, _) = run(
        "struct Point { x: i32\ny: i32 }\nfn main() {\nlet mut p = Point { x: 1, y: 2 }\np.x = 10\nprintln(p.x)\nprintln(p.y)\n}",
    );
    assert!(result.is_ok());
    assert_eq!(out, "10\n2\n");
}

#[test]
fn enum_match_all_variants() {
    let (result, out, _) = run(
        "enum Color { Red, Green, Blue }\nfn name(c: Color) -> str {\nreturn match c {\nColor.Red => \"red\"\nColor.Green => \"green\"\nColor.Blue => \"blue\"\n}\n}\nfn main() {\nprintln(name(Color.Red))\nprintln(name(Color.Green))\nprintln(name(Color.Blue))\n}",
    );
    assert!(result.is_ok());
    assert_eq!(out, "red\ngreen\nblue\n");
}

// ─── Collections ──────────────────────────────────────────────────────────────

#[test]
fn array_indexing_and_mutation() {
    let (result, out, _) = run(
        "fn main() {\nlet mut xs = [1, 2, 3]\nxs.push(4)\nprintln(xs.len())\nprintln(xs[3])\n}",
    );
    assert!(result.is_ok());
    assert_eq!(out, "4\n4\n");
}

#[test]
fn map_insertion_and_lookup() {
    let (result, out, _) = run(r#"fn main() {
let mut m = {"a": 1}
m["b"] = 2
println(m.get("a").unwrap())
println(m.get("b").unwrap())
}"#);
    assert!(result.is_ok());
    assert_eq!(out, "1\n2\n");
}

#[test]
fn map_insertion_order_preserved() {
    let (result, out, _) = run(r#"fn main() {
let m = {"z": 3, "a": 1, "m": 2}
for k in m { print(k) }
println("")
}"#);
    assert!(result.is_ok());
    assert_eq!(out, "zam\n");
}

#[test]
fn tuple_indexing() {
    let (result, out, _) = run(
        "fn main() {\nlet mut t = (10, \"hello\", true)\nprintln(t.0)\nprintln(t.1)\nprintln(t.2)\nt.0 = 99\nprintln(t.0)\n}",
    );
    assert!(result.is_ok());
    assert_eq!(out, "10\nhello\ntrue\n99\n");
}

// ─── Option and Result ────────────────────────────────────────────────────────

#[test]
fn option_some_and_none() {
    let (result, out, _) = run(
        "fn find(b: bool) -> Option<i32> {\nif b { return Some(42) }\nreturn None\n}\nfn main() {\nmatch find(true) {\nSome(n) => println(n)\nNone => println(0)\n}\nmatch find(false) {\nSome(n) => println(n)\nNone => println(0)\n}\n}",
    );
    assert!(result.is_ok());
    assert_eq!(out, "42\n0\n");
}

#[test]
fn option_unwrap_or() {
    let (result, out, _) = run("fn main() { println(None.unwrap_or(99)) }");
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(out.trim(), "99");
}

#[test]
fn error_propagation_chain() {
    let (result, out, _) = run(
        "struct E(str)\nfn inner() -> i32 ! E { return !E(\"fail\") }\nfn outer() -> i32 ! E { return inner()! }\nfn main() {\nmatch outer() {\nOk(n) => println(n)\nErr(_) => println(\"caught\")\n}\n}",
    );
    assert!(result.is_ok());
    assert_eq!(out.trim(), "caught");
}

// ─── Panic and abort ──────────────────────────────────────────────────────────

#[test]
fn panic_produces_nv0005() {
    let (result, _, _) = run("fn main() { panic(\"boom\") }");
    assert!(matches!(result, Err(NovaError::Panic(_))));
}

#[test]
fn panic_has_message() {
    let (result, _, _) = run("fn main() { panic(\"test message\") }");
    let Err(NovaError::Panic(p)) = result else {
        panic!("expected Panic")
    };
    assert_eq!(p.message, "test message");
}

#[test]
fn abort_does_not_run_defers() {
    let (result, out, _) =
        run("fn main() {\ndefer println(\"defer\")\nprintln(\"before\")\nabort(\"stop\")\n}");
    assert_eq!(out, "before\n");
    assert!(matches!(result, Err(NovaError::Abort(_))));
}

#[test]
fn bounds_check_panic() {
    let (result, _, _) = run("fn main() { let xs = [1, 2, 3]\nprintln(xs[99]) }");
    let Err(NovaError::Panic(p)) = result else {
        panic!("expected bounds panic")
    };
    assert!(
        p.message.contains("bounds"),
        "expected bounds in: {}",
        p.message
    );
}

// ─── Integer policies ─────────────────────────────────────────────────────────

#[test]
fn wrapping_policy_default() {
    let (result, out, _) = run("fn main() { println(255u8 +% 1) }");
    assert!(result.is_ok());
    assert_eq!(out.trim(), "0");
}

#[test]
fn safe_checked_overflow_panics() {
    let opts = NovaOptions {
        integer_policy: IntegerPolicy::SafeChecked,
        ..NovaOptions::default()
    };
    let source = "fn main() { let mut x: u8 = 255\nx = x + 1 }";
    let stdout = Rc::new(RefCell::new(String::new()));
    let stderr = Rc::new(RefCell::new(String::new()));
    let host = Cap {
        stdout: stdout.clone(),
        stderr: stderr.clone(),
    };
    let result = match build_interpreter(source, opts, Box::new(host)) {
        Ok(mut i) => i.run(),
        Err(e) => Err(e),
    };
    assert!(
        matches!(result, Err(NovaError::Panic(_))),
        "safe-checked overflow should panic"
    );
}

// ─── Tasks ────────────────────────────────────────────────────────────────────

#[test]
fn spawn_runs_eagerly() {
    let (result, out, _) = run(
        "fn main() {\nlet task = spawn || { println(\"task\") }\ntask.join()\nprintln(\"after\")\n}",
    );
    assert!(result.is_ok());
    assert_eq!(out, "task\nafter\n");
}

#[test]
fn spawn_returns_value() {
    let (result, out, _) =
        run("fn main() {\nlet task = spawn || { 21 * 2 }\nprintln(task.join())\n}");
    assert!(result.is_ok());
    assert_eq!(out.trim(), "42");
}

#[test]
fn task_panic_fails_scope() {
    let (result, out, _) =
        run("fn main() {\nlet task = spawn || { panic(\"task panic\") }\nprintln(\"after\")\n}");
    assert_eq!(out, "after\n");
    assert!(matches!(result, Err(NovaError::Panic(_))));
}

// ─── Builtins ─────────────────────────────────────────────────────────────────

#[test]
fn dbg_writes_to_stderr() {
    let (result, out, err) = run("fn main() { dbg(\"hello\") }");
    assert!(result.is_ok());
    assert_eq!(out, "");
    assert!(err.contains("\"hello\""));
}

#[test]
fn assert_passes() {
    let (result, _, _) = run("fn main() { assert(true) }");
    assert!(result.is_ok());
}

#[test]
fn assert_fails_is_panic() {
    let (result, _, _) = run("fn main() { assert(false) }");
    assert!(matches!(result, Err(NovaError::Panic(_))));
}

// ─── Constants ───────────────────────────────────────────────────────────────

#[test]
fn constants_evaluated_lazily_once() {
    let (result, out, _) = run("const X: i32 = 21 * 2\nfn main() {\nprintln(X)\nprintln(X)\n}");
    assert!(result.is_ok());
    assert_eq!(out, "42\n42\n");
}

// ─── Missing entry point ──────────────────────────────────────────────────────

#[test]
fn missing_main_is_nv0002() {
    let (result, _, _) = run("fn helper() {}");
    assert!(matches!(result, Err(NovaError::MissingEntry { .. })));
}

// ─── Manual memory rejection ─────────────────────────────────────────────────

#[test]
fn manual_memory_in_system_mode_rejected_nv0001() {
    let (result, _, _) = run_mode(
        "fn main() { {@memory:manual;\nlet x = 1\n} }",
        astra_core::mode::Mode::System,
    );
    assert!(matches!(
        result,
        Err(NovaError::ManualMemoryUnsupported { .. })
    ));
}

// ─── Complex program ─────────────────────────────────────────────────────────

#[test]
fn complex_linked_list_program() {
    let source = r#"
enum List {
    Nil,
    Cons(i32, List),
}

fn sum(list: List) -> i32 {
    return match list {
        List.Nil => 0,
        List.Cons(head, tail) => head + sum(tail),
    }
}

fn main() {
    let list = List.Cons(1, List.Cons(2, List.Cons(3, List.Nil)))
    println(sum(list))
}
"#;
    let (result, out, _) = run(source);
    assert!(result.is_ok(), "expected ok: {:?}", result);
    assert_eq!(out.trim(), "6");
}
