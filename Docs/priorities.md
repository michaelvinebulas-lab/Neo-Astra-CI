# Priorities

Urgency order, not version order. Version detail stays in [`todo.md`](todo.md);
this file answers "what do we do next and who owns it".

Last updated: 2026-11-11.

## Current gate

`1.0.1` is blocked on the native Rust CLI/NoPM port. Slices 1, 2a, 2b, and 2c
are landed; slice 3a is open after two rejected handbacks; slices 3b and 4 are
not started. Native parity is 11/12, with only the JS build case red. Do not
tag, publish, or describe `1.0.1` as shipped until parity is 12/12 and the
release checklist is green.

## Status truth

- **Core:** complete and the only mode that should be described as stable.
- **App:** partial; usable parts work, but it is not a completed surface.
- **Web:** preview; capabilities and backend pieces exist, the library surface
  is not complete.
- **System:** preview; same caveat as web.
- **High:** planned, not implemented.
- **Targets:** C17, JavaScript ESM, and TypeScript ESM are the shipped
  backends; Nova is the development runtime.
- **Distribution:** public source, three release archives, four npm packages.
  Windows x64, Linux x64, and macOS arm64 only.
- **Public source:** sanitized production tree with onboarding files and
  examples; tests and private docs stay out. Release tags remain immutable.

Do not describe V1 as "all modes complete". Core is complete; app is partial;
web and system are preview; high is future work.

## P0 - urgent, before promoting or advertising

### P0-1. Rotate the exposed npm credential

Owner: principal + lead.

- Revoke the token shared during setup. The 1.0.0 token is still exposed.
- The release workflow now uses npm trusted publishing / OIDC for all four
  packages; the trusted relationships were added on 2026-11-07 but are not
  validated until the first successful OIDC publish.
- `npm trust list` currently requires passkey re-authentication and returns
  `EOTP`; record that as a principal action, do not bypass it.
- Configure or refresh the trusted relationships immediately before the
  `1.0.1` release, because unvalidated configurations expire after two days.
- Keep `NPM_TOKEN` only as a rollback until the first successful OIDC publish,
  then delete it. Never put a token in a tracked file or workflow log.
- Acceptance: old token revoked, OIDC publish works without a long-lived token,
  and `NPM_TOKEN` is deleted after the verified release.

### P0-2. Freeze the published release

Owner: lead + principal.

- `v1.0.0` is published; never force-push the tag or rewrite its commit.
- Any source change after 1.0.0 becomes `1.0.1`; never publish two trees under
  one version. `1.0.1` is not tagged yet.
- The release checklist now states that published tags are immutable.
- The branch and tag rulesets are active: pull requests required, owner review
  required, no force pushes, no deletion, no direct pushes to `master`, and no
  tag updates or deletion. Raise the approval count when a second maintainer
  exists.
- The public `v1.0.0` GitHub release still reports `immutable: false`; do not
  change its assets. Enable release immutability for future releases when the
  repository setting is available.
- Acceptance: rules active; checklist says tags are immutable; no public
  history rewrite.

### P0-3. Make the public repo a real project

Owner: lead. Status: implemented; keep the public mirror current.

The public tree now carries `LICENSE`, `NOTICE`, `CONTRIBUTORS.md`,
`README.md`, `INSTALL.md`, `CHANGELOG.md`, `SECURITY.md`, `CONTRIBUTING.md`,
`.github/CODEOWNERS`, pull-request and issue templates, and `examples/`.

Keep tests private unless the principal reverses that decision. Public source
comments are stripped, including `.ns` example comments; generated output
remains comment-free.

Acceptance: a new user can land on the repo, understand what is stable,
install it, run an example, and know how to contribute. License metadata
matches `package.json` and the Cargo workspace.

### P0-4. Tell the truth about modes and platforms

Owner: lead. Status: implemented; keep the public support matrix current.

- Public README and install docs carry the support matrix:
  - core: stable
  - app: partial
  - web: preview
  - system: preview
  - high: planned
  - supported hosts: Windows x64, Linux x64, macOS arm64
- Roadmap language distinguishes built surfaces from planned ones.
- Inline JS/TS/C/C++ works; other script targets are rejected explicitly.
- Acceptance: no public claim says "web complete", "system complete", or
  "all modes complete" while those surfaces are partial.

### P0-5. Release provenance

Owner: lead.

- `npm publish --provenance` with `id-token: write`.
- GitHub artifact attestations or Sigstore for release archives.
- Keep `.sha256` sidecars; explain that they detect corruption, not malicious
  replacement.
- `1.0.0` has registry signatures but no npm provenance attestation, and its
  GitHub assets return 404 from the attestation API. Do not claim provenance
  for 1.0.0; verify it on `1.0.1` and later.
- Acceptance: npm provenance is visible for published packages; archive
  attestations can be verified from the release page.

## P1 - adoption-critical

### P1-1. Native Rust CLI

Owner: Kilo + lead review. Status: in progress; `1.0.1` blocker.

- Replace the Bun-compiled `astra` executable with a native Rust CLI.
- Cut the ~190 MB executable and ~420 MB npm install substantially.
- Keep command parity: `check`, `build`, `run`, `fmt`, `ast`, `explain`,
  `doctor`, and `nopm` dispatch.
- JS/TS codegen and NoPM may fetch a pinned runtime on demand; baseline CLI and
  NoPM flows must not require Node, npm, or Bun.
- Current checkpoints:
  - `7ca69be` - slice 1, core project commands.
  - `778bbc5` - slices 2a/2b, toolchain/doctor and native/C executable build.
  - `501e91f` - slice 2c, pinned Zig and library artifacts.
  - slice 3a - native NoPM baseline; open, two handbacks rejected.
  - slice 3b - remote adapters/registry/system tools; blocked on 3a.
  - slice 4 - JS/TS bridge, packaging, CI; blocked on 3a/3b.
- Native parity is 11/12; only the JS build case remains red. The current NoPM
  parity fixture is too narrow: it only covers the hardcoded
  `local-utils`/`web` path project.
- Acceptance: native install is much smaller, no Node/Bun runtime needed for
  core commands, native parity 12/12, and all Rust/npm gates green.

### P1-2. LSP and editor support

Owner: Kilo (phases 1-3), lead wires CLI/tests.

- Phases 1-3 are implemented in `crates/astra-lsp`: stdio diagnostics and
  formatting, project graph/hover/definitions/completion, document symbols,
  signature help, references, highlights, workspace symbols, and symbol rename.
  The server is not in the 1.0 package and still needs packaging/CI and editor
  integration.
- Use the existing compiler boundary for diagnostics; do not duplicate the
  type checker.
- Field rename remains refused until constructor/pattern edits are modeled.
- Acceptance: a test client receives diagnostics for a broken file and clean
  results for a valid one, and a packaged server is available.

### P1-3. Public examples and docs

Owner: lead + Kilo for example code.

- At least: hello world, CLI tool, package dependency, native build, JS/TS
  build, and one core concurrency example.
- Document install, first project, build targets, NoPM, and troubleshooting.
- Acceptance: every example builds from a clean checkout and from the
  packaged toolchain.

### P1-4. Package ecosystem

Owner: Kilo + lead.

- `nopm publish` for Neo Astra packages.
- Define package naming, metadata, and a first-party index or registry gateway.
- Document how to consume npm/Cargo/PyPI/Git/path packages.
- Acceptance: a library can be packed, published, installed, and locked by a
  second project.

### P1-5. Standard library audit and completion

Owner: lead audit, Kilo implementation slices.

- Audit spec section 13 against implementation for core, app, web, and system.
- Finish core and app gaps first.
- For web/system, either implement the advertised API or reject it with a
  clear diagnostic and mark it preview.
- Acceptance: a table maps every advertised standard-library surface to
  implemented, partial, or rejected.

### P1-6. Install channels and host coverage

Owner: lead.

- Homebrew tap, Winget manifest, Scoop bucket.
- Expand to macOS x64, Linux arm64, and Windows arm64 where runners and
  toolchains exist.
- Acceptance: at least two non-npm install paths work; supported hosts are
  documented.

### P1-7. Language v0.2 syntax cleanup

Owner: reviewer (error-model/generic-call RFCs), lead (integration), principal
(decisions). Status: Phase 0 complete; implementation not started; non-normative.

- Freeze the error model (`-> T ! E`, `?`, `fail`, `Ok`/`Err`) and demote
  `Result<T, E>` to a library/FFI type. RFC A decisions are closed.
- Freeze expression-position generics as turbofish `::<...>`. RFC B decisions
  are closed.
- Replace scoped `{@mode:...;}` / `{@memory:...;}` with `@mode(...) { ... }` /
  `@memory(...) { ... }`.
- Use commas for lists; state line continuation by delimiter kind; fix
  tuple-index lexing; deprecate `/%`, `spawn ||`, and the special `///`
  meaning without removing parser support.
- Deprecate the aliases rather than removing parser support; `var` is deprecated
  in favour of `let`/`let mut`. Add the deny-by-default `canonical-syntax` lint
  and `fmt --canonical`; `astra migrate` is the only rewrite path.
- Ship the syntax cleanup as `1.1.0` and gate syntax with
  `edition = "0.2"`; keep both editions and use `astra migrate` for source
  migration.
- Add `if let`/`while let`, ranges, bitwise operators, `char`, and lambdas only
  after the grammar is frozen.
- Phase 0 evidence: accepted plan/RFCs, `Docs/spec-v0.2.md` skeleton,
  `tools/grammar-v0.2` focused LR(1)/Pager harness (4/4 tests), and a
  non-loaded `conformance/v0.2` corpus with migration pairs.
- Acceptance: a frozen v0.2 spec, a parser-generator run with zero conflicts,
  parser diagnostics and formatter rewrites for deprecated or edition-rejected
  syntax, a conformance case per changed construct, and 10-20 realistic
  programs parsed before freeze.
- The native CLI slices continue against v0.1 until this item freezes the
  language; do not move parser, checker, codegen, and CLI at the same time.

The concrete phases, estimates, AST-freeze boundary, and gates are in
[`Report/Syntax-v0.2.md` §5](Report/Syntax-v0.2.md#5-implementation-plan).

The native CLI slices continue against v0.1 until this item freezes the
language; do not move parser, checker, codegen, and CLI at the same time.

## P2 - product expansion

- `high` mode (1.3).
- Browser playground (1.2), after native CLI and LSP.
- Rust, Go, Lua, Ruby, and Java targets.
- Managed WebAssembly target.
- First-party machine-code backend.
- Debugger and profiler.
- Performance work, fuzzing expansion, and a security audit.

## P3 - later

- Plugin host, Web Workers, additional script embeddings, advanced GC work,
  and cross-target triples. Keep the existing `Docs/todo.md` deferred list as
  the source of truth.

## Parallel lanes

- **Lead:** P0 trust/public repo, release provenance, docs/examples, platform
  packaging, review and landing. Current lead action: hold the tree at slice
  3a, review Kilo's next handback, and keep `context.md`/status docs current.
- **Kilo:** one isolated code slice at a time. Current order: native CLI slice
  3a (NoPM baseline), then 3b (remote adapters/registry/system tools), then 4
  (JS/TS bridge, packaging, CI), then release validation. The LSP phases 1-3
  are landed. Do not edit `Docs/**`, `scripts/**`, `.github/**`,
  `orchestrator/tests/**`, or `conformance/**` unless the active slice grants it.

## Decisions

1. **License:** Apache-2.0 with `NOTICE` and `CONTRIBUTORS.md`. This is a
   permissive license: companies may use, modify, and distribute Neo Astra
   without a contribution obligation. Commercial contribution and maintenance
   are a community expectation stated in `CONTRIBUTING.md`, not a license
   condition. Do not describe Apache-2.0 as requiring companies to contribute.
2. **Public source comments:** strip all source comments, including `.ns`
   example comments. Documentation carries the explanation. Generated C/JS/TS
   output remains comment-free.
3. **LSP architecture:** native Rust server in `crates/astra-lsp`, using the
   existing compiler boundary. Phase 1 is stdio diagnostics plus formatting;
   project graph, hover, definitions, and completion follow.
4. **Parallel ownership:** Kilo owns both the native Rust CLI and the native
   LSP phase 1, in separate crates. The lead owns public docs, license,
   packaging, review, and tests.
