# Neo Astra context handoff

**Snapshot:** 2026-11-19
**Working copy:** `C:\Projects\Neo Astra`
**Branch:** `master`
**Published release:** `1.0.0` (tag `v1.0.0` -> `9d832e0`)
**Next release:** `1.0.1` native Rust CLI/NoPM - slices 1-4 accepted, release validation next, not tagged or published
**Current order:** `Docs/Report/Task.md` says `DONE - lead`; slice 4 accepted

This file is a handoff snapshot, not a normative spec. Read the linked documents
for detail. Update this file when the working state changes materially.

## Read first

1. [`Docs/Report/Task.md`](Docs/Report/Task.md) - active lead/Kilo order, status,
   and the slice-4 acceptance review.
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
- The native Rust port has landed slices 1, 2a, 2b, 2c, 3a, 3b, and 4. Slice 4
  is accepted.
- Slice 4 packaging:
  - `bin/astra` and `bin/nopm` are Cargo-built native binaries.
  - The JS/TS bridge payload is under `lib/cli`; the npm platform package and
    postinstall copy it plus the pinned TypeScript compiler.
  - The isolated package smoke covers no-Node native/core commands, `astra
    nopm` dispatch, C/native builds, and `@script:ts`/`@script:js` builds.
  - The npm postinstall path has a lead-owned fake-platform test and a real
    generated-package install simulation.
- Native parity is **12/12**. `npm test` is 242/242, `npm run ci` is green,
  and `npm run package` succeeds from an isolated package copy.
- Release validation for `1.0.1` is the next step. Tagging and publishing
  remain principal-gated.

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

- `1.0.1` is not tagged. Slices 1-4 are accepted and native parity is 12/12;
  run and accept release validation before tagging.
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

Dirty because the native port and packaging fixes are uncommitted:

- Modified: the workspace manifests, `crates/astra-cli/**`,
  `crates/astra-nopm/**`, CI/packaging scripts, `orchestrator/tests/package-npm.test.mjs`,
  and the lead docs.
- Untracked: the new `astra-nopm` modules/tests, `Docs/Report/Orchestrator-v2.md`,
  and `Docs/error-code-ledger.md`.
- Pre-existing user/untracked files: `432+5`, `700`, `tmp.py`, `tmp_err.txt`.
- Do not stage any of the above without principal approval.
- Lead commits are local. `origin` has not been pushed from this tree.

## Work order

1. Run the `1.0.1` release validation checklist against the accepted native
   package and npm artifacts.
2. If validation is green, the lead/principal tags and publishes `1.0.1`.
3. Then start v0.2 Phase 1 edition plumbing and the rest of the cleanup phases.
4. The V2 C++20 orchestrator rewrite is a later track; see
   `Docs/Report/Orchestrator-v2.md`. TypeScript remains the v1 oracle and
   isolated JS/TS bridge until C++ parity is proven.

## Slice 4 acceptance checklist

- Native parity is 12/12, including JS build and no-Node native/core paths.
- Packaged `astra`/`nopm` are Cargo binaries; the bridge payload is under
  `lib/cli` with its package marker and runtime dependencies.
- Isolated package smoke covers `--target js|ts`, C/native builds, `astra nopm`
  dispatch, and `nopm --version` with Node/npm removed from `PATH` for native
  paths.
- npm postinstall copies `bin`, `typescript`, and `lib`; a fake-platform test
  and a generated-package install simulation pass.
- `npm test` is 242/242, `npm run ci` is green, and `npm run package` exits 0.
- README text distinguishes native/core runtime requirements from the Node
  bridge needed by JS/TS builds.

## Key files

- `Docs/Report/Task.md` - current order and slice-4 acceptance review.
- `Docs/Report/Orchestrator-v2.md` - C++20 V2 scope and boundary decision.
- `Docs/Report/Syntax-v0.2.md` - v0.2 plan and accepted defaults.
- `Docs/spec-v0.2.md` - non-normative v0.2 skeleton and reserved diagnostics.
- `conformance/v0.2/` - non-loaded v0.2 corpus.
- `tools/grammar-v0.2/` - focused grammar conflict harness.
- `crates/astra-nopm/` - native NoPM implementation and tests.
- `crates/astra-cli/src/main.rs` - native CLI dispatch, including `astra nopm`.
- `scripts/package.mjs` - archive packaging and isolated package smoke.
- `scripts/package-npm.mjs` / `scripts/npm/postinstall.mjs` - npm channel and
  bridge payload copy.
- `orchestrator/tests/native-parity.test.mjs` - 12-case parity gate.
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

- native parity: 12/12 green.
- `npm test`: 242/242 green.
- `npm run ci`: green.
- `npm run verify`: green.
- `npm run package`: green; archive sha256
  `5aafcd86272bd1a229aacf452d3bcb6bfd114eb97b74e2d0caf9282ac12e8400`.
- npm generated-package postinstall simulation: green; `astra`/`nopm` report
  `1.0.0` and the bridge/TypeScript payloads are present.

## Do not

- Do not treat the accepted slice 4 as release validation; run the checklist
  before tagging.
- Do not tag, push `origin`, publish, revoke tokens, or delete `NPM_TOKEN`.
- Do not commit Kilo's uncommitted code, the pre-existing untracked files, or
  the active `Docs/Report/Task.md` log without review.
- Do not describe `1.0.1` or v0.2 as shipped.
