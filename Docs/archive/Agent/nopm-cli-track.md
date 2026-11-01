# NoPM and CLI track — task list (owned by the lead)

Status: **ready; implementation starts when Kilo's TASK-001 fix round lands.**
The lead owns design, sequencing, review, and integration; the coder implements
one task at a time. Nothing here runs concurrently with another task in the
working tree.

Reference: `Docs/Design/toolchain-and-packages.md` (decisions), spec §12
(D26), `Docs/architecture.md` D26.

## Track rules

1. One task in flight. A task starts only after the previous one is committed,
   pushed, and Tier-1 green.
2. Every task: `npm run ci` green locally, `cargo fmt` where Rust is touched,
   docs updated by the lead, commit message via UTF-8 file, push to `ci`,
   Tier-1 poll recorded.
3. Integration contracts (binding, from the plan doc):
   - `manifest` is the **only** module that knows the two-file layout.
   - `nopm` is the **only** lockfile writer (including the `toolchain` section).
   - `cli` never imports `nopm`; the `toolchain` package is the bridge.
   - `astra build` never elevates; downloads are user-space into the project
     cache; `sys:` installs happen only from explicit commands.
   - `--locked`/`--frozen` must refuse downloads.
   - The gate stays offline: fixture transports, `--no-download` in tests.

---

## T-D26 — manifest split (`core.*` + `build.*`)

**Goal:** implement the two-manifest layout decided in D26.

**Deliverables**
- `manifest` package: discovery (`core.*` required, at most one, `build.*`
  optional), per-file schema, wrong-file-key diagnostic naming the right file,
  defaults for absent `build.*` (spec §12.2), canonical formatting for both.
- CLI: `check`/`build`/`fmt` read both; `fmt` formats `build.toml|json` with
  comment preservation.
- `nopm`: dependencies/packageSources/registry/capabilities/ffi from `core.*`;
  ignores `build.*` except for `[toolchain]` (written later by T-TOOLCHAIN-001a).
- Fixture migration: every test/template writing `core.json` inline.

**Acceptance**
- New tests: project with `build.toml`; project with no `build.*` (defaults);
  key in the wrong file → diagnostic; both formats; `astra fmt` idempotent on
  `build.*` with comments.
- Existing suite green; `npm run ci`; Tier-1.

**Risk:** fixture breadth — this is the widest mechanical slice; do it alone.

---

## T-NOPM-005 — `nopm add` / `nopm remove`

**Goal:** npm-install UX with manifest editing.

**Deliverables**
- `manifest` package: comment-preserving edit API (`addDependency`,
  `removeDependency`), canonical output, duplicate rejection.
- `nopm add <source-qualified-id>[@version] [dir]` → resolve → edit `core.*` →
  rewrite `core.lock.json` → install; `nopm remove <name>`; `--json`,
  `--dry-run`.

**Acceptance**
- Edit tests: comments survive, formatting stable, duplicate rejected, JSON and
  TOML forms.
- E2E: path + git + npm fixture add/remove; lockfile diff correct; store
  populated; `--dry-run` writes nothing.

**Depends on:** T-D26.

---

## T-NOPM-006 — `sys:` system-package-manager source

**Goal:** move the bootstrap install logic into NoPM as a source.

**Deliverables**
- Adapter: winget (win), brew (mac), apt/dnf/zypper (linux); explicit
  `nopm add sys:<tool>`; elevation handled by the adapter; tools-only.
- `scripts/bootstrap.mjs` delegates (same behavior, no duplicated logic).

**Acceptance**
- Tests assert the exact command lines per OS (no real installs in CI).
- Bootstrap dry-run output unchanged vs today.
- `nopm add sys:` never lands in `dependencies` (it is not a package).

**Depends on:** none for the adapter; T-D26 for the `add` path.

---

## T-TOOLCHAIN-001a — `[toolchain]` + ladder + `astra doctor` (no network)

**Goal:** make build-time toolchain resolution explicit and inspectable.

**Deliverables**
- New `@neo-astra/toolchain` package: parse `[toolchain]` from `build.*`,
  resolve in order **manifest pin → `.tools/toolchain` cache → PATH**, env
  override `ASTRA_CC`, result object (rung, path, version).
- `astra doctor` prints the ladder result and refuses unknown pins.
- `nopm` records the resolved toolchain in `core.lock.json` (single writer).
- `astra build` uses the resolved compiler instead of ad-hoc detection.

**Acceptance**
- Unit tests for order (mock PATH/cache/missing), pin mismatch, unknown pin.
- Frozen build refuses resolution that would need a download (flag present,
  download path is 001b).
- Existing native tests pass with PATH rung.

**Depends on:** T-D26.

---

## T-TOOLCHAIN-001b — pinned download / verify / unpack

**Goal:** the reproducible rung.

**Deliverables**
- Pinned Zig artifact table (per OS/arch) with SHA-256, download via NoPM
  transport, verify, unpack to `.tools/toolchain/<id>/`, lockfile record.
- `--no-download` and offline behavior (clear error naming the rung and the
  manual instructions); never elevates.

**Acceptance**
- Fixture transport test (no network in gate): success, checksum mismatch,
  truncated archive, offline.
- `astra doctor` reports "downloaded <id>" and the cache path.
- Windows/Linux/macOS artifact naming covered by unit tests; one real download
  smoke test gated behind an env flag (not in the default gate).

**Depends on:** T-TOOLCHAIN-001a, T-NOPM-006.

---

## T-TOOLCHAIN-001c — cross-target triples (post-V1)

`--target-triple` for cross compilation with the pinned toolchain. Deferred.

---

## Ledger

| Task | State |
|---|---|
| T-D26 | queued — starts after TASK-001 lands |
| T-NOPM-005 | queued |
| T-NOPM-006 | queued |
| T-TOOLCHAIN-001a | queued |
| T-TOOLCHAIN-001b | queued |
| T-TOOLCHAIN-001c | deferred (post-V1) |
