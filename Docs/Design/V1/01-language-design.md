# V1 Language Design

**Status:** V1 design proposal  
**Depends on:** D1–D25 in [`../../architecture.md`](../../architecture.md)

This document defines the V1 language surface. It is more concrete than the architecture review but remains subordinate to the eventual `spec-v0.1.md`.

---

## 1. Language identity

Neo Astra is a statically typed, multi-target language with:

- Explicit effects for recoverable errors
- Fixed-width numeric types
- UTF-8 strings
- Structured concurrency
- Compile-time capability modes
- GC by default
- Optional manual memory on native/Wasm
- A regular, formatter-friendly grammar

The language should feel bare like Rust and JavaScript, but with the low-level clarity of C and C++.

---

## 2. Source model

- Source encoding: UTF-8.
- File extension: `.ns`.
- Comments: `//` only.
- Block comments, `#`, `--`, and other comment forms are syntax errors.
- Newlines are the normal statement and item boundaries.
- Semicolons are optional soft separators for same-line statements/items; a trailing semicolon is allowed but unnecessary.
- The semicolon after a scope annotation is required because it terminates the block header.
- There is no automatic semicolon insertion.
- The canonical formatter preserves explicit same-line semicolon groups and is deterministic.

---

## 3. Lexical rules

### 3.1 Identifiers

- Start with a Unicode letter or `_`.
- Continue with Unicode letters, digits, or `_`.
- Case-sensitive.
- Keywords are reserved.
- Canonical style:
  - `snake_case` for values and functions
  - `PascalCase` for types, constructors, and modules
  - `SCREAMING_SNAKE_CASE` for constants

### 3.2 Literals

- Integers: `0`, `42`, `0xFF`, `0b1010`, `1_000_000`
- Floats: `1.0`, `1e9`, `1.5e-3`
- Booleans: `true`, `false`
- Strings: `"hello"`, with escapes
- Bytes: `b"..."` or a byte-array literal
- Characters: not a separate type in V1; use `str` or `u8`
- Arrays: `[1, 2, 3]`
- Maps: `{ "a": 1, "b": 2 }` in value position
- Unit: `()`

### 3.3 Keywords

```text
abort    as         break      const      continue
default  defer      else       enum       export
false    fn         for        foreign    from
if       import     in         let        match
module   mut        not        panic      private
pub      return     spawn      struct     true
type     while
```

Accepted keyword aliases (exact lexical synonyms; the formatter emits the short form):

```text
fn/function   let/var      const/constant   type/typedef
struct/record mut/mutable  pub/public
```

`defer` is a V1 proposal for scoped cleanup in manual-memory code. It is not a general-purpose destructor mechanism. Aliases are style options only; they do not change semantics.

Declaration visibility is optional. A normal declaration is module-local; `pub`/`public` and `export` make it part of the module export surface; `private` keeps it module-local and forbids export. Private declarations must not leak through public signatures, public aliases, or public fields. The formatter canonicalizes `public` to `pub`, and redundant `pub export` combinations to `export`.

---

## 4. Grammar baseline

This is a design-level grammar sketch, not the final EBNF.

### 4.1 Module

```ns
module app.main

import std.io
import { read, write as write_file } from "std:fs"
import * as math from "std:math"
import fallback from "./thing.ns"
import "./effects.ns"

fn helper() -> i32 {
    return 1
}

private fn secret() -> i32 {
    return 2
}

pub fn main() {
    print("hello")
}

export { helper }
export { read } from "std:fs"
export * from "./shared.ns"
export default main
```

`import path` is the project-path namespace form; the final path segment becomes the alias. `import path as alias` uses an explicit alias. JS/TS-style named, default, namespace, side-effect, re-export, and default-export forms are also accepted. `export { ... }` can promote a normal declaration; `private` declarations cannot be exported.

### 4.2 Items

```ns
const MAX: i32 = 100

type UserId = u64

struct User {
    id: UserId
    name: str
    active: bool
}

enum Status {
    Pending
    Active
    Suspended(reason: str)
}

fn add(a: i32, b: i32) -> i32 {
    return a + b
}

export function exported_add(a: i32, b: i32) -> i32 {
    return add(a, b)
}
```

### 4.3 Attributes and modes

Item-level:

```ns
@requires(system)
@mode(system)
fn read_file(path: str) -> bytes ! IoError {
    // ...
}
```

Block-level:

```ns
fn mixed() {
    {@mode:core;
        let x = 1
    }

    {@mode:web;
        // browser-only code
    }
}
```

### 4.4 Foreign modules

```ns
foreign module math_engine {
    fn compute_matrix(data: [f64]) -> [f64]
}
```

### 4.5 Inline script blocks

`@script` is a declaration-level escape hatch for a bounded foreign-language
implementation. It is proposed for V1 in
[`10-inline-script-interop.md`](./10-inline-script-interop.md).

```ns
{@script:js as math;
    @mode(core)
    @exports {
        fn add(a: i32, b: i32) -> i32
    }

    @impl
    export function add(a, b) {
        return a + b
    }
@end
}

fn main() {
    print(math.add(1, 2))
}
```

The body between `@impl` and the `@end` sentinel is raw foreign source. It is not
tokenized as Nova, and calls are checked only against the signatures in
`@exports`. A script namespace is module-local in V1. Unsupported language/target
combinations are compile-time errors.

### 4.6 Statements

```ns
let x = 1
let mut total = 0
total = total + x

if x > 0 {
    print("positive")
} else {
    print("non-positive")
}

match status {
    Status.Pending => print("pending")
    Status.Active => print("active")
    Status.Suspended(reason) => print(reason)
}
```

### 4.7 Expressions

```ns
1 + 2
a *% b
foo.bar()
arr[0]
Channel.create<str>()
spawn || { work() }
```

---

## 5. Type system

### 5.1 Primitive types

| Type | Meaning |
|---|---|
| `i8`, `i16`, `i32`, `i64` | Signed fixed-width integers |
| `u8`, `u16`, `u32`, `u64` | Unsigned fixed-width integers |
| `f32`, `f64` | IEEE floating point |
| `bool` | Boolean |
| `str` | Immutable UTF-8 text |
| `bytes` | Mutable byte buffer |
| `usize` | Pointer-sized unsigned integer |
| `isize` | Pointer-sized signed integer |
| `()` | Unit |

`i128`/`u128` are post-V1 unless a target-independent implementation is proven.

### 5.2 Composite types

- `[T]` — ordered sequence
- `[T; N]` — fixed-size array
- `{K: V}` — map type in type position
- `(A, B)` — tuple
- `struct` — named product type
- `enum` — tagged union
- `Option<T>` — optional value
- `Result<T, E>` — explicit value-style result
- Function types: `fn(A, B) -> C`

### 5.3 Generics

V1 supports monomorphized generics:

```ns
fn identity<T>(value: T) -> T {
    return value
}

struct Box<T> {
    value: T
}
```

V1 does not include typeclasses, traits, higher-kinded types, or specialization. Those are post-V1 proposals.

### 5.4 Inference

- Local inference is allowed.
- Public API boundaries are explicit.
- No implicit numeric coercion.
- No implicit truthiness.
- No implicit nullable conversion.
- No implicit stringification.
- `null` does not exist. Use `Option<T>`.

### 5.5 Exhaustiveness

`match` must be exhaustive.

```ns
match value {
    Some(v) => use(v)
    None => fallback()
}
```

A missing case is a compile-time error.

---

## 6. Integer semantics

Default policy: fixed-width, wrapping.

### 6.1 Manifest policy

```json
{
  "math": {
    "integers": "wrapping"
  }
}
```

Allowed values:

- `wrapping` — default; raw machine arithmetic
- `safe-checked` — overflow traps at runtime
- `arbitrary` — arbitrary-precision behavior where the target supports it

### 6.2 Local overrides

Local code can force behavior regardless of the project policy:

```ns
let wrapped = a +% b
let checked = a.checked_add(b)!
let called = a.wrapping_add(b)
```

Rules:

- `+%` and `-%` are wrapping operators.
- `.wrapping_add`, `.wrapping_sub`, `.wrapping_mul` are explicit APIs.
- `.checked_add` and friends return `Option<T>` or an error effect.
- `safe-checked` does not change explicit wrapping operations.
- `arbitrary` must still respect explicit fixed-width behavior.

### 6.3 Target mapping

| Target | `wrapping` | `safe-checked` | `arbitrary` |
|---|---|---|---|
| Native | Machine wrapping ops | Checked intrinsics | Runtime big integer |
| Wasm | Wasm integer ops | Checked helpers | Runtime big integer |
| JS/TS | `Number`/`BigInt` plus wrapping helpers | Checked helpers | `BigInt` or math runtime |

The selected policy is recorded in build metadata and the lockfile.

---

## 7. String and byte semantics

### 7.1 Canonical model

`str` is an immutable UTF-8 byte sequence.

```ns
let s = "héllo😀"
```

V1 defines:

- `s.len()` — UTF-8 byte length
- `s.char_count()` — Unicode scalar value count
- `s.bytes()` — byte iterator or byte view
- `s.chars()` — scalar value iterator
- `s.is_empty()` — byte length is zero

Byte indexing is explicit:

```ns
let first = s.byte_at(0) // Option<u8>
```

Character indexing is also explicit and may be O(n):

```ns
let first = s.char_at(0) // Option<str>
```

### 7.2 Representation policy

```json
{
  "strings": {
    "model": "utf8"
  }
}
```

Allowed values:

- `utf8` — native/Wasm pointer + length; JS may use encoded helpers where needed
- `native` — high-level targets may use native strings, but `len()`, `char_count()`, and byte operations must retain the contract

The representation may change. The observable contract may not.

### 7.3 Bytes

`bytes` is a mutable byte buffer:

```ns
let buf: bytes = bytes.new(16)
buf[0] = 0xFF
```

Bounds are checked. The exact ownership model follows the active memory policy.

---

## 8. Error model

### 8.1 Declaration

A fallible function declares its error effect:

```ns
fn parse_age(input: str) -> i32 ! InvalidAge | ParseIntError {
    if input.is_empty() {
        return !InvalidAge("Age cannot be empty")
    }
    return input.to_int()!
}
```

### 8.2 Propagation

A fallible call must use `!`:

```ns
fn process() -> str ! InvalidAge | ParseIntError {
    let age = parse_age(get_input())!
    return "ok"
}
```

Rules:

- Calling a fallible function without `!` is a compile error.
- The current function must declare a compatible error effect.
- Error effects are part of the function type.
- Error unions use `|`.
- `!` is not an exception operator. It is explicit propagation.
- Every `!` call site is visible in source.

### 8.3 Handling

V1 provides explicit handling through `match` over a value-style result or an error boundary construct. The exact syntax belongs in `spec-v0.1.md`; the semantic requirement is that no error is silently discarded.

### 8.4 Panic and abort

- `panic("message")` reports a programmer bug and terminates the current task.
- `abort("message")` terminates immediately without unwinding.
- Panics must produce a clean stack trace where the target supports it.
- Panics must not cross FFI boundaries silently.

### 8.5 Target mapping

| Target | Representation |
|---|---|
| Native | Value + tag / error register |
| Wasm | Value + tag / multi-value where available |
| JS/TS | Generated `try`/`catch` at function boundaries |
| FFI | Explicit error mapping |

---

## 9. Concurrency model

### 9.1 Tasks

```ns
fn main() {
    let channel = Channel.create<str>()

    spawn || {
        channel.send("hello")!
    }

    let message = channel.receive()!
    print(message)
}
```

Rules:

- `spawn` creates a task owned by the enclosing scope.
- The parent scope waits for children at scope exit.
- A task may return a value through `Task<T>`.
- Cancellation is cooperative and scoped.
- A panic in a child is propagated to the parent scope.
- Detached tasks are not allowed in the portable core.

### 9.2 Channels

- `Channel.create<T>()` creates a typed channel.
- `send` and `receive` are fallible.
- Closing a channel is explicit.
- Sending to a closed channel fails with an error.
- Backpressure is defined by the channel capacity.
- V1 default: bounded channel; the capacity must be explicit or use a documented default.

### 9.3 Isolation

The portable core does not allow shared mutable state between tasks.

- Messages are copied or serialized.
- Immutable values may be shared safely.
- `SharedArrayBuffer` is an explicit `web` opt-in.
- Native tasks use isolated heaps or synchronized message queues.

### 9.4 Target mapping

| Target | Mapping |
|---|---|
| Native | OS thread pool |
| Web/JS | Web Workers + `postMessage` |
| Wasm | Threads when available; worker fallback |
| Async I/O | Separate I/O runtime, not a replacement for CPU tasks |

---

## 10. Modes and capabilities

### 10.1 Built-in modes

| Mode | Purpose |
|---|---|
| `core` | Portable libraries and shared code |
| `app` | Applications and scripts |
| `web` | Browser, DOM, fetch, workers |
| `system` | Filesystem, networking, raw memory, FFI |

### 10.2 Selection

```text
--mode
  > ASTRA_MODE
  > target-bound mode
  > build.defaultMode
  > least-privileged inference
```

Source tags and scoped blocks are validated after selection.

### 10.3 Requirements

```ns
@requires(system)
@tags(fs, ffi)
module io
```

Rules:

- Requirements are viral.
- A core function cannot hide a system dependency.
- Nested blocks may narrow capabilities.
- Widening requires an explicit signature or boundary.
- `astra explain` must show the reason tree.

### 10.4 Modes do not change semantics

Modes add or remove capabilities. They do not change:

- Integer behavior
- String behavior
- Core syntax
- Core type rules
- Error propagation

Those are controlled by explicit semantic policies.

---

## 11. Memory surface

### 11.1 Managed code

Default behavior:

- GC-managed values
- No user-visible finalizers
- No manual `free`
- Bounds-checked arrays
- No raw pointers in `core`

### 11.2 Manual code

Manual memory is available only in `system`-capable native/Wasm code:

```ns
{@memory:manual;
    let buffer = Memory.alloc<u8>(1024)!
    defer Memory.free(buffer)

    // use buffer
}
```

V1 design requirements:

- `Memory.alloc` is fallible.
- `Memory.free` is explicit.
- `defer` runs in reverse order at scope exit.
- Manual memory cannot target JavaScript.
- Manual regions cannot be silently captured by GC-managed code.
- FFI ownership must be explicit.

### 11.3 Precedence

```text
block {@memory:...;}
  > file wrapper
  > manifest memory.files
  > manifest memory.default
```

---

## 12. Standard library surface

### 12.1 Core

- `str`, `bytes`
- arrays, maps, tuples
- math
- `Option`, `Result`
- iterators
- formatting
- time and duration
- channels
- scoped tasks

### 12.2 Web

- DOM
- `fetch`
- storage
- events
- Web Workers
- browser-specific APIs

### 12.3 System

- filesystem
- networking
- processes
- environment
- threads
- raw memory
- OS-specific APIs

The standard library is mode-gated. A `core` module cannot import a `web` or `system` API without an explicit requirement.

---

## 13. V1 language boundaries

Included:

- Functions, closures, structs, enums, pattern matching
- Monomorphized generics
- Local inference
- Effect-tracked errors
- Structured concurrency
- Compile-time modes
- GC and manual memory
- FFI declarations

Not included in V1:

- User macros
- Ownership/borrowing as the default model
- Typeclasses/traits
- Higher-kinded types
- Runtime reflection
- Detached tasks
- Shared mutable state in the portable core
- Exceptions as a source-level error mechanism
- `null`

---

## 14. Open language questions

These require resolution in `spec-v0.1.md`:

1. Exact `match` syntax and error-handling boundary syntax.
2. Whether `defer` is part of V1 or a later manual-memory convenience.
3. ~~Exact syntax for imports and package-qualified paths.~~ Resolved by D23 and `Docs/spec-v0.1.md` §3.16/§12.5.
4. Whether maps are insertion-ordered in V1.
5. Exact `Task<T>` join and cancellation API.
6. Channel capacity defaults and close semantics.
7. Whether `arbitrary` integers are available on every mode or only `app`/`web`.
8. Exact public API rules for generic bounds, if any.
9. Whether `str` indexing is entirely method-based or supports explicit index syntax.
10. ~~Final keyword set and formatter rules.~~ Resolved by D22/D23 and `Docs/spec-v0.1.md` §1.6/§3.15: aliases are accepted and normalized by the formatter.
11. Whether type-only imports and exports need explicit `import type`/`export type` syntax.
