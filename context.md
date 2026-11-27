# Neo Astra context handoff

**Snapshot:** 2026-11-27
**Working copy:** `C:\Projects\Neo Astra`
**Branch:** private `master` at the Phase 7 `char` commit (pushed to `ci` = `michaelvinebulas-lab/Neo-Astra-CI`); public `master` is the sanitized `1.1.1` tree
**Published release:** `1.1.1` (tag `v1.1.1` -> public `868007e`)
**Next release:** `1.1.2` - Phase 7 feature gaps behind the edition-`0.2` boundary
**Current state:** `if let` / `while let`, ranges, bitwise operators, and `char` implemented; lambdas and suffix table queued

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
- `1.0.1` stays on language v0.1; the v0.2 cleanup shipped in `1.1.1`.
- `spec-v0.2.md` is normative for edition `0.2`; `spec-v0.1.md` remains
  normative for edition `0.1`. Phase 7 feature gaps are queued for `1.1.2`.
- Prefer deprecation/canonicalization over deleting valid syntax. Only change
  semantics where necessary.

## Current reality

- `1.0.1` is published on GitHub and npm. Public master is `50817d0`; tag
  `v1.0.1` points at `3093343`; release run `45125506996` published all four npm packages with provenance and re-uploaded
  the attested archives.
- npm has `neo-astra@1.0.1`, `neo-astra-linux-x64@1.0.1`,
  `neo-astra-darwin-arm64@1.0.1`, and `neo-astra-windows-x64@1.0.1`; `latest`
  is `1.0.1`. A clean Windows install reports `astra 1.0.1`/`nopm 1.0.1`, and
  `npm audit signatures` verifies registry signatures and attestations.
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
- Native parity is **12/12**. `npm test` is 243/243, `npm run verify` and
  `npm run ci` are green, and `npm run package` succeeds from an isolated
  package copy.
- Release validation is green. Tier-1 runs `44963562468` and `44965046279`
  are green on Ubuntu, macOS, and Windows; cross-platform package validation
  run `44964210902` is green on all three, including the isolated
  `@script:ts`/`@script:js` smoke and a clean archive extraction that reports
  `astra 1.0.0` / `nopm 1.0.0`.
- The macOS packaged-bridge failure was a direct-entry bug: `import.meta.url`
  is canonicalized while `process.argv[1]` keeps the invoked path, so a package
  under `/var/folders` (where `/var` -> `/private/var`) silently did nothing.
  `9dd651e` canonicalizes both sides and adds a symlinked-entry regression
  test.
- The `1.0.1` bump is applied (`4d54ac4`); native `--version` and lockfile
  metadata now derive from the workspace version. The sanitized source tree is
  regenerated and passes `npm ci`, `npm run build`, `npm run package`, and
  clippy locally.
- CI moved to `michaelvinebulas-lab/Neo-Astra-CI` after the old account hit an
  Actions billing/spending block; the new Tier-1 run `45019255462` is green on
  Ubuntu, macOS, and Windows for `fc54f28`. Deleting the old CI repo needs a
  principal `delete_repo` scope refresh.
- The npm publish failures were two separate bugs: `actions/setup-node@v4`
  wrote `github.token` into `.npmrc` when `registry-url` was set (`E404`), and
  the trusted publishers created with npm 11.13.0 omitted the required
  `createPackage` permission (`package not found`). The workflow no longer sets
  `registry-url`, and the trust configs were recreated with
  `npx npm@latest ... --allow-publish`.

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

Phase 1 (edition plumbing) is implemented on private master:

- `crates/astra-core/src/edition.rs` defines `Edition { V0_1, V0_2 }` with
  `FromStr`, `Display`, and `V0_1` as default.
- Options-aware `lex_with_options` / `parse_with_options` /
  `format_source_with_options` preserve the existing default entry points.
  `CheckOptions.edition` defaults to `V0_1`.
- Compiler API requests carry `#[serde(default)] edition`; the native CLI reads
  the nearest manifest and passes the edition to check/fmt/ast/explain/build.
  Invalid manifest editions are rejected with `NS2008` and a migration hint.
- No syntax behavior changed. Gate tests prove v0.1 output is byte-identical
  and explicit `"0.2"` still parses the v0.1 surface.

Phase 2 (lexer) is implemented on private master:

- `Keyword::Fail` maps `fail` only under edition `0.2`; under `0.1` it stays an
  identifier.
- `Punctuation::Question` (`?`) and `Punctuation::DoubleColon` (`::`) are
  edition-`0.2` tokens. A standalone `::` or `:: <` reports `NS2011`.
- Tuple indexes after a postfix `.` are integer-only under edition `0.2`, so
  `t.0.1` is two indexes while `1.0` remains a float. The rule is edition-gated
  so v0.1 forms such as `t.0u8` keep their existing tokenization.
- `///` remains an ordinary comment; no doc-comment trivia was added.
- Lexer tests cover `?`, `::`, `:: <`, `fail` in both editions, `1.0`, `t.0.1`,
  `t.0.1.2`, and comments.

Phase 3 (parser and checker) is implemented on private master:

- Under `0.2`, `fail Type(args)` and `fail Type { field: value }` parse to
  `ExprKind::Fail`; `return fail ...` reports `NS4014`; prefix `!Error(...)` and
  postfix `!` report `NS2015` with migration hints. Edition `0.1` keeps both
  old forms and treats `fail` as an identifier.
- Postfix `?` parses to `ExprKind::Propagate` at postfix precedence.
- `::<...>` turbofish parses on calls and constructors; a second turbofish is
  `NS2011`, a bare `path<T>(...)` or qualified variant path is `NS2010`, and the
  v0.1 lookahead heuristic is edition-`0.1` only.
- `@mode(name) { ... }` and `@memory(name) { ... }` parse at item and statement
  positions; the old `{@mode:name; ...}` form is unchanged for `0.1`.
- Edition `0.2` allows trailing commas in every list, requires commas between
  expression-bodied match arms, continues after trailing binary operators and
  before `.`, and rejects a leading binary operator.
- The checker validates `Err` member heads (`NS4017`), member exhaustiveness
  (`NS4010`), and D7 declared-set subset checking; variant constructors are in
  value scope. An AST serialization contract test pins `Fail`/`Propagate`.
- Two v0.2 corpus `valid` cases remain deferred because they need behavior
  changes outside the cleanup: `scopes.ns` (mode widening) and `turbofish.ns`
  (generic struct-initializer inference). The parser tests cover their syntax.

Phase 4 (formatter and canonical lint) core work is landed:

- The formatter carries the edition. Under `0.2` it emits `fail`, postfix `?`,
  `::<...>` for expression-position type arguments, and
  `@mode(name) { ... }` / `@memory(name) { ... }`. Under `0.1` it keeps
  `!Error(...)`, postfix `!`, bare `<...>`, and `{@mode:name; ...}`.
- `/%` is preserved in both editions; `spawn ||` is canonicalized to `spawn`
  under `0.2`. Round-trip and idempotence tests cover both editions.
- `crates/astra-core/src/canonical.rs` is a source/token-aware deny lint for
  deprecated aliases, `/%`, `spawn ||`, and old scope blocks under `0.2`. It is
  check-only and exposed as `astra_compiler_api::check_canonical`.
- CLI wiring landed: `astra check --canonical-syntax`, `astra fmt --canonical`,
  and `astra migrate` are implemented in `crates/astra-cli/src/migrate_cmd.rs`.
  `astra migrate` remains the only rewrite path.

Phase 5 (migration and validation) core work is landed:

- `crates/astra-core/src/migrate.rs` is the only rewrite path. It converts
  aliases, prefix `!Error(...)` / `!Error { ... }` (including dropping a
  redundant `return`), postfix `!`, old scope blocks, confident `path<T>(...)`
  calls, and `spawn ||`. `/%` is preserved with a warning; manifests and edition
  fields are never edited without an explicit opt-in.
- Migration is idempotent and exposed through `astra_compiler_api`. The v0.2
  migrate corpus is exercised by `crates/astra-core/tests/edition_v0_2_migrate.rs`.
- Added conformance cases for tuple indexes, continuation, and leading binary
  operators, plus parser fixes for multiline struct-init lookahead and
  `spawn { ... }` under edition `0.2`.
- CLI commands (`astra migrate` with `--dry-run`/`--check`/`--json`, plus
  `check --canonical-syntax` and `fmt --canonical`) remain unwired because the
  CLI files are owned by a parallel agent.

The error-output rework (Slices A, B, C) is landed at `8caf2ea`, including the
review cleanup: ledger committed and enforced, LSP project `relatedInformation`,
error-set secondary labels, and bounded parser recovery. `parse_error.ns` emits
2 diagnostics with `NS2001` retained; the NS2002 depth guard is untouched.

Phase 6 (freeze) is complete on the tree: `spec-v0.2.md` is normative for
`edition = "0.2"`, `spec-v0.1.md` remains normative for `edition = "0.1"`, and
the lockstep `1.1.0` bump is applied across Cargo, npm, lockfiles, and CLI
version constants. The CLI commands `astra migrate`, `check --canonical-syntax`,
and `fmt --canonical` are wired in `crates/astra-cli/src/migrate_cmd.rs`. No
`1.1.0` tag or publish was cut. Remaining release blockers: two deferred corpus
cases (`scopes.ns`, `turbofish.ns`) and the principal-owned release steps.

## Release and npm state

- `1.1.1` is released. Tag `v1.1.1` points at public master `868007e`; release
  run `45750335485` is green on Windows/macOS/Linux. GitHub release has three
  archives with `.sha256` sidecars; all four npm packages are at `1.1.1` with
  provenance, and archive attestation verifies. The four npm trusted
  publishers were recreated with `--allow-publish` and validated by this
  publish. `v1.1.0` is a failed immutable tag with no published artifacts —
  its release stopped at a packaging smoke fixture that used edition `2026`.
- The npm token exposed during `1.0.0` has been revoked; the next principal
  action is only to re-create trusted publishers before a future release.

- `v1.0.1` is tagged (`3093343`) and the GitHub release has verified archives.
  npm has all four `1.0.1` packages with SLSA provenance. The old CI repo and
  the `NPM_TOKEN` secret are deleted.
- `v1.0.0` is immutable by policy, but its GitHub release reports
  `immutable: false`. Do not modify its assets.
- `1.0.0` has registry signatures but no npm provenance attestation; its GitHub
  archive attestations return 404. `1.0.1` has both.
- npm trusted publishing uses `--allow-publish` configs created with npm
  12.2.3+. `npm trust` still requires passkey re-authentication and returns
  `EOTP`; that is a principal action. Unvalidated configs expire after two
  days, so recreate them immediately before the next release.
- The token exposed during 1.0.0 was revoked; `NPM_TOKEN` stays deleted.
- The LSP `serverInfo` version still reports `2.0.0`; correct before shipping
  the server.
- No public CI workflow exists yet; do not add required status checks until the
  check names exist.

## Working tree snapshot

Private `master` is at the `1.1.2` prep commit, pushed to `ci`. Public `master`
has the sanitized `1.1.1` release; tag `v1.1.1` is immutable. Only one
lead/user-owned leftover remains:

- Modified: `.gitignore` (adds `test_comparison/`); leave it unstaged.

## Work order

1. `if let` / `while let` are done for edition `0.2` via desugaring to `match` /
   `while` (see `Docs/Report/Phase7-if-let.md`); no AST or backend changes.
2. Ranges (`a..b`, `a..=b`) are done for edition `0.2` with `for`-loop iteration,
   `str`/`bytes`/array slicing, and all three backends (see
   `Docs/Report/Phase7-ranges.md`). Index-position ranges lower to `.slice(...)`;
   the formatter preserves range spelling in `for` headers.
3. Bitwise operators (`& | ^ << >>`) are done for edition `0.2`, including
   expression `|` vs error-set `|` disambiguation and all three backends (see
   `Docs/Report/Phase7-bitwise.md`).
4. `char` and scalar literals are done for edition `0.2` with equality, literal
   patterns, and all three backends (see `Docs/Report/Phase7-char.md`).
5. Next: lambdas and the complete suffix table, each with a design note before
   implementation.
6. Add conformance and serialization-contract tests for each new construct.
7. Re-create npm trusted publishers immediately before the `1.1.2` release;
   the `1.1.1` configs are validated already but expire after two days.
8. The V2 C++20 orchestrator rewrite is a later track; see
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
- `npm test` is 243/243, `npm run ci` is green, and `npm run package` exits 0.
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
- `npm test`: 243/243 green.
- `npm run ci`: green; Tier-1 run `44963562468` green on Ubuntu/macOS/Windows.
- `npm run verify`: green.
- `npm run package`: green from an isolated package copy.
- Cross-platform package validation run `44964210902`: green on
  Ubuntu/macOS/Windows, including the isolated `@script:ts`/`@script:js` smoke
  and clean archive extraction (`astra 1.0.0`, `nopm 1.0.0`).
- Pre-bump archive sha256 (1.0.0 tree):
  - macOS `neo-astra-1.0.0-darwin-arm64.tar.gz`:
    `081d134b561e3bde229d8c0b351226f4687ba761db73df9035f0490bfdd1de47`
  - Windows `neo-astra-1.0.0-win32-x64.zip`:
    `aeacf97031c3178b8fdf17b67540cacf2f07a2e4151ba4b2f63fe5600e3491c4`
  - Linux `neo-astra-1.0.0-linux-x64.tar.gz`:
    `260906f55246a225bf41e53519b9ea63bcb8a3431a42e0c0b1697f8a1fcbaa3f`
- npm generated-package postinstall simulation: green; `astra`/`nopm` report
  `1.0.0` and the bridge/TypeScript payloads are present.

## Do not

- The bump is applied and approved; do not tag, push `origin` from the private
  tree, revoke tokens, or delete `NPM_TOKEN` without principal approval.
- Do not commit Kilo's uncommitted code, the pre-existing untracked files, or
  the active `Docs/Report/Task.md` log without review.
- `1.1.1` contains the v0.2 cleanup only. The `1.1.2` tree adds `if let` /
  `while let`, ranges, bitwise operators, and `char`; the remaining Phase 7 gaps
  are not shipped.
