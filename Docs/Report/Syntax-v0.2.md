# Neo Astra syntax v0.2 - cleanup and freeze plan

**Status:** Proposal, not normative  
**Date:** 2026-11-08  
**Owners:** principal (decisions), lead (integration), reviewer (error-model and
generic-call RFCs)  
**Normative source today:** [`spec-v0.1.md`](../spec-v0.1.md)  
**Related:** [`syntax-legend.md`](../syntax-legend.md), [`todo.md`](../todo.md),
[`roadmap.md`](../roadmap.md)

This document records the v0.2 syntax cleanup direction and the work needed to
freeze it. It does not change the v0.1 contract. Kilo's native CLI order
continues to target v0.1 until this document is superseded by a frozen
`spec-v0.2.md`.

## 1. Goal

Reduce the v0.1 surface before adding features. The v0.1 review found that the
language spends too much complexity on duplicate spellings, overloaded symbols,
and lexical shortcuts. The fix order is:

1. remove or merge duplicate syntax;
2. make ambiguous grammar explicit;
3. freeze the result;
4. implement parser/checker/codegen and migration together;
5. only then add missing features.

Do not add bitwise operators, ranges, lambdas, or other new surface while `!`,
generics, scope syntax, or separators are still ambiguous.

## 2. Decisions already taken

- **Keep the effect-based error model.** A function may declare
  `-> T ! E1 | E2`. Use `?` for propagation. Do not keep postfix `!` as a
  second propagation spelling.
- **Demote `Result<T, E>`.** It may remain as a library/FFI value type and be
  convertible to the effect model, but it is never the idiomatic source-level
  error path.
- **Use `fail Type(...)` for construction.** `fail` is greppable and frees `!`
  entirely. The RFC must decide whether `fail` is an expression or a statement
  and whether `return fail X(...)` is legal.
- **Use turbofish `::<...>` for expression-position type arguments.** Examples:
  `Memory.alloc::<u8>(1024)`, `Channel.create::<str>()`,
  `value.method::<T>()`. `::` exists only for this disambiguation; it is not a
  general path separator.
- **Use `@mode(name) { ... }` and `@memory(name) { ... }` for scoped blocks.**
  Remove `{@mode:name; ...}` and the only mandatory scope-header semicolon.
  Keep `@requires(...)`, `@tags(...)`, and `@export_as(...)` as item
  attributes.
- **Choose one separator rule for lists.** The current recommendation is
  commas for parameters, fields, variants, map entries, arrays, arguments, and
  match arms, with trailing commas allowed; newlines remain statement
  boundaries. If newlines are chosen instead, apply them uniformly to every
  list, not only struct fields.
- **State line continuation explicitly.** A trailing binary operator continues
  the expression; a newline followed by `.` continues a method chain; newlines
  inside `(...)`, `[...]`, and `{...}` are whitespace. Leading binary operators
  are not allowed unless the final rule says otherwise.
- **Remove `/%` and `spawn ||`.** Use `spawn { ... }`.
- **Keep the aliases for adoption, but make them contextual.** The principal
  chose to keep `function`, `var`, `constant`, `typedef`, `record`,
  `mutable`, and `public` as an adoption aid. They must not remain reserved
  keywords, `var` must be fixed or dropped, and a `--strict`/canonical mode
  must reject them. Alias work is last priority.
- **Freeze before implementation.** Do not move parser, checker, codegen, and
  the CLI at the same time.

## 3. Open decisions

| # | Decision | Options | Owner |
|---|---|---|---|
| 1 | `fail` form | expression of type `Never` vs statement only; allow or reject `return fail X(...)` | principal |
| 2 | List separators | commas everywhere vs newlines everywhere | principal |
| 3 | `@memory` form | `@memory(manual) { ... }` vs separate block form | principal |
| 4 | Package version | language spec v0.2 under 1.x vs breaking package 2.0.0 | principal |
| 5 | `char` scope | add `char`/`'a'` or explicitly keep `str`-scalar APIs | principal |
| 6 | Lambda scope | add `fn(x) => expr` or state function types are function-pointer-only | principal |
| 7 | Alias strict mode | name and behaviour of `--strict`/canonical mode | principal + lead |

Rule 10 applies: do not silently choose these in code or prose.

## 4. Reviewer RFCs

The reviewer should draft two focused, non-normative RFCs. Each RFC must include
EBNF, precedence, positive examples, negative examples, and unresolved
questions.

### RFC A - error model

- `-> T ! E` return type
- `fail` construction and its type/position
- postfix `?` propagation and its precedence
- `Ok`/`Err` handling
- `Never` and unreachable positions
- interaction with `return`, `if`, `match`, blocks, and `?`
- migration from `!Type(...)` and `expr!`
- comparison with `Result<T, E>` and its demotion

### RFC B - generic calls

- `::<...>` token and legal positions
- method calls: `value.method::<T>()`
- associated calls: `Memory.alloc::<u8>(...)`
- generic constructors and struct literals
- precedence relative to `.`, `()`, and `[]`
- inference when type arguments are omitted
- negative cases that must remain comparisons
- migration from the v0.1 lookahead heuristic

Ask for **proposals with decision points**, not final normative text.

## 5. Work order

### Phase 1 - grammar fixes (correctness first)

- freeze RFC A and RFC B decisions
- write the v0.2 grammar for `!`, `?`, `fail`, `Ok`/`Err`, and turbofish
- replace scope syntax with `@mode(...) { ... }` / `@memory(...) { ... }`
- settle list separators and line continuation
- fix tuple-index lexing so `t.0.1` does not become `t` + float `0.1`
- decide `default`/`from`/`module`/`type` contextual-keyword policy; at minimum
  contextualize `default` and `from`

### Phase 2 - cheap deletions

- remove `/%`
- remove the `spawn ||` marker
- remove the special `///` documentation meaning unless doc extraction is a
  real feature
- prune the lookalikes table after duplicate syntax is gone

### Phase 3 - aliases

Only if the principal keeps them:

- make aliases contextual, not reserved
- fix or drop `var`
- add `--strict`/canonical mode that rejects aliases
- keep the formatter rewriting aliases to canonical spellings

### Phase 4 - feature gaps

Add in this order, each with its runtime/iterator/slice story defined at the
same time:

1. `if let` / `while let`
2. ranges (`a..b`, `a..=b`) with `for`, iterators, and slicing
3. bitwise `& | ^ << >>`, with `|` in expression position and error-set
   position documented as separate contexts
4. `char` and literals only if `char_at`/scalar APIs remain
5. lambdas (`fn(x) => expr`) only if iterators/callbacks need them; otherwise
   state that function types are function-pointer-only and remove closure
   language
6. complete integer/float suffix table

### Phase 5 - migration and validation

- parser diagnostics for every removed spelling, with the canonical replacement
- formatter/codemod rewrites for aliases, `!` forms, scope blocks, and
  turbofish
- one conformance case per changed construct, including negative parses
- update the error-code ledger for new syntax diagnostics
- write 10-20 realistic programs in the new syntax before freezing:
  CLI tool, parser, concurrent worker, FFI binding, web program, library with
  re-exports, manual-memory system program, ranges/slicing program,
  lambda/callback program, and one deliberately ambiguous program per rule
- run the corpus through parser prototypes and the formatter; do not review
  v0.2 only as prose
- freeze `spec-v0.2.md`, then update `syntax-legend.md`, parser, checker,
  formatter, codegen, conformance, and examples together

## 6. Migration map (proposal)

| v0.1 | v0.2 |
|---|---|
| `function` | `fn` |
| `var` | `let` (or `let mut`; `var` removed or documented) |
| `constant` | `const` |
| `typedef` | `type` |
| `record` | `struct` |
| `mutable` | `mut` |
| `public` | `pub` |
| `!Error(...)` | `fail Error(...)` |
| `expr!` | `expr?` |
| `{@mode:system; ...}` | `@mode(system) { ... }` |
| `{@memory:manual; ...}` | `@memory(manual) { ... }` |
| `Memory.alloc<u8>(n)` | `Memory.alloc::<u8>(n)` |
| `spawn || { ... }` | `spawn { ... }` |
| `a /% b` | remove; use `/` or an explicit API |
| `/// docs` | `//` unless doc tooling is added |

## 7. Version and release note

Changing syntax is incompatible with existing `.ns` source. The roadmap says
an incompatible change is a major release, but the normative document is still
the v0.1 language draft. The principal must choose whether the package version
is `1.x` (language still pre-1.0) or `2.0.0` (product-level breaking change).
Do not tag or publish v0.2 until that choice is recorded.

## 8. What this means for the native CLI

Kilo's slices 3-4 continue against `spec-v0.1.md`. Do not implement v0.2 syntax
inside the CLI order. If v0.2 is frozen before those slices land, the lead will
re-scope the CLI work explicitly; otherwise the CLI will need a separate syntax
update slice after the language freeze.
