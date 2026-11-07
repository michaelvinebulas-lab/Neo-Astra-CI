# Changelog

All notable changes to Neo Astra are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Apache-2.0 license, `NOTICE`, and `CONTRIBUTORS.md`.
- Public onboarding files: `README.md`, `INSTALL.md`, `SECURITY.md`,
  `CONTRIBUTING.md`, `.github/CODEOWNERS`, and `examples/hello`.
- npm provenance and GitHub release artifact attestations for future releases.

### Changed

- The public source mirror is append-only after the `v1.0.0` tag. Published
  tags and release assets are immutable.
- Public source comments are removed by the source mirror, including comments
  in `.ns` example sources.

### Work in progress

- Native Rust CLI phase 1: `check`, `fmt`, `ast`, and `explain` for a single
  source file.
- Native language server phase 1: stdio LSP with diagnostics and formatting.

## [1.0.0] - 2026-11-06

### Added

- The v0.1 language frontend: lexer, parser, AST, module resolver,
  capability and mode inference, type checker, error effects, and structured
  diagnostics.
- `core`, `app`, `web`, and `system` modes with least-privileged inference.
  `core` is stable; `app` is partial; `web` and `system` are preview.
- C17, JavaScript ESM, and TypeScript ESM backends, including inline
  `@script:js`, `@script:ts`, `@script:c`, and `@script:cpp` blocks.
- The `astra` CLI: `check`, `fmt`, `build`, `doctor`, `ast`, and `explain`.
- The `nova` development runtime: `run`, `eval`, `watch`, `repl`, `test`, and
  `version`.
- NoPM package management with path, Git, npm, Cargo, PyPI, NuGet, OCI, and
  `sys:` sources.
- Pinned Zig 0.13.0 C toolchain downloads with size and SHA-256 verification,
  a project-local cache, and `--no-download` failure guidance.
- Standalone release archives for Windows x64, Linux x64, and macOS arm64,
  plus the `neo-astra` npm meta package and three platform packages.
- Structured `TaskError` results at `join`, `task.canceled()`, task-handle
  copy-boundary rejection (`NS4019`), and the system-mode evaluation-order rule
  (`NS5007`).
- Project-local `.astra/` and `.tools/` state, with no elevation and no writes
  outside the project.

### Known limitations

- `app` mode is partial.
- `web` and `system` modes are preview.
- `high` mode is planned, not implemented.
- The language server, plugin host, Wasm execution backend, Web Worker code
  generation, and browser playground are not shipped in 1.0.
- Inline scripts for languages other than JS, TS, C, and C++ are rejected
  explicitly rather than emulated.
