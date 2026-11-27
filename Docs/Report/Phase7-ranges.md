# Phase 7 slice 2: ranges

Status: implemented for edition `0.2`. Edition `0.1` is unchanged.

## Syntax

```astra
for value in 0..10 { ... }   // exclusive: 0 through 9
for value in 0..=10 { ... }  // inclusive: 0 through 10

let part = text[0..2]?        // str slice, returns Result<str, SliceError>
let bytes = data[1..=3]       // bytes/array slice, infallible
```

Only `a..b` and `a..=b` are recognized. Open-ended ranges (`..b`, `a..`, `..`)
are not part of this slice.

## Representation

- The lexer emits `DotDot` / `DotDotEq` only under `Edition::V0_2`; edition
  `0.1` still sees two `Dot` tokens and reports its existing parse error.
- `a..b` / `a..=b` parse to `ExprKind::Range { start, end, inclusive }`, a
  genuine AST node, so the formatter keeps range spelling in `for` headers.
- The checker gives a range the internal type `Type::Range(T)`. It unifies both
  bounds, so `0..10` has element type `i32` and `0u8..10u8` has `u8`. Non-integer
  bounds are rejected with `NS4006`.
- `for` consumes `Type::Range(T)` directly and binds the loop variable as `T`;
  no separate iterable type or new backend value layout is needed.

## Backend semantics

Bounds are evaluated exactly once. All three backends lower a range `for` to a
counter loop:

- C: `int32_t limit = end; for (int32_t i = start; i < limit; ++i) { ... }`
  (`<=` for inclusive ranges). The loop variable is republished as a GC root
  for its body.
- JavaScript/TypeScript: `const limit = end; for (let i = start; i < limit;
  i += 1) { const value = i; ... }`.
- Nova: materializes the integer sequence once and iterates it.

`break` and `continue` keep their existing loop semantics because the counter
increment lives in the loop header, not after the user body.

## Slicing

`target[start..end]` in index position is sugar for `target.slice(start, end)`;
for `..=` the end is `end + 1`. Lowering happens in the parser, so slicing
reuses the existing method checker and all three backend method emitters:

- `str.slice` keeps its `Result<str, SliceError>` effect and UTF-8 boundary
  checks.
- `bytes.slice` is infallible and bounds-checked (panics on reversed or
  out-of-range ends).
- Array/slice `.slice(start, end)` is new in this slice: it returns a freshly
  allocated `Array<T>` (C: `astra_array_slice`; JS/TS: bounds-checked
  `$arraySlice`; Nova: a copied `Value::Array`).

Consequence: the formatter canonicalizes index-position range sugar to
`target.slice(start, end)`. Standalone range expressions and `for` headers keep
their range spelling.

## Edition gate and release boundary

This is part of the `1.1.2` additive release boundary for Phase 7. Nothing
changes under edition `0.1`; `spec-v0.2.md` remains the normative document for
edition `0.2`.

## Tests

- Lexer/parser unit tests assert exclusive/inclusive range nodes and the slice
  desugaring shape.
- `crates/astra-core/tests/phase7_ranges.rs` checks `for`, string slicing,
  inclusive slicing, and array slicing under edition `0.2`, plus the edition
  `0.1` rejection.
- C emission tests compile and run exclusive/inclusive loops and array slicing,
  including a hosted run that checks `2` / `2` lengths.
- JS and TS codegen integration tests run the same programs, including the
  strict `tsc` parity loop.
- `conformance/v0.2/valid/ranges.ns` joins the shared edition-`0.2` corpus.

## Deferred

- Open-ended ranges and step ranges.
- Range values stored in locals or passed to functions: the backends reject
  `Range` outside a `for` iterable.
- Floating-point or `char` ranges.
