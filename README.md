# Neo Astra

Neo Astra is a statically typed, multi-target language for web, app, and system
targets. Source files use `.ns`; the CLI is `astra`.

## Repository

- [`Docs/spec-v0.1.md`](Docs/spec-v0.1.md) - normative v0.1 language contract
- [`Docs/syntax-legend.md`](Docs/syntax-legend.md) - plain-language syntax and operator reference
- [`Docs/install.md`](Docs/install.md) - install, run, and upgrade instructions
- [`Docs/state.md`](Docs/state.md) - status, queue, and decisions
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

See [`Docs/install.md`](Docs/install.md) for prerequisites, native builds,
C-toolchain resolution, upgrades, and troubleshooting.

## Status

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
