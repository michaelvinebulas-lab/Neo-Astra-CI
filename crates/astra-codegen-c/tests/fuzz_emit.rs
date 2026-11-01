//! HARD-001 A - C emitter robustness pass.
//!
//! The frontend fuzz suite (`astra-core/tests/fuzz_frontend.rs`) proves that
//! arbitrary input never panics the lexer, parser, resolver, checker, or
//! formatter. This suite extends the same guarantee one stage further: any
//! input that parses, resolves, and checks cleanly must also emit C without
//! panicking. A `BackendError` is a valid outcome; a panic is not.

use astra_codegen_c::{CModule, IntegerPolicy, generate};
use astra_core::checker::{CheckOptions, check_resolved};
use astra_core::mode::Mode;
use astra_core::parser::parse;
use astra_core::resolver::resolve_module;

/// Deterministic LCG so the corpus is reproducible on every platform.
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223486793005)
            .wrapping_add(1442695040888963407);
        self.0
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
}

/// Tokens drawn from the real vocabulary so generated inputs reach the
/// emitter instead of dying in the lexer.
const VOCABULARY: &[&str] = &[
    "module",
    "pub",
    "fn",
    "let",
    "mut",
    "return",
    "if",
    "else",
    "match",
    "spawn",
    "struct",
    "enum",
    "type",
    "const",
    "import",
    "as",
    "in",
    "while",
    "for",
    "break",
    "continue",
    "true",
    "false",
    "null",
    "i32",
    "i64",
    "u8",
    "u32",
    "f64",
    "str",
    "bool",
    "bytes",
    "void",
    "any",
    "Option",
    "Result",
    "Task",
    "Channel",
    "Map",
    "Vec",
    "Ptr",
    "Region",
    "Memory",
    "identity",
    "main",
    "value",
    "channel",
    "message",
    "shape",
    "total",
    "x",
    "y",
    "T",
    "0",
    "1",
    "42",
    "0xFF",
    "3.14",
    "\"text\"",
    "'c'",
    "{",
    "}",
    "(",
    ")",
    "[",
    "]",
    ";",
    ",",
    ":",
    "->",
    "=>",
    "=",
    "==",
    "!=",
    "<",
    "<=",
    ">",
    ">=",
    "+",
    "-",
    "*",
    "/",
    "%",
    "+%",
    "-%",
    "*%",
    "@",
    ".",
    "?",
    "|",
    "||",
    "&",
    "&&",
    "!",
    "??",
    "?.",
    "@requires",
    "@tags",
    "@mode",
    "@memory",
    "\n",
    " ",
];

/// Valid programs the mutant generator starts from; these exercise managed
/// values, structs, enums, loops, error unions, and channels in emission.
const SEEDS: &[&str] = &[
    "module app\n\npub fn main() -> i32 {\n    let x = 1 + 2\n    return x\n}\n",
    "struct Shape { id: i32, name: str }\n\nfn area(shape: Shape) -> f64 {\n    return match shape.id {\n        0 => 1.5,\n        _ => 2.5\n    }\n}\n",
    "fn fallible(input: str) -> i32 ! ParseError {\n    return input.to_int()!\n}\n",
    "struct Point { x: i32, y: i32 }\n\nfn total(p: Point) -> i32 {\n    return p.x + p.y\n}\n",
    "enum Kind { Circle, Square }\n\nfn label(k: Kind) -> i32 {\n    return match k {\n        Kind.Circle => 1,\n        Kind.Square => 2\n    }\n}\n",
    "fn sum(values: [i32; 4]) -> i32 {\n    var mut total = 0\n    for value in values {\n        total = total + value\n    }\n    return total\n}\n",
    "fn nested(x: i32) -> i32 {\n    if x > 0 {\n        if x > 10 {\n            return x * 2\n        } else {\n            return x + 1\n        }\n    }\n    return 0\n}\n",
    "fn strings(name: str) -> str {\n    return name\n}\n",
];

fn generate_source(rng: &mut Lcg) -> String {
    let token_count = rng.below(120) + 1;
    let mut source = String::new();
    for _ in 0..token_count {
        source.push_str(VOCABULARY[rng.below(VOCABULARY.len())]);
        source.push(' ');
    }
    source
}

fn mutate(rng: &mut Lcg, source: &str) -> String {
    let boundaries: Vec<usize> = source.char_indices().map(|(index, _)| index).collect();
    if boundaries.is_empty() {
        return String::new();
    }
    let position = boundaries[rng.below(boundaries.len())];
    match rng.below(4) {
        0 => {
            let mut out = source.to_owned();
            out.remove(position);
            out
        }
        1 => {
            let mut out = source.to_owned();
            let tail = out.split_off(position);
            format!("{out}{tail}{tail}")
        }
        2 => {
            let mut out: Vec<char> = source.chars().collect();
            let len = out.len();
            let other = rng.below(len);
            out.swap(position % len, other);
            out.into_iter().collect()
        }
        _ => source[..position].to_owned(),
    }
}

fn collect_inputs() -> Vec<String> {
    let mut rng = Lcg::new(0xD1B54A32D192ED03);
    let mut inputs: Vec<String> = Vec::new();
    for _ in 0..600 {
        inputs.push(generate_source(&mut rng));
    }
    for seed in SEEDS {
        inputs.push((*seed).to_owned());
        for _ in 0..80 {
            let mut mutant = (*seed).to_owned();
            for _ in 0..3 {
                mutant = mutate(&mut rng, &mutant);
            }
            inputs.push(mutant);
        }
    }
    inputs
}

/// Emits C for one source, or records why it did not reach the emitter.
fn emit(source: &str) -> Result<(), String> {
    let module = match parse(source) {
        Ok(module) => module,
        Err(_) => return Err("parse".to_owned()),
    };
    let resolved = match resolve_module(&module) {
        Ok(resolved) => resolved,
        Err(_) => return Err("resolve".to_owned()),
    };
    let checked = match check_resolved(&module, &resolved, CheckOptions::new(Mode::Core)) {
        Ok(checked) => checked,
        Err(_) => return Err("check".to_owned()),
    };
    let c_module = CModule {
        name: checked.module.name.clone(),
        ast: module,
        hir: checked.module,
        annotations: checked.annotations,
        manual_memory_scopes: checked.manual_memory_scopes,
        integer_policy: IntegerPolicy::Wrapping,
    };
    // A BackendError is a supported outcome; only a panic is a failure.
    let _ = generate(&c_module);
    Ok(())
}

#[test]
fn fuzz_c_emission_never_panics() {
    let mut reached = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for source in collect_inputs() {
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| emit(&source)));
        match outcome {
            Ok(Ok(())) => reached += 1,
            Ok(Err(_)) => {}
            Err(_) => failures.push(source),
        }
    }

    assert!(
        failures.is_empty(),
        "C emitter panicked on {} inputs; first: {:?}",
        failures.len(),
        failures[0]
    );
    assert!(
        reached > 0,
        "no input reached the C emitter; the corpus or pipeline regressed"
    );
}
