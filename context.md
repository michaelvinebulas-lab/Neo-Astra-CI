# Neo Astra context handoff

**Snapshot:** 2026-11-11  
**Working copy:** `C:\Projects\Neo Astra`  
**Branch:** `master`  
**Published release:** `1.0.0` (tag `v1.0.0` -> `9d832e0`)  
**Next release:** `1.0.1` native Rust CLI/NoPM - in progress, not tagged or published  
**Current order:** `Docs/Report/Task.md` says `OPEN - KILO - slice 3a`

This file is a handoff snapshot, not a normative spec. Read the linked documents
for detail. Update this file when the working state changes materially.

## Read first

1. [`Docs/Report/Task.md`](Docs/Report/Task.md) - active lead/Kilo order, status,
   and the two slice-3a rejection reviews.
2. [`Docs/state.md`](Docs/state.md) - project status, decisions, and recent log.
3. [`Docs/Report/Syntax-v0.2.md`](Docs/Report/Syntax-v0.2.md) - accepted v0.2
   cleanup plan and Phase 0 boundary.
4. [`Docs/spec-v0.1.md`](Docs/spec-v0.1.md) - the only normative language
   contract today.
5. [`Docs/todo.md`](Docs/todo.md) - the queue, including the new `1.0.1`
   section.
6. [`Docs/release-checklist.md`](Docs/release-checklist.md) - release gates.

## Hard constraints

- Keep the native CLI/NoPM implementation in Rust. JS/TS codegen is the only
  thing allowed behind an isolated on-demand Node bridge.
- `check`, `fmt`, `ast`, `explain`, `doctor`, C/native builds, and baseline NoPM
  must work without Node, npm, Bun, or a nested `astra`/`nopm` child process.
- Never push `origin` from `C:\Projects\Neo Astra`; only
  `C:\Projects\Neo Astra-source` may push `origin`. The private tree may push
  its CI remote only when the lead explicitly does so.
- Kilo does not commit, push, tag, publish, revoke tokens, delete
  `NPM_TOKEN`, or edit `Docs/**`, `conformance/**`, or
  `orchestrator/tests/**`. Kilo appends only a `## Kilo log` entry and the
  Status line in `Docs/Report/Task.md`.
- Tagging, pushing, publishing, token revocation, and public immutable-release
  changes are lead/principal actions. Do not perform them from Kilo's handback.
- Do not stage or commit Kilo/user files. Preserve pre-existing untracked files.
- Apache-2.0 stays fixed across the workspace and npm packages.
- `1.0.1` stays on language v0.1. Do not inject the v0.2 cleanup into the
  native port.
- v0.2 is not normative and its parser/checker/codegen implementation has not
  started. `spec-v0.1.md` wins until the v0.2 freeze.
- Prefer deprecation/canonicalization over deleting valid syntax. Only change
  semantics where necessary.

## Current reality

- `1.0.0` is published. Public source master is `17ceaf6`; tag `v1.0.0` points
  at `9d832e0`; release run `42278798124` published the platform packages and
  the `neo-astra` meta package.
- The native Rust port has landed slices 1, 2a, 2b, and 2c:
  - slice 1 `7ca69be` - core project commands.
  - slices 2a/2b `778bbc5` - toolchain/doctor and native/C executable builds.
  - slice 2c `501e91f` - pinned Zig and staticlib/cdylib artifacts.
- Slice 3a (native NoPM baseline) is open. Kilo's implementation is
  uncommitted and has been rejected twice:
  - `ea0a2c2` - first rejection; hardcoded `local-utils`/`web` stub.
  - `a433514` - second rejection; still hardcodes web targets/dependencies,
    does not validate target support, leaves `install`/`why`/`update` as
    stubs, and still has no required lifecycle tests.
- Native parity is **11/12**. Only the JS build case is red for slice 4. The
  NoPM parity case passes because it only exercises the fixed
  `local-utils`/`web` fixture; it is not evidence that 3a is complete.
- A custom path-package check against the TypeScript oracle found:
  - root `build.json` declares `native` and `web`;
  - path package declares only `native`;
  - TS `add path:pkg --json` returns `NP0006` ("does not support target
    'web'");
  - native exits 0 and writes `targets: ["web"]`.
- The current `astra-nopm` code is in flux. At one snapshot,
  `cargo test --workspace` and `npm test` (241) completed green, but a
  subsequent `cargo clippy -p astra-nopm -p astra-cli --all-targets -- -D
  warnings` failed to compile with `E0560` (field `alias` missing from
  `DependencyRequest`) plus `unused_mut` errors. Re-run the gates; do not trust
  a cached "green" claim while Kilo is editing.
- `cargo test -p astra-nopm` currently has **0 library tests** and one SHA-256
  integration test. The required package-ID, manifest-hash, containment,
  lockfile-current, store, and add/remove rollback tests do not exist.

## v0.2 status

Phase 0 is complete on the documentation side only:

- Accepted cleanup plan and defaults: `Docs/Report/Syntax-v0.2.md` (see 5.11).
- RFC A error model and RFC B generics are closed:
  `RFC-A-error-model.md`, `RFC-B-generics.md`.
- `Docs/spec-v0.2.md` is a non-normative skeleton. It reserves front-end
  diagnostics `NS2008`-`NS2019`.
- `tools/grammar-v0.2` uses `qana-grammar` canonical LR(1)/Pager; 4/4 tests
  green for the focused grammar.
- `conformance/v0.2/` has 21 fixtures (valid, deprecated, invalid, migration
  input/golden pairs) and is deliberately not loaded by the v0.1 runner.
- Commits: `44a416d`, `ec1c3f4`, `21234aa`, `54ef1d0`.
- Decisions: aliases are deprecated, not removed; `/%`, `spawn ||`, and the
  special `///` meaning are deprecated; old scope blocks stay accepted by
  default and are rejected by `--canonical-syntax`; v0.1 formatter preserves
  v0.1 spellings and `astra migrate` is the only rewrite path.
- v0.2 direction remains effect errors with `?`, `fail` as `Never`, demoted
  `Result<T,E>`, turbofish `::<...>`, `@mode(...)`/`@memory(...)` scopes,
  commas for lists, `edition = "0.2"`, `char`, and `fn(x) => expr` lambdas.
  Feature gaps are a separate later train.
- Do not wire the v0.2 corpus into the v0.1 runner or move fixtures into
  `conformance/cases` / `conformance/rejects` before the parser phase gate.

## Release and npm state

- `1.0.1` is not tagged. Do not tag until slice 3a, 3b, and 4 are accepted and
  native parity is 12/12.
- `v1.0.0` is immutable by policy, but its GitHub release reports
  `immutable: false`. Do not modify its assets.
- `1.0.0` has registry signatures but no npm provenance attestation; its GitHub
  archive attestations return 404. Verify provenance/attestations on `1.0.1`.
- npm trusted publishing is configured for the four packages, but not validated
  until the first successful OIDC publish. `npm trust list` currently requires
  passkey re-authentication and returns `EOTP`; that is a principal action.
  Refresh the relationship immediately before the next release, because
  unvalidated configurations expire after two days.
- The token exposed during 1.0.0 still needs revocation. Keep `NPM_TOKEN` only
  as a rollback until the first successful OIDC publish, then delete it.
- The LSP `serverInfo` version still reports `2.0.0`; correct before shipping
  the server.
- No public CI workflow exists yet; do not add required status checks until the
  check names exist.

## Working tree snapshot

Dirty because Kilo's slice-3a work is uncommitted:

- Modified: `Cargo.toml`, `Cargo.lock`, `crates/astra-cli/Cargo.toml`,
  `crates/astra-cli/src/main.rs`, `crates/astra-nopm/Cargo.toml`,
  `crates/astra-nopm/src/main.rs`.
- Untracked Kilo files: `crates/astra-nopm/src/lib.rs`,
  `crates/astra-nopm/src/nopm.rs`, `crates/astra-nopm/src/sha256.rs`,
  `crates/astra-nopm/tests/`.
- Pre-existing user/untracked files: `432+5`, `700`,
  `Docs/error-code-ledger.md`, `tmp.py`, `tmp_err.txt`.
- Do not stage any of the above until Kilo hands back and the lead reviews.
- Lead commits are local. `origin` has not been pushed from this tree.

## Work order

1. Kilo replaces the hardcoded `astra-nopm` implementation with the real shared
   path lifecycle: JSON/TOML project read, path resolution/containment,
   manifest hash, lockfile generation/parsing/staleness, `--locked`/`--frozen`,
   store index, `add`/`remove`/`lock`/`update`/`tree`/`install`/`why`, and
   add/remove rollback.
2. Kilo adds the required unit tests and a complete handback with changed
   files, commands, output tails, deviations, and gaps.
3. Lead strengthens the native parity test with a different package
   name/path/target set and an invalid-project `NP0006` case, then reviews 3a.
4. After 3a acceptance: 3b (remote adapters, registry, `sys:` tools) and then
   4 (JS/TS bridge, packaging, CI).
5. Release validation, then lead/principal tag/publish `1.0.1`.
6. Only after the native front-end handbacks: v0.2 Phase 1 edition plumbing and
   the rest of the cleanup phases.

## Slice 3a acceptance checklist

- Native parity is 11/12 with the three NoPM cases green and only the JS build
  red.
- `nopm` and `astra nopm` produce identical results.
- No-Node NoPM path lifecycle passes with Node/npm removed from `PATH`.
- All Rust/npm gates are green, including `cargo clippy ... -D warnings`.
- Unit tests cover package IDs, manifest-hash normalization, path containment,
  lockfile format/current checks, store writing, and add/remove rollback.
- The implementation is not a fixed fixture: different package names, paths,
  target sets, and invalid inputs must match the TypeScript oracle.

## Key files

- `Docs/Report/Task.md` - current order and rejection reviews.
- `Docs/Report/Syntax-v0.2.md` - v0.2 plan and accepted defaults.
- `Docs/spec-v0.2.md` - non-normative v0.2 skeleton and reserved diagnostics.
- `conformance/v0.2/` - non-loaded v0.2 corpus.
- `tools/grammar-v0.2/` - focused grammar conflict harness.
- `crates/astra-nopm/src/nopm.rs` - Kilo's active slice-3a implementation.
- `crates/astra-cli/src/main.rs` - native CLI dispatch.
- `orchestrator/tests/native-parity.test.mjs` - 12-case parity gate; the NoPM
  case is currently too narrow.
- `orchestrator/packages/nopm/dist/cli.js` - TypeScript oracle for behavior
  comparisons.

## Verification commands

```powershell
cargo fmt --all --check
cargo clippy -p astra-nopm -p astra-cli --all-targets -- -D warnings
cargo test --workspace
cargo test -p astra-nopm
npm test
npm run test:native-parity
```

Last known results at this snapshot:

- native parity: 11/12, JS build only failure.
- `npm test`: 241/241 green (while native code was in flux).
- `cargo test --workspace`: green at one snapshot, with `astra-nopm` warnings.
- targeted clippy: failed after a concurrent edit (`E0560` and `unused_mut`);
  re-run before accepting anything.
- `cargo test -p astra-nopm`: 0 library tests + 1 SHA test.

## Do not

- Do not accept "status set to slice 3b" without a reviewed 3a handback.
- Do not treat the green NoPM parity case as proof of the lifecycle; it only
  covers the hardcoded fixture.
- Do not tag, push `origin`, publish, revoke tokens, or delete `NPM_TOKEN`.
- Do not commit Kilo's uncommitted code, the pre-existing untracked files, or
  the active `Docs/Report/Task.md` log without review.
- Do not describe `1.0.1` or v0.2 as shipped.
