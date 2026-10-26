# V1 Inline Script Interop (`@script`)

**Status:** Slices A+B and the JavaScript backend slice (C) are implemented; the
TypeScript target and C/C++ native slice remain.  
**Date:** 2026-10-25, updated 2026-10-26  
**Depends on:** D10, D19, D25  
**Related:** [`01-language-design.md`](./01-language-design.md), [`02-compiler-design.md`](./02-compiler-design.md), [`04-targets-and-backends.md`](./04-targets-and-backends.md), [`05-manifest-and-packages.md`](./05-manifest-and-packages.md), [`06-tooling-and-workflow.md`](./06-tooling-and-workflow.md), [`07-quality-security-and-delivery.md`](./07-quality-security-and-delivery.md), [`08-nova-and-nopm.md`](./08-nova-and-nopm.md)

---

## 1. Purpose

`@script` lets a `.ns` file contain a bounded block of foreign source:

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

The block is a **compiler directive**, not a comment or an opaque string. It tells
the lexer and parser to stop reading Nova syntax, capture the body as raw foreign
source, and resume Nova syntax at an explicit terminator. This is why `@script`
belongs next to `{@mode:name; ...}` and `{@memory:policy; ...}`: all three tell the
compiler how to treat the enclosed region.

The purpose is practical escape-hatch interop, not a second program language:

- Keep `.ns` as the program, module, type, mode, and capability language.
- Let a project embed a small amount of JavaScript, C, C++, or Python where a
  typed foreign boundary is genuinely useful.
- Keep the foreign body opaque to Nova parsing and type checking.
- Check every Nova call against an explicit, typed export surface.
- Reject unsupported language/target combinations at compile time.

`@script` does not replace `foreign module`, NoPM package adapters, or the normal
`.js` / `.c` files used by external dependencies. It is an inline source form for
the same polymorphic FFI model. A package or a large foreign library should still
use a `foreign module` declaration plus a manifest mapping.

### 1.1 Non-goals

- No target-specific grammar in the Nova parser.
- No type inference into the foreign body.
- No automatic translation from JavaScript to Nova or Nova to JavaScript.
- No remote source fetching from a `.ns` file.
- No promise that arbitrary foreign source is safe, portable, or deterministic.
- No JVM or Python runtime in V1.

---

## 2. Directive model

`@script` is a **declaration-level block** in V1. It defines a named, module-local
foreign namespace with one or more exported functions.

| Directive | Scope | Meaning |
|---|---|---|
| `@script:<language> as <name>` | block header | Switches the parser into raw foreign-source mode and names the resulting namespace |
| `@mode(<name>)` | script header | Declares the minimum mode for the foreign implementation; required in V1 |
| `@exports { ... }` | script header | Declares the Nova-facing function signatures; at least one export is required in V1 |
| `@impl` | script header | Marks the start of the verbatim foreign source |
| `@end` | raw body | Ends raw capture; the following `}` closes the script block |

`@exports` entries are signatures only. They have no Nova bodies.

`@exports` functions are visible through the block namespace:

```ns
math.add(2, 3)
```

The namespace is module-local in V1. Existing `foreign module` declarations remain
the cross-module FFI surface until a follow-up design defines exporting an inline
script namespace from a module.

### 2.1 Why the block is named

A stable name gives the compiler:

- A collision-free namespace in the module.
- A stable key for diagnostics, build metadata, and manifest `ffi` mappings.
- A clear explanation target for `astra explain`.
- A stable identity for source maps and cache entries.

Anonymous script blocks are intentionally deferred. They create name-injection,
manifest-identity, and visibility questions that are not worth solving in the
first slice.

### 2.2 Why `@end` is explicit

The closing `}` of a Nova block cannot safely terminate arbitrary foreign source.
JavaScript object literals, C++ lambdas, Python sets, and strings can all contain
braces. Counting braces is not language-neutral, and a Svelte-style `</script>`
terminator only works when the outer document has an HTML parser.

V1 therefore uses a sentinel line:

```text
@end
```

Rules:

- Only a line whose first non-whitespace token is exactly `@end`, followed only by
  optional whitespace, ends raw capture.
- `@end` is not included in the foreign body.
- The raw body is captured byte-for-byte, including blank lines, indentation,
  comments, and line endings normalized to `\n`.
- The line `@end` is reserved inside a `@script` body. Foreign code that needs
  that exact line must split the string or use an escape supported by the foreign
  language. This restriction is documented and checked.
- A missing `@end`, `@impl`, or closing `}` is a compile-time error with the span
  of the opening `@script`.

The canonical form is:

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

The `@end` line and the closing `}` are separate. The formatter must emit that
canonical layout and must not reformat the raw body.

---

## 3. Syntax

The proposed grammar extension is:

```text
script_block    = "{@script:" language "as" identifier ";" script_header
                  raw_foreign_source "@end" "}" ;

script_header   = mode_attribute
                  "@exports" "{" script_export { script_export } "}"
                  "@impl" ;

script_export   = "fn" identifier "(" [ parameters ] ")" [ "->" type ] ;
```

`language` is a lowercase language identifier. The proposed vocabulary is:

- `js`
- `ts`
- `c`
- `cpp`
- `python` (deferred, see §5.3)

`java` is reserved but rejected until a JVM target exists. Unknown language names
are errors, not extension points.

A bare `{@script:java; ... }` block is not valid V1. The language must be supported
by the target, the block must declare `@mode`, and it must expose at least one
typed export through `@exports`. This prevents an opaque, uncallable block from
becoming an unchecked side channel.

The export signature uses the same function-signature grammar as Nova, without a
body. Generic exports are not allowed in V1. Error effects are allowed:

```ns
fn parse(input: str) -> i32 ! ParseError
```

The error type must be a nominal Nova error type with a defined boundary mapping.
Foreign exceptions must be caught and mapped; they never unwind into Nova code.

### 3.1 Required mode

Because the body is opaque, the compiler cannot infer its capabilities. Every
`@script` block must declare its minimum mode:

```ns
@mode(core)
```

or:

```ns
@mode(system)
```

The declaration is a contract, not a proof:

- Calls to the block require at least the declared mode.
- A contradictory explicit project/mode requirement reports `NS5001`/`NS5006` as
  usual.
- `astra check` records the script as an unsafe boundary in metadata.
- The body is trusted to honor the declaration. A script that secretly uses
  filesystem or network APIs without declaring `system` is a safety bug, just as
  a lying native FFI declaration is.

The first implementation may restrict `@script` to `core` and `app` for JS/TS and
to `app`/`system` for C/C++ until boundary tests exist.

### 3.2 Exported names

- Export names must be unique within the block.
- Export names must not conflict with the block namespace.
- The raw body must provide an implementation for every declared export.
- Extra implementation symbols are allowed as private helpers.
- The compiler must not require the foreign source to repeat the Nova type
  signatures.
- Export visibility is namespace-level in V1. A `pub` modifier on an export is
  invalid until cross-module script namespaces are designed.

For JavaScript, the backend can validate the ESM export names. For C/C++, the
linker or a generated symbol table validates them. For Python, the runtime import
path validates them at startup.

### 3.3 Type boundary

Exported signatures use the FFI type mapping from
[`05-manifest-and-packages.md`](./05-manifest-and-packages.md) §7.4.

V1 allows:

- Fixed-width integers and floats
- `bool`
- `str`
- `bytes`
- `[T]` and fixed arrays of boundary-safe types
- structs with boundary-safe fields
- `Option<T>` at the boundary
- `T ! E` with a declared nominal error type
- function values only when the backend can represent them safely

V1 rejects:

- Generic exports
- Managed closures crossing the boundary
- Raw borrowed pointers in `core`/`app`
- Trait objects or host objects without an explicit boundary type
- Nova enums with data variants, unless the boundary mapping defines a tagged
  representation
- Recursive structs whose layout is not stable

Every conversion must be generated or explicit. There is no implicit structural
conversion.

---

## 4. Compiler behavior

### 4.1 Lexing and parsing

The lexer must recognize `@script` before it attempts to tokenize the body as
Nova. The parser then:

1. Reads the language and namespace.
2. Reads the required `@mode` header.
3. Reads and validates `@exports`.
4. Requires `@impl`.
5. Captures raw source until the `@end` sentinel.
6. Requires the closing `}`.
7. Resumes normal Nova parsing.

The raw body is stored as an AST `ScriptBlock` with:

- Language
- Namespace
- Mode
- Export signatures
- Raw source bytes
- Exact source span for each part

The body is never passed to the Nova expression parser or type checker.

### 4.2 Resolution and checking

The resolver registers the namespace as a foreign module with a script origin.
The checker verifies:

- The namespace is unique in the module.
- The required mode is valid and compatible with the project.
- Every export signature is well typed and boundary-safe.
- Calls use the declared signatures.
- Error effects propagate like any other foreign call.
- The active target supports the script language.
- A required manifest `ffi` mapping exists when link or runtime configuration is
  needed.

The checker does not inspect foreign function bodies. A compile can still fail at
backend link time if the foreign source does not define a declared export.

### 4.3 HIR and backend

HIR retains the script block by ID, including its raw source and declared
signatures. Each backend decides how to package it:

| Language | JavaScript/TypeScript target | C/native target | Wasm target |
|---|---|---|---|
| `js` | Emit as an ESM module and import it | Reject in V1 | Reject in V1 |
| `ts` | Transpile and emit, or reject until the transpiler is pinned | Reject in V1 | Reject in V1 |
| `c` | Reject | Compile as a C translation unit and link | Deferred |
| `cpp` | Reject | Compile as a C++ translation unit behind a C ABI shim | Deferred |
| `python` | Reject | Deferred; requires embedded CPython | Reject |
| `java` | Reject | Reject until a JVM backend exists | Reject |

Unsupported combinations report a compile-time error before any foreign code is
executed.

### 4.4 Build metadata

The build summary records:

- Script namespace
- Language
- Declared mode
- Export names and signatures
- Source span
- Backend artifact path
- Whether the boundary is considered unsafe

No raw source is copied into JSON output unless a debug flag explicitly asks for
it.

### 4.5 Nova

Nova is a typed-HIR interpreter, not a host for JavaScript, Python, or native
code. In V1 it must reject a script block with a clear `NV0008` diagnostic rather
than silently ignoring it or executing untrusted host code.

A later adapter may allow a specific language (likely JavaScript through an
embedded engine) in `app` mode. That requires a separate decision covering
runtime packaging, capabilities, and sandboxing.

---

## 5. Manifest and packaging

Inline scripts do not create a new dependency source. They are source code in the
project and follow the same trust rules as other project source.

A named script block is manifest-addressable as an FFI module. The `ffi` entry is
optional when the compiler can derive all required information, and required when
link or runtime configuration is needed.

Example for inline C:

```json
{
  "ffi": {
    "math": {
      "native": {
        "type": "inline-c",
        "source": "inline",
        "ownership": "borrowed",
        "threading": "single-threaded",
        "errors": "status-code"
      }
    }
  }
}
```

Rules:

- `source: "inline"` is a sentinel meaning the implementation is inside a `.ns`
  file; it is not a path.
- `type` is one of `inline-js`, `inline-ts`, `inline-c`, `inline-cpp`, or
  `inline-python`.
- `ownership`, `threading`, and `errors` are required for native and Python
  mappings.
- JS/TS mappings may omit the entry when the target is also JS/TS and no external
  runtime is needed. If present, it must agree with the source.
- A mapping for a target with no compatible script language is an error.
- `astra check` validates mappings before `astra build`.
- NoPM lifecycle scripts remain disabled. An inline script is project source, not
  a package lifecycle hook.

### 5.1 C/C++ symbol policy

C symbols are global. The compiler must prevent an inline script's `add` from
colliding with another translation unit's `add`.

V1 leaves the exact symbol-mangling policy open, but requires that:

- Each script block compiles into its own translation unit.
- Generated Nova symbols and script symbols cannot collide silently.
- A linker error caused by a missing or duplicate export is reported with the
  script span.
- A C++ block exposes only a C ABI surface to Nova.

### 5.2 JS/TS packaging

For JavaScript and TypeScript targets, the backend emits each script block as a
real ESM module (or an equivalent inline module with explicit imports) and maps
the declared exports to the namespace. It must not use `eval`, `new Function`, or
a hidden string wrapper.

### 5.3 Python packaging

Python is deferred. A future implementation needs:

- An embedded CPython runtime or a packaged host process.
- A pinned interpreter version per target.
- A `.pyi`-compatible signature source if the inline export surface is not enough.
- Explicit exception, GIL, and ownership rules.
- A decision about whether Python is allowed in `nova` at all.

---

## 6. Diagnostics

Provisional codes use the `NS7xxx` FFI range:

| Code | Meaning |
|---|---|
| `NS7001` | Unknown or unsupported script language |
| `NS7002` | Script language is not supported by the active target |
| `NS7003` | Missing, empty, or duplicate `@exports` declaration |
| `NS7004` | Malformed script block: missing `@mode`, `@impl`, `@end`, or closing `}` |
| `NS7005` | Unsupported or non-boundary-safe export signature |
| `NS7006` | Script namespace collision |
| `NS7007` | Missing or inconsistent manifest `ffi` mapping |
| `NS7008` | Script backend/link failure, or missing implementation export |

Nova uses `NV0008` for "foreign script requires a production target".

Diagnostics must point at the script header or the specific export, not at the
whole file. The raw body is never partially parsed for Nova diagnostics.

---

## 7. Security

`@script` is an unsafe FFI boundary with source-level convenience.

- The foreign body is not type-checked, sandboxed, or capability-verified.
- `@mode` is a declared contract; it is not a proof.
- Inline source is trusted like the rest of the project, not like a remote
  package.
- The compiler must not fetch code while checking a script block.
- The compiler must not execute foreign source during `astra check` or `astra fmt`.
- Builds that compile C/C++ or embed a Python runtime must surface the toolchain
  and runtime in build metadata.
- Diagnostics must redact environment variables and credentials in the same way
  as other compiler diagnostics.
- `nova` must reject foreign scripts until a sandboxed host adapter is approved.

---

## 8. Implementation slices

**Ordering:** the JavaScript slice landed on `JS-001`; the TypeScript-target
slice waits for `TS-001` or a pinned transpiler. The native C/C++ slice follows
the C backend.

### Slice A — frontend and tooling

- Lexer/parser recognition of `@script`
- Raw source capture with `@end`
- AST and HIR representation
- Formatter preservation of the raw body
- Diagnostics `NS7001`–`NS7006`
- Parser, formatter, and AST tests

### Slice B — typed surface

- Resolver registration of script namespaces
- Checker validation of `@mode`, `@exports`, boundary types, and calls
- Target/language compatibility checks
- Compiler API and metadata fields
- `astra explain` reasons for script mode requirements

### Slice C - JavaScript backend

- [x] Emit ESM script modules and import their namespaces
- [x] Validate declared exports with `NS7008`
- [x] Source maps and build artifacts
- [x] Boundary conversion for `Option<T>` and declared error effects
- [x] End-to-end tests on the `js` target
- [ ] End-to-end tests on the `typescript` target after `TS-001`

### Slice D — C/C++ native backend

- Generate a translation unit per script
- Generate wrappers and symbol isolation
- Compile and link through the native toolchain
- Map error tags and ownership
- Native boundary tests

### Slice E — deferred runtime work

- Python runtime decision and packaging
- Wasm script support
- Java/JVM support
- Nova host adapter

---

## 9. Open questions

1. Should `@script` eventually be allowed as a statement or expression inside a
   function, or remain declaration-level forever?
2. Should anonymous blocks inject their exports into the enclosing module, or is a
   named namespace always required?
3. What is the exact C/C++ symbol-mangling policy and its effect on debugging?
4. Should TypeScript blocks require a pinned TypeScript compiler version, or
   should the `ts` language be rejected until one is bundled?
5. Should Python be a V1 feature at all, or a post-V1 embedded-runtime project?
6. Should an inline script namespace be exportable across modules with `pub`, or
   should cross-module FFI always use `foreign module`?
7. How should source maps represent raw foreign segments and generated wrappers?
8. Should `@script` be allowed in `core` mode at all, given that the body cannot be
   checked for capability violations?
9. Should the formatter allow a compact `@end}` form, or require `@end` and `}` on
   separate lines?
10. Should the compiler treat the `@end` sentinel as a hard restriction, or allow
    a language-specific escape sequence?

---

## 10. Decision summary

If accepted, this proposal adds:

- An item-level `{@script:<language> as <name>; ... }` block.
- A required `@mode(...)` declaration and `@exports { ... }` surface.
- An explicit `@impl` marker and `@end` terminator.
- Opaque raw bodies that never enter the Nova parser or checker.
- Namespace calls such as `math.add(...)`.
- JavaScript/TypeScript on JS targets first, then C/C++ on native.
- Compile-time rejection of unsupported language/target combinations.
- A strict "unsafe boundary" security posture.

It deliberately does not add Java, Python, Wasm script embedding, anonymous blocks,
or Nova execution in the first slice.
