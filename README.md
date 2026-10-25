# Neo Astra

Neo Astra is a statically typed, multi-target language for web, app, and system
targets. Source files use `.ns`; the CLI is `astra`.

## Repository

- [`Docs/spec-v0.1.md`](Docs/spec-v0.1.md) — normative v0.1 language contract
- [`Docs/syntax-legend.md`](Docs/syntax-legend.md) — plain-language syntax and operator reference
- [`Docs/architecture.md`](Docs/architecture.md) — confirmed decisions D1-D25
- [`Docs/Design/V1/`](Docs/Design/V1/) — V1 design baseline, including Nova and NoPM
- [`Docs/Agent/`](Docs/Agent/) — agent operating system
- [`orchestrator/`](orchestrator/) — TypeScript CLI, manifest, LSP, plugin, and codegen workspace
- [`crates/`](crates/) — Rust compiler core, capabilities, protocol, and backend boundaries

## Quick start

On a fresh machine with Node.js 22+ installed, the bootstrap command installs
the JavaScript dependencies, Rust toolchain, Rust components, and future Wasm
targets:

```sh
npm run bootstrap
```

Then build and verify:

```sh
npm run ci
npm run astra -- --help
npm run astra -- fmt . --check
```

The current scaffold can discover and parse `core.json` / `core.toml`, lex and
parse `.ns` source into an AST with byte spans and structured diagnostics, format
`.ns` source canonically through the Rust core while preserving `//` comments,
resolve modules and visibility, type-check the supported language subset, and
expose a JSON-RPC 2.0 compiler boundary through a native binary and raw Wasm
exports. `astra fmt` also formats `core.json` / `core.toml` canonically and
preserves TOML `#` comments. The `astra-nova` Rust crate executes checked HIR for
the managed development subset and provides the `nova` binary with `run`, `eval`,
`repl`, `watch`, `test`, `version`, inferred or explicit mode selection, text
diagnostics, JSON reports with the resolved mode, and manifest entry discovery.
The compiler infers least-privileged modes from annotations, built-ins, and the
fixed-point call graph, and reports `NS5006` for unsatisfiable `web`/`system`
joins. Error effects retain their construction and propagation sites, including
cross-module function surfaces. NoPM is complete through its first four
milestones: package model, target/mode resolution, deterministic graph, lockfile
I/O, content-addressed store, path/Git/npm/Cargo/PyPI/NuGet/OCI adapters,
`nopm lock`/`tree`/`install`/`search`/`audit`/`why`/`update`, and an optional
registry gateway with signing and federation. The first JavaScript ESM backend
slice emits readable, framework-free modules for the supported language subset,
and `astra build` writes runnable ESM for a single entry module. The backend
supports recoverable failures, `?` propagation, checked integer methods, eager
spawn/tasks with `join`/`cancel`, and line-level source maps. Project imports,
channels and a scheduler, the `arbitrary` integer policy, `.d.ts` artifacts, and
TypeScript generation remain staged. Inline `@script` foreign-language blocks are designed in
`Docs/Design/V1/10-inline-script-interop.md`; they are not implemented yet.
