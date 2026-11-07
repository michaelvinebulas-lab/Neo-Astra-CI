# Priorities

Urgency order, not version order. Version detail stays in [`todo.md`](todo.md);
this file answers "what do we do next and who owns it".

Last updated: 2026-11-07.

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
- **Public source:** sanitized production tree; no tests, docs, or examples
  today. This is being changed by P0-3.

Do not describe V1 as "all modes complete". Core is complete; app is partial;
web and system are preview; high is future work.

## P0 - urgent, before promoting or advertising

### P0-1. Rotate the exposed npm credential

Owner: principal + lead.

- Revoke the token shared during setup.
- Create a replacement read/write token for the repository secret.
- Configure npm trusted publishing / OIDC for all four packages when possible.
- Remove the long-lived `NPM_TOKEN` dependency after OIDC works.
- Acceptance: old token revoked, secret updated, publish path works without a
  long-lived token, no token in any tracked file or workflow log.

### P0-2. Freeze the published release

Owner: lead + principal.

- `v1.0.0` is published; never force-push the tag or rewrite its commit.
- Any source change after 1.0.0 becomes `1.0.1`; never publish two trees under
  one version.
- Update `Docs/release-checklist.md` to remove the "move the tag" instruction.
- Enable repository rules/branch protection: pull requests required, owner
  review required, no force pushes, no deletion, no direct pushes to `master`.
- Acceptance: rules active; checklist says tags are immutable.

### P0-3. Make the public repo a real project

Owner: lead.

Public tree adds:

- `LICENSE`
- `NOTICE`
- `CONTRIBUTORS.md`
- `README.md`
- `INSTALL.md` (or `docs/install.md`)
- `CHANGELOG.md`
- `SECURITY.md`
- `CONTRIBUTING.md`
- `.github/CODEOWNERS`
- pull-request and issue templates
- `examples/`

Keep tests private unless the principal reverses that decision. Public source
comments are stripped, including `.ns` example comments; generated output
remains comment-free.

Acceptance: a new user can land on the repo, understand what is stable,
install it, run an example, and know how to contribute. License metadata
matches `package.json` and the Cargo workspace.

### P0-4. Tell the truth about modes and platforms

Owner: lead.

- Add a support matrix to README and install docs:
  - core: stable
  - app: partial
  - web: preview
  - system: preview
  - high: planned
  - supported hosts: Windows x64, Linux x64, macOS arm64
- Update roadmap language that currently reads as if all four modes are
  equally complete.
- Note that inline JS/TS/C/C++ works, other script targets are rejected, and
  unsupported targets fail explicitly.
- Acceptance: no public claim says "web complete", "system complete", or
  "all modes complete" while those surfaces are partial.

### P0-5. Release provenance

Owner: lead.

- `npm publish --provenance` with `id-token: write`.
- GitHub artifact attestations or Sigstore for release archives.
- Keep `.sha256` sidecars; explain that they detect corruption, not malicious
  replacement.
- Acceptance: npm provenance is visible for published packages; archive
  attestations can be verified from the release page.

## P1 - adoption-critical

### P1-1. Native Rust CLI

Owner: Kilo + lead review.

- Replace the Bun-compiled `astra` executable with a native Rust CLI.
- Cut the ~190 MB executable and ~420 MB npm install substantially.
- Keep command parity: `check`, `build`, `run`, `fmt`, `ast`, `explain`,
  `doctor`, `nopm` dispatch.
- JS/TS codegen and NoPM may fetch a pinned runtime on demand.
- Acceptance: native install is much smaller, no Node/Bun runtime needed for
  core commands, existing gate green.

### P1-2. LSP and editor support

Owner: Kilo (phase 1), lead wires CLI/tests.

- Phase 1: stdio language server; initialize, `didOpen`, `didChange`,
  `publishDiagnostics`, formatting, shutdown/exit.
- Use the existing compiler boundary for diagnostics; do not duplicate the
  type checker.
- Phase 2: completion, hover, go-to-definition, and project graph.
- Acceptance: a test client receives diagnostics for a broken file and clean
  results for a valid one.

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

## P2 - product expansion

- `high` mode (1.2).
- Browser playground (1.1), after native CLI and LSP.
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
  packaging, review and landing.
- **Kilo:** one isolated code slice at a time. Current order: native Rust CLI
  phase 1, then native LSP phase 1. Do not edit `Docs/**`, `scripts/**`,
  `.github/**`, `orchestrator/tests/**`, or `conformance/**`.

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
