# Release checklist (HARD-001 B)

Use this checklist for every release. No step may be skipped.

## 1. Versioning
- Bump `version` in root `package.json` and `[workspace.package]` in `Cargo.toml`.
- Update the version string in `orchestrator/packages/*/package.json`.
- Add a dated section to `CHANGELOG.md` moving `[Unreleased]` entries under it.

## 2. Build and test
- `npm run bootstrap:verify` (install + build + test + fmt-check + clippy).
- `npm run ci` green on Linux, macOS, and Windows (Tier-1 matrix).
- `cargo test --workspace` green.
- `npm run test:boundary` green.

## 3. Packaging
- `npm run build` produces `orchestrator/packages/*/dist`.
- `cargo build --release -p astra-compiler-api --bin astra-compiler`.
- Verify the bootstrap script installs from a clean machine (run `npm run bootstrap:check`).

## 4. Documentation
- Update `Docs/Agent/state-log.md` "Next action" and `Docs/Agent/todo.md`.
- Confirm `README.md`, `scripts/README.md`, and `orchestrator/README.md` match the shipped commands.

## 5. Tag and publish
- Create an annotated tag `vX.Y.Z`.
- Push the tag; CI publishes artifacts.
- Write release notes from the changelog section.
