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
  `-> T ! E1 | E2`. Under edition 0.2, use `?` for propagation. Postfix `!`
  remains valid only under edition 0.1; 0.2 rejects it with the `?` fix.
- **Demote `Result<T, E>`.** It may remain as a library/FFI value type and be
  convertible to the effect model, but it is never the idiomatic source-level
  error path.
- **Use `fail Type(...)` for construction.** `fail` is an expression of type
  `Never`, so it works in `match` arms and other value positions. Reject
  `return fail X(...)` as redundant. Under edition 0.2, `!` is removed from
  expression positions; it survives only in the return-type effect (`-> T ! E`)
  and in `!=`. Edition 0.1 keeps the old expression spelling.
- **Use turbofish `::<...>` for expression-position type arguments.** Examples:
  `Memory.alloc::<u8>(1024)`, `Channel.create::<str>()`,
  `value.method::<T>()`. `::` exists only for this disambiguation; it is not a
  general path separator. The v0.1 lookahead form remains only under 0.1.
- **Use `@mode(name) { ... }` and `@memory(name) { ... }` for scoped blocks.**
  Under edition 0.2, replace `{@mode:name; ...}` and remove the only mandatory
  scope-header semicolon. Edition 0.1 keeps the old form. Keep
  `@requires(...)`, `@tags(...)`, and `@export_as(...)` as item attributes. The
  block is a capability/memory scope, not a new name scope; items inside are
  still module-level declarations and are exportable, with duplicate names
  reported as normal module duplicates.
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
- **Deprecate `/%` and `spawn ||`; do not remove parser support.** Use `spawn { ... }`
  as the canonical form. Keep `/%` because its overflow semantics differ from
  `/`; canonical docs omit it and strict `canonical-syntax` rejects it.
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
| 4 | Package version | Next minor release, not `2.0.0`. The language edition is recorded as `edition = "0.2"` in `core.toml`; the compiler accepts v0.1 and v0.2 syntax gated by the edition, and the migration command (working name `astra migrate`) performs the rewrite. |
| 5 | `char` | Add `char` and a scalar literal; `\u{...}` already denotes a scalar. |
| 6 | Lambdas | Add `fn(x) => expr`. Define parameter-type inference and capture rules in the RFC. |
| 7 | Strict mode | Deny-by-default `canonical-syntax` lint plus check-only `fmt --canonical`; it errors, not warns. Stdlib CI runs it. |
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

## 5. Implementation plan

### 5.1 Release boundary and leverage

v0.2 is a separate train from `1.0.1`. The `1.0.1` release stays on v0.1 and is
not blocked by v0.2 source work. Spec, RFC, corpus, and parser-generator work
can proceed in parallel; parser/formatter commits wait until Kilo's 3a/3b
handbacks are reviewed, so two people do not move the same front-end files at
once. No v0.2 syntax change is injected into the `1.0.1` release.

The cleanup is mostly a front-end change. The existing AST already has the
shapes v0.2 needs:

| v0.2 spelling | Existing AST |
|---|---|
| `fail Error(...)` / `fail Error { ... }` | `ExprKind::Fail` + `FailArguments` |
| `expr?` | `ExprKind::Propagate` |
| `value.method::<T>()` | `ExprKind::TypeArguments` |
| `Box::<T> { ... }` | `StructInit.path.arguments` |
| `@mode(core) { ... }` / `@memory(manual) { ... }` | `ScopeAnnotation` + `ScopedItems` / `ScopedStatements` |
| canonical aliases | canonical `Keyword` variants and `Visibility` |

Therefore **the cleanup needs no new AST node**. Freeze this AST subset now:
slice 4 can target it without the JS/TS bridge being built twice. The checker,
C codegen, Nova, and the JS/TS codegens keep consuming the same nodes. Feature
gaps that do add AST shapes (ranges, `char`, lambdas) are a later train and must
update every backend explicitly.

### 5.2 Plan at a glance

| Phase | Deliverable | Depends on | Estimate | Gate |
|---|---|---|---:|---|
| 0 | Closed decisions, EBNF prototype, AST-freeze note | principal sign-off | 3-4 d | zero grammar conflicts; no open blocking RFC question |
| 1 | Edition plumbing, default v0.1 | 0 | 3-4 d | all existing v0.1 tests unchanged |
| 2 | Lexer tokens and tuple-index rule | 1 | 3-4 d | lexer matrix green; no float/tuple regression |
| 3 | Parser accepts v0.1 and v0.2 | 1, 2 | 8-10 d | both editions parse; old forms rejected in 0.2 with fixes |
| 4 | Edition-aware formatter and strict lint | 3 | 5-7 d | formatter round-trips both editions; lint catches deprecated forms |
| 5 | Migration command, conformance, corpus | 3, 4 | 6-8 d | migration idempotent; 10-20 programs parse and format |
| 6 | Frozen `spec-v0.2.md` and release notes | 0-5 | 3-4 d | spec, docs, conformance, and all backends agree |
| 7 | Feature gaps | 6 | 15-25 d | separate feature gates, not part of the cleanup freeze |

Cleanup total: **31-41 engineer-days, about 6-8 weeks for one engineer.** The
full v0.2 surface including feature gaps remains about 46-66 days, roughly
9-13 weeks. These estimates assume the AST stays stable for the cleanup.

### 5.3 Phase 0 - freeze the contract

No source changes in this phase.

- Close RFC A **D3**: define what `Err(e)` binds, whether nested destructuring is
  allowed, and how exhaustiveness works.
- Close RFC A **D6**: choose the effect/`Result` bridge names and whether the
  bridge is in v0.2 at all.
- Confirm the named-initializer form `fail TypeName { field: value }`.
- Close RFC A **D4**, **D5**, and **D7** with the current recommendations:
  no error-set polymorphism in v0.2, no `Task.join()` redesign here, subset
  checking only.
- Close RFC B: reject general `::` paths, reject wildcard type arguments for
  now, allow a trailing comma in `::<...>`, decide enum-variant spelling
  (`None::<i32>` recommended), and apply the same turbofish rule to builtins.
- Write `Docs/spec-v0.2.md` as a skeleton with the complete EBNF. It is **not**
  normative yet.
- Add a scratch parser-generator crate under `tools/` (LALRPOP recommended)
  containing the v0.2 expression/type/item grammar. It is a check, not a shipped
  dependency. Require zero conflicts for turbofish, struct literals versus
  blocks, `fail` versus calls, and scope blocks.
- Record the AST-freeze note: cleanup reuses existing nodes; feature gaps add
  nodes later.
- Reserve diagnostic codes in `Docs/error-code-ledger.md`: one code for each
  deprecated or edition-rejected spelling, one for `:: <` whitespace, one for
  tuple-index misuse, and the canonical-syntax lint family.

### 5.4 Phase 1 - edition plumbing (no syntax yet)

- Add `Edition` in `crates/astra-core/src/edition.rs` with `V0_1`, `V0_2`,
  `FromStr`, and display. Default is `V0_1`.
- Add options and preserve the existing entry points:
  - `lex_with_options` / `parse_with_options`
  - `format_source_with_options`
  - `CheckOptions.edition`
- Thread the edition through `astra-compiler-api`:
  - `CompileRequest`, `ProjectCheckRequest`, AST and format requests
  - `#[serde(default)]` so old callers keep v0.1
- Validate `PackageManifest.edition` in `astra-manifest`: accept `"0.1"` and
  `"0.2"`; missing means `"0.1"`. Reject unknown values with a migration hint.
- Read the manifest edition in the native CLI and pass it to check, fmt, ast,
  explain, and build. Do not change syntax behavior yet.
- Add tests proving v0.1 output is byte-identical to today and that explicit
  `0.2` still parses the v0.1 surface at this gate.

### 5.5 Phase 2 - lexer

- Add `Keyword::Fail` and map `fail` to it **only under edition 0.2**. Under 0.1
  it remains an identifier, so existing programs do not break.
- Add `Punctuation::Question` and `Punctuation::DoubleColon`. `::` is only
  legal before `<`; a standalone `::` and `:: <` are diagnostics.
- Fix tuple-index lexing with a lexer-context rule: after a postfix `.`, a digit
  starts an integer-only tuple index; it never starts a float. This makes
  `t.0.1` two indexes while `1.0` remains a float and `.5` remains invalid or a
  range concern for later.
- Keep `///` as an ordinary `//` comment. Do not add doc-comment trivia.
- Do not add `..`, bitwise tokens, `char`, or lambda tokens in this phase.
- Add lexer tests for `?`, `::`, `:: <`, `fail` in both editions, `1.0`,
  `t.0.1`, `t.0.1.2`, and comments.

### 5.6 Phase 3 - parser and checker

Parser work:

- `fail` expression under 0.2: parse `fail Type(args)` and
  `fail Type { field: value }` into the existing `ExprKind::Fail`; reject
  `return fail X(...)` as redundant. Keep `!Type(...)` under 0.1.
- Postfix `?` under 0.2: parse into `ExprKind::Propagate` at the same
  precedence as call, index, and member. Keep postfix `!` under 0.1; under 0.2
  it produces a diagnostic with the `?` fix.
- Turbofish under 0.2: parse `::<...>` in the postfix chain and in struct
  initializers. Keep the v0.1 lookahead heuristic only under 0.1. Reject a
  second turbofish and turbofish without a call/constructor.
- Scope blocks under 0.2: parse `@mode(name) { ... }` and
  `@memory(name) { ... }` at item and statement positions into the existing
  `ScopedItems` / `ScopedStatements`. Keep `{@mode:name; ...}` under 0.1.
  The new form has no mandatory semicolon.
- Separators and continuation: enforce commas in every list, allow trailing
  commas, allow a block-bodied match arm to omit the comma, continue after a
  trailing binary operator, continue before `.`, and reject a leading binary
  operator. Keep newlines as statement boundaries in blocks.
- Reject a float token after `.` as a tuple index with a targeted hint.

Checker work:

- Add only the semantic checks the closed RFCs require. Expected v0.2 changes
  are narrow: `return fail` rejection, the chosen `Err` pattern rules, and the
  D7 subset-checking rule. `fail` already types as `Never`; `?` already merges
  effects; turbofish already resolves explicit type arguments.
- Do not change C/Nova/JS/TS codegen for cleanup constructs. Add an AST
  serialization contract test so an accidental node change is caught.

### 5.7 Phase 4 - formatter and canonical lint

- Make the formatter edition-aware:
  - v0.1 output uses only v0.1-valid spellings; aliases may still canonicalize.
  - v0.2 output uses `fail`, `?`, `::<...>`, and `@mode(...)` / `@memory(...)`.
  - `/%` is preserved as `/%` under both editions; never rewrite it to `/`.
  - `spawn ||` formats as `spawn`.
- Keep comment attachment, idempotence, and blank-line behavior intact. Add
  round-trip tests for both editions and for `format(parse(x)) == x` on
  canonical sources.
- Add `canonical-syntax`: a source/token-aware pass that reports deprecated
  aliases, `/%`, `spawn ||`, and v0.1 spellings when the edition is 0.2. It is a
  deny lint, not a warning. It must not rewrite by itself.
- CLI decision: use `astra check --canonical-syntax` for enforcement and
  `astra fmt --check` for formatting. Keep `fmt --canonical` as a check-only
  mode for v0.2 canonical source: it errors on deprecated spellings and never
  rewrites. `astra migrate` is the only rewrite path.
- Run the canonical lint in stdlib CI once the stdlib is migrated.

### 5.8 Phase 5 - migration and validation

- Implement the migration command (working name `astra migrate`; the docs'
  `ns fix` is a placeholder). It:
  - rewrites aliases to canonical spellings;
  - rewrites `!Error(...)` / `!Error { ... }` only in expression position to
    `fail`, never touching `-> T ! E` or `!=`;
  - rewrites postfix `!` to `?`;
  - rewrites old scope blocks to `@mode(...)` / `@memory(...)`;
  - rewrites confident `path<T>(` uses to `path::<T>(`; ambiguous matches are
    reported, not guessed;
  - rewrites `spawn ||` to `spawn`;
  - leaves `/%` unchanged with a warning, because `/` changes overflow
    semantics;
  - changes `edition` only with an explicit flag.
- Add `--dry-run`, `--check`, and `--json`; make the command idempotent.
- Add conformance cases (lead-owned) for every changed construct and every
  negative parse. Add tuple-index, separator, and continuation regressions.
- Write the 10-20 realistic programs listed in the old plan: CLI tool, parser,
  concurrent worker, FFI binding, web program, library with re-exports,
  manual-memory system program, ranges/slicing program, lambda/callback program,
  and one deliberately ambiguous program per rule. Ranges/lambdas programs are
  allowed to be feature-gated; the rest must parse today.
- Run the corpus through both the parser and the formatter, and through the
  parser-generator prototype. Do not freeze from prose alone.
- Run the full gate: `cargo fmt --all --check`, workspace clippy,
  `cargo test --workspace`, `npm test`, native parity, and the canonical lint.

### 5.9 Phase 6 - freeze and release

- Promote `Docs/spec-v0.2.md` to normative only after Phases 0-5 are green.
- Update `syntax-legend.md`, `spec-v0.1.md`'s pointer, `todo.md`, `roadmap.md`,
  `priorities.md`, `state.md`, and the error-code ledger in the same change.
- Update examples and templates to emit `edition = "0.2"`; existing projects
  stay on `"0.1"` until migrated.
- Freeze the v0.2 grammar and AST subset. Later feature work must add a new
  edition gate or a clearly additive release; it must not silently change the
  frozen grammar.
- Do not tag or publish v0.2 until the edition field, parser diagnostics,
  migration path, conformance, and all three backends are green.

### 5.10 Phase 7 - feature gaps (separate train)

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

These features add AST nodes and backend work. They are **not** part of the
v0.2 cleanup freeze and must not delay the cleanup release.

### 5.11 Decisions required before Phase 1

These are recommendations, not taken decisions:

| Question | Recommendation |
|---|---|
| Edition values and default | Accept only `"0.1"` and `"0.2"`; missing means `"0.1"`; new templates use `"0.2"` after the release. Reject `"2026"` with a hint to `"0.1"`. |
| Formatter behavior | Edition-aware: v0.1 input formats to v0.1-valid spellings; v0.2 input formats to v0.2 canonical spellings. |
| Strict enforcement spelling | `astra check --canonical-syntax` errors; `astra fmt --check` only checks formatting; `astra fmt --canonical` is check-only for v0.2 canonical source. `astra migrate` is the only rewrite path. |
| Migration command | Implement `astra migrate` (not `ns fix`); keep `ns fix` only as a documentation alias if the principal wants one. |
| Feature release boundary | Ship `0.2.0` as cleanup only; ship feature gaps in a later minor so the grammar freezes first. |

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
- The compiler accepts both editions; `astra migrate` rewrites v0.1 source to
  v0.2.
- The exact next minor number is scheduled when the release line is chosen.
  The plan assumes `0.2.0` is the cleanup release.

Do not tag or publish v0.2 until the edition field, parser diagnostics, and
migration path are implemented and green.

## 8. What this means for the native CLI

Kilo's 3a/3b slices continue against `spec-v0.1.md`; do not interrupt them with
v0.2 source changes. Spec, RFC, corpus, and parser-generator work may proceed in
parallel. Parser/formatter commits wait for the 3a/3b handbacks.

Slice 4 should target the frozen cleanup AST. That is safe because v0.2 cleanup
reuses the existing AST nodes (section 5.1); the JS/TS bridge does not need to be
built twice for `fail`, `?`, turbofish, or scope spelling. Features that add AST
nodes are a later train and will each require an explicit bridge update.

If v0.2 is frozen before the remaining native slices land, the lead will
re-scope the CLI work explicitly. No v0.2 syntax is shipped inside `1.0.1`.
