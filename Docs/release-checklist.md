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
- `npm run ci` green on Linux, macOS, and Windows (Tier-1 matrix).
- Confirm the Tier-1 run recorded in `Docs/state.md` is for the release commit.

## 3. Packaging

- `npm run build` produces `orchestrator/packages/*/dist`.
- `cargo build --release -p astra-compiler-api --bin astra-compiler`.
- Verify the bootstrap script installs from a clean machine
  (`npm run bootstrap:check`).

## 4. Documentation

- Add the release to `CHANGELOG.md`.
- Update `Docs/state.md` (queue state and a log row).
- Confirm `README.md`, `scripts/README.md`, and `orchestrator/README.md` match
  the shipped commands. Do not add status prose to `Docs/spec-v0.1.md`.

## 5. Tag and publish

- Create an annotated tag `vX.Y.Z`.
- Push the tag; CI publishes artifacts.
- Write release notes from the changelog section.
