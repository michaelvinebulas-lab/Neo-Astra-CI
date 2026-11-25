# Neo Astra error output - comparison and rework plan

**Status:** Implemented in the working tree (uncommitted); not normative. Does
not change `spec-v0.1.md`.
**Date:** 2026-11-23
**Owners:** reviewer (diagnosis and slices), lead (integration and `Docs/**`),
principal (decisions in section 11)
**Normative source today:** [`spec-v0.1.md`](../spec-v0.1.md) §14
**Related:** [`Syntax-v0.2.md`](Syntax-v0.2.md),
[`RFC-A-error-model.md`](RFC-A-error-model.md),
[`error-code-ledger.md`](../error-code-ledger.md)

This document records the review of the current diagnostic surface against the
Rust (rustc) standard, and splits the rework into slices that can land without
colliding with the v0.2 Phase 3 parser and checker work. It does not change the
v0.1 language contract. No slice is normative until the lead accepts it.

## 1. Goal

Make the human-readable diagnostic output carry the same information as the
structured one, and make that output comparable to rustc. The structured layer
already meets `spec-v0.1.md` §14; the text layer does not.

The fix order is:

1. render what is already in the diagnostic model (span, hint);
2. add the missing label model (`related`) the spec already defines;
3. fix parser recovery last, after the v0.2 parser work lands.

## 2. Baseline evidence

Measured on `5832ef7` with `target/debug/astra-native.exe` reporting
`astra 1.0.1`, against `rustc 1.100.0`.

Corpus sweep: 25 files in `conformance/rejects`, text and `--json` modes.

| Measure | Result |
|---|---|
| Diagnostics emitted | 37 |
| Diagnostics with a byte span | 37 / 37 |
| Diagnostics with a hint | 37 / 37 |
| Diagnostics that are not `error` | 0 / 37 |
| Diagnostics with a related span | 0 / 37 |

Single-error cases behave as expected. `parse_error.ns` (one missing token)
emits 9 diagnostics: `NS2001` x6, `NS2003` x2, `NS2002`. A hand-written file
with a malformed `struct` body emitted 11 diagnostics.

### 2.1 What the text layer loses

`astra check <file>` calls `eprintln!("{diagnostic}")` (`crates/astra-cli/src/main.rs:1224`)
and `Display` prints only `code: message` (`crates/astra-core/src/diagnostic.rs:70`).
The span and hint are present in the response and discarded.

Project mode uses a second renderer, `print_reported`
(`crates/astra-cli/src/main.rs:614`, called from `build.rs:1107`), which does
print `path:line:col` and the hint. The two renderers disagree, so the same
file reports differently depending on whether the argument is a file or a
directory.

## 3. Comparison with rustc

Same three errors, same order. rustc:

```text
error[E0308]: mismatched types
  --> rust_errors.rs:11:30
   |
11 |     let p = Point { x: 1, y: "two" };
   |                              ^^^^^ expected `i32`, found `&str`

error[E0061]: this function takes 2 arguments but 1 argument was supplied
   |
12 |     let n: i32 = add(1);
   |                  ^^^--- argument #2 of type `i32` is missing
   |
note: function defined here
 6 | fn add(a: i32, b: i32) -> i32 {
   |    ^^^         ------
help: provide the argument
12 |     let n: i32 = add(1, /* i32 */);

error: aborting due to 3 previous errors
```

Neo Astra, single file:

```text
NS4001: Type mismatch: expected 'i32', found 'str'.
NS4004: Function expects 2 argument(s), but 1 were provided.
NS4002: Unknown identifier 'missing_name'.
```

Neo Astra, project:

```text
src/main.ns:11:30: error[NS4001]: Type mismatch: expected 'i32', found 'str'.
  help: Convert the value explicitly; Neo Astra has no implicit coercion.
```

| Capability | rustc | Neo Astra | Note |
|---|---|---|---|
| Stable code | yes | yes | `NS4001` is more granular than `E0308` |
| Byte span in JSON | yes | yes | 37/37 in the corpus |
| Char-based columns | yes | yes | `byte_offset_to_line_col`, `main.rs:790` |
| Deterministic order | yes | yes | source order, module order |
| Source line in output | yes | no | no snippet renderer exists |
| Caret underline | yes | no | no snippet renderer exists |
| Primary plus secondary labels | yes | no | `related` is unimplemented |
| Actionable hint | usually | yes, in data | 37/37, dropped in single-file text mode |
| Machine-applicable suggestion | often | no | hints are prose only |
| Summary line | yes | no | no "aborting due to N errors" |
| Error index lookup | yes | no | `explain NS4001` is read as a file path |
| Color | tty only | no | out of scope for this plan |
| Recovery without cascades | good | poor | 9 diagnostics from one missing token |

Verdict: comparable in structure, not in presentation. The model is close to
rustc; the renderer and the label graph are the gap.

## 4. Non-goals

- No new language surface and no change to any `NS` code meaning.
- No color, no terminal width handling, no Unicode box drawing beyond ASCII.
- No machine-applicable fix (`rustfix` equivalent).
- No change to `Docs/spec-v0.1.md` or `conformance/**`. The TS oracle in
  `orchestrator/packages/cli/**` and the parity harness in
  `orchestrator/tests/native-parity.test.mjs` are updated in lockstep so the
  native and TS output stay byte-identical.
- The LSP gained `relatedInformation` and Nova received one clippy allowance;
  neither changes its language semantics. Section 6 records those follow-ups.

## 5. Slice A - render pipeline

**Goal:** one renderer, used by every CLI path, that shows the source line and
the primary span.

**Files:** `crates/astra-cli/src/main.rs`, `crates/astra-cli/src/build.rs`,
new `crates/astra-cli/src/render.rs`, new `crates/astra-cli/tests/cli_output.rs`.

**Work:**

1. Extract a single `render(diagnostic, source, path) -> String` and delete the
   ad hoc formatting in `cmd_check` and `print_reported`.
2. Render the source line with an ASCII caret underline covering `span.start`
   through `span.end`, clamped to the line.
3. Print the hint as a labelled `help:` line under the snippet, matching the
   existing wording.
4. Print a trailing summary: `error: N diagnostic(s) emitted` (wording open,
   decision D2).
5. Route every failing path through the renderer: `check` file, `check` dir,
   `build`, `doctor`, `ast`, `explain`, and the manifest errors that currently
   print bare `code: message` (`main.rs:589`, `main.rs:1075`, `main.rs:1099`).
6. Drop the `Manifest:` / `Format:` / `Package:` header from failing runs, or
   move it behind `--json` (decision D3).
7. Make `explain NS4001` look up the code ledger row instead of treating the
   argument as a path.

**Done when:** `astra check <file>` and `astra check <dir>` print byte-identical
diagnostic blocks for the same file, each with a caret line and a hint line, and
the summary count matches the number of emitted diagnostics.

**Conflict profile:** low. Phase 3 does not touch `astra-cli`. `main.rs` was
last touched by the Phase 1-2 edition work, which is committed.

## 6. Slice B - diagnostic label model

**Goal:** let a diagnostic point at more than one location, and carry the file
inside the diagnostic, as `spec-v0.1.md` §14.2 already requires.

**Files:** `crates/astra-core/src/diagnostic.rs`,
`crates/astra-compiler-api/src/lib.rs`, `crates/astra-cli/src/main.rs`,
`crates/astra-cli/src/build.rs`, `crates/astra-core/src/checker.rs`,
`crates/astra-core/src/type_surface.rs`.

**Work:**

1. Add `related: Vec<RelatedSpan>` to the wire diagnostic, where each entry has
   `file`, `span`, and `message`. Add builder methods so existing construction
   sites keep compiling unchanged.
2. Add `file` to the per-diagnostic JSON object, sourced from the owning module.
3. Populate `related` for the high-value cases first: `NS4001` (declaration
   site), `NS4004` (declaration site), `NS4010` (missing match arm), `NS4011`
   (declared error set), `NS3010` (leaking declaration).
4. Render `related` as rustc-style secondary labels and `note:` lines in slice A
   renderer.

**Done when:** the four populated codes emit at least one `related` entry in
`--json`, the renderer shows them, and the existing 37-diagnostic corpus still
reports every code in its `.codes` file.

**Conflict profile:** high. `checker.rs` carries 119 `NS` code sites and is a
Phase 3 file. Populating `related` means editing those exact sites. Defer until
Phase 3 is committed.

**Governance:** this closes a gap against `spec-v0.1.md` §14.2, so it needs no
spec change. Any new code needed for slice B must be reserved in
`Docs/error-code-ledger.md` and checked against the `NS2008`-`NS2019` range that
`Syntax-v0.2.md` §5.3 reserves for v0.2 front-end diagnostics.

## 7. Slice C - parser recovery and cascade control

**Goal:** one primary error per mistake, with a bounded cascade.

**Files:** `crates/astra-core/src/parser.rs`, `crates/astra-core/src/lexer.rs`,
`crates/astra-cli/src/main.rs`.

**Work:**

1. Suppress diagnostics that are pure consequences of an earlier error in the
   same statement or item, or cap emissions per item.
2. Add a recovery anchor so one missing token does not desynchronise the rest of
   the file.
3. Keep the `NS2002` depth guard behaviour.
4. Pin the new behaviour with golden outputs for `parse_error.ns` and the
   malformed-struct case.

**Done when:** `parse_error.ns` emits at most 2 diagnostics and every code in its
`.codes` file still appears.

**Conflict profile:** hard. `parser.rs` is the Phase 3 core file, and recovery
behaviour interacts with `fail`, `?`, turbofish, and the new scope blocks. Defer
until Phase 3 and Phase 4 land.

## 8. Ordering and gates

| Order | Slice | Gate |
|---|---|---|
| 1 | A | **landed** - one renderer, snippet, summary |
| 2 | B | **landed** - `related` labels, protocol, renderer, LSP |
| 3 | C | **landed** - bounded parser cascade |

### 8.1 Slice A status

Landed in the working tree (uncommitted). Every CLI path renders through
`crates/astra-cli/src/render.rs`; the TS oracle in
`orchestrator/packages/cli/src/index.ts` mirrors it so native and TS stderr stay
byte-identical. `ReportedDiagnostic` carries span and snippet with
`#[serde(skip)]`; `related` is serialized only when non-empty, so old JSON
consumers keep their previous shape while diagnostics with secondary labels
gain one additive field. D3 (header) and D5 (`explain NSxxxx`) are implemented.

Verified on `297eddc`:

- `cargo fmt --all --check` pass.
- `cargo clippy -p astra-cli --all-targets --no-deps -- -D warnings` exit 0.
- `cargo test --workspace` pass; `cargo test -p astra-cli` 8/8.
- `npm test` 243/243.
- `npm run test:native-parity` 12/12.

Known limitation: `build` on a multi-module request cannot attribute a flat
diagnostic to its owning module, so those diagnostics render without a snippet
(`compiler_diagnostic_unattributed`).

### 8.2 Slice B status

Landed. `Diagnostic.related` carries `RelatedSpan { file, span, message }`
(empty `file` means the primary file), skipped in JSON when empty. Populated
codes: `NS4001` (argument type mismatch), `NS4004` (arity), `NS4010`
(non-exhaustive match), `NS4011` (unhandled error), `NS3010` (private leak).
The renderer prints `note:` lines, the TS oracle mirrors them, and the LSP emits
`relatedInformation` for same-file spans in both single-buffer and project mode.

### 8.3 Slice C status

Landed. `dedupe_cascades` in `parser.rs` keeps the first parser diagnostic per
`(code, line)`, so one malformed construct no longer emits a run of identical
errors. `conformance/rejects/parse_error.ns` drops from 9 diagnostics to 3.
Non-`NS2xxx` codes and distinct codes on the same line are preserved, so
conformance subset checks still hold.

## 9. Acceptance

- `astra check <file>` output contains a caret line and a hint line for every
  diagnostic with a span.
- File and directory modes produce identical diagnostic blocks for the same
  source.
- Exit codes unchanged: 1 on any error diagnostic, 0 otherwise.
- Every code emitted still has a row in `spec-v0.1.md` §14 and a row in
  `Docs/error-code-ledger.md`.
- No existing conformance `.codes` file loses a code.

## 10. Verification

```powershell
cargo fmt --all --check
cargo clippy -p astra-cli --all-targets -- -D warnings
cargo test --workspace
cargo test -p astra-core --test conformance_rejects
npm test
npm run verify
```

Plus a manual sweep of `conformance/rejects` in both text and `--json` mode.

## 11. Decisions

1. **D1 (resolved):** summary counts errors only; wording is
   `error: aborting due to previous error` / `... N previous errors`.
2. **D2 (resolved):** rustc-style gutter with `|`, a `-->` location line, and
   `= help:`.
3. **D3 (resolved):** header prints only on success; `Format:` is dropped.
4. **D4 (resolved):** `related` ships with the slice B change.
5. **D5 (resolved):** `explain NSxxxx` is recognised by
   `^(NS|NP|NR|NV|PG)\d{4}$` and reports that explanations are not bundled; the
   catalog is generated by `scripts/check-error-codes.mjs --emit` and reserved.
6. **D6 (resolved):** the LSP emits `relatedInformation` for same-file spans;
   the hint is still appended to `message`.

## 12. Risks

- **Concurrent work.** The working tree is shared with the Phase 3 agent. Two
  commits landed during the review. Slice A avoids its files, but any slice that
  touches `main.rs` should be rebased immediately before handoff.
- **Ledger drift.** `Docs/error-code-ledger.md` is untracked, so the "keep in
  sync" rule is not enforced by CI. Slice B depends on it being committed.
- **Golden churn.** Slice C changes output for every parse failure. Expect
  conformance expectation updates, which are lead-owned.