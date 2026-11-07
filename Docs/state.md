# State

Single source of truth for project status, the queue, and decisions.
Everything else is either normative (the spec), operational (the work order),
or frozen history (`Docs/archive/`).

**Last updated:** 2026-11-07
**Phase:** `1.0.0` released; the public docs are reworked and the native CLI/NoPM port is open.
**Master:** private `master` on the native CLI work line; public `17ceaf6` (reworked docs + native phase-1 crates); tag `v1.0.0` targets `9d832e0`.
**Blockers:** rotate the npm publish token that was shared during setup; apply the GitHub rulesets in `Docs/github-trust.md`; finish the native CLI/NoPM slices before swapping the package.

---

## Living documents

| File | Holds |
|---|---|
| `Docs/spec-v0.1.md` | Normative language contract. No status, no queue. |
| `Docs/state.md` | This file: status, queue, decisions, short log. |
| `Docs/roadmap.md` | What is built, what comes next, and the versioning scheme. |
| `Docs/todo.md` | Open tasks only, grouped by version. |
| `Docs/Report/Task.md` | Active work order (lead to Kilo). Pruned each round. |
| `Docs/release-checklist.md` | Steps for a release. |
| `Docs/syntax-legend.md` | Plain-language syntax reference. |
| `CHANGELOG.md` | User-visible changes, one section per release. |
| `Docs/archive/` | Frozen history: old design baseline, proposals, every earlier status doc. Read-only. |

## Implemented (verified by the gate and Tier-1)

- **Frontend:** lexer, parser (nesting limit 64, `NS2002`), canonical formatter
  with comment preservation, resolver/visibility, type checker, effect and
  error-union checking, structured diagnostics (`NSxxxx`).
- **Modes:** `core`, `app`, `web`, `system` with least-privileged inference;
  native-order side-effect rejection (`NS5007`).
- **Backends:** C17 (managed GC runtime, generics + monomorphization, native
  FFI, `@script:c`, `@export_as` author-named exports, multi-module linking),
  JavaScript ESM, TypeScript ESM, and the Nova interpreter (typed HIR).
- **Memory:** managed arena + precise root frames + mark-sweep collector with
  `ASTRA_GC_STRESS=1`, boundary pinning, manual alloc/free, `Region`, C ABI.
- **Concurrency:** OS-thread spawn bound to structured scopes, per-task heaps,
  cooperative cancellation, bounded channels with a deadlock watchdog, a
  thread-safe process-lived shared heap for channel handles and messages,
  structured `TaskError` results at `join`, and `NS4019` rejection of task
  handles at every copy boundary.
- **Toolchain:** `astra` CLI (check, fmt, build, doctor, ast, explain), NoPM
  (lock, tree, install, search, audit, why, update), manifest formatting,
  registry adapters, toolchain policy, the resolution ladder, the lockfile
  record, and verified pinned downloads (Zig 0.13.0, `zig cc`/`zig ar`,
  `--no-download`). `astra check` validates both manifests, runs the frontend over the
  project entry, and validates `ffi` mappings (`NS7007`). `nova` provides the
  development `run`, `eval`, `watch`, `repl`, and `test` commands.
- **Compiler API:** JSON-RPC/native + raw Wasm: `check`, `check_project`,
  `compile`, `compile_project`, `format`, `ast`, `explain`.
- **Quality:** Tier-1 CI on three OSes; frontend and C-emitter fuzz suites
  (arbitrary input never panics; clean input emits without panicking);
  conformance corpus across backends.

## Current work

Open tasks, grouped by version, live in [`todo.md`](todo.md). The `1.0.0`
release is cut. Public master is `17ceaf6`: the reworked launch docs plus the
phase-1 native `astra-cli` and `astra-lsp` crates. Tag `v1.0.0` remains
`9d832e0`; its release run `42278798124` is green on Ubuntu, macOS, and Windows
and published the three platform npm packages plus the `neo-astra` meta
package. A clean npm install was verified on Windows: `astra`, `nopm`, `nova`,
and `neo-astra` all report `1.0.0`.

The native CLI order is split into four reviewed slices: core project commands,
C/native build and doctor, native NoPM, and the JS/TS bridge plus CI/packaging.
The JavaScript and TypeScript code generators stay in their current TypeScript
implementation; the native bridge may use Node only for JS/TS builds and
manifest-driven JS/TS package work. Baseline `nopm` must run without Node/npm.

The public source is generated from the sanitized tree: no comments, tests,
docs, or examples; generated C has no comments; JavaScript and TypeScript
backend output no longer carries generator comments. The generator now runs
`cargo fmt` on the mirror, so the public tree is fmt-clean as well as
clippy-clean. `nova run` without a file reads `[nova].entry` from the optional
build manifest and defaults to `src/main.ns`; the Windows NoPM npm adapter
executes `npm.cmd` through `cmd.exe` instead of failing with `EINVAL`.

**Release:** the `1.0.0` source, tag, GitHub assets, and npm packages are
published. Future sanitized public source/tag pushes trigger the public release
workflow. All package versions move in lockstep: new mode/backend/language =
minor, fixes = patch, incompatible rework = major.

## Decisions (index)

Detailed rationale and rejected options: `Docs/archive/architecture.md` (D1-D25),
`Docs/archive/Design/pending-decisions.md` (P-series and the 2026-11-01
addendum), and the proposal files in `Docs/archive/Design/`.

| ID | Date | Decision |
|---|---|---|
| D26 | 2026-10-31 | Two manifests: required `core.*` (package, dependencies, packageSources, registry, capabilities, ffi) and optional `build.*` (build, targets, nova, math, strings, memory, toolchain). Wrong-file keys rejected (`NS9002`). `core.lock.json` stays. Clean break, no dual-read. |
| D27 | 2026-10-31 | Call arguments are strictly left-to-right. Binary operands, array elements, struct fields, and map entries are target-native (no portability guarantee). |
| P1 | 2026-10-31 | System-mode side-effect rejection `NS5007` for two or more state-changing calls in target-native positions; call arguments exempt (D27). |
| P2 | 2026-10-31 | `@export_as("symbol", T = type, ...)`: repeatable, exported generics only, verbatim C symbols, `NS7010`-`NS7013`. |
| P3 | 2026-10-31 | Transitive `NS6002` worklist replay to a fixpoint. |
| FFI-001 | 2026-10-31 | Explicit instantiation exports via attributes (spec first). |
| `@script:c` | 2026-10-31 | §5.1 boundary slice: enter/root/call/exit at managed-argument boundaries. |
| GC-001 | 2026-10-30/31 | Phases 0/A/B/C landed; collection default at 1 MiB safepoints; pinning for library returns; Phase D (Wasm) deferred. |
| CONC-001 | 2026-11-01 | A2 OS thread per spawn bound to the structured scope (A1 eager fallback for Web/JS/Nova); B3 parked channel ops + 5 s watchdog (`ASTRA_CHANNEL_DEADLOCK_TIMEOUT`); C2 per-task heaps + a thread-safe process-lived shared heap for channel handles/messages, interned strings, and static descriptors; D1 cooperative cancellation at GC safepoints and channel ops. Implementation landed in V1. |
| `high` mode | 2026-11-01 | New mode: `app` capabilities + ban list (`Memory`/`@memory:manual`, `Ptr`, `Region`, `ffi`, `@script`, C-ABI/`@export_as`) + full sugar (top-level statements, implicit main/exit 0, `print`, tail return, `fn main` mixing error, optional return-type inference); profile via manifest `defaultMode`; importable as app. Post-1.0. |
| Languages | 2026-11-01 | Extend the C/Wasm/JS/TS pattern (targets, interop, toolchain rungs) to Rust, Go, Lua, Ruby, Java. Per-language scope decided on pickup; V2+. |
| Versioning | 2026-11-01 | SemVer, lockstep, cut `1.0.0` at V1 end. |
| D28 | 2026-11-03 | **Nothing half-made lands.** A slice is complete only when it implements every normative line it touches end to end, tests the success path and each failure mode (ownership and lifetime, cancellation, cross-heap copies, error paths), and contains no stub, silent fallback, or approximation. A known gap is either closed in the slice or recorded as an explicit principal decision before landing; "later" is not a deferral. `1.0.0` requires the 1.0 queue empty and zero known gaps in the shipped surface. |
| TaskError | 2026-11-05 | `join() -> T`. A child panic or cancellation before a value raises a structured `TaskError` in the joining task; a child that returns `Result<T, E>` keeps that `Result` as the task value (no nested `Result`). `task.canceled() -> bool` reports whether cancellation was accepted while running, without joining. A cancellation after completion, or after the task died, is a no-op and is not recorded. A produced value wins the cancellation race; a recorded panic wins over cancellation at join. The canonical message is `TaskError: <detail>`, and nested propagation does not duplicate the prefix. |
| Toolchain download | 2026-11-05 | Versioned C toolchain pins download official upstream archives only. A per-OS/arch table pins the URL, byte size, and SHA-256; the existing transport fetches it, the system `tar` unpacks it, and a project-local cache plus a lockfile record make it reproducible. Nothing elevates. Zig `0.13.0` is pinned on Linux/macOS/Windows for x64/arm64 and drives `zig cc` plus `zig ar`. `--no-download` disables the fetch; every failure names the archive and prints manual-install instructions. |
| D28 audit | 2026-11-05 | Build every gap in the shipped surface before 1.0. Do not reclassify the JavaScript backend gaps or the private scaffolds as post-1.0. The non-shipped LSP and direct Wasm/LLVM scaffold packages are removed; V1 machine code stays on the C toolchain. |
| FFI scope | 2026-11-05 | V1 FFI ships on the native C target. The JS/TS backend rejects a foreign module with `NS8001` and names the native target; this is the recorded stable unsupported path, not a silent fallback. The spec's v0.1 slice defines FFI for later target backends (§1, §3.6, §11). |
| CI remote | 2026-11-05 | Tier-1 runs on `luminousvalentine-cell/Neo-Astra-CI` (remote `ci`). The previous `voidgod79-debug/Neo-Astra-CI` mirror cannot start jobs because of its Actions billing/spending limit. `origin` remains the source mirror at `1199998` and is never pushed. |
| Distribution | 2026-11-06, revised 2026-11-07 | 1.0.0 shipped the Bun-compiled `astra` executable plus the `nopm` launcher and the stripped Rust compiler/Nova binaries. The next package swaps both commands to the native Rust `astra` and `nopm` built by Cargo. The JavaScript and TypeScript code generators remain TypeScript; the isolated bridge may use Node on demand for JS/TS builds and for NoPM manifests that explicitly need JS/TS package work. Baseline CLI and NoPM flows must not require Node or npm. |

**Open questions:** the final shared-task-handle design. The safety rule is
conservative: `NS4019` rejects a task handle in a value that would cross a task
boundary by copy (channel elements, spawn captures, and task results). A future
single-owner shared-handle design that allows such a value is a separate
principal decision, not an implicit part of this guard. Everything else is
governed by D28 - a gap is not allowed to exist just because it has a task.

## D28 pre-1.0 audit (2026-11-05)

| Surface | Result | Evidence |
|---|---|---|
| Frontend, checker, formatter | No stub; diagnostics are frozen by `scripts/check-error-codes.mjs` | `cargo test -p astra-core`; gate |
| C backend | Managed/manual memory, GC, FFI, inline C/C++, tasks, channels, map index, `@export_as` | `cargo test -p astra-codegen-c`; conformance |
| JavaScript/TypeScript | ESM/TS output, recoverable failures, tasks/`TaskError`, map index, expression-block control flow, `abort`; FFI and non-JS/TS scripts reject explicitly | `orchestrator/tests/codegen-js.test.mjs` and `codegen-ts.test.mjs`; conformance |
| Nova | Typed interpreter, `TaskError`, map index, fallible operations | `cargo test -p astra-nova`; stress suite |
| CLI and toolchain | `check`, `fmt`, `build`, `doctor`, `ast`, `explain`; pinned download/verify/unpack; `--no-download`; lockfile record | `toolchain.test.mjs`; CLI tests |
| Packages and registry | Manifest editing, resolver/store, adapters, registry gateway | `nopm.test.mjs`, `registry.test.mjs` |
| Private scaffolds | Removed the LSP stub and direct Wasm/LLVM backend crates; V1 machine code uses the C toolchain | workspace manifests; this audit |
| Deferred, not shipped surface | Language server and plugin host, Wasm execution/Web Worker codegen, browser playground UI, `high` mode, extra language targets, thread pool, GC Wasm phase | `README.md`, `roadmap.md`, `todo.md` |

No `TODO`/`FIXME`/`unimplemented!`/`todo!` remains in the shipped Rust or
TypeScript code. The remaining "not implemented" strings are runtime shape
guards for unsupported receivers and internal "type checking should have
rejected this" invariant errors, not unfinished features.

## Log (recent, pruned)

| Date | Change | Evidence |
|---|---|---|
| 2026-11-07 | Native LSP phase 3: added document symbols for declarations and struct fields, signature help with active-parameter tracking for local/imported/namespace calls, project-file import-path completion with precise edits, project-wide references, document highlights, query-filtered workspace symbols, and safe symbol rename. Rename updates declarations, imports (including aliased imports without touching local aliases), and call sites; field rename is refused until constructor/pattern edits are modeled. The server advertises the new capabilities and keeps buffer/project snapshots authoritative. | `af100bb` + `b59d634` + `c64d2a6` + `f0d4e0b`; `cargo test --workspace` green; `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo test -p astra-lsp` 40 green; `npm test` 241 green; manual stdio smoke returned symbol, signature, import-completion, reference, highlight, workspace-symbol, and rename results |
| 2026-11-07 | Native LSP phase 2: project snapshots load and order the `.ns` module graph, overlay open buffers, publish diagnostics for every project module, and preserve standalone checking when another module fails. Added project-wide hover, go-to-definition for locals/imports/namespaces/struct fields, and keyword/type/symbol/member completion. | `73a3d34`; `cargo test -p astra-lsp` 28 green; `cargo clippy -p astra-lsp --all-targets -- -D warnings` clean; manual stdio smoke returned hover, imported definition, and completion results |
| 2026-11-07 | Public docs reworked and published at `17ceaf6` with the phase-1 native `astra-cli`/`astra-lsp` crates. The mirror generator strips comments and private tests, runs `cargo fmt`, and is clippy-clean. Release workflow now has provenance/attestation permissions. Native CLI/NoPM work is split into four reviewed slices; JS/TS codegen stays TypeScript with an on-demand Node bridge. | public `17ceaf6`; sanitized `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`; local `npm test` 241 green; `cargo test -p astra-cli -p astra-lsp` 13 green; parity gate intentionally red at 9 tests |
| 2026-11-07 | `neo-astra@1.0.0` released: public source/tag pushed, all three host archives published, and all four npm packages published. The sanitized tree contains 102 files, one root commit, and no source comments or generated declaration comments. | public `9d832e0`, tag `v1.0.0`; release run `42278798124`; `npm view` reports `1.0.0` for `neo-astra` and all platform packages; clean Windows npm install runs `astra`, `nopm`, and `nova` at `1.0.0` |
| 2026-11-06 | Comment-free release line: C/C++ generated units, host shims, headers, and inline scripts are post-processed to remove comments while preserving literals; JavaScript/TypeScript generator banners and runtime comments are removed; the C header export marker is now a code macro a comment stripper cannot erase. The sanitized public source drops tests, docs, examples, and comments while remaining buildable. `nova run` reads the build manifest and defaults to `src/main.ns`; Windows NoPM runs `npm.cmd` through `cmd.exe`. | local `npm test` 238 green; `cargo test --workspace` green; clippy/fmt green; sanitized `npm ci`, `npm run build`, `cargo check`, and `npm run package` green; packaged `nova run`, native build, and `nopm add npm:is-number` green |
| 2026-11-06 | Standalone 1.0 package: one Bun-compiled `astra` executable with `--minify --bytecode`, `astra nopm` dispatch, and a tiny `nopm` launcher; stripped Rust compiler/Nova binaries; pinned native `tsc` for `@script:ts`; no Node.js/npm/Cargo at runtime; `astra` discovers its compiler beside the executable and the release workflow packages all three hosts. | `cadb04f`; local `npm run ci` green; Tier-1 run 42093449495 green on Ubuntu/Windows, macOS job cancelled before start; prior release tree `e40aee4`, run 42084631705 |
| 2026-11-05 | `1.0.0` release commit: `TaskError` at join, `task.canceled()`, pinned Zig 0.13.0 download/verify/unpack with `zig cc`/`zig ar`, JavaScript map index/expression-block control flow/`abort`, removal of the LSP and Wasm/LLVM scaffolds, and the D28 audit. | `694ff63`; local `npm run ci` green; Tier-1 run 42081733665 could not start (GitHub Actions billing/spending limit) |
| 2026-11-05 | Release tooling: the lockstep bump rewrites internal `@neo-astra/*` dependency ranges, the version gate rejects drift, and the release test covers the change. The clean-install failure on the new CI is fixed; CI moved to `luminousvalentine-cell/Neo-Astra-CI`. | release fix commit; local `npm run ci` and clean `npm ci` green |
| 2026-11-05 | Task handles are rejected at every copy boundary: channel elements, spawn captures, and spawn results report `NS4019`, and the emitter no longer treats `Task` as shared. The error-code audit freezes one meaning per code, splits the overloaded `NS8001`/`NS9008` paths, documents every emitted code, adds `NS7007` FFI mapping validation to `astra check`, and adds the `scripts/check-error-codes.mjs` release gate. `astra check` now runs the frontend and reports source diagnostics, not just manifest validation. | `e52ee12`, run 41996931100 |
| 2026-11-04 | CLI inspection commands: `astra ast` prints the parsed AST and `astra explain` prints mode inference for a file or project entry; adds `--json`, `--mode`, `--target`, and `--compiler`; install/upgrade documentation and README/package status refreshed; release checklist corrected to the source-install model. | `adeedf0`, run 41851239620 |
| 2026-11-03 | `sys:` names are validated before the installed probe, so a command like `node` that exists on `PATH` is still `NP0015` when it is not a supported system tool. Adds the `sys:node` CLI regression. | `5b6b5fc`, run 41846362813 |
| 2026-11-03 | Spawn captures resolve free names through the active lexical scope: capture and republish lookup is truncated at block, loop, conditional, block-expression, and match-arm boundaries, and arm pattern bindings enter the arm scope. Adds block/loop/match shadow and managed block-arm regressions. | `915137d`, run 41686527626 (attempt 2; attempt 1 hit a transient macOS crates.io DNS failure) |
| 2026-11-03 | Channel messages landed: a once-initialized, thread-safe shared heap owns channel handles, queue buffers, and queued copies; `send` deep-copies the element by shape, `receive` copies into the receiver heap and frees the shared subgraph, and channel values are shared handles; adds bytes-after-join, reclamation, shared-heap, stress, and cross-backend conformance tests. `Channel<T>` rejects task handles (`NS4019`). TS match bindings over fallible calls now type the materialized `Result`. | `7b7dc29` + `1a61817` + `88d771e`, run 41633106216 |
| 2026-11-03 | Release engineering started: lockstep version gate (root package, Cargo workspace, lockfile, orchestrator packages, both CLI constants) and a release script that bumps every version, moves `[Unreleased]` under a dated heading, refreshes `Cargo.lock`, and supports `--dry-run`; covered by a new test | `866a7dd` + `5890c23`, runs 41600987283 + 41602074448 |
| 2026-11-02 | Cooperative cancellation landed (D1): tasks poll the flag at GC safepoints and managed allocations, blocked channel waits are sliced so cancellation interrupts them, and cancelling a finished task is a no-op; 3 lead tests plus the GC-stress leg | `6346339`, run 41472912629 |
| 2026-11-02 | Multithreaded spawn landed: one OS thread per spawn bound to its structured scope, per-task heaps with shape-directed capture and result copies, blocking channels with the 5 s deadlock watchdog, and single-evaluation match scrutinees; lead tests cover capture/result copies, block joins, child panic, backpressure, and the watchdog; channels conformance re-pinned | `ec4c5a8`, run 41470469664 |
| 2026-11-02 | System package source landed: `nopm add sys:<tool>`, planner/executor, `NP0015`-`NP0018`, 6 tests (lead-written), bootstrap delegation with identical `--check` output | `4fb3554`, run 41427650964 |
| 2026-11-02 | Package add/remove landed: comment-preserving JSON/TOML edit API, CLI add/remove with `--json`/`--dry-run`, rollback, 9 acceptance tests (lead-written under a one-time exception) | `36dbccd`, run 41410494240 |
| 2026-11-01 | Two-manifest project layout landed (core + build, strict keys, single read path, ordering, tests) | `07274f9`, run 57 |
| 2026-11-01 | HARD-001 A slice 2: C-emitter fuzz leg + panic/OOM audit | `0af2b9d`, run 54 |
| 2026-11-01 | Compiler API `ast` + `explain`; `codegen-js` added to the gate | `80eb4e9`, run 53 |
| 2026-11-01 | `high` mode design approved; versioning + language decisions recorded | `7c41bb3`, run 52; `75d491d`, run 51 |
| 2026-11-01 | HARD-001 A slice 1: frontend fuzz, nesting limit, parser backtracking/drain fixes | `d65397f`, run 49 |
| 2026-11-01 | CONC-001 verdicts recorded (A2/B3/C2/D1) | `8f3e35f`, run 48 |
| 2026-11-01 | TASK-002 review filed (F1-F5, then F6-F9) | `be10ecd`, run 47 |
| 2026-10-31 | `@export_as` landed | `c24a358` + `2a169ff`, runs 44/45 |
| 2026-10-31 | `NS5007` system-mode side-effect pass | `2adaab6`, run 43 |
| 2026-10-31 | First Kilo pass (scope, D27, NS6002) after F1-F11 review | `2b8de7a`, run 42 |
| 2026-10-30 | GC-001 Phase C: collection on by default | runs 29-36 |
| 2026-10-25 | Repository initialized | baseline `2ce78ad` |

## Working rules

1. **Code first.** A slice touches docs only when (a) a decision changes - one
   row in the Decisions table, or (b) the slice lands - one CHANGELOG line and
   one Log row. No per-step narration.
2. **The spec is normative.** Status, queue, and review notes never go in it.
3. **`Task.md` is the only work order.** It holds the current task, the review
   verdict, and nothing older than the current round.
4. Keep this file under ~250 lines. Superseded detail goes to `Docs/archive/`.
5. No docs-only commits, except this consolidation.
6. Kilo owns implementation in `orchestrator/**`; the lead owns its tests from
   2026-11-02 and owns `crates/**` and `conformance/**`. The lanes may run in
   parallel: neither edits the other's files, and commits use explicit paths. A
   handback without tests is complete work, not an incomplete one - coverage is
   added by the lead before landing.
7. **Nothing half-made (D28).** No known correctness, memory-safety, or
   spec-conformance gap lands. A slice with a stub, a silent fallback, or an
   approximation is unfinished. Every failure mode it can hit gets a test. A
   toolchain handback that fixes only part of its review is returned, not
   landed. `1.0.0` waits for a clean gap audit.

## Verification protocol

Every slice: `npm run ci` (TypeScript build + tests, boundary/Wasm build, fmt
check, clippy `-D warnings`, workspace tests, GC stress leg), then commit with
explicit paths only, push to `ci`
(`luminousvalentine-cell/Neo-Astra-CI`), poll the Tier-1 run for the head SHA,
and record the run number in the Log row. Tier-1 on the committed tree is
authoritative; the mirror `origin` (`luminousvalentine-cell/Neo-Astra`) stays at
`1199998` and is synced only on request.

Before landing, the lead runs the D28 gap check: re-read the normative lines
the slice implements, confirm the failure-mode tests exist, and confirm no
stub, silent fallback, or raw approximation remains in the landed path. Before
`1.0.0`, the lead audits the whole shipped surface (stubs, TODOs,
approximations, deferred items) and closes each one or records a principal
decision. The release waits until that audit is empty.
