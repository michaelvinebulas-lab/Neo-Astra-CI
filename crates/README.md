# Rust Core Crates

The Rust workspace owns parsing, resolution, type checking, capability inference,
IR, and the native/Wasm backend boundaries. TypeScript must never re-parse `.ns`
source.

| Crate | Purpose | Status |
|---|---|---|
| `astra-core` | Spans, diagnostics, AST, HIR, IR, types, lexer, parser, formatter, resolver, type checker | Lexer, parser, formatter with comment preservation, resolver, and type checker implemented |
| `astra-capabilities` | Mode ordering and capability inference boundary | `Mode` and `ModeRequirement` re-exported from `astra-core`; inference pending |
| `astra-compiler-api` | Versioned JSON-RPC protocol, native binary, and compiler trait | Parse, resolve, and type-check pipeline implemented; code generation pending |
| `astra-nova` | Typed-HIR development interpreter and `nova` CLI | Interpreter plus `run`/`eval`/`repl`/`watch`/`test` CLI |
| `astra-wasm` | Raw linear-memory Wasm entrypoint | Scaffold |
| `astra-codegen-c` | C backend boundary for native and Wasm | Stub |
| `astra-codegen-wasm` | Direct Wasm backend boundary | Stub |
| `astra-codegen-llvm` | Post-V1 LLVM backend boundary | Stub |

## Commands

```sh
cargo build --workspace
cargo test --workspace
cargo build -p astra-wasm --target wasm32-unknown-unknown
npm run test:boundary
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

The frontend through type checking and the `astra-nova` typed-HIR interpreter are
implemented. NoPM, code generation, and the target backends remain staged; the
crates define the shared types and boundaries so implementation can land in
order.
