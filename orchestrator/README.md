# Neo Astra Orchestrator

The TypeScript orchestrator owns the CLI, manifest handling, LSP, plugins, and
JavaScript/TypeScript code generation. It must never re-parse `.ns` source; the
Rust core is the single parser and semantic authority.

## Packages

| Package | Purpose | Status |
|---|---|---|
| `@neo-astra/manifest` | Locate and parse `core.json` / `core.toml` | Implemented |
| `@neo-astra/compiler-client` | Native JSON-RPC compiler client | Implemented, including multi-source `checkProject()` |
| `@neo-astra/cli` | `astra` command entrypoint | `check` and `fmt` implemented |
| `@neo-astra/lsp` | Language server boundary | Stub |
| `@neo-astra/plugins` | Package-source and backend adapter contracts | Stub |
| `@neo-astra/nopm` | Universal package manager, adapters, lockfile, and store | Complete package model, lockfile/store, path/Git/npm/Cargo/PyPI/NuGet/OCI adapters, and CLI lock/tree/install/search/audit/why/update |
| `@neo-astra/registry` | Optional universal registry gateway | HTTP gateway with publish/versions/search/yank/audit, Ed25519 signing, and upstream mirroring implemented; production key/hosting hardening remains |
| `@neo-astra/codegen-js` | JavaScript ESM backend boundary | Stub |
| `@neo-astra/codegen-ts` | TypeScript backend boundary | Stub |

## Commands

```sh
npm install
npm run build
npm test
npm run astra -- --help
npm run astra -- check .
npm run astra -- fmt . --check
```

`check` performs manifest discovery and parsing. `fmt` sends `.ns` source to the
Rust core over JSON-RPC and preserves comments; it also formats `core.json` /
`core.toml`, preserving TOML `#` comments. The CLI discovers
`target/debug/astra-compiler` automatically; set `ASTRA_COMPILER_BIN` or pass
`--compiler <path>` to select a different binary. Code generation and semantic
checking land in later milestones. NoPM is the universal package manager for
Nova and compiled builds; its package model, lockfile, store, adapters, and
registry gateway live in `@neo-astra/nopm` and `@neo-astra/registry`. The
compiler client exposes the additive multi-source `checkProject()` method for
project-wide checking.

Tooling diagnostics use the `NS9xxx` range: `NS9003` missing compiler binary,
`NS9004` compiler process failure, `NS9005` invalid compiler response, `NS9006`
JSON-RPC request failure, `NS9007` invalid UTF-8 source, and `NS9008` source file
access failure.

## Rules

- One parser: Rust.
- One semantic contract across targets.
- No hidden runtime in JavaScript output.
- Manifest files are strict and exactly one of `core.json` or `core.toml` is allowed.
