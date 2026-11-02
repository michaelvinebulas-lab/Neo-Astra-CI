# Task channel - lead to Kilo

Working channel between the **lead** (planner/reviewer) and **Kilo** (coder).
The **principal** (the user) is the final authority. Rule 10 applies to both:
do not invent decisions - mark the question instead.

## Protocol

- The Status line is the lock for `orchestrator/**`. While it says
  `OPEN - KILO`, Kilo owns that tree; the lead works only in `crates/**`,
  `conformance/**`, and lead-owned tests. Neither side edits the other's files,
  and the lead commits explicit paths only.
- While the lead's crates work is uncommitted, it verifies with the Rust legs
  (`cargo fmt --all --check`, clippy with `-D warnings`, workspace tests, GC
  stress). The full `npm run ci` gate runs when the tree has no other work in
  progress; every push still gets a Tier-1 run.
- Neither side rewrites the other's section. Append only.
- Kilo never commits, never pushes, never edits `Docs/**` or the spec. The
  deliverable is code, a `## Kilo log` entry, and a Status update. The lead
  reviews and lands.
- Questions that need the principal: write them under `## Kilo log` and set
  Status to `BLOCKED - principal`.
- Before handback: `cargo fmt --all` and the full gate (`npm run ci`) green,
  with the output tail reported.
- The lead writes the tests for `orchestrator/**` (principal's decision,
  2026-11-02). Your deliverable is implementation code, the log entry, and a
  green gate; the lead adds coverage before landing.

## Status: OPEN - KILO (fix round: toolchain; see Lead review)

Read the task below, write your `## Kilo log` entry, then code. When the gate
is green, write the report in the same block and set Status to `DONE - lead`.

---

# Task: toolchain policy, resolution ladder, `astra doctor`

## Goal

Make build-time toolchain selection explicit, inspectable, and recorded, with
no downloads and no elevation. The spec already defines the policy and the
record: `build.*` `toolchain.c` is `auto` or a pinned id (§12.2), and the
resolved toolchain goes into `core.lock.json` (§12.3). This slice implements
that; it does not change the spec.

**Scope.** You own `orchestrator/**`. You may edit the root `tsconfig.json`
only to add the reference for the new package. Do not touch `package.json`,
`scripts/**`, or `Docs/**`.

## 1. New package `@neo-astra/toolchain`

Must have no dependency on the CLI or nopm, and must be testable without
spawning: inject filesystem probes and command probes.

- `parseToolchainPolicy(buildData)` -> `{ request: "auto" | string }`.
  - `toolchain.c` defaults to `auto`; a non-string or empty value is
    `NS9101`.
  - A pin id matches `auto` or `<family>[@<version>]`, family in
    `clang | gcc | zig | cc` (for example `zig@0.13.0`, `clang`, `gcc@14`).
    Anything else is `NS9101`.
- `resolveToolchain(options)` -> resolution record:
  `{ rung: "env" | "cache" | "path", id, family, path, version }`.
  `id` is the pin string when the policy is pinned; for `auto` and
  `ASTRA_CC` it is the resolved family.
  - **CLI `--compiler <path>`** is handled by the CLI, not this function; it
    wins over everything.
  - **`ASTRA_CC`** (non-empty) is rung `env`: the path must exist and answer
    `--version`; otherwise `NS9103`.
  - **Pinned id**: cache first at
    `<project>/.tools/toolchain/<id>/`, which must contain
    `toolchain.json` = `{ "id", "family", "version" }` and the binary at
    `bin/<family>[.exe]` or `<family>[.exe]`. A missing marker, a marker whose
    `id` differs, or a missing binary is `NS9102` ("pin mismatch"). A bare
    family pin (`clang`, `gcc`) may fall back to PATH when the cache is absent;
    a versioned pin (`clang@18`, `zig@0.13.0`) never trusts PATH - it fails
    `NS9102` and names the missing cache entry.
  - **`auto`**: PATH candidates `clang`, `gcc`, `cc` in that order, each
    required to pass the hosted probe already used by `findNativeCompiler`
    (`-std=c17 -fsyntax-only -x c -` with `#include <stdio.h>`), falling back
    to the first that only answers `--version`. Family is derived from the
    binary name (`clang` -> clang, `gcc`/`cc` -> gcc).
  - No network, no `sys:` calls, no elevation anywhere in this slice.
- `doctor(projectRoot)` -> the ladder attempts plus the winner, for printing.
  It reports each rung tried (env, cache, path), the winner's rung/id/family/
  path/version, or the failure reason.

## 2. CLI

- `astra doctor [dir]`: load the project, print the resolved policy and the
  ladder result. Resolved -> exit 0. Unresolved -> exit 1 with the reason.
  `--json` may be added if it falls out naturally, but text is the contract.
- `astra build` (and the second native path at the other
  `findNativeCompiler()` call site): resolve through the new package instead of
  the ad-hoc list. Precedence: `--compiler`, then `ASTRA_CC`, then
  `toolchain.c`. Keep the current hosted-probe behavior for `auto` so existing
  native tests pass on the PATH rung.
- Update the help text for `doctor` and for `ASTRA_CC`.

## 3. Lockfile

- Add `toolchain: { rung, id, family, path, version } | null` to the lockfile
  model, formatter, parser, and validator. nopm remains the only writer.
- `nopm lock` and the re-lock inside `nopm add` / `nopm remove` resolve the
  policy from the build manifest and record it.
  - `auto` unresolved -> record `null`, do not fail the lock.
  - pinned and unresolved -> fail `NS9102`.
- `nopm tree` need not print it; the field is for `doctor`-level inspection
  and reproducibility. Do not change `lockfileVersion`.

## Acceptance

- `npm run ci` green; existing native tests still pass on the PATH rung.
- `astra doctor` names the rung and the binary for a cache hit, a PATH hit, and
  an `ASTRA_CC` override; exits 1 with a clear reason when nothing resolves.
- A versioned pin with no cache entry fails `NS9102` and never falls back to
  PATH or to a download.
- A cache marker whose id differs fails `NS9102`.
- `nopm lock` writes the toolchain record; `auto` with no compiler writes
  `null` and still succeeds; a pinned unresolved toolchain fails.
- No network, no package-manager invocation, no elevation in any new path.

## Lead review - toolchain round 1 (2026-11-02)

The tree builds and the 171 existing TypeScript tests pass, so the layer is
coherent. It is not landable yet: the lockfile writer is missing, the rung
report is incomplete, and the error path loses the toolchain code. The lead
writes the acceptance tests and lands. Fix the items below in
`orchestrator/**`, append your report under `## Kilo log`, and set Status to
`DONE - lead`.

### `@neo-astra/toolchain`

1. **Typed errors.** Throw a `ToolchainError` carrying `code` and a human
   message (`NS9101` bad policy, `NS9102` pin mismatch, `NS9103` bad
   `ASTRA_CC`). Never throw the bare string `"NS9101"`; the CLI must be able
   to print the code and reason.
2. **Policy validation.** If `toolchain` is present but not an object, or `c`
   is not a string, fail `NS9101` (do not silently default to `auto`). Reject
   a second `@`, an empty version, and a version containing whitespace.
   Trim the pin.
3. **`auto` order.** Make the two passes distinct: first the hosted probe
   (`-std=c17 -fsyntax-only -x c -` with stdin), then the first candidate that
   only answers `--version`. `hostedProbe` must not return true merely because
   `--version` works. Record the candidate's family (`cc` -> gcc).
4. **Pinned cache.**
   - A marker whose `id` differs is `NS9102` for **both** bare and versioned
     pins. Only a **missing** cache entry/marker may fall back to PATH for a
     bare family pin.
   - Validate the marker's `family`; for a versioned pin also validate its
     `version` against the pin.
   - A marker without a binary is `NS9102`.
5. **`ASTRA_CC`.** Path must exist and answer `--version` (else `NS9103` with
   a clear message). A bare command name is not a path; either resolve it on
   PATH or fail with a reason that says so.
6. **`doctor`.** Report every rung actually attempted, in order, with the
   concrete detail: the env value, the cache marker path and why it failed,
   each PATH candidate and its probe result. Then the winner
   (`rung/id/family/path/version`) or the failure reason. No duplicate env
   entry.

### CLI

7. **Do not swallow toolchain errors.** `resolveNativeCompiler` must let
   `NS9101`/`NS9102`/`NS9103` reach the user with their code and message.
   Only an unresolved `auto` may keep the current `NS9008` "no native
   compiler" path. Keep the hosted-probe behavior so the existing native
   tests still pass on the PATH rung.
8. **Remove the unrelated `ast`/`explain` implementations** and the
   `findCompilerBinary` import. This slice is `doctor` plus the build
   resolver; those commands belong to a separate task.
9. **`doctor` UX.** Parse `--json` independently of the directory argument
   (`astra doctor --json` must work). Load only the build manifest; absent
   means `auto`. Print policy, each rung, winner or reason, and exit 0/1.
   Document `ASTRA_CC` in the help text.
10. **Correction to the task:** the existing `--compiler` flag selects the
    astra-compiler binary, not the C toolchain. Do not repurpose it. Native
    precedence is `ASTRA_CC`, then `toolchain.c`. A dedicated `--cc` flag is
    proposed to the principal; do not add it in this slice.

### nopm

11. Add `@neo-astra/toolchain` to nopm's dependencies and resolve + record
    `toolchain` in the single lock writer used by `nopm lock`, `add`, and
    `remove`:
    - parse the build manifest policy and resolve with the real probe;
    - `auto` unresolved -> `toolchain: null`, lock still succeeds;
    - pinned unresolved -> fail `NS9102`;
    - do not change `lockfileVersion`.
    The model/formatter/validator round-trip is already there; the writer is
    the missing acceptance item.

### Housekeeping

12. Restore the original formatting in the root and CLI `tsconfig.json`; add
    only the new reference line.
13. Do not add `dist/` or `*.tsbuildinfo` (both are ignored).

Note: the Tier-1 failure on `d8ed12e` was in the lead's concurrency slice
(glibc hides `clock_gettime` under strict `-std=c17`), fixed in `ec4c5a8`;
run 41470469664 is green. It was not the per-task heap work in `emit.rs` /
`ty.rs`.

## Kilo log

(Append the fix-round report here.)
