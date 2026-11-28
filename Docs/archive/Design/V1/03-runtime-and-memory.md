# V1 Runtime and Memory Design

**Status:** V1 design proposal  
**Depends on:** D1, D6–D10, D21, D25  
**Related:** [`01-language-design.md`](./01-language-design.md), [`04-targets-and-backends.md`](./04-targets-and-backends.md), [`08-nova-and-nopm.md`](./08-nova-and-nopm.md)

---

## 1. Runtime philosophy

Neo Astra has **no virtual machine**.

The runtime is a set of pay-as-you-go support libraries:

- GC support when managed memory is used
- Manual allocation helpers when manual memory is used
- Error and panic support
- Structured concurrency support
- FFI boundary support
- Target-specific glue for JS, native, and Wasm

A program that uses none of these features should not pay for them.

The honest promise:

> **Zero VM, pay-as-you-go runtime.**

This promise applies to **production artifacts**. Nova is a separate development
interpreter described in [`08-nova-and-nopm.md`](./08-nova-and-nopm.md). Nova is
not linked into compiled output, is not a deployment target, and does not change
the production runtime contract.

---

## 2. Runtime layers

```text
Program
  │
  ├── Core runtime
  │     ├── allocation
  │     ├── bounds checks
  │     ├── string helpers
  │     └── panic/abort
  │
  ├── GC runtime
  │     ├── heap
  │     ├── roots
  │     ├── mark/sweep
  │     └── task heaps
  │
  ├── Concurrency runtime
  │     ├── task scheduler
  │     ├── channels
  │     ├── cancellation
  │     └── thread pool
  │
  ├── FFI runtime
  │     ├── string conversion
  │     ├── error mapping
  │     └── ownership metadata
  │
  └── Platform layer
        ├── native OS
        ├── browser
        └── WASI
```

Each layer is optional at link/bundle time unless used by the program.

---

## 3. Value representation

Neo Astra does not require one universal boxed value. The compiler lowers each type to a target-appropriate representation.

### 3.1 Common representations

| Type | Native/Wasm | JS/TS |
|---|---|---|
| `i32`, `u32` | 32-bit integer | `number` with range checks |
| `i64`, `u64` | 64-bit integer | `bigint` where required |
| `f32`, `f64` | IEEE float | `number` |
| `bool` | byte / Wasm i32 | `boolean` |
| `str` | pointer + byte length | `string` or encoded view |
| `bytes` | pointer + length + capacity | `Uint8Array` |
| `[T]` | slice or owned array | `Array<T>` |
| `Option<T>` | niche optimization or tag | `T \| null` at boundary |
| `T ! E` | value + tag or out-parameter | return value + thrown boundary error |
| `struct` | layout-defined record | object |
| `enum` | tag + payload | discriminated object |

### 3.2 Layout rules

- Struct field order is source order unless a `packed`/ABI attribute is added.
- Alignment follows the target ABI.
- FFI structs use an explicit ABI layout.
- The compiler must not expose unstable layout as a public guarantee in V1.
- Bounds checks are not optional in managed code.

---

## 4. Managed memory

### 4.1 Default

- GC by default.
- Non-moving mark-sweep in V1.
- Precise roots from compiler-generated stack maps.
- No user-visible finalizers.
- No weak references in V1.
- Allocation failure is a panic or an explicit allocation error, depending on the API.

### 4.2 Task-local heaps

The portable concurrency model forbids shared mutable state. V1 therefore uses **task-local heaps** on native/Wasm:

- Each task owns its heap.
- Channel messages are copied or serialized.
- No cross-task heap references.
- Immutable shared values may use read-only sharing where safe.
- `SharedArrayBuffer` is web-only and explicit.

This simplifies collection and avoids cross-thread stop-the-world pauses.

### 4.3 Collection

- Collection is triggered by allocation pressure.
- Each task collects its own heap.
- A task may be paused during its own collection.
- The scheduler must not assume a global collection.
- Stack maps identify roots precisely.
- FFI frames require explicit root registration.

### 4.4 What GC does not promise

- No real-time pause guarantee in V1.
- No compacting.
- No finalizers.
- No resurrection.
- No weak maps.
- No cross-task object graph.

---

## 5. Manual memory

### 5.1 Availability

Manual memory is available only when:

- Mode is `system` or a mode that grants manual memory.
- Target is native or Wasm.
- The file or block policy is `manual`.

JavaScript builds must reject manual memory with a clear diagnostic.

### 5.2 API surface

Proposed V1 surface:

```ns
let buffer = Memory.alloc<u8>(1024)!
defer Memory.free(buffer)
```

```ns
let region = Region.new(4096)!
defer region.free()
```

Rules:

- Allocation returns an error effect on failure.
- `free` is explicit.
- `defer` runs at scope exit in reverse order.
- Double-free is a bug; V1 may detect it in debug builds.
- Use-after-free is undefined behavior in manual mode unless a checked mode is enabled.
- Manual values cannot be captured by GC-managed closures without an explicit boundary.

Region strategy (decided for V1): `Region.new(bytes)` reserves one
fixed-capacity block; `region.alloc<T>(count)` carves `count * sizeof(T)`
bytes from it bump-style (each carv 8-byte aligned, so sequential carvings of
any scalar or struct stay naturally aligned) and fails when the block is
exhausted; `region.free()` releases the whole block. A region never moves, so
every pointer it hands out stays valid until the region is freed.

### 5.3 Ownership at boundaries

- Passing a manual pointer to FFI remains owned by the caller unless the mapping says otherwise.
- Returning a manual pointer transfers ownership only if the FFI contract says so.
- The compiler cannot infer foreign ownership.
- The manifest/FFI declaration must state ownership rules.

---

## 6. Error representation

### 6.1 Native/Wasm

A fallible function returns a discriminated result:

```text
{ tag, value }  // conceptual
```

The exact ABI is target-specific and not stable in V1.

Rules:

- Success and failure are distinguishable without inspecting the payload.
- Error payloads are type-checked.
- Error propagation is a direct branch, not stack unwinding.
- Panics are separate from recoverable errors.

### 6.2 JavaScript/TypeScript

- Generated code may use `throw` and `try`/`catch` at function boundaries.
- The source-level `!` propagation maps to generated control flow.
- Foreign exceptions must be caught and mapped at the FFI boundary.
- A JS exception must not silently become a Neo Astra error without a declared mapping.

### 6.3 Panic

- `panic` terminates the current task.
- `abort` terminates the process or worker immediately.
- In native debug builds, panic includes a stack trace.
- In release builds, the trace may be reduced unless a flag enables it.
- A panic in a child task is propagated to the parent scope.

---

## 7. Structured concurrency runtime

### 7.1 Tasks

A task has:

- An owned heap
- A state: created, running, waiting, cancelled, completed, failed
- A result slot
- A cancellation token
- A parent scope reference

The parent scope joins all children before exiting.

### 7.2 Channels

A channel has:

- A bounded buffer
- A send queue
- A receive queue
- A closed flag
- A type-specific message representation

V1 default is bounded. Unbounded channels are an explicit opt-in and must be documented as a memory risk.

### 7.3 Scheduling

| Target | Scheduler |
|---|---|
| Native | OS thread pool |
| Web | Web Worker pool |
| Wasm | Threads if available; worker fallback |
| Async I/O | Separate event loop / reactor |

CPU tasks and I/O readiness are not the same abstraction. V1 must not pretend they are.

### 7.4 Cancellation

- Cancellation is cooperative.
- A task checks cancellation at defined safepoints.
- Blocking channel operations are cancellation-aware.
- Parent cancellation propagates to children.
- Cleanup uses `defer` where available.

### 7.5 Failure propagation

- A child panic fails the parent scope.
- A child recoverable error is returned through the task result.
- The parent must handle or propagate the error.
- Detached task failures are not allowed in the portable core.

---

## 8. JavaScript runtime

The JS backend uses the host GC and host event loop.

Rules:

- No custom GC.
- No manual memory.
- No blocking the main thread for CPU tasks.
- CPU work uses Web Workers.
- I/O uses the host event loop.
- Runtime helpers are ES modules and tree-shakable.
- Generated code should not import a monolithic runtime if it uses only one helper.

---

## 9. Wasm runtime

### 9.1 `wasm32-web`

- Browser host.
- JS glue for DOM, fetch, storage, and workers.
- No WASI assumption.
- Memory growth must be handled explicitly.
- Threads are optional and feature-detected.

### 9.2 `wasm32-wasi`

- Server/edge host.
- WASI APIs for filesystem, environment, and clocks.
- No DOM.
- Capabilities must match the host.

The compiler must reject capabilities the target cannot provide.

---

## 10. FFI runtime

### 10.1 Strings

- Native: pointer + UTF-8 length.
- JS: native string or encoded view.
- Wasm: pointer + length in linear memory.
- Conversion must be explicit at the boundary.

### 10.2 Ownership

- Borrowed pointers are valid only for the call.
- Owned pointers transfer according to the declaration.
- The compiler cannot guess foreign lifetime rules.
- FFI declarations must include ownership metadata.

### 10.3 Errors

- C ABI errors use a tag/result convention.
- JS exceptions are caught and mapped.
- Foreign panics/aborts are not allowed to unwind into Neo Astra code.
- A missing mapping is a compile-time error.

---

## 11. Runtime initialization

### 11.1 Native

Entry point initializes:

1. Panic handler
2. GC for the main task
3. Thread pool if concurrency is used
4. Platform services used by the program

Only used subsystems are linked.

### 11.2 Web

The generated module:

1. Loads required helpers.
2. Creates workers only if `spawn` is used.
3. Does not install global patches.
4. Does not require a framework.

### 11.3 Wasm

- The host provides imports.
- The module exports the entry point.
- Memory is explicit.
- Workers/threads are host-feature-dependent.

---

## 12. Runtime size budgets

Initial V1 targets:

| Artifact | Budget |
|---|---|
| Minimal native hello world | under 64 KB stripped |
| Minimal Wasm hello world | under 32 KB after `wasm-opt` |
| Minimal JS hello world | under 2 KB gzipped |
| GC runtime | under 32 KB native |
| Concurrency runtime | under 24 KB native |
| JS helper set | under 4 KB gzipped for a typical app |

Budgets are reviewed at release. Missing a budget requires a documented reason.

---

## 13. Runtime security

- Bounds checks are mandatory in managed code.
- Manual memory is explicitly unsafe.
- FFI is explicitly unsafe.
- Stack traces must not leak secrets.
- Panic messages must be user-controlled but sanitized for logs.
- Worker messages must be serialized safely.
- Wasm imports must be allowlisted by capability.

---

## 14. Open runtime questions

1. Is non-moving mark-sweep sufficient for V1, or is a compacting GC required earlier?
2. Should task-local heaps be mandatory on native, or optional for shared immutable graphs?
3. What is the exact `Task<T>` join/cancel API?
4. Should `defer` be in V1 or deferred?
5. How are FFI roots registered with the GC?
6. Are channels always copying, or can they transfer ownership?
7. What is the default channel capacity?
8. How are panic traces symbolized in release builds?
9. Should the JS runtime use native `Error` for recoverable errors or only at boundaries?
10. What is the minimum Wasm thread story for V1?
