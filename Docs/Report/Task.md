# Task channel - lead to Kilo

Working channel between the **lead** (planner/reviewer) and **Kilo** (coder).
The **principal** (the user) is the final authority. Rule 10 applies to both:
do not invent decisions - mark the question instead.

## Protocol

- The Status line is the lock. While it says `OPEN - KILO`, the lead runs no
  shell or git operations in this repository.
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

## Status: OPEN - KILO (task: toolchain resolution, no network)

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
