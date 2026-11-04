# Neo Astra Orchestrator

The TypeScript orchestrator owns the CLI, manifest handling, LSP, plugins, and
the JavaScript/TypeScript code-generation boundary. It must never re-parse `.ns`
source; the Rust core is the single parser and semantic authority.

## Packages

| Package | Purpose | Status |
|---|---|---|
| `@neo-astra/manifest` | Locate and parse the project manifests (`core.*`, `build.*`) | Implemented, including strict key ownership |
| `@neo-astra/compiler-client` | Native JSON-RPC compiler client | Implemented, including multi-source `checkProject()` |
| `@neo-astra/cli` | `astra` command entrypoint | `check`, `fmt`, `build`, `doctor`, `ast`, and `explain` implemented |
| `@neo-astra/toolchain` | C toolchain policy, resolution ladder, cache validation, and `doctor` reporting | Implemented (no downloads in v1) |
| `@neo-astra/lsp` | Language server boundary | Stub |
| `@neo-astra/plugins` | Package-source and backend adapter contracts | Contracts implemented; no plugin host |
| `@neo-astra/nopm` | Universal package manager, adapters, lockfile, and store | Complete package model, lockfile/store, path/Git/npm/Cargo/PyPI/NuGet/OCI/sys adapters, and CLI add/remove/lock/tree/install/search/audit/why/update |
| `@neo-astra/registry` | Optional universal registry gateway | HTTP gateway with publish/versions/search/yank/audit, Ed25519 signing, and upstream mirroring implemented; production key/hosting hardening remains |
| `@neo-astra/codegen-js` | JavaScript ESM backend boundary | Implemented |
| `@neo-astra/codegen-ts` | TypeScript backend boundary | Implemented |
| `@neo-astra/playground` | Browser playground boundary | Stub (1.1) |

## Commands

```sh
npm install
npm run build
npm test
npm run astra -- --help
npm run astra -- check .
npm run astra -- fmt . --check
npm run astra -- ast .
npm run astra -- explain .
npm run astra -- build . --target native
npm run astra -- doctor
```

`check` performs manifest discovery and validation. `fmt` sends `.ns` source to
the Rust core over JSON-RPC and preserves comments; it also formats `core.json`
/ `core.toml`, preserving TOML `#` comments. `ast` prints the parsed tree and
`explain` prints the mode-inference result, both from the Rust compiler.
`build` runs the selected backend (`js`, `ts`, `c`, or `native`) and writes the
artifact; `--json` returns a machine-readable report. `doctor` prints the C
toolchain resolution ladder and the chosen compiler.

The CLI discovers `target/debug/astra-compiler` automatically; set
`ASTRA_COMPILER_BIN` or pass `--compiler <path>` to select a different compiler
binary. `--compiler` is not the C compiler: use `ASTRA_CC` or the build
manifest's `toolchain.c` for that. NoPM is the universal package manager for
Nova and compiled builds; its package model, lockfile, store, adapters, and
registry gateway live in `@neo-astra/nopm` and `@neo-astra/registry`. The
compiler client exposes the additive multi-source `checkProject()` method for
project-wide checking.

Tooling diagnostics use the `NS9xxx` range: `NS9003` missing compiler binary,
`NS9004` compiler process failure, `NS9005` invalid compiler response, `NS9006`
JSON-RPC request failure, `NS9007` invalid UTF-8 source, `NS9008` source file
access failure, and `NS9101`/`NS9102`/`NS9103` toolchain policy, pin, and
`ASTRA_CC` failures.

## Rules

- One parser: Rust.
- One semantic contract across targets.
- No hidden runtime in JavaScript output.
- Manifest files are strict: exactly one of `core.json` or `core.toml`, and
  optionally one of `build.json` or `build.toml`.
