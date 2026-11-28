# Neo Astra

Neo Astra is a statically typed, multi-target language for web, app, and system
targets. Source files use `.ns`; the CLI is `astra`.

## Repository

- [`Docs/spec-v0.1.md`](Docs/spec-v0.1.md) - normative v0.1 language contract
- [`Docs/syntax-legend.md`](Docs/syntax-legend.md) - plain-language syntax and operator reference
- [`Docs/install.md`](Docs/install.md) - install, run, and upgrade instructions
- [`Docs/state.md`](Docs/state.md) - status, queue, and decisions
- [`context.md`](context.md) - current handoff snapshot for a new session
- [`Docs/roadmap.md`](Docs/roadmap.md) - what is built, what is next, and the versioning scheme
- [`Docs/todo.md`](Docs/todo.md) - open tasks, grouped by version
- [`Docs/archive/`](Docs/archive/) - frozen history: design baseline, proposals, earlier status docs
- [`Docs/Agent/`](Docs/Agent/) - agent operating system
- [`orchestrator/`](orchestrator/) - TypeScript CLI, manifest, plugin contracts, and codegen workspace
- [`crates/`](crates/) - Rust compiler core, capabilities, protocol, and backend boundaries

## Quick start

On a fresh machine with Node.js 22+ installed:

```sh
npm run bootstrap
npm run bootstrap:verify
npm run astra -- --help
npm run astra -- fmt . --check
```

Build a standalone host-native package that does not need Node.js or npm:

```sh
npm run package
dist/neo-astra-*/bin/astra --version
```

See [`Docs/install.md`](Docs/install.md) for prerequisites, packaged
installations, native builds, C-toolchain resolution, upgrades, and
troubleshooting.

## Status

> **Development note (2026-11-11):** `1.0.0` is the published release. The
> native Rust `astra`/`nopm` replacement for `1.0.1` is in progress: slices
> 1-2c are landed, slice 3a is under review after two rejected handbacks, and
> slices 3b/4 are pending. Native parity is 11/12. Nothing in this checkout
> should be described as a shipped `1.0.1` until the release checklist is
> green. See [`context.md`](context.md) for the exact tree state.

The `1.0.0` checkout provides:

- The two-manifest project model (`core.json` / `core.toml`, optional
  `build.json` / `build.toml`), strict key ownership, canonical formatting, and
  the package manifest edit API.
- The Rust frontend: lexer, parser, AST, module resolver, capability/mode
  inference, type checker, error effects, the system-mode evaluation-order
  rule (`NS5007`), the task-handle copy-boundary guard (`NS4019`), and a
  JSON-RPC compiler boundary exposed as a native binary and raw Wasm exports.
- The `astra` CLI: `check`, `fmt`, `build`, `doctor`, `ast`, and `explain`.
  `check` validates both manifests, runs the frontend over the project entry,
  and validates `ffi` mappings; the other commands cover formatting, all four
  targets, JavaScript/TypeScript/C/native code generation,
  executable/staticlib/cdylib emission, and `--json` reports.
  `ASTRA_COMPILER_BIN` and `--compiler` select the compiler binary;
  `ASTRA_CC` and `toolchain.c` select the C toolchain.
- Pinned C toolchains: verified Zig 0.13.0 downloads for Linux, macOS, and
  Windows, a project-local cache, `zig cc`/`zig ar`, lockfile records, and
  `--no-download` with manual instructions.
- A standalone package: one host-native `astra` executable with
  `astra nopm`, a tiny `nopm` launcher, the stripped Rust compiler and Nova
  runtime, and the pinned native TypeScript compiler. It runs without Node.js,
  npm, Cargo, or a source checkout; NoPM downloads other package managers only
  when a project requests them.
- The C backend: managed GC, manual memory, regions, FFI, generics, packages,
  native libraries, export attributes, per-task heaps, scoped OS-thread tasks,
  task cancellation, and channel messages. Inline `@script:js`,
  `@script:ts`, `@script:c`, and `@script:cpp` blocks are emitted with boundary
  validation; Python scripts are parsed but not generated.
- The JavaScript and TypeScript backends: readable ESM/TS output, recoverable
  failures, `?` propagation, checked integer methods, tasks, linked
  multi-module projects, source maps, and the `@export_as` ABI surface.
- The `nova` development runtime: `run`, `eval`, `watch`, `repl`, `test`, and
  `version`, with inferred or explicit modes and JSON reports.
- NoPM: package model, deterministic graph, lockfile/store, path/Git/npm/
  Cargo/PyPI/NuGet/OCI adapters, `add`/`remove`/`lock`/`tree`/`install`/
  `search`/`audit`/`why`/`update`, a `sys:` system-package source, and the
  optional signed registry gateway.

Still staged for a later release: the language server and plugin host, the Wasm
execution backend, Web Worker code generation, and the browser playground UI.
Inline scripts for languages other than JS, TS, C, and C++ are not generated in
v1; unsupported targets reject explicitly rather than emulating different
semantics.
