# Phase 7 slice 4: `char` and scalar literals

Status: implemented for edition `0.2`. Edition `0.1` is unchanged.

## Syntax

```astra
let letter = 'a'
let newline = '\n'
let emoji = '\u{1F600}'

fn label(value: char) -> str {
    return match value {
        'a' => "first",
        _ => "other",
    }
}
```

A character literal contains exactly one Unicode scalar value. Escapes mirror
string escapes (`\\`, `\'`, `\n`, `\r`, `\t`, `\0`, `\u{...}`); an empty,
multi-scalar, or surrogate literal is rejected with `NS1002`.

`char` is a distinct primitive type under edition `0.2`. Under edition `0.1`,
`'...'` is not a token and `char` is not a primitive type.

## Representation

- AST: `Literal::Character(char)`; no new expression node.
- Checker: `Type::Char`, a standalone scalar. It is neither numeric nor a
  string, so arithmetic, bitwise operators, ordering, and ranges do not apply.
- Nova and JS/TS represent a `char` as a one-scalar string value. The C backend
  stores the Unicode code point in `uint32_t` and emits UTF-8 when printing.
- Equality (`==` / `!=`) and literal patterns are supported. Conversion to
  `u32`, `str.chars()` returning `char`, and `char` ranges are deferred.

## Edition gate and release boundary

Part of the `1.1.2` additive Phase 7 boundary. Nothing changes under edition
`0.1`; `spec-v0.2.md` remains normative for edition `0.2`.

## Tests

Lexer/parser tests cover accepted escapes and rejection of empty/multi-scalar
literals. Checker tests cover `char` annotations, equality, patterns, and the
edition `0.1` rejection. C, JS, and TS parity programs print ASCII and
non-ASCII characters. A conformance case joins the edition-`0.2` corpus.

## Deferred

- `char` to integer conversion and integer to `char` conversion.
- `str` iteration/indexing that yields `char` instead of one-character `str`.
- Character ranges and ordering comparisons.
