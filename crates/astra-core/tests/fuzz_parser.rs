//! Structural fuzzing pass (HARD-001 A): pseudo-random inputs must never
//! panic the lexer/parser; they may only succeed or report diagnostics.

use astra_core::parser::parse;

#[test]
fn fuzz_parser_never_panics() {
    let alphabet: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789 \n{}();,->:<[]=\"'!@#%\n";
    let mut state: u64 = 0x9E3779B97F4A7C15;
    let mut inputs: Vec<String> = Vec::new();
    for seed in 0u64..1000 {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(seed.wrapping_add(1));
        let len = (state as usize % 100) + 1;
        let mut s = String::with_capacity(len);
        for _ in 0..len {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let idx = (state as usize) % alphabet.len();
            s.push(alphabet[idx] as char);
        }
        inputs.push(s);
    }
    let seeds = [
        "fn f(x: i32) -> i32 { return x }",
        "module m\npub fn main() -> i32 { return 0 }",
    ];
    for seed in seeds {
        inputs.push(seed.to_string());
        let s = seed.to_string();
        for i in 0..seed.len() {
            let mut t = s.clone();
            if i < t.len() {
                t.remove(i);
            }
            inputs.push(t);
        }
    }
    let mut failures: Vec<String> = Vec::new();
    for src in &inputs {
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = parse(src);
        }))
        .is_err()
        {
            failures.push(src.clone());
        }
    }
    if !failures.is_empty() {
        panic!(
            "parser panicked on {} inputs; first: {:?}",
            failures.len(),
            failures[0]
        );
    }
}
