# Rust Core Crates

The Rust workspace owns parsing, resolution, type checking, capability inference,
IR, the C backend, the Nova interpreter, and the compiler API. TypeScript must
never re-parse `.ns` source.

| Crate | Purpose | Status |
|---|---|---|
| `astra-core` | Spans, diagnostics, AST, HIR, IR, types, lexer, parser, formatter, resolver, type checker, capability inference | Implemented |
| `astra-capabilities` | Mode ordering and capability inference boundary | Re-exported from `astra-core`; implemented |
| `astra-compiler-api` | Versioned JSON-RPC protocol, native binary, and compiler trait | `check`, `check_project`, `compile`, `compile_project`, `format`, `ast`, and `explain` implemented |
| `astra-nova` | Typed-HIR development interpreter and `nova` CLI | Interpreter plus `run`/`eval`/`repl`/`watch`/`test` CLI |
| `astra-wasm` | Raw linear-memory Wasm entrypoint for the compiler API | Implemented; built for `wasm32-unknown-unknown` by `npm run test:boundary` |
| `astra-codegen-c` | C17 backend for native executables, static/shared libraries, FFI, and inline scripts | Implemented |

V1 uses the C toolchain for machine code. A first-party LLVM backend and a
direct Wasm code generator are post-1.0 work and are not part of this workspace,
so no scaffold ships.

## Commands

```sh
cargo build --workspace
cargo test --workspace
cargo build -p astra-wasm --target wasm32-unknown-unknown
npm run test:boundary
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

The frontend through type checking, the C backend, the Nova typed-HIR
interpreter, the compiler API, and NoPM are implemented. The JavaScript and
TypeScript ESM backends live in `orchestrator/packages/codegen-js` and
`orchestrator/packages/codegen-ts`.
