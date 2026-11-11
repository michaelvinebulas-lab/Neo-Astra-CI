# Release checklist

Use this checklist for every release. No step may be skipped.

## 1. Versioning (policy in `Docs/state.md`)

- All packages move in lockstep: root `package.json`, `[workspace.package]` in
  `Cargo.toml`, and every `orchestrator/packages/*/package.json`.
- `1.0.0` is cut when the V1 queue completes. After that: a new mode, backend,
  or language surface bumps the minor; fixes bump the patch; incompatible
  rework bumps the major.
- `1.0.1` is the native Rust `astra`/`nopm` replacement. Do not tag it until
  slices 3a, 3b, and 4 are accepted and native parity is 12/12. The language
  stays v0.1; do not inject the v0.2 cleanup into this patch.
- Move the `[Unreleased]` entries in `CHANGELOG.md` under the new dated
  version section and leave a fresh `[Unreleased]`.

## 2. Build and test

- `npm run bootstrap:verify` (install + build + test + fmt-check + clippy).
- `node scripts/check-error-codes.mjs` green: every emitted code has a spec row
  and every spec row is implemented or explicitly reserved.
- `npm run test:native-parity` is 12/12. For `1.0.1`, the NoPM path lifecycle,
  `astra nopm`/`nopm` dispatch identity, and no-Node core/NoPM checks must all
  be green; only a genuinely unrelated case may be allowed to remain red, and
  that requires a principal decision.
- `cargo test -p astra-nopm` includes the required lifecycle unit tests:
  package IDs, manifest-hash normalization, path containment, lockfile
  format/current checks, store writing, and add/remove rollback.
- `npm run ci` green on Linux, macOS, and Windows (Tier-1 matrix).
- Confirm the Tier-1 run recorded in `Docs/state.md` is for the release commit.

## 3. Packaging

- `npm run build` produces `orchestrator/packages/*/dist`; every package's
  `main`/`bin` entry exists.
- `cargo build --release -p astra-compiler-api --bin astra-compiler`; the
  release binary answers an `initialize` JSON-RPC request.
- `npm run package` on each Tier-1 OS. For `1.0.1` and later, the resulting
  `dist/neo-astra-<version>-<os>-<arch>/` contains the Cargo-built native
  `astra` and `nopm` binaries, the stripped `astra-compiler` and `nova`
  binaries, and the pinned native `typescript/tsc`. For the historical `1.0.0`
  package, the `astra` binary is the Bun-compiled executable and `nopm` is a
  launcher; do not use that layout for `1.0.1`. Run `astra --version`,
  `astra nopm --version`, `nopm --version`, `astra check`, a native build, and
  a `@script:ts` build directly from the package with Node.js and npm absent
  from the native/core paths.
- Confirm the archive and `.sha256` sidecar exist; record the digest in the
  release notes.
- Verify the bootstrap install on a clean Tier-1 runner
  (`npm run bootstrap:verify`); `bootstrap:check` is the report-only probe.

## 4. Documentation

- Add the release to `CHANGELOG.md` and `public/CHANGELOG.md`.
- Update `Docs/state.md` (queue state and a log row) and `context.md` (handoff
  snapshot).
- Confirm `README.md`, `INSTALL.md`, `SECURITY.md`, `CONTRIBUTING.md`, and
  `examples/` match the shipped commands and the real support matrix: core
  stable, app partial, web/system preview, high planned.
- Confirm `LICENSE`, `NOTICE`, and `CONTRIBUTORS.md` are present and that
  `package.json`, `Cargo.toml`, and generated npm package metadata all report
  `Apache-2.0`.
- Confirm the public source mirror strips comments from every source file,
  including `.ns` examples. Documentation carries the explanation.

## 5. Tag and release

- Create an annotated tag `vX.Y.Z` on the release commit in the sanitized
  public source tree (`C:\Projects\Neo Astra-source`).
- Push the sanitized `source` branch to `origin/source`, open a pull request,
  merge it to `master`, then push the release tag to `origin`. Never push
  directly to `master`; the branch ruleset requires a pull request.
- Once a tag is pushed or published, it is immutable. Never force-move a
  published tag and never rewrite public history. Any change after the tag
  becomes the next patch or minor release; do not publish two trees under one
  version.
- Protect `master`: require pull requests; block force pushes, deletion, and
  non-linear history. The current single-maintainer ruleset permits owner
  self-merge with zero approvals; add approvals and green status checks when a
  second maintainer and public CI exist. Details are in
  `Docs/github-trust.md`.
- Protect release tags: block tag deletion and updates. Once a tag is public it
  is immutable; fixes go into the next patch or minor release.
- The public `Release packages` workflow builds Linux, macOS, and Windows
  assets from the tag, creates the GitHub release, and uploads the archives
  plus `.sha256` files.
- The workflow attests the archives with
  `actions/attest-build-provenance@v2` and publishes npm packages with
  `--provenance`. Verify both after publishing; see
  `Docs/github-trust.md`.
- The workflow publishes the platform npm packages first and the `neo-astra`
  meta package last through npm trusted publishing (OIDC). It must not read
  `NPM_TOKEN`; the trusted relationships are configured once per package with
  `npm trust github`, using `release.yml` and
  `luminousvalentine-cell/Neo-Astra`.
- The trusted relationships were added on 2026-11-07 but are unvalidated until
  the first successful OIDC publish. `npm trust list` currently requires
  passkey re-authentication and returns `EOTP`; record that as a principal
  action. Refresh the relationships immediately before the release because
  unvalidated configurations expire after two days.
- Keep the old `NPM_TOKEN` secret until the first successful OIDC release, then
  delete it. Revoke the exposed 1.0.0 token immediately; see
  `Docs/github-trust.md`.
- Verify npm provenance and archive attestations for `1.0.1`; `1.0.0` has
  neither.
