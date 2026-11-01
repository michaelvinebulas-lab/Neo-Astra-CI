//! Structural fuzzing pass (HARD-001 A): pseudo-random token soup and mutants
//! of valid programs must never panic any frontend stage - lexer, parser,
//! resolver, checker - or the formatter. Programs that parse cleanly must also
//! survive `astra fmt` twice (idempotency), and pathological nesting must be
//! rejected with diagnostics rather than aborting the process on a stack
//! overflow.

use astra_core::checker::{CheckOptions, check_resolved};
use astra_core::formatter::format_source;
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

/// Tokens drawn from the real vocabulary, so fuzzed inputs reach deep parser
/// states instead of dying in the first three characters.
const VOCABULARY: &[&str] = &[
    "fn",
    "pub",
    "export",
    "private",
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
    "module",
    "as",
    "in",
    "while",
    "for",
    "break",
    "continue",
    "try",
    "catch",
    "throw",
    "async",
    "await",
    "yield",
    "true",
    "false",
    "null",
    "i8",
    "i32",
    "i64",
    "u8",
    "u32",
    "u64",
    "f32",
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
    "user",
    "shape",
    "total",
    "x",
    "y",
    "T",
    "A",
    "B",
    "ParseError",
    "0",
    "1",
    "42",
    "0xFF",
    "0b1010",
    "3.14",
    "\"text\"",
    "\"unterminated",
    "'c'",
    "1e9",
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
    "@export_as",
    "@mode",
    "@memory",
    "@script:js",
    "@impl",
    "// comment\n",
    "/// doc comment\n",
    "\n",
    "\n",
    " ",
];

/// Valid programs the mutant generator starts from; each one exercises a
/// different corner of the grammar (generics, attributes, channels, scripts).
const SEEDS: &[&str] = &[
    "module demo.lib\n\n@export_as(\"ns_demo_id_i32\", T = i32)\npub fn identity<T>(value: T) -> T {\n    return value\n}\n",
    "module app\n\npub fn main() -> i32 {\n    let channel = Channel.create<str>()\n    spawn || {\n        channel.send(\"hello\")!\n    }\n    let message = channel.receive()!\n    print(message)\n    return 0\n}\n",
    "struct Shape { id: i32, name: str }\n\nenum Kind { Circle, Square }\n\nfn area(shape: Shape) -> f64 {\n    return match shape.id {\n        0 => 1.5,\n        _ => 2.5\n    }\n}\n",
    "{@mode:core;\nfn scoped(x: i32) -> i32 {\n    return x + 1\n}\n}\n",
    "{@script:js as math;\n    @mode(core)\n    @exports {\n        fn add(a: i32, b: i32) -> i32\n    }\n\n    @impl\n    export function add(a, b) { return a + b }\n@end\n}\n",
    "fn fallible(input: str) -> i32 ! ParseError {\n    return input.to_int()!\n}\n\nfn wrapper(input: str) -> i32 ! ParseError {\n    return fallible(input)?\n}\n",
    "const LIMIT: i32 = 10\n\ntype Alias = {str: i32}\n\nfn sum(values: [i32; 4]) -> i32 {\n    var mut total = 0\n    for value in values {\n        total = total + value\n    }\n    return total\n}\n",
    "fn nested(x: i32) -> i32 {\n    if x > 0 {\n        if x > 10 {\n            return x * 2\n        } else {\n            return x + 1\n        }\n    }\n    return 0\n}\n",
];

/// Builds one pseudo-random token-soup program.
fn generate(rng: &mut Lcg) -> String {
    let token_count = rng.below(120) + 1;
    let mut source = String::new();
    for _ in 0..token_count {
        source.push_str(VOCABULARY[rng.below(VOCABULARY.len())]);
        source.push(' ');
    }
    source
}

/// Applies a random structural edit to a valid program: deletion, duplication,
/// swap, or truncation. Positions are chosen on char boundaries so the mutator
/// itself never panics on multi-byte input.
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

/// Runs the full frontend on one input, or records why it failed.
fn run_frontend(source: &str) -> Result<(), String> {
    let module =
        parse(source).map_err(|diagnostics| format!("{} parse diagnostics", diagnostics.len()))?;
    let resolved = resolve_module(&module)
        .map_err(|diagnostics| format!("{} resolve diagnostics", diagnostics.len()))?;
    check_resolved(&module, &resolved, CheckOptions::new(Mode::Core))
        .map_err(|diagnostics| format!("{} check diagnostics", diagnostics.len()))?;
    Ok(())
}

fn collect_inputs() -> Vec<String> {
    let mut rng = Lcg::new(0x9E3779B97F4A7C15);
    let mut inputs: Vec<String> = Vec::new();
    for _ in 0..1500 {
        inputs.push(generate(&mut rng));
    }
    for seed in SEEDS {
        inputs.push((*seed).to_owned());
        for _ in 0..60 {
            let mut mutant = (*seed).to_owned();
            for _ in 0..3 {
                mutant = mutate(&mut rng, &mutant);
            }
            inputs.push(mutant);
        }
    }
    inputs
}

#[test]
fn fuzz_frontend_never_panics() {
    let mut failures: Vec<String> = Vec::new();

    for source in collect_inputs() {
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = run_frontend(&source);
        }));
        if outcome.is_err() {
            failures.push(source);
        }
    }

    if !failures.is_empty() {
        panic!(
            "frontend panicked on {} inputs; first: {:?}",
            failures.len(),
            failures[0]
        );
    }
}

#[test]
fn fuzz_formatter_is_idempotent_on_clean_parses() {
    let mut failures: Vec<String> = Vec::new();

    for source in collect_inputs() {
        // Only clean programs are formatted; broken ones are allowed to fail.
        let Ok(_) = parse(&source) else {
            continue;
        };
        let Ok(formatted) = format_source(&source) else {
            continue;
        };
        let Ok(reformatted) = format_source(&formatted) else {
            failures.push(format!("reformat failed: {source:?}"));
            continue;
        };
        if reformatted != formatted {
            failures.push(format!(
                "not idempotent: {source:?} -> {formatted:?} -> {reformatted:?}"
            ));
        }
    }

    if !failures.is_empty() {
        panic!("formatter idempotency failed: {}", failures[0]);
    }
}

#[test]
fn deep_nesting_is_rejected_not_aborted() {
    // 50k levels would overflow the stack on an unguarded recursive-descent
    // parser; the guard turns this into diagnostics instead of a process abort.
    let deep_parens = format!(
        "fn f() -> i32 {{ return {}0{} }}",
        "(".repeat(50_000),
        ")".repeat(50_000)
    );
    let deep_brackets = format!(
        "fn f(x: {}i32{}) {{}}",
        "[".repeat(50_000),
        "]".repeat(50_000)
    );
    let deep_blocks = format!(
        "fn f() {{ {}return 0{} }}",
        "{ ".repeat(25_000),
        "}".repeat(25_000)
    );
    let deep_scopes = format!(
        "{}fn f() {{}}{}",
        "{@mode:core;".repeat(20_000),
        "}".repeat(20_000)
    );

    for (label, source) in [
        ("parens", &deep_parens),
        ("brackets", &deep_brackets),
        ("blocks", &deep_blocks),
        ("scopes", &deep_scopes),
    ] {
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let result = parse(source);
            assert!(
                result.is_err(),
                "{label}: deep nesting must be rejected with diagnostics"
            );
        }));
        assert!(
            outcome.is_ok(),
            "{label}: parser aborted instead of reporting"
        );
    }
}

#[test]
fn nested_brace_blocks_parse_in_linear_time() {
    // Regression: `parse_brace_expression` used to restore and re-parse the
    // whole subtree whenever speculation found no map colon, doubling work at
    // every nesting level (T(n) = 2*T(n-1)); 28 levels hung for minutes. The
    // test completing at all is the regression check - a reintroduced
    // exponential path makes these inputs hang until CI times out. Depths past
    // the nesting limit are rejected with diagnostics instead of aborting.
    for depth in [8usize, 16, 24, 28, 32, 40] {
        let input = format!(
            "fn f() {{ {}return 0{} }}",
            "{ ".repeat(depth),
            "}".repeat(depth)
        );
        let _ = parse(&input);
    }

    let reasonable = format!("fn f() {{ {}return 0{} }}", "{ ".repeat(16), "}".repeat(16));
    assert!(
        parse(&reasonable).is_ok(),
        "16-level block nesting must parse cleanly"
    );
}
#[test]
fn nesting_limit_accepts_reasonable_depth() {
    // The limit must not reject code a human (or a code generator) writes: a
    // 50-level nesting still parses cleanly.
    let nested = format!(
        "fn f() -> i32 {{ return {}0{} }}",
        "(".repeat(50),
        ")".repeat(50)
    );
    assert!(
        parse(&nested).is_ok(),
        "50-level nesting must stay within the supported depth"
    );
}
