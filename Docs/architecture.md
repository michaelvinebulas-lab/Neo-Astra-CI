# Neo Astra — Architecture Review and v0.1 Plan

**Status:** Draft for discussion  
**Date:** 2026-10-24  
**Project:** Neo Astra  
**Language:** Neo Astra  
**File extension:** `.ns`  
**CLI name:** `astra`

---

## 0. Purpose of this document

This document captures the architecture review of the original Neo Astra specification and turns it into a set of concrete decisions, recommendations, and milestones.

The goal is not to approve everything at once. The goal is to review the open questions one by one, record decisions, and then write the actual engineering specification.

Use the **Decision Log** at the end as the review checklist.

---

## 1. Executive verdict

The original vision is strong. Most of it should be kept:

- Multi-target compilation through a shared compiler pipeline
- Target-scoped dependencies
- Aggressive dead-code elimination
- A unified manifest
- Native, Wasm, and JavaScript output
- FFI and ecosystem interop
- No framework runtime

The main issue is that the current specification is a **product pitch**, not yet an **engineering specification**.

The hardest problem is not parsing source code or generating JavaScript. The hardest problem is defining **one semantic contract** that native, Wasm, and JavaScript can all honor.

If that contract is not defined, Neo Astra becomes:

> “Write once, debug three times.”

---

## 2. What we keep from the original specification

| Original idea | Verdict | Notes |
|---|---|---|
| Multi-target compiler | Keep | Use a typed intermediate representation shared by all backends |
| Native target | Keep, but later | Start with a C backend and `clang`; add LLVM later |
| Wasm target | Keep, but split | `wasm32-web` and `wasm32-wasi` are different targets |
| JavaScript target | Keep, and do first | Best path to a usable language and browser playground |
| Zero-framework runtime | Keep | Reframe as “zero VM, pay-as-you-go runtime” |
| Tree-shaking | Keep | Do it on the IR call graph, not just the manifest |
| Unified manifest | Keep | Add lockfile, target triples, and capabilities |
| npm interop | Keep, but scope it | Generate typed bindings; external packages are mostly opaque to shaking |
| C/C++/Rust FFI | Keep, but scope it | C ABI is the base layer; C++ needs a C shim |
| Structural types for web | Keep, with rules | Define representation and conversion rules at target boundaries |
| Fixed primitive types for native | Keep | Use fixed-width integers everywhere in the portable core |
| `core.json` or `core.toml` | Keep | Support both formats with one strict semantic schema and `core.lock.json` |

---

## 2.5 Design principles: human-readable and AI-friendly

Two goals are non-negotiable:

1. Neo Astra must remain readable even when writing low-level code.
2. Neo Astra must be predictable, explainable, and tool-friendly for AI systems.

### 2.5.1 Human-readable low-level code

Low-level should not mean cryptic. A systems programmer should be able to read a function and immediately understand:

- What memory it touches
- What can fail
- What is unsafe
- What mode it requires
- What it allocates

Illustrative example:

```ns
{@mode:system;
    fn copy_bytes(dst: *u8, src: *u8, len: usize) -> () ! MemoryError {
        let dst_region = Region::new(dst, len)
        let src_region = Region::new(src, len)
        dst_region.copy_from(src_region)
        return
    }
}
```

Readability rules:

1. **Explicit effects over punctuation.**
   Use readable keywords for `unsafe`, `manual`, `async`, and similar concepts.

2. **No implicit numeric conversions.**
   Converting between `i32`, `i64`, `u32`, and `usize` must be visible.

3. **No hidden allocation.**
   If an operation allocates, it should be obvious from the call or the type.

4. **No hidden control flow.**
   Avoid implicit trampolines, magic getters, and surprising operator behavior.

5. **No operator overloading in v0.1.**
   `+` should mean addition, not “whatever this type decides.”

6. **Low-level operations use named verbs.**
   Prefer `copy_from`, `read_u32`, `write_bytes`, and `allocate` over symbol soup.

7. **Recoverable errors are explicit.**
   `! E` is visible in signatures, and `!` is required at fallible call sites. `Result<T, E>` remains available for value-style handling. Panics are reserved for programmer bugs.

8. **Modes and capabilities are visible.**
   A `{@mode:system; ... }` block should be obvious in source review.

9. **One canonical spelling.**
   Avoid multiple syntaxes for the same operation. Aliases increase ambiguity for humans and tools.

10. **Public APIs require explicit types.**
    Local variables may use inference, but exported functions, structs, and modules should make their contracts clear.

### 2.5.2 AI-friendly by design

“AI-friendly” does not mean adding AI features to the language. It means the language is easy to:

- Parse
- Analyze
- Generate
- Verify
- Repair

Rules:

1. **Regular, unambiguous grammar.**
   Prefer an LL(k) grammar with no context-sensitive parsing.

2. **One canonical AST.**
   Every construct should have exactly one structural representation.

3. **Stable keywords and naming.**
   Avoid synonyms and syntax variants that fragment training examples and tooling.

4. **Explicit semantics.**
   Inference is allowed, but it must be explainable. `astra explain` should show why a type, mode, or dependency was chosen.

5. **Structured diagnostics.**
   Every error should include an error code, source span, expected/actual values, and a suggested fix. Support `--json` output.

6. **Machine-readable compiler output.**
   Provide `astra ast --json`, `astra check --json`, and a stable IR dump for agents and external tools.

7. **Canonical formatter.**
   `astra fmt` should produce one deterministic style. `astra fmt --check` should work in CI.

8. **Deterministic builds.**
   The lockfile and build metadata should record modes, dependencies, and compiler versions.

9. **Small, consistent standard library.**
   Prefer a few well-named APIs over many overlapping ones.

10. **Examples are part of the specification.**
    Every language feature should have a short, runnable example in the repo.

11. **Versioned grammar.**
    Grammar changes should be explicit and documented, not accidental.

12. **No macros or heavy metaprogramming in v0.1.**
    They make code harder for humans and AI to reason about. Add them later if needed.

The practical rule is:

> **Explicit where it matters, inferred where it is obvious, explainable everywhere.**

### 2.5.3 Strict but ergonomic

Strictness should prevent compile-time bugs, not create ceremony.

Rules:

1. **Strict at boundaries, inferred locally.**
   Public APIs and module boundaries require explicit types. Local variables may use inference.

2. **No implicit numeric or string coercions.**
   Conversions must be visible.

3. **Exhaustive matching.**
   `match` must cover every case or have an explicit fallback.

4. **No `null`.**
   Use `Option<T>`.

5. **All imports are explicit.**
   No hidden globals or ambient dependencies.

6. **Unknown manifest keys are errors.**
   A typo in `core.json` or `core.toml` must not silently change the build.

7. **Warnings can be errors in CI.**
   Local development can be forgiving; release builds should be strict.

8. **Every error has a code and, where possible, a fix.**
   Strictness without good diagnostics is just friction.

9. **No dynamic `eval` in the portable core.**
   It breaks static analysis, tree-shaking, and AI verification.

10. **Strictness must be explainable.**
    If the compiler rejects code, it should say exactly which rule was violated and how to fix it.

The goal is to avoid both extremes:

- **“Rust 2.0”** — strictness that makes simple code exhausting.
- **“JavaScript weirdness”** — implicit coercions and syntax surprises.

### 2.5.4 Syntax principles

1. **Only `//` line comments are valid in `.ns` source.**
   `/* ... */`, `#`, `--`, and every other comment form are syntax errors in Neo Astra source. This is a hard rule, not a style preference.

2. **Documentation uses `//` with a defined tag.**
   Do not add a second comment syntax for docs. For example, `// @doc ...` can be the convention.

3. **Braced blocks.**
   Blocks use `{ ... }`. Mode and memory annotations use the same block-header form: `{@mode:core; ... }`.

4. **No automatic semicolon insertion.**
   Newlines are explicit statement and item boundaries. Semicolons are optional soft separators for same-line constructs and are required after scope annotations. No JavaScript-style ASI surprises.

5. **Keywords over sigils.**
   Prefer readable words such as `async`, `manual`, and `unsafe` over punctuation-heavy syntax.

6. **One canonical form.**
   Every construct has one preferred spelling. The formatter enforces it.

7. **Extensive features, not extensive punctuation.**
   The language can be feature-rich while staying visually clean.

8. **Learnable in layers.**
   A beginner should be able to write useful code after learning a small subset. Low-level and mode-specific features come later.

9. **Every construct has a plain-language explanation.**
   If a feature cannot be explained simply, its syntax is probably too clever.

---

## 3. The central problem: semantics before syntax

### 3.1 Why this matters

The same source program can produce different results on different targets unless the language defines the behavior precisely.

Example — integer overflow:

```ns
let x: i32 = 2147483647
print(x + 1)
```

Possible results:

- Native with wrapping integers: `-2147483648`
- JavaScript `Number`: `2147483648`
- Native C signed overflow: undefined behavior unless explicitly compiled with wrapping operations

Example — string length:

```ns
let s = "héllo😀"
print(s.len())
```

Possible results:

- JavaScript UTF-16 code units: `7`
- UTF-8 bytes: `10`
- Unicode scalar values / code points: `6`

These differences must be decided by the language, not by the target.

### 3.2 Proposed semantics matrix

This table is a starting point for discussion. It should eventually become part of `spec-v0.1.md`.

| Feature | Portable core rule | JS backend | Native backend | Wasm backend | Status |
|---|---|---|---|---|---|
| Integers | Fixed-width; project default is wrapping; manifest `math.integers` can change the policy | `i32`/`f64` via `Number`; `i64` via `BigInt`; policy helpers | Machine integers with wrapping/checked ops | Wasm integers | Decided |
| Integer overflow | `math.integers`: `wrapping`, `safe-checked`, or `arbitrary`; explicit `+%`, `wrapping_add`, `checked_add` override locally | Helpers / `BigInt` | Emit wrapping/checked ops explicitly | Native Wasm behavior | Decided |
| Strings | `str` is canonically UTF-8 bytes; manifest `strings.model` can select `utf8` or `native` representation | Native JS string at explicit web boundaries | UTF-8 pointer + length | UTF-8 pointer + length | Decided |
| Arrays | Ordered, bounds-checked | JS array with bounds checks | Runtime bounds checks | Runtime bounds checks | Proposed |
| Maps | Define iteration order explicitly; recommendation: insertion-ordered in v0.1 | `Map` | Ordered hash map or equivalent | Ordered hash map | Proposed |
| Null | No `null`; use `Option<T>` | `null`/`undefined` only internally | Tagged optional value | Tagged optional value | Proposed |
| Errors | Effect-tracked explicit bubbling with `!`; panic for programmer bugs | Generated `try`/`catch` at target boundaries | Value + tag / error register | Value + tag | Decided |
| Destructors | No user-visible finalizers in the portable core | Host GC | GC; explicit free in manual mode | GC; explicit free in manual mode | Proposed |
| Async | Structured concurrency is the portable model | Web Workers + `postMessage` | OS thread pool | Wasm threads later | Decided |
| Threads | Channels and scoped tasks; no shared mutable state in portable core | Web Workers / `SharedArrayBuffer` opt-in | OS threads | Wasm threads later | Decided |
| FFI | Polymorphic foreign modules mapped by manifest | Direct ESM imports | C ABI | Wasm imports/exports | Decided |
| Inline scripts | Declaration-level `@script` block with a typed export surface and opaque foreign body; target-bound, unsafe boundary | Inline ESM first | Inline C/C++ second | Deferred | Proposed |

### 3.3 Semantics rule

A feature belongs in the portable core only if all targets can implement the same observable behavior.

If a feature cannot meet that rule, it belongs in a mode:

- `web` mode
- `system` mode
- future platform-specific modes

### 3.4 Semantic policies versus modes

There is an important distinction between a **mode** and a **semantic policy**.

| Layer | Changes | Example |
|---|---|---|
| Mode | Capabilities, standard library, memory rules, target constraints | `web`, `system`, `app` |
| Semantic policy | Observable language behavior | `math.integers`, `strings.model` |

Modes do **not** rewrite core syntax or core semantics. Semantic policies can change observable behavior, but only when explicitly set in the manifest and recorded in build metadata.

Proposed manifest section:

```json
{
  "math": {
    "integers": "wrapping"
  },
  "strings": {
    "model": "utf8"
  }
}
```

- `math.integers`: `wrapping`, `safe-checked`, or `arbitrary`
- `strings.model`: `utf8` or `native`

Local overrides remain explicit regardless of the manifest policy:

```ns
let wrapped = a +% b
let checked = a.checked_add(b)!
let wrapped_call = a.wrapping_add(b)
```

This separation resolves the apparent conflict between “modes do not change core semantics” and “the manifest can change integer or string behavior.” The mode controls capabilities. The semantic policy controls the math and string contract.

### 3.5 Error model: effect-tracked explicit bubbling

Recoverable errors are explicit in the function signature:

```ns
fn parse_age(input: str) -> i32 ! InvalidAge | ParseIntError {
    if input.is_empty() {
        return !InvalidAge("Age cannot be empty")
    }
    return input.to_int()!
}
```

Call sites must explicitly propagate a possible error:

```ns
fn process_user_data() -> str ! InvalidAge | ParseIntError {
    let age = parse_age(get_input())!
    return "User is verified"
}
```

Rules:

- `! E` in a return type declares the error effect.
- `!ErrorValue(...)` constructs and returns a failure.
- `fallible_call()!` propagates the error to the caller.
- A call to a fallible function without `!` is a compile-time error.
- Multiple error types may be combined, for example `! InvalidAge | ParseError`.
- Recoverable errors are not hidden exceptions.
- Unrecoverable programmer bugs use `panic` or `abort` and produce a clean stack trace.
- Every effect carries its construction or propagation span. Diagnostics point at
  the exact call or failure rather than at the enclosing function.
- Effects merge by nominal type in a deterministic order. Duplicate effects keep
  the first site.
- A `match` consumes a scrutinee effect only when the `Ok`/`Err` shape is
  exhaustive; a non-exhaustive match reports `NS4010`, not `NS4011`.
- Error types must be nominal and constructible. Multi-variant enum error
  constructors are rejected as ambiguous instead of silently choosing a variant.
- Imported error types resolve through exported type surfaces. Imported type
  definitions are registered before local function signatures are resolved so a
  signature such as `-> i32 ! ImportedError` is checked against the real type.

This is not a full algebraic effect system. It is a **typed error effect** with explicit propagation.

Target representations:

- Native/Wasm: value plus tag or error register; no stack-unwinding requirement.
- JS/TS/Python: generated `try`/`catch` at the boundary, with source-level semantics preserved.
- FFI boundaries: errors must be mapped explicitly; foreign exceptions never cross silently.

### 3.6 Structured concurrency

Tasks and channels are part of the portable concurrency model:

```ns
fn main() {
    let channel = Channel.create<str>()

    spawn || {
        channel.send("Hello from a parallel task!")
    }

    let message = channel.receive()
    print(message)
}
```

Rules:

- `spawn` creates a structured task owned by its enclosing scope.
- The parent waits for or cancels child tasks; detached tasks are not allowed in the portable core.
- Channels use message passing.
- Shared mutable state is not allowed in the portable core.
- `SharedArrayBuffer` is an explicit web-mode opt-in, not the default.
- Task failures propagate through the parent scope.
- Channel close, cancellation, and backpressure must have defined behavior.

Target mapping:

- Native: OS thread pool.
- Web/JS: Web Workers with `postMessage` and structured-clone serialization.
- Wasm: threads when available; otherwise worker-based fallback.
- Async I/O is a separate concern from CPU parallelism.

### 3.7 JavaScript output fidelity

The JavaScript backend follows a hybrid rule:

- **Exact semantics in the core.**
- **JS-native behavior at explicit web boundaries.**

This means:

- `core` code keeps Neo Astra semantics, even if that requires small generated helpers.
- `web` boundary types can map directly to JavaScript objects, arrays, promises, and strings.
- `strings.model: "native"` can opt into native JS string representation, but byte-level string operations must still have defined behavior.
- Generated code should be readable ESM, not a framework runtime.
- The compiler should not introduce implicit JS coercions into Neo Astra semantics.
- The boundary must be visible in the type system or in explicit `web` mode code.

The generated JavaScript should look natural to a web developer while remaining faithful to the source contract.

---

## 4. Targets versus modes

The original specification mixes two different concepts. They should be separated.

| Concept | Meaning | Examples |
|---|---|---|
| **Target** | What the compiler emits | `native`, `wasm`, `js` |
| **Mode** | What language features, libraries, and memory rules are allowed | `core`, `app`, `web`, `system` |

The user-facing name is **mode**. The compiler may refer to the same concept internally as a **profile**.

### 4.1 Portable core

The `core` mode is the universal subset.

It should include:

- Fixed-width integers
- Floating-point numbers
- Booleans
- Unicode strings
- Byte buffers
- Arrays
- Maps
- Structs
- Enums
- Pattern matching
- Functions and closures
- Modules
- `Option<T>`
- `Result<T, E>`
- Effect-tracked errors with `!`
- Channels and scoped tasks
- Managed memory with no user-visible finalizers

It should exclude:

- Raw pointers
- Direct syscalls
- DOM APIs
- npm imports
- Raw OS thread APIs
- Detached tasks and shared mutable state
- Manual allocator control
- FFI

### 4.2 Web mode

The `web` mode adds:

- DOM APIs
- Browser storage
- `fetch`
- Event loop and async
- npm/JS interop
- Web Workers
- Web-specific string and number boundaries

### 4.3 System mode

The `system` mode adds:

- Raw pointers
- Arenas
- Manual allocation
- OS threads
- Filesystem, networking, process APIs
- C ABI FFI
- Unsafe blocks

### 4.4 Illustrative syntax

This is not final syntax. It is only a way to express the concept.

```ns
@mode(core)
fn parse(input: str) -> Ast ! ParseError {
    // portable code
}

@mode(system)
fn read_file(path: str) -> bytes ! IoError {
    // system-only code
}
```

The compiler must reject attempts to call a system-only function from core code unless the call is explicitly marked as a boundary.

### 4.5 Fluid modes and tags

Neo Astra is designed to be **fluid at development time**: the same language can be used for scripting, application development, web frontends, or low-level systems work.

The user-facing mechanism is a **mode**. A mode is a named bundle of:

- Available capabilities
- Standard library surface
- Memory rules
- Target constraints
- Default optimization settings (not semantics)

#### Built-in modes

| Mode | Purpose | Memory | Typical targets |
|---|---|---|---|
| `core` | Portable libraries and shared code | GC by default; no user-visible finalizers | All targets |
| `app` | High-level applications and scripts | GC by default | Native, Wasm, JS |
| `web` | Browser and frontend work | Host GC | JS, `wasm32-web` |
| `system` | Low-level systems work | GC by default or manual memory | Native, `wasm32-wasi` |

`app` mode includes scripting-style code, top-level statements, and normal application development. Additional modes such as `game`, `data`, or domain-specific modes can be added later by composing capabilities, not by inventing new semantics.

#### How a mode is selected

A mode is selected at **build time**, using this strict pipeline:

1. `--mode` CLI flag
2. `ASTRA_MODE` environment variable
3. Target-bound mode in the manifest, for example `targets.web.mode`
4. `build.defaultMode` in `core.json` or `core.toml`
5. Least-privileged inference from the static call graph

After the mode is resolved, source tags are checked. A source tag does not secretly change the project's mode; it declares what the module needs, and the build fails if the selected mode does not provide it.

Example manifest:

```json
{
  "build": {
    "defaultMode": "app"
  },
  "modes": {
    "web": {
      "capabilities": ["dom", "fetch", "async"]
    },
    "system": {
      "capabilities": ["fs", "net", "threads", "ffi"]
    }
  },
  "targets": {
    "web": {
      "mode": "web",
      "backend": "js"
    },
    "native": {
      "mode": "system",
      "backend": "c"
    }
  }
}
```

Example source requirements (illustrative syntax):

```ns
@requires(system)
@tags(fs, ffi)
module io
```

This module compiles only when the resolved mode provides `system`, `fs`, and `ffi`.

#### Scoped mode blocks

A single environment — a source file, module, or project — can contain multiple modes. Developers can annotate a block with `{@mode:...; ... }`.

Illustrative syntax:

```ns
{@mode:core;
    fn parse(input: str) -> Ast ! ParseError {
        // portable code
    }
}

{@mode:web;
    fn mount(root: Element) {
        // browser-only code
    }
}
```

The `;` separates the mode annotation from the block body.

Mode blocks are lexical capability scopes:

- They do not introduce a separate grammar.
- They enable or restrict capabilities inside the block.
- They do not hide requirements from callers. If a `core` function contains a reachable `system` block, the function itself requires `system` mode.
- Nested blocks may narrow capabilities, but they may not silently widen them beyond the parent context.
- If a file is marked `@requires(core)`, a nested `{@mode:system;}` block is a compile error unless the enclosing file or function signature is explicitly upgraded to `@requires(system)`.
- If a core function calls a system-only function, the requirement is viral: the caller must become system-required or use an explicit boundary.

This is similar to an `unsafe` block: the annotation is a local opt-in, not a new language.

The compiler should explain the result:

```text
Resolved mode: system (via manifest target: native)
  ├── src/main.ns -> uses app capabilities
  └── src/io.ns   -> forces system mode
       └── line 42: @requires(system) due to raw pointer usage
```

#### Mode inference

If a mode is not explicitly specified, the compiler infers the minimum capability set required by the scope:

1. Parse and resolve imports.
2. Collect direct requirements from module/item `@requires`, `@tags`, foreign
   modules, manual-memory scopes, and capability-bearing built-ins.
3. Build the local static call graph.
4. Propagate requirements over calls to a fixed point, including recursion and
   mutual recursion.
5. Join scoped requirements, allowing narrowing and rejecting silent widening
   beyond an explicit enclosing requirement.
6. Select the least-privileged built-in mode that contains every requirement.
7. Record the mode, requirement, per-module inference, reasons, and explanation
   in build/compiler metadata.

Mode join is containment, not numeric rank: `app` contains `core`, `web`
contains `core`/`app`, and `system` contains `core`/`app`. `web` and `system` are
incomparable. If no single built-in mode fits, the build fails with `NS5006`,
listing the conflicting requirements. A project joins every module requirement;
an explicit module requirement narrower than the project requirement is
`NS5001`.

Rules for inference:

- Explicit annotations always win. An explicit `--mode` or request `mode` also
  disables inference and remains authoritative, but is still validated by the
  checker.
- Inference never changes core syntax or semantics.
- Inference is static. Dynamic calls and dynamic imports need explicit declarations.
- `astra explain` should show why a mode was selected; until the command lands,
  `ModeInference::explain()` and compiler metadata carry the same reason tree.
- The inferred mode is part of the build cache key.

The compiler should be **smart but explainable**. It should not silently guess between incompatible modes.

#### Rules for fluid modes

1. **Modes are compile-time, not runtime.**
   A built artifact does not switch from high-level to low-level semantics while running. If runtime behavior must differ, use capability checks or compile-time conditional blocks, not a mode change.

2. **One grammar.**
   The parser accepts the full language everywhere. Modes gate features during type checking and code generation. A raw pointer type is parsed in every mode, but it is only valid in `system` mode.

3. **Modes add capabilities; they do not rewrite the core.**
   A mode does not change the meaning of `+` or `str`. Those are controlled by explicit `math` and `strings` policies in the manifest, not by switching modes. `system` mode adds raw pointers, arenas, and manual allocation; it does not silently change integer or string behavior.

4. **The resolved mode is recorded.**
   Build metadata records the selected mode so CI and release builds are reproducible. `ASTRA_MODE` is a convenience, not hidden magic.

5. **Custom modes compose capabilities.**
   Users can define project-specific modes, but they cannot define different core semantics. This prevents the language from fragmenting into incompatible dialects.

6. **A project can build multiple targets with different modes.**
   A web target can use `web` mode while a native target uses `system` mode. Both can share libraries written in `core` mode.

The design principle is:

> **Fluid at development time, deterministic at build time.**

---

## 5. Runtime philosophy

### 5.1 What “zero-runtime” should mean

“Zero runtime” is not literally achievable for a language with strings, arrays, bounds checks, allocation, and panics. Even C has a runtime startup layer and libc.

What is achievable:

> **No language VM. No framework. Only the runtime routines the program actually uses.**

Recommended phrasing:

> **Zero-VM architecture with a pay-as-you-go runtime.**

### 5.2 How pay-as-you-go works

- **JavaScript target:** Emit small ESM helpers only when needed. No framework, no hidden engine.
- **Native target:** Link only used runtime functions. Use linker garbage collection (`--gc-sections`).
- **Wasm target:** Only include used routines. Strip unused code with `wasm-opt` and linker GC.
- **Portable core:** Programs that do not use a feature should not pay for that feature.
- **Memory:** The GC runtime is linked only when managed allocation is used. `@memory:manual` programs do not include it.

### 5.3 What we should not promise

Avoid claiming:

- “There is no runtime.”
- “The output is always identical to hand-written JavaScript.”
- “Any npm package can be perfectly tree-shaken.”

Those claims are easy to disprove.

---

## 6. Memory model

The core decision is now: **garbage collection is the default, but it is never forced.** Developers can opt out and manage memory explicitly in `system` mode.

### 6.1 Options

#### Option A — Tracing garbage collection everywhere

**Pros:**

- Simplest and most uniform semantics
- Pleasant for scripting
- Cycles handled automatically

**Cons:**

- Native runtime is larger
- Latency and memory overhead
- Harder to call “zero-runtime”
- Less control for systems programming

#### Option B — Reference counting plus arenas

**Pros:**

- Deterministic destruction
- Smaller runtime than a tracing GC
- Good fit for systems programming
- Can be pay-as-you-go

**Cons:**

- Cycles leak unless a cycle collector is added
- Reference counting overhead
- More complex than tracing GC in some ways

#### Option C — Ownership and borrow checking

**Pros:**

- Excellent systems performance
- No GC
- Strong safety story

**Cons:**

- Much steeper learning curve
- Less pleasant for scripting and web development
- Harder to market as a universal language

#### Option D — Hybrid model

**Pros:**

- Managed memory for normal code
- Explicit control in system code
- Keeps the portable core pleasant

**Cons:**

- More than one memory strategy to document and test
- Requires a clear boundary between managed and manual code

### 6.2 Decision: GC by default, manual memory by choice

| Context | Memory strategy |
|---|---|
| Manifest `memory.default: "auto"` | Managed memory with garbage collection |
| Manifest `memory.files` override | Per-file `auto` or `manual` policy |
| `{@memory:auto; ... }` | Managed memory with garbage collection |
| `{@memory:manual; ... }` | Explicit allocation, deallocation, and arenas; no GC |
| JS target | Host garbage collector; manual memory is not supported |
| Native target | Neo Astra GC in `auto`; explicit memory in `manual` |
| Wasm target | Neo Astra GC in `auto`; explicit memory in `manual` |

Portable code must not be able to observe when memory is reclaimed. That means:

- No user-visible finalizers in the portable core
- No guarantees about collection timing
- No raw pointer arithmetic in the portable core
- Weak references, if added, must have defined semantics

### 6.3 Opting out of GC

Manual memory management is explicit and scoped:

```ns
{@mode:system; @memory:manual;
    fn allocate_buffer(size: usize) -> *u8 ! MemoryError {
        let buffer = Memory.alloc(size)!
        return buffer
    }

    fn release_buffer(buffer: *u8) {
        Memory.free(buffer)
    }
}
```

Proposed memory operations:

- `Memory.alloc(size)` — allocate a raw block
- `Memory.free(pointer)` — release a raw block
- `Memory.realloc(pointer, new_size)` — resize a raw block
- `Region` — arena-style allocation with bulk release
- `Owned<T>` — explicit ownership wrapper for manual scopes

Rules:

- Memory policy is set by the manifest and can be overridden per file or per block.
- Manual scopes cannot use GC-managed types without an explicit boundary.
- The compiler must reject manual memory operations in `auto` scopes.
- JS output uses the host GC; true manual memory is a native/Wasm capability.
- A program that opts out of GC does not link the GC runtime.
- If a manual file is part of a JS target, the compiler must reject the build.

### 6.4 Proposed GC design

The first native/Wasm GC should be:

- Precise
- Non-moving
- Mark-sweep or similar
- Able to run on a dedicated runtime thread later

Non-moving is important because `system` mode exposes raw pointers, FFI, and manual memory. A moving collector would invalidate those pointers unless pinning is added. Generational and incremental collection can come later.

### 6.5 Open memory questions

- Can manual code allocate GC-managed objects?
- Can GC code hold raw pointers into a manual region?
- Do we expose `Drop` or destructors?
- Do we expose weak references in v0.1?
- How does FFI ownership work at the boundary?
- How do we test memory behavior across targets?
- Should `system` mode default to GC or manual memory? Current decision: GC by default, manual on request.

---

## 7. Compilation architecture

### 7.1 Pipeline

The TypeScript orchestrator resolves the manifest, mode, and target, then invokes the Rust core. The Rust core owns the shared frontend and IR.

```text
.ns source
    ↓
lexer
    ↓
parser
    ↓
resolver
    ↓
type checker
    ↓
typed HIR
    ↓
Astra IR (SSA)
    ↓
optimizer
    ↓
┌──────────────┬──────────────┬──────────────┐
│ JS/TS backend│ C backend    │ LLVM backend │
│              │ -> clang     │  (later)     │
└──────┬───────┴──────┬───────┴──────┬───────┘
       ↓              ↓              ↓
   ESM / TS     native / wasm      native
```

Backends are optional. A web-only build should not contain native code generation, and a native build should not contain the JavaScript backend.

### 7.2 Backend implementation order

This is the order in which backends become runnable, not the order in which the architecture is designed.

1. **JavaScript backend first**
   - Fastest path to a usable language
   - Enables a browser playground
   - Produces readable ESM output

2. **C backend second**
   - Compile to C, then use `clang`
   - Native output
   - `wasm32` output
   - Natural C ABI FFI
   - Easier to debug than a direct machine-code backend

3. **LLVM backend later**
   - Optimization upgrade
   - Not required for the first usable version
   - Replace the C backend without changing the frontend

### 7.3 Why not start with LLVM?

LLVM is an optimization multiplier, not a bootstrap tool.

Starting with LLVM adds:

- Large build dependency
- Complex IR lowering
- Difficult debugging
- More time before a usable language exists

A C backend is less glamorous but gets native and Wasm output much sooner. LLVM can come after the language semantics are stable.

### 7.4 Why not write a direct machine-code backend?

That is a multi-year project by itself, especially once you include:

- Register allocation
- Instruction selection
- Calling conventions
- Object file formats
- Linkers
- Debug information
- Platform ABIs

Use C or LLVM instead.

### 7.5 Hybrid compiler architecture: TypeScript orchestrator + Rust core

The compiler uses a split architecture:

| Layer | Language | Responsibilities |
|---|---|---|
| Orchestrator | TypeScript | CLI, manifest parsing, plugin loading, NoPM package resolution, LSP, JS/TS code generation |
| Core engine | Rust | Lexer, parser, resolver, type checker, capability inference, IR, optimizer, Nova interpreter |
| Low-level backends | Rust | C, Wasm, LLVM, future direct machine code |
| Web backends | TypeScript | JavaScript ESM, TypeScript output |

The Rust core is the single source of truth for parsing and semantics. TypeScript must never re-parse `.ns` source with a second grammar.

The boundary between them should be a stable compiler API:

- A versioned JSON protocol for CLI and tooling.
- A WASM build of the Rust core for the browser playground.
- A native Rust binary for full builds and low-level backends.
- The orchestrator passes source text plus resolved manifest data in; the core returns typed HIR/IR, diagnostics, and mode/inference metadata.

Why this split:

- TypeScript is excellent for CLI tools, web tooling, LSP, and JS code generation.
- Rust is excellent for parsing, type checking, IR optimization, and native/Wasm codegen.
- The web-facing tool stays native to the web ecosystem.
- The heavy compiler core stays fast and memory-safe.
- Backends can be included or excluded without shipping the whole compiler everywhere.

Risks:

- Cross-language boundary overhead
- Protocol versioning
- Debugging across two runtimes

Mitigation: keep the boundary narrow, version it explicitly, and make the Rust core runnable as both a native binary and a WASM module.

### 7.6 Browser playground

Two options:

1. **Compile the Rust compiler to Wasm**
   - Fully client-side playground
   - Larger download
   - Cooler and more self-contained

2. **Hosted compile API**
   - Faster to ship
   - Smaller client
   - Requires a server

Recommendation: start with a hosted API if speed matters, or compile to Wasm if the playground is a core part of the product identity.

### 7.7 One frontend, selectable backends

“Multi-compiler” should mean **modular backend selection**, not several independent compilers with duplicated frontends.

Separate compilers would cause:

- Semantic drift between targets
- Duplicated parser and type-checker fixes
- Inconsistent diagnostics
- A much larger testing matrix
- Higher maintenance cost

Recommended structure:

```text
orchestrator/          # TypeScript
  packages/
    cli/               # astra CLI
    manifest/          # core.json / core.toml parsing, validation, formatting
    compiler-client/   # JSON-RPC client for the Rust core
    lsp/               # language server
    plugins/           # package-source and backend plugins
    nopm/              # NoPM universal package manager (planned)
    codegen-js/        # JavaScript ESM
    codegen-ts/        # TypeScript output
  tests/

crates/                # Rust
  astra-core/          # lexer, parser, resolver, type checker, IR
  astra-capabilities/  # mode and dependency inference
  astra-nova/          # typed-HIR development interpreter
  astra-codegen-c/     # C output, then clang for native/Wasm
  astra-codegen-wasm/  # future direct Wasm backend
  astra-codegen-llvm/  # future LLVM backend
  astra-compiler-api/  # stable JSON/WASM boundary
  astra-wasm/          # raw linear-memory Wasm boundary

scripts/               # bootstrap, CI, and boundary test runners
```

Use Cargo features to compile only the selected Rust backends into the core. A web-only build should not include LLVM or C code generation. A native build should not include the JavaScript backend.

The shared Rust core is what keeps one language semantically consistent. The TypeScript layer is replaceable tooling around that core, not a second language implementation.

### 7.8 Transpiler bootstrap and direct codegen

The first version should compile into another language, then use that language’s toolchain:

| Target | Bootstrap path | Later path |
|---|---|---|
| JavaScript | Emit ESM directly | Keep direct backend |
| TypeScript | Emit `.ts` directly | Keep direct backend |
| Native | Emit C, compile with `clang` | LLVM, then direct machine code |
| Wasm | Emit C, compile with `clang --target=wasm32` | Direct Wasm backend |
| PHP | Emit PHP | Community or later backend |

This gives native and Wasm output without building a machine-code backend ourselves.

The long-term plan can include a direct machine-code backend, but it is not a v0.1 requirement.

PHP is a much larger semantic jump than TypeScript or JavaScript. It should be treated as a later or community backend, not a launch requirement.

### 7.9 Backend and mode compatibility

Not every mode can target every backend.

| Backend | Compatible modes | Notes |
|---|---|---|
| JavaScript | `core`, `app`, `web` | Host GC; no manual memory |
| TypeScript | `core`, `app`, `web` | Same runtime model as JavaScript |
| C → native | `core`, `app`, `system` | Supports manual memory |
| C → Wasm | `core`, `app`, `system` | Supports manual memory with Wasm constraints |
| LLVM | all modes | Later milestone |
| PHP | `app`, `web` later | Not a v0.1 target |

The compiler must reject incompatible combinations with a clear diagnostic.

---

## 8. Manifest and build system

### 8.1 What the manifest does

The project manifest is either `core.json` or `core.toml`. It should:

- Resolve dependencies
- Select target-specific dependencies
- Define build targets
- Declare capabilities
- Select build profiles
- Set the default memory policy

It should **not** be described as the thing that performs tree-shaking. Tree-shaking happens on the compiler’s IR call graph.

Exactly one manifest file must exist at the project root. If both `core.json` and `core.toml` exist, the build fails with an explicit error.

Both formats use the same semantic schema. JSON is familiar to web developers; TOML is more compact and supports comments. The compiler parses both into the same internal manifest structure.

### 8.2 Proposed manifest schema

JSON version:

```json
{
  "$schema": "https://neovastra.dev/schema/core-0.1.json",
  "package": {
    "name": "my-universal-app",
    "version": "0.1.0",
    "edition": "2026"
  },
  "build": {
    "defaultTarget": "native",
    "defaultMode": "app",
    "profile": "release"
  },
  "math": {
    "integers": "wrapping"
  },
  "strings": {
    "model": "utf8"
  },
  "memory": {
    "default": "auto",
    "files": {
      "src/io.ns": "manual",
      "src/drivers/*.ns": "manual"
    }
  },
  "targets": {
    "native": {
      "backend": "c",
      "triple": "x86_64-pc-windows-msvc",
      "mode": "system"
    },
    "web": {
      "backend": "js",
      "module": "esm",
      "mode": "web"
    },
    "wasm": {
      "backend": "wasm32",
      "wasi": false
    }
  },
  "dependencies": {
    "fast-crypto": {
      "version": "1.2",
      "targets": ["native", "wasm"]
    },
    "ui-framework": {
      "version": "0.4",
      "targets": ["web"]
    }
  },
  "capabilities": {
    "web": ["dom"],
    "native": ["fs"]
  }
}
```

Equivalent TOML version:

```toml
[package]
name = "my-universal-app"
version = "0.1.0"
edition = "2026"

[build]
defaultTarget = "native"
defaultMode = "app"
profile = "release"

[math]
integers = "wrapping"

[strings]
model = "utf8"

[memory]
default = "auto"

[memory.files]
"src/io.ns" = "manual"
"src/drivers/*.ns" = "manual"

[targets.native]
backend = "c"
triple = "x86_64-pc-windows-msvc"
mode = "system"

[targets.web]
backend = "js"
module = "esm"
mode = "web"

[targets.wasm]
backend = "wasm32"
wasi = false

[dependencies.fast-crypto]
version = "1.2"
targets = ["native", "wasm"]

[dependencies.ui-framework]
version = "0.4"
targets = ["web"]

[capabilities]
web = ["dom"]
native = ["fs"]
```

### 8.3 Strict manifest rules

The manifest is strict on purpose:

- Exactly one of `core.json` or `core.toml` must exist.
- Both formats validate against the same versioned semantic schema.
- Reject unknown keys.
- Reject duplicate keys.
- Reject wrong value types.
- Reject invalid target, mode, memory, `math.integers`, or `strings.model` values.
- Reject dependency paths that escape the project root.
- Reject missing or invalid FFI mappings for required foreign modules.
- Require `astra check` to validate the manifest before building.
- `astra fmt` should canonicalize both `core.json` and `core.toml`.

This prevents a manifest from becoming a garbage dump of compile-time surprises.

The `//`-only comment rule applies to `.ns` source files. `core.toml` follows standard TOML `#` comment rules; `core.json` has no comments.

### 8.4 Memory policy in the manifest

Memory policy is set globally, then overridden by more specific rules.

```json
{
  "memory": {
    "default": "auto",
    "files": {
      "src/io.ns": "manual",
      "src/drivers/*.ns": "manual"
    }
  }
}
```

- `"auto"` means managed memory with GC.
- `"manual"` means no GC; the file uses explicit allocation and deallocation.
- `memory.files` supports exact paths and glob patterns.
- A source-level `{@memory:...;}` block overrides the file policy.
- A file-level `{@memory:...; ... }` wrapper overrides the manifest.
- The project default applies only when no more specific rule exists.

Precedence:

1. Block-level `{@memory:...;}` annotation
2. File-level `{@memory:...; ... }` wrapper
3. Manifest `memory.files` override
4. Manifest `memory.default`

If a file declares `manual`, the compiler follows manual for that file even when the project default is `auto`.

Manual memory files cannot target JavaScript. If a manual file is part of a JS build, the compiler must produce a clear error instead of pretending the host GC is manual memory.

### 8.5 Inferred dependencies

When a dependency is not explicitly target-scoped, the compiler can infer the variant from static imports and the call graph:

- A UI framework used only in `web` mode selects its web variant.
- A crypto library used by `core` or `system` code selects the native/Wasm variants.
- Variants that are not reachable are not downloaded or linked.

Rules:

- Inference uses static analysis only.
- Dynamic imports and reflective calls need explicit declarations.
- The resolved dependency graph is recorded in `core.lock.json`.
- Adding or removing a call can change the selected variant, so CI should show the diff.

### 8.6 Package sources

Neo Astra should not require its own registry at launch. It should integrate with existing ecosystems:

- Local path packages
- Git packages
- npm for JS/TS
- Cargo for Rust
- Later adapters for NuGet, PyPI, and other ecosystems

The manifest records package sources and target compatibility. `core.lock.json` records the resolved graph, integrity hashes, and source package-manager metadata.

```json
{
  "nova": {
    "entry": "src/main.ns",
    "watch": ["src/**/*.ns"]
  },
  "registry": {
    "default": "https://registry.neovastra.dev",
    "defaultSource": "nova"
  },
  "packageSources": {
    "path": {
      "enabled": true
    },
    "git": {
      "enabled": true
    },
    "npm": {
      "enabled": true,
      "lifecycleScripts": false
    },
    "cargo": {
      "enabled": true
    }
  }
}
```

Rules:

- Pin exact versions in the lockfile.
- Verify integrity hashes.
- Disable package lifecycle scripts by default.
- Require explicit opt-in for scripts that run during install.
- Keep ecosystem-specific build steps isolated per target.

### 8.6.1 NoPM and the universal registry

D25 makes the package layer first-class:

- `nopm` is the single package manager for Nova and compiled builds.
- It resolves path, Git, npm, and Cargo first, with later PyPI, NuGet, and OCI adapters.
- Package IDs are source-qualified, such as `npm:react@19.0.0` or `nova:@scope/name@1.2.3`.
- `core.lock.json` records the universal graph, target variants, capabilities, integrity hashes, and lifecycle-script policy.
- `registry.neovastra.dev` is an optional federated gateway: it hosts `nova:` packages and can proxy or mirror upstream registries.
- Local development must work without the gateway.
- “Any ecosystem” means any ecosystem with an adapter and target-compatible bindings.

`[nova]` configures the development entry point and watch roots. `[registry]`
configures the default gateway and source policy. The manifest remains exactly
one of `core.json` or `core.toml`.

### 8.7 FFI mappings in the manifest

Foreign modules are declared abstractly in source and resolved per target:

```ns
foreign module math_engine {
    fn compute_matrix(data: [f64]) -> [f64]
}
```

Manifest mapping:

```json
{
  "ffi": {
    "math_engine": {
      "native": {
        "source": "./libmath.so",
        "type": "c-abi"
      },
      "typescript": {
        "source": "./npm-package/index.js",
        "type": "esm"
      }
    }
  }
}
```

The compiler must reject a target that has no mapping for a required foreign module.

---

## 9. Tree-shaking reality

### 9.1 Compiler-owned code

For code written in Neo Astra:

- Build a reachability graph from entry points and exported symbols
- Remove unreachable functions, types, and modules
- Do this on the typed IR, not on raw source text
- Run again after monomorphization

### 9.2 Native and Wasm

- Use linker garbage collection
- Strip debug sections in release builds
- Run `wasm-opt` for Wasm output
- Verify final binary size in CI

### 9.3 JavaScript

- Emit ESM
- Let a bundler perform final shaking
- Preserve side-effect metadata
- Avoid generating runtime code that references unused features

### 9.4 External packages

Do not promise exact tree-shaking of arbitrary npm packages.

Instead:

- Treat external packages as opaque boundaries by default
- Use their side-effect metadata when available
- Generate typed wrappers
- Allow package authors to declare purity
- Report bundle composition so users can see what was included

### 9.5 Honest promise

> Compiler-owned code is precisely shaken. External packages are shaken as well as their metadata allows.

That is achievable. “Every dependency is reduced to the exact subset used” is not, in general.

---

## 10. FFI and cross-language interop

### 10.1 Abstract foreign modules

Source declares the foreign interface without committing to a target implementation:

```ns
foreign module math_engine {
    fn compute_matrix(data: [f64]) -> [f64]
}
```

The manifest maps that module per target:

```json
{
  "ffi": {
    "math_engine": {
      "native": {
        "source": "./libmath.so",
        "type": "c-abi"
      },
      "typescript": {
        "source": "./npm-package/index.js",
        "type": "esm"
      }
    }
  }
}
```

This is the polymorphic FFI model: one source contract, target-native bindings.

### 10.2 Native FFI

Base layer: **C ABI**.

- C: direct
- C++: require a C shim
- Rust: `extern "C"` plus `#[no_mangle]`
- Other languages: expose a C ABI

The FFI boundary must define:

- Ownership and lifetime rules
- Error propagation
- Threading rules
- String encoding
- Struct layout and alignment
- Calling convention

### 10.3 JavaScript interop

- Generate direct ESM imports where possible
- Generate typed wrappers from `.d.ts` files
- Do not rely on “seamless” structural magic
- Gate Node-only APIs out of browser targets
- Handle `null`/`undefined`, numbers, and strings deliberately

### 10.4 Wasm interop

- `wasm32-web`: browser target, requires JS glue for DOM, `fetch`, storage, etc.
- `wasm32-wasi`: server/edge target, uses WASI APIs
- Do not claim that browser Wasm has WASI by default

### 10.5 Type mapping

Foreign types must map predictably:

| Neo Astra type | JS/TS | Native C ABI |
|---|---|---|
| `i32`, `i64`, `f64` | `number` / `bigint` | Fixed-width integer / `double` |
| `str` | `string` | `const char*` + length |
| `bytes` | `Uint8Array` | Pointer + length |
| `[T]` | `Array<T>` | Pointer + length |
| `Option<T>` | `T \| null` at boundary | Tagged value or nullable pointer |
| `T ! E` | Thrown error mapped at boundary | Error tag + value |

### 10.6 Inline script blocks

V1 proposes an inline form of the same foreign-module model: a declaration-level
`@script` block in a `.ns` file. The header switches the parser into raw
foreign-source mode, declares a typed export surface, and names a module-local
namespace.

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
```

Rules:

- The foreign body is opaque to Nova parsing and type checking.
- Calls are checked against the declared `@exports` signatures.
- The required `@mode` declaration is trusted, not verified.
- The explicit `@end` sentinel terminates raw capture; brace counting is not
  language-neutral.
- JavaScript/TypeScript targets come first, then C/C++ through a C ABI.
- Java requires a JVM backend; Python requires an embedded runtime and is not V1.
- Unsupported language/target combinations are compile-time errors.
- Nova rejects foreign script calls in V1 rather than executing untrusted host
  code.

This extends D10; it does not replace `foreign module`, the manifest `ffi`
mapping, or NoPM package adapters. The full proposal is in
[`Design/V1/10-inline-script-interop.md`](./Design/V1/10-inline-script-interop.md).

### 10.7 Honest promise

> Typed, generated, and checked bindings.

Not “seamless FFI.” FFI is always a boundary with rules.

---

## 11. Standard library and capabilities

The standard library should be split by mode.

### 11.1 Core

- Strings
- Byte buffers
- Arrays
- Maps
- Math
- `Option`
- `Result`
- Iterators
- Formatting
- Basic time and duration types
- Channels
- Scoped tasks / `spawn`

### 11.2 Web

- DOM
- `fetch`
- Storage
- Events
- Web Workers
- Browser-specific APIs

### 11.3 System

- Filesystem
- Networking
- Processes
- Threads
- Environment
- Raw memory
- OS-specific APIs

### 11.4 Capability declarations

Dependencies should be able to declare what they need:

```json
{
  "capabilities": {
    "web": ["dom", "fetch"],
    "native": ["fs", "net"]
  }
}
```

The compiler and package manager can then reject incompatible combinations early.

---

## 12. v0.1 scope

### 12.1 Goal

Build the full architecture with every target interface designed from day one. The first runnable slice is web-focused: syntax, type system, error model, JS/TS output, and playground.

Nothing should need to be rewired when native, Wasm, FFI, or concurrency implementations land later.

### 12.2 In scope

- TypeScript orchestrator
- Rust core engine
- Shared lexer, parser, resolver, type checker, and IR
- `fn`, `let`, `struct`, `enum`, `match`
- Modules and imports
- `i32`, `i64`, `f64`, `bool`, `str`, `bytes`
- Arrays and maps
- `Option<T>` and `Result<T, E>` as library types
- Effect-tracked errors with `!`
- Structured concurrency syntax and type checking
- Simple monomorphized generics
- Basic type inference
- Pattern matching
- String formatting
- CLI:
  - `astra new`
  - `astra run`
  - `astra build --target web`
  - `astra fmt`
  - `astra check --json`
  - `astra ast --json`
  - `astra explain`
- `core.json` / `core.toml` parsing
- `core.lock.json`
- Browser playground
- Conformance test suite
- Source maps for JavaScript output
- Canonical formatting rules

### 12.3 Not required for the first runnable slice

These interfaces are still designed and scaffolded, but their full implementations can land in later milestones:

- Native backend implementation
- Wasm backend implementation
- Native GC implementation
- Manual-memory runtime
- FFI runtime
- Custom package registry
- Async I/O runtime
- Native thread-pool runtime
- Web Worker code generation
- Advanced generics
- Macros
- Incremental compilation
- IDE language server

### 12.4 Why this scope

It proves the most important things:

- The syntax is usable
- The type system works
- The error model works
- The IR is viable
- The JavaScript backend produces real programs
- The playground gives people something to try immediately
- The architecture has no dead ends that require a rewrite

---

## 13. Roadmap

### M0 — Specification and conformance

- ✅ Write `Docs/spec-v0.1.md`
- ✅ Finalize the v0.1 semantics matrix
- ✅ Define the v0.1 grammar
- Write 30–50 conformance programs
- Define expected behavior for every target

### M1 — Core engine, orchestrator, Nova, and NoPM

- ✅ Rust core workspace
- ✅ TypeScript orchestrator workspace
- ✅ Stable compiler API
- ✅ Lexer
- ✅ Parser
- ✅ AST
- ✅ Resolver
- ✅ Type checker
- Capability and mode inference
- Effect-tracked error checking
- ✅ Nova typed-HIR tree-walking interpreter (managed subset)
- `nova run`, `nova repl`, `nova watch`, and `nova test`
- `core.json` / `core.toml` parsing, validation, and formatting
- `core.lock.json` and target-specific dependencies
- NoPM universal package IDs and descriptor
- Path, Git, npm, and Cargo package-source adapters
- `nopm install`, `nopm lock`, `nopm add`, and `nopm remove`
- Diagnostics with source spans and error codes
- JSON diagnostic output

### M2 — JavaScript/TypeScript backend and playground

- Typed HIR
- Astra IR
- JavaScript ESM codegen
- TypeScript output
- Source maps
- Browser playground
- `astra build --target web`
- `astra fmt`
- `astra ast --json`
- `astra explain` for inferred modes and dependencies

### M3 — C backend, native, and Wasm

- C code generator
- Minimal runtime
- GC runtime
- `clang` integration
- Native target
- `wasm32` target
- Binary-size checks

### M4 — Registry, ecosystem expansion, and reachability

- Optional `registry.neovastra.dev` gateway
- PyPI, NuGet, and OCI adapters
- Package signing, yanking, and audit metadata
- FFI manifest mappings
- IR-level reachability analysis
- Tree-shaking reports

### M5 — FFI, standard library, and concurrency

- Polymorphic FFI
- C ABI native bindings
- Generated JS/TS bindings
- Web standard library
- System standard library
- Ownership and error rules at boundaries
- Structured concurrency runtime
- OS thread pool
- Web Worker code generation

### M6 — LLVM, direct codegen, and performance

- LLVM backend
- Direct Wasm backend
- Future direct machine-code backend
- Performance benchmarking
- Advanced optimization

---

## 14. Risks

| Risk | Why it matters | Mitigation |
|---|---|---|
| Scope creep | The vision includes native, Wasm, JS/TS, FFI, concurrency, npm, and Cargo | All-target architecture, staged implementation |
| Semantic drift | Targets behave differently | Semantics matrix, explicit policies, conformance tests |
| Runtime bloat | “Zero-runtime” claims collapse under real features | Pay-as-you-go runtime and size budgets |
| GC complexity | A fast, correct GC is a major project | Non-moving mark-sweep first; optimize later |
| Hybrid boundary | TS and Rust can drift or add overhead | Stable compiler API; Rust is the only parser |
| Semantic policy drift | `math` and `strings` policies can change behavior | Record in build metadata; test every policy |
| FFI complexity | Ownership and errors at boundaries are hard | Polymorphic bindings plus explicit boundary rules |
| Package security | npm/Cargo packages can run install scripts | Disable lifecycle scripts by default; pin and hash |
| Two runtimes | Nova could drift from compiled semantics | Reuse typed HIR; conformance tests; Nova is development-only |
| Universal registry scope | A hosted gateway can become an unbounded infrastructure project | Optional for local development; adapters first; gateway later |
| Concurrency complexity | Web Workers and OS threads behave differently | Structured tasks/channels plus conformance tests |
| Tree-shaking promises | npm packages are dynamic and opaque | Honest metadata-based shaking |
| LLVM trap | Starting with LLVM delays a usable language | C backend first |
| Ecosystem cold start | A language without libraries is not useful | Native npm/Cargo package adapters |
| Naming consistency | Mixed names confuse users, documentation, and tooling | Use **Neo Astra** consistently, with `.ns` files and the `astra` CLI |
| Documentation debt | A language is a specification, not just a compiler | Write spec and conformance tests early |

---

## 15. Decision log / review checklist

Review these one by one. Record the chosen option and any notes.

### D1 — Memory model

- [x] GC by default; manifest `memory.default` can be `auto` or `manual`; per-file and per-block overrides; host GC on JS; manual memory on native/Wasm *(confirmed)*
- [ ] Reference counting + arenas everywhere
- [ ] Ownership and borrow checking everywhere

**Decision:** GC is the default but never forced. Memory policy is configurable in `core.json` or `core.toml`, per file, and per block.  
**Notes:** Manual memory is a native/Wasm capability. JS targets use the host GC and must reject manual-memory files.

### D2 — JavaScript output fidelity

- [x] Hybrid: exact semantics in the core, JS-native behavior at explicit web boundaries *(confirmed)*
- [ ] Exact portable semantics everywhere
- [ ] Cleaner JavaScript with target-specific semantics everywhere

**Decision:** Core code preserves Neo Astra semantics; web boundaries may use native JS objects, arrays, promises, and strings.  
**Notes:** Generated output should be readable ESM, not a framework runtime. Boundary conversions must be explicit.

### D3 — Compiler implementation language

- [x] Hybrid: Rust core engine plus TypeScript orchestrator *(confirmed via D19)*
- [ ] Rust only
- [ ] TypeScript only

**Decision:** Rust owns the shared frontend and low-level backends; TypeScript owns the CLI, manifest handling, LSP, plugins, and JS/TS orchestration.  
**Notes:** See D19 for the split.

### D4 — First milestone

- [x] Whole-system architecture from day one; all components designed around shared interfaces *(confirmed)*
- [ ] Web-first and retrofit the rest later
- [ ] Native-first

**Decision:** Design all targets, modes, FFI, and backends together so nothing has to be rewired later.  
**Notes:** All components are scaffolded together, but they can become runnable in dependency order. The first runnable path is still likely JS/playground, while native/Wasm/FFI slots are wired from the start.

### D5 — Syntax style

- [x] Custom hybrid: Rust/JavaScript bareness with C/C++ sophistication *(confirmed)*
- [ ] Pure Rust-like
- [ ] Pure JavaScript-like
- [ ] Indentation-based

**Decision:** Use clean braced blocks, explicit keywords, moderate punctuation, and low-level constructs with readable names.  
**Notes:** Extensive features without ugly, symbol-heavy syntax.

### D6 — Integer semantics

- [x] Fixed-width wrapping by default; manifest policy can select `safe-checked` or `arbitrary`; explicit `+%`, `wrapping_add`, `checked_add` overrides *(confirmed)*
- [ ] Overflow is always an error
- [ ] Arbitrary precision by default

**Decision:** Default is raw, high-performance wrapping. The manifest can switch the project policy via `math.integers`. Local code can override with explicit operators/APIs.  
**Notes:** Integer policy is a semantic policy, not a mode. It must be recorded in build metadata.

### D7 — String semantics

- [x] UTF-8 bytes as the canonical core model; manifest `strings.model` can map to native target strings; native/Wasm use pointer + length *(confirmed)*
- [ ] Abstract Unicode text everywhere
- [ ] UTF-16 code units as the primary model

**Decision:** `str` is canonically UTF-8. JS/TS/Python backends may use native strings at explicit boundaries via `strings.model`.  
**Notes:** Representation can change; observable byte-level operations must still be defined. This is a semantic policy, not a mode.

### D8 — Error model

- [x] Effect-tracked explicit bubbling with `!`; panic for programmer bugs *(confirmed)*
- [ ] `Result<T, E>` everywhere
- [ ] Exceptions
- [ ] Error codes only

**Decision:** Fallible functions declare `! ErrorType`. Call sites must use `!` to propagate. Unrecoverable bugs use `panic`/`abort`.  
**Notes:** Native uses value + tag/register; JS/TS/Python may use generated `try`/`catch`.

### D9 — Concurrency model

- [x] Structured concurrency in the portable core: scoped tasks and channels, mapped to OS threads or Web Workers *(confirmed)*
- [ ] Web-only async and native-only threads
- [ ] No concurrency in the portable core

**Decision:** `spawn` tasks and `Channel` message passing are portable. Manifest/target mapping decides OS threads vs Web Workers.  
**Notes:** No shared mutable state in the portable core. `SharedArrayBuffer` is an explicit web-mode opt-in.

### D10 — FFI scope

- [x] Polymorphic manifest FFI: abstract `foreign module` mapped to npm/ESM or C ABI depending on target *(confirmed)*
- [ ] C ABI only
- [ ] JS bindings only
- [ ] No FFI in v0.1

**Decision:** Foreign modules are abstract in source and resolved by the manifest per target.  
**Notes:** JS targets emit direct imports; native targets use C ABI; C++ needs a C shim. Errors and ownership must be mapped explicitly. A proposed V1 extension adds declaration-level inline `@script` blocks under the same boundary rules; it is designed in `Docs/Design/V1/10-inline-script-interop.md` and is not part of the confirmed decision until accepted.

### D11 — Package sources and registry

- [x] Local packages plus native integration with npm, Cargo, and other ecosystem package managers; no custom registry required initially *(confirmed)*
- [ ] Custom registry in v0.1
- [ ] Local packages only

**Decision:** Rely on existing package ecosystems through package-source adapters. Record everything in `core.lock.json`.  
**Notes:** D25 makes the adapter layer a first-class `nopm` package manager and adds an optional universal registry gateway; upstream registries remain sufficient for local development. We depend heavily on other ecosystems, so package interoperability is a core feature, not an afterthought.

### D12 — Public naming

- [x] **Neo Astra** is the official project and language name *(confirmed)*
- [x] Source files use the **`.ns`** extension *(confirmed)*
- [x] The CLI remains **`astra`** *(confirmed)*

**Decision:** Official name is **Neo Astra**; source files use `.ns`; the CLI is `astra`.  
**Notes:** Use this naming consistently across the compiler, documentation, packages, and website. D25 adds `nova` as the development runtime and `nopm` as the package manager; these are separate commands, not replacements for `astra`.

### D13 — How modes are selected

- [x] Strict pipeline: `--mode` > `ASTRA_MODE` > target-bound mode > `defaultMode` > least-privileged inference *(confirmed)*
- [x] Scoped `{@mode:...;}` blocks and source tags are validated after resolution *(confirmed)*
- [ ] Explicit mode declarations only; no inference
- [ ] Runtime-switchable modes

**Decision:** Compile-time only. Nested blocks may narrow capabilities but may not silently widen them. Requirements are viral.  
**Notes:** `astra explain` must show why the final mode was selected. Inferred mode is part of the build cache key.

### D14 — What modes may change

- [x] Capabilities, standard library, memory rules, and target constraints only; core syntax and semantics stay fixed *(confirmed)*
- [ ] Modes may change core syntax and semantics
- [ ] Modes are only standard-library presets

**Decision:** Modes are capability layers, not different languages.  
**Notes:** Integer and string behavior are controlled by explicit `math` / `strings` semantic policies, not by mode switches. Memory policy is selected by manifest/file/block configuration, not silently by mode. All policies are recorded in build metadata.

### D15 — Automatic mode and dependency inference

- [x] Infer the least-privileged mode and target-specific dependencies from static imports and the call graph; explicit annotations win; results are recorded and explainable *(confirmed)*
- [ ] Require explicit modes and dependencies everywhere
- [ ] Infer silently without diagnostics

**Decision:** Inference is static, least-privilege, and explainable. Explicit annotations always win.  
**Notes:** Dynamic imports and reflective calls require explicit declarations. Inference results go into build metadata and `astra explain`.

### D16 — Readability and AI-friendliness

- [x] Explicit, regular grammar; canonical formatter; structured diagnostics; machine-readable AST; explainable inference *(confirmed direction)*
- [ ] Terse expert-oriented syntax with more implicit behavior
- [ ] Highly implicit or magical syntax

**Decision:** Human-readable and AI-friendly are core design goals.  
**Notes:** The language should be explicit where it matters and explainable everywhere.

### D17 — Comment syntax

- [x] Only `//` line comments are valid *(confirmed)*
- [ ] Support block comments
- [ ] Support multiple comment syntaxes

**Decision:** Only `//` is valid in `.ns` source. `/* ... */`, `#`, `--`, and every other comment form are syntax errors.  
**Notes:** Documentation should use `//` with a defined tag. `core.toml` still uses standard TOML `#` comments because it is a config file, not Neo Astra source.

### D18 — Manifest format

- [x] Support both `core.json` and `core.toml`, with exactly one present *(confirmed)*
- [x] Both formats use the same strict semantic schema *(confirmed)*
- [x] Lockfile is `core.lock.json` *(confirmed)*
- [ ] JSON only
- [ ] TOML only
- [ ] YAML manifest

**Decision:** Use `core.json` or `core.toml`; exactly one may exist. Both are schema-validated, unknown-key-rejecting, and canonically formatted.  
**Notes:** JSON is familiar to web developers; TOML is more compact and supports comments. The `//`-only rule applies to `.ns` source, not to TOML comments.

### D19 — Compiler architecture

- [x] TypeScript orchestrator + Rust core engine + modular backend modules/crates *(confirmed)*
- [ ] Rust-only compiler
- [ ] TypeScript-only compiler
- [ ] Separate independent compilers per target

**Decision:** TypeScript owns the CLI, manifest handling, plugins, LSP, and JS/TS orchestration. Rust owns the shared parser, type checker, capability inference, IR, and native/Wasm backends.  
**Notes:** Rust remains the single source of truth for parsing and semantics. TypeScript must never re-parse `.ns` source with a second grammar.

### D20 — Strictness policy

- [x] Strict at API boundaries, local inference, no implicit coercions, exhaustive matching, structured diagnostics *(confirmed)*
- [ ] Maximally strict everywhere, with no inference
- [ ] Loose, JavaScript-style implicit behavior

**Decision:** Strict at boundaries, inferred locally, explicit everywhere it affects correctness.  
**Notes:** Avoid both “Rust 2.0” ceremony and JavaScript-style coercion surprises.

### D21 — Memory policy precedence

- [x] Block `{@memory:...;}` > file wrapper > manifest `memory.files` > manifest `memory.default` *(confirmed)*
- [ ] Manifest always wins
- [ ] Source annotations always win, ignoring the manifest

**Decision:** More specific declarations win. If a file declares `manual`, the compiler follows manual for that file even when the project default is `auto`.  
**Notes:** Manual memory is native/Wasm only. JS builds reject manual-memory files instead of faking manual memory on top of the host GC.

### D22 — Keyword aliases

- [x] Accept exact lexical aliases and normalize them in the formatter *(confirmed)*
- [ ] One canonical spelling only
- [ ] Allow semantically different aliases

**Decision:** Accept `fn`/`function`, `let`/`var`, `const`/`constant`, `type`/`typedef`, `struct`/`record`, `mut`/`mutable`, and `pub`/`public`. They are exact lexical synonyms. The canonical formatter emits the short form: `fn`, `let`, `const`, `type`, `struct`, `mut`, and `pub`.  
**Notes:** Aliases are a developer-style choice, not a mode and not a semantic difference. `var` does not imply mutability; mutability still requires `mut`/`mutable`. The parser and diagnostics accept both spellings; generated code and formatter output use the canonical spelling.

### D23 — Declaration and module export system

- [x] Optional visibility: normal by default, `public`/`pub` or `export` to export, `private` to forbid export; support both project-path and JS/TS import/export forms *(confirmed)*
- [ ] Every declaration must be explicitly public or private
- [ ] Only JS/TS-style exports; no `pub`
- [ ] Only Neo Astra-style imports; no JS/TS forms

**Decision:** A declaration with no visibility modifier is normal and module-local, but may be exported explicitly with `export { ... }`. Imported bindings may be re-exported. `pub`/`public` and `export` mark a declaration as exported. `private` declarations cannot be exported, imported, or re-exported. The language accepts both the existing `import path` / `import path as alias` form and JS/TS-style imports/exports: named, default, namespace, side-effect, export lists, re-exports, and `export default`.  
**Notes:** `pub`/`public` and `export` are equivalent export markers at item level; the formatter canonicalizes redundant combinations to `export`. Visibility is compile-time only. Private declarations cannot leak through public signatures, public aliases, or public fields. Exports do not bypass modes, capabilities, FFI allowlists, or package security, and capability requirements remain viral through re-exports.

### D24 — Semicolon separators

- [x] Optional soft separator between same-line statements/items; required after scope annotations; newlines remain the normal boundary *(confirmed)*
- [ ] Semicolons required at every statement end
- [ ] Semicolons forbidden everywhere except scope annotations
- [ ] JavaScript-style automatic semicolon insertion

**Decision:** Newlines are the normal statement and item boundaries. A semicolon may separate two statements or items on the same line and is required when they share a line. A trailing semicolon at the end of a line is allowed but unnecessary. The semicolon after a scope annotation is required because the annotation is a block-header directive.  
**Notes:** There is no automatic semicolon insertion. Semicolons are not list separators and are not used between struct fields, enum variants, arguments, parameters, map entries, or match arms. The canonical formatter preserves explicit same-line semicolon groups.

### D25 — Nova development runtime and NoPM

- [x] Nova is a separate development runtime that interprets typed `.ns` source; production remains compiled to JS/native/Wasm; `nova` and `nopm` are separate commands; NoPM is the universal package manager; the first adapters are path, Git, npm, and Cargo; the manifest is extended rather than duplicated *(confirmed)*
- [ ] Nova replaces production targets
- [ ] A bytecode VM instead of an HIR interpreter in the first version
- [ ] A second parser or a second semantic implementation
- [ ] A separate environment manifest

**Decision:** Nova is a development runtime for direct `.ns` execution, REPL, watch mode, and tests. It reuses the Rust lexer, parser, resolver, and type checker and starts as a typed-HIR interpreter. Production artifacts remain compiled through `astra build`; Nova may become a deployment runtime later without changing the language contract. NoPM resolves dependencies for both Nova and Astra builds, supports path, Git, npm, and Cargo first, and records a universal package graph in `core.lock.json`. The universal registry gateway can proxy upstream ecosystems and host Neo Astra packages, but it is not required for local development. `core.json` / `core.toml` was the only manifest, extended with `[nova]` and `[registry]` sections (superseded by D26: a separate optional `build.*` manifest now carries build directives and the `[toolchain]` section).  
**Notes:** Nova is not a second language implementation. It always parses and resolves source, and type checking is on by default. “Any ecosystem” means any ecosystem with a package adapter; packages still need target-compatible bindings. Package lifecycle scripts are disabled by default. The zero-VM production promise still applies; Nova is a development interpreter, not the deployment VM.
---

### D26 — Two manifests: package core and build directives

- [x] Split the manifest by purpose: `core.*` is the package manifest (identity, dependencies, package sources, registry, capabilities, ffi) and `build.*` is the build manifest (build, targets, nova, math, strings, memory, toolchain) *(confirmed 2026-10-31)*
- [x] `build.*` is optional; absent means documented defaults *(confirmed)*
- [x] The lockfile stays `core.lock.json` and records the resolved toolchain *(confirmed)*
- [ ] Dual-read compatibility with the single-manifest layout
- [ ] A third manifest

**Decision:** Two manifests, split by purpose. `core.json`/`core.toml` (required) is what the package is: `[package]`, `[dependencies]`, `[packageSources]`, `[registry]`, `[capabilities]`, `[ffi]`. `build.json`/`build.toml` (optional) is how it is built and run: `[build]`, `[targets]`, `[nova]`, `[math]`, `[strings]`, `[memory]`, `[toolchain]`. A key lives in exactly one file; a wrong-file key is rejected with a diagnostic naming the right file. An absent `build.*` means the defaults in spec §12.2. The lockfile stays `core.lock.json`. Clean break: no dual-read compatibility layer (pre-1.0).
**Notes:** Supersedes D25's "core.json / core.toml remains the only manifest" line. Package identity and dependency resolution are what get published and shared; build directives are environment- and machine-specific, so they must not travel inside the published package.

---

### D27 — Evaluation order: call arguments portable, everything else target-native

- [x] Call arguments (including constructor and `fail` calls) evaluate strictly left-to-right on every backend *(confirmed 2026-10-31)*
- [x] Binary operands, array elements, struct fields, and map entries use target-native order with no portability guarantee *(confirmed)*
- [x] A frontend side-effect detection pass gated to system mode rejects expressions that combine multiple state-changing subexpressions in a target-native-order position and tells the developer to hoist the calls into named bindings *(confirmed; implementation queued)*
- [ ] Normative ordering for every expression position
- [ ] Relying on target-native order as a portability guarantee

**Decision:** Call-argument evaluation is a portable, normative left-to-right rule. Every other expression position is target-native; the system-mode side-effect pass is the guard that keeps the difference from becoming a silent portability trap. Pinned by the `arg_order` conformance case (call arguments); the pass is its own queued slice with a diagnostic prompt to hoist calls.
**Notes:** The C backend implements the call-argument rule by hoisting side-effecting arguments into temporaries in source order (materialisation, GC-001 §5.3). Target-native positions deliberately reorder nothing in the compiler; they simply do not promise an order.

---
---

## 16. Proposed repository layout

```text
neo-astra/
  orchestrator/            # TypeScript
    packages/
      cli/                 # astra CLI
      manifest/            # core.json / core.toml parsing, validation, formatting
      compiler-client/     # JSON-RPC client for the Rust core
      lsp/                 # language server
      plugins/             # package-source and backend plugins
      nopm/                # NoPM universal package manager (planned)
      codegen-js/          # JavaScript ESM
      codegen-ts/          # TypeScript output
    tests/
  crates/                  # Rust
    astra-core/            # lexer, parser, resolver, type checker, IR
    astra-capabilities/    # mode and dependency inference
    astra-compiler-api/    # stable JSON/WASM boundary
    astra-nova/            # typed-HIR development interpreter
    astra-codegen-c/       # C backend for native and Wasm
    astra-codegen-wasm/    # future direct Wasm backend
    astra-codegen-llvm/    # future LLVM backend
    astra-wasm/            # raw linear-memory Wasm boundary
  registry/                # optional universal registry gateway (planned)
  runtime/
    js/                    # small pay-as-you-go JS helpers
    native/                # native runtime support
    gc/                    # GC runtime
  scripts/                 # bootstrap, CI, and boundary test runners
  docs/
    architecture.md
    spec-v0.1.md
    semantics.md
  examples/
  playground/
  tests/
    conformance/
```

---

## 17. Next step

1. ✅ Review the Decision Log one item at a time.
2. ✅ Record the chosen options in this document.
3. ✅ Write `Docs/spec-v0.1.md` with the v0.1 grammar and semantics.
4. Scaffold the TypeScript orchestrator workspace.
5. Scaffold the Rust core workspace.
6. Implement the compiler API boundary.
7. Implement the lexer, parser, type checker, error model, and interpreter.
8. Add the JavaScript/TypeScript backends and playground.

The first usable Neo Astra program should run in a browser and compile to clean ESM. The architecture for native, Wasm, FFI, and concurrency is defined now, even when the implementation lands in later milestones.
