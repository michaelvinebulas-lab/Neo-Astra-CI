# V1 Targets and Backends

**Status:** V1 design proposal  
**Depends on:** D2, D6, D7, D10, D19  
**Related:** [`02-compiler-design.md`](./02-compiler-design.md), [`03-runtime-and-memory.md`](./03-runtime-and-memory.md)

---

## 1. Target model

A **target** is what the compiler emits. A **mode** is what the program is allowed to use. They are not interchangeable.

```text
target = output platform and format
mode   = compile-time capability profile
```

A target may support only some modes. The compiler must reject incompatible combinations explicitly.

---

## 2. V1 support tiers

| Tier | Target | Status |
|---|---|---|
| 1 | JavaScript ESM | Release-blocking |
| 1 | TypeScript ESM | Release-blocking |
| 1 | `x86_64-pc-windows-msvc` | Release-blocking |
| 1 | `x86_64-unknown-linux-gnu` | Release-blocking |
| 1 | `aarch64-apple-darwin` | Release-blocking |
| 2 | `wasm32-web` | Supported, best effort |
| 2 | `wasm32-wasi` | Supported, best effort |
| 2 | `aarch64-unknown-linux-gnu` | Supported, best effort |
| 2 | `x86_64-apple-darwin` | Supported, best effort |
| 3 | Other C-ABI targets | Community or later |

The tier is part of the build metadata and diagnostics.

---

## 3. Backend compatibility matrix

| Backend | Compatible modes | Manual memory | Notes |
|---|---|---|---|
| JavaScript | `core`, `app`, `web` | No | Host GC |
| TypeScript | `core`, `app`, `web` | No | Host GC |
| C → native | `core`, `app`, `system` | Yes | C17 plus `clang` |
| C → Wasm | `core`, `app`, `system` | Yes, with Wasm constraints | JS glue or WASI |
| LLVM | all modes | Yes | Post-V1 |
| PHP | `app`, `web` later | No | Post-V1 |

A backend must declare unsupported features before code generation.

### 3.1 Inline script language support

Inline `@script` blocks are target-bound. The full contract is in
[`10-inline-script-interop.md`](./10-inline-script-interop.md).

| Script language | `js` / `typescript` | C native | Wasm | Status |
|---|---|---|---|---|
| `js` | Yes | No | No | First slice |
| `ts` | Yes, with a pinned transpiler | No | No | Planned |
| `c` | No | Yes, C17 + `clang` | Deferred | Second slice |
| `cpp` | No | Yes, behind a C ABI shim | Deferred | Second slice |
| `python` | No | Deferred, embedded CPython | No | Post-V1 |
| `java` | No | No | No | Requires a JVM backend |

An unsupported combination is a compile-time error. The compiler must not fall
back to `eval`, a host subprocess, or a silently ignored block.

---

## 4. JavaScript backend

### 4.1 Output shape

- ES modules only.
- No framework runtime.
- No global patches.
- No hidden class system.
- No monolithic helper import.
- Readable function and type names.
- Source maps for debugging.
- Value-bearing public/exported declarations map to ESM exports; normal and private declarations stay module-local.
- Type-only declarations and type-only imports/exports are erased.
- Named, default, namespace, and re-export forms map directly to ESM syntax.
- Project-path `import path` / `import path as alias` maps to a namespace import.
- No runtime export wrapper or synthetic module object.
- Inline `@script:js` blocks emit as real ESM modules with explicit imports and
  named exports, never as `eval` or `new Function` strings.

The output should feel like intentional modern JavaScript, not a decompiled IR dump.

### 4.2 Runtime helpers

Helpers are separate ES modules:

```text
@astra/runtime/string
@astra/runtime/int
@astra/runtime/errors
@astra/runtime/channel
@astra/runtime/worker
```

A program imports only what it uses. The bundler can shake unused helpers.

### 4.3 Integer mapping

| Source policy | JS representation |
|---|---|
| `wrapping` | `number` for i32/f64; `bigint` for i64/u64 where needed |
| `safe-checked` | checked helper calls |
| `arbitrary` | `bigint` or math runtime |
| Explicit `+%` | wrapping helper where JS number semantics differ |

The backend must not silently use JavaScript’s unlimited `number` semantics for fixed-width operations.

### 4.4 String mapping

Canonical contract:

- `str.len()` is UTF-8 byte length.
- `str.char_count()` is Unicode scalar count.
- Byte operations are explicit.

`strings.model: "utf8"`:

- Use a UTF-8 view or encoded representation.
- Convert only at explicit web boundaries.

`strings.model: "native"`:

- Use native JavaScript strings where safe.
- Preserve `len()` and byte operations with helpers where needed.
- Document the cost.

### 4.5 Error mapping

- Recoverable errors use generated control flow or `try`/`catch` at boundaries.
- Foreign exceptions are caught and mapped.
- A JS `Error` is not a Neo Astra error unless the FFI declaration maps it.
- Panics become generated abort/throw behavior with a clean message.

### 4.6 Web boundary types

Explicit web types may map directly:

- `JsString`
- `JsArray<T>`
- `JsObject`
- `Promise<T>`
- `Element`
- `Response`

These are boundary types, not replacements for core types. Conversion must be visible.

---

## 5. TypeScript backend

The TypeScript backend shares the JavaScript runtime model.

Additional rules:

- Emit strict TypeScript.
- No `any` except at an explicitly marked FFI boundary.
- Emit `.d.ts` alongside generated modules when building a library.
- Preserve source maps.
- Do not emit types that depend on the compiler’s internal IR.
- Public types must be stable and documented.
- Inline `@script:ts` blocks use one pinned TypeScript compiler version; a missing
  or mismatched transpiler is a hard error, not a best-effort fallback.

The TypeScript output should be usable by ordinary TypeScript tooling.

---

## 6. C backend

### 6.1 Role

The C backend is the bootstrap path for native and Wasm:

```text
Astra IR
  → C17
  → clang
  → native binary or Wasm module
```

It is not a permanent excuse to avoid a better backend, but it is the pragmatic V1 path.

### 6.2 C rules

- Emit C17.
- Use fixed-width integer types.
- Respect target alignment and calling conventions.
- Emit explicit wrapping or checked operations.
- Emit explicit error returns.
- Emit explicit GC roots or stack maps.
- Do not rely on C signed-overflow undefined behavior for wrapping math.
- Do not expose internal structs as stable ABI unless declared.
- Compile each inline `@script:c` or `@script:cpp` block into its own translation
  unit, with symbol isolation and generated wrappers around the declared export
  surface.

### 6.3 Native runtime

The native runtime provides:

- Panic/abort
- GC
- Bounds checks where needed
- Channels and task scheduling
- FFI helpers
- Platform abstractions

The runtime is linked only for the features used.

### 6.4 Native targets

Tier 1:

- `x86_64-pc-windows-msvc`
- `x86_64-unknown-linux-gnu`
- `aarch64-apple-darwin`

Tier 2:

- `aarch64-unknown-linux-gnu`
- `x86_64-apple-darwin`

The minimum supported `clang` version must be recorded in the toolchain manifest.

---

## 7. Wasm backend

### 7.1 `wasm32-web`

- Browser host.
- JS glue for DOM, fetch, storage, and workers.
- No WASI assumptions.
- Memory is explicit.
- Threads are optional and feature-detected.
- Workers are used for CPU tasks when available.

### 7.2 `wasm32-wasi`

- Server/edge host.
- WASI filesystem, environment, clocks, and networking where available.
- No DOM.
- Capabilities must match the host.

### 7.3 Wasm constraints

- Linear memory growth must be handled.
- Manual memory is allowed with target constraints.
- GC across the Wasm boundary requires a clear ownership model.
- Wasm imports are allowlisted by capability.
- Wasm exports are typed and documented.

---

## 8. PHP and later backends

PHP is not a V1 launch target.

When it arrives, it should be:

- A direct source-to-source backend.
- Honest about its runtime model.
- Limited to modes that make sense.
- Validated by conformance tests, not by claims.

Do not let PHP’s constraints distort the portable core.

---

## 9. Semantic fidelity matrix

| Feature | Core rule | JS/TS | Native C | Wasm |
|---|---|---|---|---|
| Integer wrapping | Fixed-width wrapping by default | Helpers where JS differs | Explicit wrapping ops | Wasm integer ops |
| Checked integers | Trap on overflow | Checked helpers | Checked intrinsics | Checked helpers |
| Strings | UTF-8 byte contract | Native/encoded plus helpers | Pointer + length | Pointer + length |
| Arrays | Ordered, bounds-checked | Array plus checks | Bounds checks | Bounds checks |
| Maps | Insertion-ordered in V1 | `Map` | Ordered map | Ordered map |
| Option | No null | `null` only at boundary | Tag/niche | Tag/niche |
| Errors | `!` effect | Generated control flow | Tag/register | Tag/register |
| Concurrency | Scoped tasks/channels | Web Workers | Thread pool | Threads/workers |
| Manual memory | Native/Wasm only | Reject | Supported | Supported with constraints |
| FFI | Manifest-resolved | ESM imports | C ABI | Wasm imports |

Maps are marked insertion-ordered in V1. This is a design proposal that should be confirmed before the spec freezes.

---

## 10. Generated-code rules

Generated code must:

- Be valid for the target.
- Preserve source spans where possible.
- Use stable names derived from source names.
- Avoid collisions with target keywords.
- Avoid unnecessary global state.
- Include source maps for JS/TS.
- Include debug info where the toolchain supports it.
- Avoid comments that claim behavior the code does not implement.

Generated code should not:

- Depend on a private runtime that is larger than the feature used.
- Re-parse source.
- Use target-specific semantics without a documented boundary.
- Hide FFI conversions inside unrelated helpers.

---

## 11. Artifacts

| Target | Artifacts |
|---|---|
| JS | `.js`, `.js.map`, optional `.d.ts` |
| TS | `.ts`, `.ts.map`, `.d.ts` |
| Native | executable or library, debug symbols, metadata |
| Wasm | `.wasm`, JS glue, optional `.wat`, source map |
| Library | public `.d.ts` / headers / manifest |

Artifacts must be reproducible for the same inputs and toolchain.

---

## 12. Target selection

Precedence:

1. `--target` CLI flag
2. `ASTRA_TARGET` environment variable
3. `build.defaultTarget`
4. Error if none is available

The manifest target entry supplies backend, triple, mode, module format, and output directory.

If a target is unsupported:

- Fail before code generation.
- Name the target and the unsupported feature.
- Suggest a supported target.
- Do not silently fall back.

---

## 13. Cross-target conformance

Every portable feature needs:

- A source program.
- Expected output or behavior.
- A test for each Tier 1 target.
- A documented result for Tier 2 targets.
- A clear failure for unsupported combinations.

Target-specific features need their own conformance tests and must be marked as boundary behavior.

---

## 14. JS-001 implementation status (2026-10-25)

The first JavaScript ESM slice is implemented. The TypeScript orchestrator owns
code generation; the Rust core exposes a typed module over the versioned
`compile` JSON-RPC method. The Rust core never emits JavaScript, and the
TypeScript backend never parses `.ns` source.

**Pipeline**

1. `astra build` loads the manifest and selects a target using the precedence in
   §12.
2. It reads `nova.entry` and sends the source to `compile`.
3. The Rust compiler parses, resolves, checks, infers or validates the mode, and
   returns `ModuleIr` containing the AST, HIR, type annotations, manual-memory
   scopes, target, and integer policy.
4. `@neo-astra/codegen-js` emits one ESM module and tree-shaken helpers.
5. The CLI writes the module under the target `outDir` and adds a
   `package.json` with `"type": "module"` when the out directory has none.

**Implemented**

- Functions, constants, locals, `if`, `while`, `for`, blocks, `defer`, calls,
  assignments, and supported operators.
- Fixed-width integer wrapping and `safe-checked` helpers, including `BigInt`
  for `i64`/`u64`; tagged float values preserve `f64`/`f32` rendering, and `f32`
  operations use `Math.fround`.
- Arrays, structural maps, tuples, structs, enums, `Option`, `Result`, `match`,
  struct/enum/tuple patterns, and `Some`/`None`/`Ok`/`Err` patterns.
- `print`, `println`, `dbg`, `assert`, bounds-checked indexing, collection
  methods, string length/to-int, option/result methods, and checked integer
  methods.
- Recoverable failures: `!Error(...)`, `?` propagation across calls, function
  boundary conversion to failure values, and matching a failure as `Err`.
- Eager `spawn` tasks with `join`/`cancel`, joined-panic propagation, and
  unjoined task panics at function-scope exit.
- Line-level source maps with embedded `sourcesContent` and a
  `sourceMappingURL` comment from `astra build`.
- Named/default/`pub` exports; type-only exports are erased.
- Source-located diagnostics for unsupported constructs.

**Current boundaries**

- Project imports, namespace imports, and re-exports are rejected with `NS8001`.
  Multi-module emission waits for a project-wide typed-module request.
- Channels and a structured-concurrency scheduler are not implemented. Spawned
  tasks run eagerly and deterministically in the current function scope.
- `.d.ts`, the `arbitrary` integer policy, and full UTF-8 byte-slice/index
  operations are not implemented.
- `Option` and `Result` use tagged objects. Maps use a structural-key helper, not
  a JavaScript `Map`, so structural key equality and insertion order match the
  language contract.
- The backend rejects `@memory:manual` with `NS8003`.

---

## 15. Open target questions

1. What is the minimum `clang` version for V1?
2. Should TypeScript be a first-class target or a mode of the JS backend?
3. Is `wasm32-wasi` the exact target name, or should it be `wasm32-wasip1`?
4. What is the default JS module output for libraries?
5. Should native builds support static and dynamic libraries in V1?
6. What is the source-map format for Wasm?
7. Are maps insertion-ordered in V1? If not, what is the order?
8. Should `bigint` be used for all `i64`/`u64` operations in JS?
9. How are Web Worker transferables exposed without weakening safety?
10. What is the minimum browser baseline for V1?
