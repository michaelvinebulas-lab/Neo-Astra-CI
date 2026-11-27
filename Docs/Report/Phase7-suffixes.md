# Phase 7 slice 6: complete numeric suffix table

Status: implemented for edition `0.2`. Edition `0.1` keeps its existing `u8`-only suffix.

## Syntax

```astra
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
```

Suffixes bind to the numeric token and are case-sensitive lowercase. They may
follow decimal, hexadecimal, binary, or exponent forms (`0xFFu8`, `0b10u16`,
`1e3f64`). Underscores remain allowed in the digits.

| Kind | Suffixes |
|---|---|
| signed integer | `i8`, `i16`, `i32`, `i64`, `isize` |
| unsigned integer | `u8`, `u16`, `u32`, `u64`, `usize` |
| float | `f32`, `f64` |

There are no `i128` / `u128` suffixes because those types are not part of the
language surface.

## Edition gate and matching

`u8` is accepted in both editions. The full table is recognized only in edition
`0.2`; other suffixes in edition `0.1` remain identifier characters and produce
the existing parse/type diagnostics. The lexer matches the longest suffix first
(`usize` before `u16`, `f32` before `f64` is irrelevant because they differ), and
requires the suffix not be followed by an identifier-continue character.

## Semantics

An explicit suffix fixes the literal type; contextual expectation is ignored.
Without a suffix, integer literals keep the existing `I32` default (or expected
integer type) and float literals keep `F64` (or expected float type). Out-of-range
values still produce `NS4001`.

The shared helper `astra_core::literal::{integer_suffix, float_suffix}` returns
the digit text plus the resolved `Type`; parser annotations, the Nova
interpreter, C emission, and JS/TS emission all strip the same spellings so the
literal digits are never emitted with a Rust-style suffix into C or JavaScript.

## Tests

Lexer tests cover every suffix, longest-match behavior, and the edition `0.1`
boundary. Checker tests assert each suffix's resulting type and range checking.
C, JS, TS, and Nova parity programs print suffixed values. A conformance case
joins the edition-`0.2` corpus.

## Deferred

- Uppercase suffix spellings (`1U8`) and suffixes on character literals.
- `i128` / `u128` suffixes.
