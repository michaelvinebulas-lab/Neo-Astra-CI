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

1. deprecate or merge duplicate syntax without paying for a full removal;
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
- **Use `fail Type(...)` for construction.** `fail` is an expression of type
  `Never`, so it works in `match` arms and other value positions. Reject
  `return fail X(...)` as redundant. `!` is removed from expression positions;
  it survives only in the return-type effect (`-> T ! E`) and in `!=`.
- **Use turbofish `::<...>` for expression-position type arguments.** Examples:
  `Memory.alloc::<u8>(1024)`, `Channel.create::<str>()`,
  `value.method::<T>()`. `::` exists only for this disambiguation; it is not a
  general path separator.
- **Use `@mode(name) { ... }` and `@memory(name) { ... }` for scoped blocks.**
  Remove `{@mode:name; ...}` and the only mandatory scope-header semicolon.
  Keep `@requires(...)`, `@tags(...)`, and `@export_as(...)` as item
  attributes. The block is a capability/memory scope, not a new name scope;
  items inside are still module-level declarations and are exportable, with
  duplicate names reported as normal module duplicates.
- **Use commas everywhere for lists.** Parameters, fields, variants, map
  entries, arrays, arguments, and match arms use commas; trailing commas are
  allowed. A match arm whose body is a block may omit the comma. Newlines
  remain statement boundaries.
- **State line continuation by delimiter kind.**
  - Inside `(...)` and `[...]`, newlines are whitespace.
  - Inside a block `{ ... }`, newlines separate statements; `;` is still a
    soft same-line separator.
  - Inside struct or map literals, newlines follow the list-separator rule;
    they are not statement boundaries.
  - A trailing binary operator continues the expression.
  - A newline followed by `.` continues a method chain.
  - A leading binary operator is not allowed.
  - These rules must be written for each brace kind; `{}` is overloaded.
- **Remove `/%` and `spawn ||`.** Use `spawn { ... }`.
- **Deprecate the aliases; do not spend a slice removing them.** All seven
  aliases remain accepted by the parser for compatibility. Canonical docs omit
  them, the formatter writes the canonical spelling, and the deny-by-default
  `canonical-syntax` lint rejects them in strict mode. This is deprecation, not
  removal: no parser/checker/codegen deletion work. Making the words
  contextual so they can also be identifiers is a separate optional change;
  it is not required for deprecation.
- **Freeze before implementation.** Do not move parser, checker, codegen, and
  the CLI at the same time.

## 3. Decisions taken

| # | Decision | Result |
|---|---|---|
| 1 | `fail` form | Expression of type `Never`; `return fail X(...)` is rejected as redundant. |
| 2 | List separators | Commas everywhere, trailing commas allowed; block-bodied match arms may omit the comma. |
| 3 | `@memory` form | `@memory(manual) { ... }`, matching `@mode`. |
| 4 | Package version | Next minor release, not `2.0.0`. The language edition is recorded as `edition = "0.2"` in `core.toml`; the compiler accepts v0.1 and v0.2 syntax gated by the edition, and `ns fix` performs the migration. |
| 5 | `char` | Add `char` and a scalar literal; `\u{...}` already denotes a scalar. |
| 6 | Lambdas | Add `fn(x) => expr`. Define parameter-type inference and capture rules in the RFC. |
| 7 | Strict mode | Deny-by-default `canonical-syntax` lint plus `fmt --canonical`; it errors, not warns. Stdlib CI runs it. |
| 8 | `@mode` item visibility | Items inside are module-level declarations and remain exportable; the block scopes only capabilities/memory, not names. Duplicate names are normal module duplicates (`NS3011`). This matches the existing `scoped_items_are_resolved_at_module_scope` behavior. |
| 9 | Aliases | Deprecated, not removed: parser accepts them, canonical docs/formatter omit them, and `canonical-syntax` rejects them in strict mode. Contextualization is optional. |

There are no remaining decisions in this table. Future RFC questions are listed
in the individual RFC files.

## 4. Reviewer RFCs

The reviewer drafts are recorded as
[`RFC-A-error-model.md`](RFC-A-error-model.md) and
[`RFC-B-generics.md`](RFC-B-generics.md). They are non-normative and include
lead review notes. Each RFC has EBNF, precedence, positive examples, negative
examples, and unresolved questions.

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
- fix tuple-index lexing with a concrete rule: after `.` in postfix position,
  read an integer-only token and never a float. This makes `t.0.1` two tuple
  indexes and keeps leading-dot continuation separate from `..` ranges and `.5`.
- decide `default`/`from`/`module`/`type` contextual-keyword policy; at minimum
  contextualize `default` and `from`

### Phase 2 - deprecate, do not remove

- mark `/%` deprecated; keep parsing it, omit it from canonical docs, and do not
  rewrite it to `/` because the overflow semantics differ
- mark `spawn ||` deprecated; accept it and have the formatter emit `spawn`
- stop giving `///` any special meaning; keep it as a normal `//` comment
- prune the lookalikes table to canonical forms only

### Phase 3 - aliases

Deprecate all aliases; do not remove parser support:

- keep `function`, `var`, `constant`, `typedef`, `record`, `mutable`, and
  `public` accepted for compatibility
- omit them from canonical docs and examples
- keep the formatter rewriting aliases to canonical spellings
- add the deny-by-default `canonical-syntax` lint and `fmt --canonical`
- run the canonical lint in stdlib CI
- optionally make aliases contextual later so the words can also be identifiers

### Phase 4 - feature gaps

Add in this order, each with its runtime/iterator/slice story defined at the
same time:

1. `if let` / `while let`
2. ranges (`a..b`, `a..=b`) with `for`, iterators, and slicing
3. bitwise `& | ^ << >>`, with `|` in expression position and error-set
   position documented as separate contexts
4. `char` and scalar literals
5. lambdas (`fn(x) => expr`), including parameter-type inference and capture
   rules; `spawn` blocks already capture, so define move/borrow and mutation
   permissions in the same RFC
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
- encode the v0.2 EBNF in a parser generator (tree-sitter, LALRPOP, or pest)
  and require zero grammar conflicts, especially for turbofish, struct
  literals versus blocks, and lambda `=>` versus match `=>`
- run the corpus through the parser prototype and the formatter; do not review
  v0.2 only as prose
- freeze `spec-v0.2.md`, then update `syntax-legend.md`, parser, checker,
  formatter, codegen, conformance, and examples together

## 6. Migration map (proposal)

| v0.1 | v0.2 |
|---|---|
| `function` | `fn` |
| `var` | canonical `let` or `let mut`; deprecated alias accepted |
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
| `spawn || { ... }` | formatter emits `spawn { ... }`; old form accepted |
| `a /% b` | deprecated; keep existing semantics; use `/` or an explicit API |
| `/// docs` | `//` unless doc tooling is added |

## 7. Version and release note

Changing syntax is incompatible with existing `.ns` source, but the language is
still pre-1.0. The package version is the next minor release, not `2.0.0`.
Compatibility is selected per project with `edition = "0.2"` in `core.toml`:

- `edition = "0.1"` keeps the v0.1 syntax.
- `edition = "0.2"` selects the cleaned syntax and its diagnostics.
- The compiler accepts both editions; `ns fix` rewrites v0.1 source to v0.2.
- The exact next minor number is scheduled when the release line is chosen.

Do not tag or publish v0.2 until the edition field, parser diagnostics, and
migration path are implemented and green.

## 8. What this means for the native CLI

Kilo's slices 3-4 continue against `spec-v0.1.md`. Do not implement v0.2 syntax
inside the CLI order. If v0.2 is frozen before those slices land, the lead will
re-scope the CLI work explicitly; otherwise the CLI will need a separate syntax
update slice after the language freeze.
