# Release checklist

Use this checklist for every release. No step may be skipped.

## 1. Versioning (policy in `Docs/state.md`)

- All packages move in lockstep: root `package.json`, `[workspace.package]` in
  `Cargo.toml`, and every `orchestrator/packages/*/package.json`.
- `1.0.0` is cut when the V1 queue completes. After that: a new mode, backend,
  or language surface bumps the minor; fixes bump the patch; incompatible
  rework bumps the major.
- Move the `[Unreleased]` entries in `CHANGELOG.md` under the new dated
  version section and leave a fresh `[Unreleased]`.

## 2. Build and test

- `npm run bootstrap:verify` (install + build + test + fmt-check + clippy).
- `node scripts/check-error-codes.mjs` green: every emitted code has a spec row
  and every spec row is implemented or explicitly reserved.
- `npm run ci` green on Linux, macOS, and Windows (Tier-1 matrix).
- Confirm the Tier-1 run recorded in `Docs/state.md` is for the release commit.

## 3. Packaging

- `npm run build` produces `orchestrator/packages/*/dist`; every package's
  `main`/`bin` entry exists.
- `cargo build --release -p astra-compiler-api --bin astra-compiler`; the
  release binary answers an `initialize` JSON-RPC request.
- `npm run package` on each Tier-1 OS. The resulting
  `dist/neo-astra-<version>-<os>-<arch>/` contains one minified, bytecode
  `astra` executable (which also serves `astra nopm`), the `nopm` launcher, the
  stripped `astra-compiler` and `nova` binaries, and the pinned native
  `typescript/tsc`. Run `astra --version`, `astra nopm --version`, `astra
  check`, and a native and `@script:ts` build directly from the package with
  Node.js and npm absent from `PATH`.
- Confirm the archive and `.sha256` sidecar exist; record the digest in the
  release notes.
- Verify the bootstrap install on a clean Tier-1 runner
  (`npm run bootstrap:verify`); `bootstrap:check` is the report-only probe.

## 4. Documentation

- Add the release to `CHANGELOG.md`.
- Update `Docs/state.md` (queue state and a log row).
- Confirm `README.md`, `scripts/README.md`, and `orchestrator/README.md` match
  the shipped commands. Do not add status prose to `Docs/spec-v0.1.md`.

## 5. Tag and release

- Create an annotated tag `vX.Y.Z` on the release commit in the sanitized
  public source tree (`C:\Projects\Neo Astra-source`).
- Push the public source branch and tag to `origin`.
- Once a tag is pushed or published, it is immutable. Never force-move a
  published tag and never rewrite public history. Any change after the tag
  becomes the next patch or minor release; do not publish two trees under one
  version.
- Protect `master`: require pull requests, owner review, and green checks; block
  force pushes and branch deletion.
- The public `Release packages` workflow builds Linux, macOS, and Windows
  assets from the tag, creates the GitHub release, and uploads the archives
  plus `.sha256` files.
- When `NPM_TOKEN` is configured, the workflow publishes the platform npm
  packages first and the `neo-astra` meta package last. Without the secret it
  reports the skip and leaves the GitHub assets as the install channel.
