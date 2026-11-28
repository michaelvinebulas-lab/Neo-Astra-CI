//! Phase 7 slice 6: complete integer/float suffix table.

use astra_core::checker::{CheckOptions, check_source};
use astra_core::edition::Edition;

fn check_v02(source: &str) -> Vec<String> {
    match check_source(source, CheckOptions::default().with_edition(Edition::V0_2)) {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.code)
            .collect(),
    }
}

#[test]
fn every_integer_and_float_suffix_typechecks() {
    let diagnostics = check_v02(
        r#"
module app.main

pub fn main() -> i32 {
    let a = 1i8
    let b = 2i16
    let c = 3i32
    let d = 4i64
    let e = 5isize
    let f = 6u8
    let g = 7u16
    let h = 8u32
    let i = 9u64
    let j = 10usize
    let x = 1.5f32
    let y = 2.5f64
    println(a)
    println(b)
    println(c)
    println(d)
    println(e)
    println(f)
    println(g)
    println(h)
    println(i)
    println(j)
    println(x)
    println(y)
    return 0
}
"#,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn suffixed_literals_fix_the_expected_type() {
    let diagnostics = check_v02(
        r#"
module app.main

pub fn main() -> i32 {
    let a: i8 = 1i8
    let b: f32 = 1.5f32
    return 0
}
"#,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");

    let mismatched = check_v02(
        r#"
module app.main

pub fn main() -> i32 {
    let a: i8 = 1i16
    return 0
}
"#,
    );
    assert!(mismatched.contains(&"NS4001".to_owned()), "{mismatched:?}");
}

#[test]
fn out_of_range_suffixed_literals_are_rejected() {
    let diagnostics = check_v02(
        r#"
module app.main

pub fn main() -> i32 {
    let value = 256u8
    return 0
}
"#,
    );
    assert!(
        diagnostics.contains(&"NS4001".to_owned()),
        "{diagnostics:?}"
    );
}

#[test]
fn suffixes_work_on_hex_binary_and_exponent_forms() {
    let diagnostics = check_v02(
        r#"
module app.main

pub fn main() -> i32 {
    let a = 0xFFu16
    let b = 0b1010i8
    let c = 1e3f64
    println(a)
    println(b)
    println(c)
    return 0
}
"#,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn edition_0_1_keeps_u8_only() {
    let diagnostics = match check_source("let value = 1i16\n", CheckOptions::default()) {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.code)
            .collect(),
    };
    assert!(!diagnostics.is_empty(), "{diagnostics:?}");
}
