# Phase 7 slice 1: `if let` / `while let`

Status: implemented for edition `0.2`. Edition `0.1` is unchanged.

## Syntax

```astra
if let Some(value) = optional { ... } else { ... }
while let Some(value) = incoming() { ... }
```

`if let` is an expression; `while let` is a statement. Both are recognized only
when the active edition is `0.2`. The `let` keyword after `if`/`while` is the
only new spelling; patterns use the existing pattern grammar (`_`, bindings,
literals, tuples, constructors, variants).

## Desugaring (implementation decision)

No new AST nodes are introduced. The parser rewrites the syntax immediately:

- `if let P = v { T } else { E }` becomes
  `match v { P => T, _ => E }`.
  With no `else`, the wildcard arm yields `()`.
  `else if` keeps its existing `ElseBranch::If` form and is rewritten
  recursively, so mixed chains work.
- `while let P = v { B }` becomes
  `while true { match v { P => B, _ => { break } } }`.

Consequences:

- Checker, formatter, migrator, capabilities, LSP, native interpreter, and all
  three backends need no changes: they already consume `match` / `while`.
- Runtime semantics (binding, exhaustiveness, effect handling, C/JS/TS code
  generation) are exactly the existing `match` semantics.
- Type errors come from the existing match checker. A non-exhaustive pattern
  is intentional here because the wildcard arm is always synthesized.

## Edition gate

`Edition::V0_1` parses `if let` / `while let` exactly as before: `let` is not a
valid condition, so the existing `NS2001`/`NS2004` diagnostics apply. The new
desugaring path runs only for `Edition::V0_2`.

## Diagnostics

- Missing `=` after the pattern: `NS2001` (`Expected '='`).
- Invalid pattern: `NS2004`, from `parse_pattern`.
- Type mismatch between branches: existing match arm diagnostics.

## Tests

- Parser unit tests assert the syntactic forms and the desugared shape.
- Edition-`0.2` checker tests compile representative programs and assert no
  diagnostics; edition-`0.1` tests assert the old rejection still holds.

## Deferred

`let else` and `if let` chains with guards are not part of this slice. Ranges
and bitwise operators are separate slices.
