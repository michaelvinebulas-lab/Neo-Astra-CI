# State

Single source of truth for project status, the queue, and decisions.
Everything else is either normative (the spec), operational (the work order),
or frozen history (`Docs/archive/`).

**Last updated:** 2026-11-01
**Phase:** V1 completion - packaging, toolchain, then concurrency.
**Master:** `0af2b9d` (Tier-1 run 54 green: Linux, macOS, Windows).
**Blockers:** none.

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
- **Toolchain:** `astra` CLI (check, fmt, build, run), NoPM (lock, tree,
  install, search, audit, why, update), manifest formatting, registry adapters.
- **Compiler API:** JSON-RPC/native + raw Wasm: `check`, `check_project`,
  `compile`, `compile_project`, `format`, `ast`, `explain`.
- **Quality:** Tier-1 CI on three OSes; frontend and C-emitter fuzz suites
  (arbitrary input never panics; clean input emits without panicking);
  conformance corpus across backends.

## Current work

Open tasks, grouped by version, live in [`todo.md`](todo.md). In progress:
package add/remove (the two-manifest layout has landed).

**Release:** cut `1.0.0` when the `1.0.0` tasks in `todo.md` are complete. All
packages move in lockstep: new mode/backend/language = minor, fixes = patch,
incompatible rework = major. Packages stay `0.1.0` until the cut.

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
| CONC-001 | 2026-11-01 | A2 OS thread per spawn bound to the structured scope (A1 eager fallback for Web/JS/Nova); B3 parked channel ops + 5 s watchdog (`ASTRA_CHANNEL_DEADLOCK_TIMEOUT`); C2 per-task heaps + thread-safe immortal heap; D1 cooperative cancellation at GC safepoints and channel ops. Implementation last in V1. |
| `high` mode | 2026-11-01 | New mode: `app` capabilities + ban list (`Memory`/`@memory:manual`, `Ptr`, `Region`, `ffi`, `@script`, C-ABI/`@export_as`) + full sugar (top-level statements, implicit main/exit 0, `print`, tail return, `fn main` mixing error, optional return-type inference); profile via manifest `defaultMode`; importable as app. Post-1.0. |
| Languages | 2026-11-01 | Extend the C/Wasm/JS/TS pattern (targets, interop, toolchain rungs) to Rust, Go, Lua, Ruby, Java. Per-language scope decided on pickup; V2+. |
| Versioning | 2026-11-01 | SemVer, lockstep, cut `1.0.0` at V1 end. |

**Open questions:** none. If a new one comes up, it goes in `Task.md` marked
**proposed** until the principal decides - never into the spec.

## Log (recent, pruned)

| Date | Change | Evidence |
|---|---|---|
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

## Verification protocol

Every slice: `npm run ci` (TypeScript build + tests, boundary/Wasm build, fmt
check, clippy `-D warnings`, workspace tests, GC stress leg), then commit with
explicit paths only, push to `ci`
(`voidgod79-debug/Neo-Astra-CI`), poll the Tier-1 run for the head SHA, and
record the run number in the Log row. Tier-1 on the committed tree is
authoritative; the mirror `origin` (`luminousvalentine-cell/Neo-Astra`) stays at
`1199998` and is synced only on request.
