# Neo Astra Syntax Legend

**Status:** Quick reference for the v0.1 language  
**Normative source:** [`spec-v0.1.md`](./spec-v0.1.md)  
**Rule:** If this legend and the specification disagree, the specification wins.

This is a plain-language map of every syntax family and operator in Neo Astra. It is meant for reading code, writing examples, and explaining diagnostics.

---

## 1. Source basics

| Syntax | Meaning |
|---|---|
| `.ns` | Neo Astra source file |
| `// text` | Line comment; the only valid comment form in `.ns` |
| `/// text` | Documentation-style line comment; still a normal `//` comment |
| `/* ... */` | **Invalid** in `.ns`; use `//` |
| `# ...` | **Invalid** in `.ns`; valid only in `core.toml` |
| `-- ...` | **Invalid** in `.ns`; there is no decrement operator |
| newline | Normal statement/item boundary |
| `;` | Optional soft separator between related statements/items on one line; required after a scope annotation |
| `{ ... }` | Block, struct initializer, map literal, or scope block depending on context |

- Source is UTF-8.
- A leading UTF-8 BOM is accepted and removed by the formatter.
- Identifiers are case-sensitive.
- Style is conventional, not semantic: `snake_case` values, `PascalCase` types, `SCREAMING_SNAKE_CASE` constants.
- Newlines are the normal boundaries. A semicolon joins statements/items that share a line. A trailing semicolon is allowed but unnecessary. There is no automatic semicolon insertion.

---

## 2. Identifiers and keywords

### 2.1 Identifiers

```text
identifier_start  = Unicode letter | "_"
identifier_continue = Unicode letter | Unicode digit | "_"
```

Examples: `main`, `user_id`, `_temp`, `café`, `τ`.

`_` alone is the wildcard pattern, not a normal identifier.

### 2.2 Reserved keywords

| Keyword | Meaning |
|---|---|
| `module` | Declares the module path |
| `import` | Imports a module or selected names |
| `export` | Adds a declaration to the module export surface |
| `from` | Introduces the module specifier in JS/TS-style imports and re-exports |
| `as` | Renames an import or export |
| `default` | Names the default export or default import |
| `pub` | Exported/public declaration |
| `private` | Module-local declaration that cannot be exported |
| `fn` | Function declaration or function type |
| `struct` | Named product type |
| `enum` | Tagged union |
| `const` | Constant declaration |
| `type` | Type alias |
| `foreign` | Starts a foreign module declaration |
| `let` | Local binding |
| `mut` | Makes a binding mutable |
| `if` / `else` | Conditional expression |
| `while` | Loop |
| `for` / `in` | Iteration over a collection |
| `match` | Exhaustive pattern matching expression |
| `break` / `continue` | Loop control |
| `return` | Returns from a function |
| `defer` | Runs a statement when the current scope exits |
| `spawn` | Starts a scoped task |
| `not` | Boolean negation |
| `true` / `false` | Boolean literals |
| `panic` | Reports a programmer bug; type `Never` |
| `abort` | Terminates immediately without unwinding; type `Never` |

### 2.3 Keyword aliases

Both spellings are accepted. The formatter emits the canonical form.

| Canonical | Alias | Meaning |
|---|---|---|
| `fn` | `function` | Function declaration or function type |
| `let` | `var` | Local binding; `var` alone does **not** mean mutable |
| `const` | `constant` | Constant declaration |
| `type` | `typedef` | Type alias |
| `struct` | `record` | Named product type |
| `mut` | `mutable` | Mutable binding |
| `pub` | `public` | Exported/public declaration |

`var mut` and `let mut` are both valid mutable bindings.

---

## 3. Literals

| Literal | Meaning |
|---|---|
| `0`, `42`, `1_000_000` | Decimal integer; no implicit type, defaults to `i32` if unconstrained |
| `0xFF`, `0b1010` | Hexadecimal or binary integer |
| `0xFFu8` | Integer with a byte suffix; a byte value |
| `1.0`, `1e9`, `1.5e-3` | Floating-point value; defaults to `f64` |
| `true`, `false` | Boolean |
| `"hello"` | Immutable UTF-8 string |
| `b"hello"` | Byte literal; produces `bytes` |
| `()` | Unit value/type |

String escapes:

| Escape | Meaning |
|---|---|
| `\\` | Backslash |
| `\"` | Double quote |
| `\n` | Line feed |
| `\r` | Carriage return |
| `\t` | Tab |
| `\0` | NUL |
| `\u{...}` | Unicode scalar value |

There is no `null` or `undefined` in the source language. Use `Option<T>`.

---

## 4. Operators and punctuation

### 4.1 Symbol quick index

| Symbol | Name | Meaning | Example |
|---|---|---|---|
| `+` | Add | Numeric addition | `a + b` |
| `+%` | Wrapping add | Addition that wraps at the fixed-width boundary | `x +% 1` |
| `-` | Subtract / unary minus | Subtraction or negation | `a - b`, `-x` |
| `-%` | Wrapping subtract | Subtraction that wraps | `x -% 1` |
| `*` | Multiply | Multiplication | `a * b` |
| `*%` | Wrapping multiply | Multiplication that wraps | `x *% 2` |
| `/` | Divide | Division | `a / b` |
| `/%` | Wrapping divide | Division that wraps | `x /% 2` |
| `%` | Remainder | Remainder/modulo | `a % b` |
| `==` | Equal | Value equality | `a == b` |
| `!=` | Not equal | Value inequality | `a != b` |
| `<` | Less than | Comparison; also opens type arguments in type context | `a < b`, `Box<T>` |
| `<=` | Less or equal | Comparison | `a <= b` |
| `>` | Greater than | Comparison; also closes type arguments | `a > b`, `Box<T>` |
| `>=` | Greater or equal | Comparison | `a >= b` |
| `&&` | Logical and | Short-circuit boolean and | `a && b` |
| `\|\|` | Logical or | Short-circuit boolean or; also follows `spawn` | `a \|\| b`, `spawn \|\| { ... }` |
| `!` | Error/propagation marker | Error effect, failure construction, or propagation; **not** boolean not | `-> T ! E`, `!E(...)`, `expr!` |
| `not` | Boolean not | Negates a boolean | `not ready` |
| `=` | Assignment | Stores a value; not equality | `x = 1` |
| `.` | Member/path dot | Field or method access, tuple index, path separator, or decimal point | `user.name`, `pair.0`, `std.io`, `1.5` |
| `,` | Separator | Separates arguments, parameters, fields, entries, patterns | `f(a, b)` |
| `:` | Colon | Type annotation, map entry, field, or scope annotation | `x: i32`, `"a": 1`, `@mode:core` |
| `;` | Soft separator | Separates same-line statements/items; required after a scope annotation | `let x = 1; let y = 2`, `{@mode:core; ... }` |
| `->` | Return arrow | Return type or function type result | `fn f() -> i32` |
| `=>` | Match arrow | Separates a pattern from its arm result | `Some(x) => x` |
| `()` | Parentheses | Call, grouping, unit, or tuple type/value | `f()`, `(a + b)`, `()`, `(A, B)` |
| `[]` | Brackets | Array/slice type, array literal, or index | `[i32]`, `[1, 2]`, `xs[0]` |
| `{}` | Braces | Block, struct initializer, map literal, or map type | `{ ... }`, `User { ... }`, `{"a": 1}`, `{K: V}` |
| `@` | At sign | Attribute or scope annotation | `@requires(system)`, `{@mode:core;}` |
| `\|` | Pipe | Error-set union only; not bitwise or | `-> T ! A \| B` |
| `_` | Underscore | Wildcard pattern or ignored value | `_ => ...`, `let _ = x` |

### 4.2 Operator precedence

From highest to lowest:

| Level | Operators/forms |
|---|---|
| 1 | Postfix call `()`, index `[]`, field access `.`, error propagation `!` |
| 2 | Unary `-`, `not` |
| 3 | `*`, `/`, `%`, `*%`, `/%` |
| 4 | `+`, `-`, `+%`, `-%` |
| 5 | `<`, `<=`, `>`, `>=` |
| 6 | `==`, `!=` |
| 7 | `&&` |
| 8 | `\|\|` |
| 9 | Assignment `=` |

`&&` and `||` short-circuit.

### 4.3 Special symbol notes

- **`!` has three roles.** It introduces an error effect in a return type (`-> i32 ! E`), constructs a failure (`!E("bad")`), or propagates one (`call()!`). Boolean negation is `not`.
- **`|` is not bitwise or.** It only joins error types in an error set. `||` is logical or.
- **`&` alone is invalid.** Use `&&` for logical and. There is no bitwise operator in v0.1.
- **`->` and `=>` are different.** `->` describes a result type; `=>` separates a match arm.
- **`=` and `==` are different.** `=` assigns; `==` compares.
- **`;` is a soft separator.** It joins related statements/items on one line. Newline is the normal boundary. A trailing `;` is allowed but unnecessary.
- **`;` is required after a scope annotation.** `{@mode:core; ... }` and `{@memory:manual; ... }` need it because the annotation is a block header.
- **`;` is not a comma.** Do not use it between parameters, arguments, fields, variants, map entries, or match arms.
- **`+%`, `-%`, `*%`, `/%` are explicit wrapping operations.** They override the project integer policy for that operation.
- **`<` and `>` are context-sensitive.** They are comparison operators in expressions and type-argument delimiters in types.
- **`.` is context-sensitive.** It can separate a decimal, access a member, index a tuple, or separate a module path.
- **`_` is not always an identifier.** A single `_` is a wildcard; `_name` is an identifier.
- **`||` after `spawn` is the task block marker**, not a boolean expression.
- **`#`, `/* */`, and `--` are invalid comment forms.** Do not use them in `.ns`.
- **`++` and `--` are not operators.** There is no increment or decrement.
- **No bitwise operators.** Do not expect `&`, `|`, `^`, `<<`, or `>>` to work as bitwise operations.

---

## 5. Modules, imports, and exports

### 5.1 Module declaration

```ns
module app.main
```

The module path uses dotted identifiers. A project has one manifest: `core.json` or `core.toml`.

### 5.2 Imports

| Form | Meaning |
|---|---|
| `import std.io` | Project-path namespace import; binds `io` from the final segment |
| `import std.io as io` | Project-path namespace import with an explicit alias |
| `import "std:fs"` | Side-effect import by string specifier |
| `import * as fs from "std:fs"` | Namespace import |
| `import { read } from "std:fs"` | Named import |
| `import { read as read_file } from "std:fs"` | Renamed named import |
| `import fallback from "./thing.ns"` | Default import |
| `import fallback, { read } from "./thing.ns"` | Default plus named import |
| `import { default as fallback } from "./thing.ns"` | Default import through a named specifier |

String specifiers resolve through the manifest package rules. Dotted paths resolve in the project module namespace.

### 5.3 Exports and visibility

| Form | Meaning |
|---|---|
| No modifier | Normal declaration; module-local but exportable later |
| `pub fn f()` | Public/exported declaration |
| `public fn f()` | Alias for `pub fn` |
| `export fn f()` | Exported declaration |
| `private fn f()` | Module-local; cannot be exported, imported, or re-exported |
| `export { f }` | Exports a normal or public declaration under its own name |
| `export { f as g }` | Exports under a different name |
| `export { f as default }` | Makes `f` the default export |
| `export { read } from "std:fs"` | Re-exports a named export |
| `export * from "./shared.ns"` | Re-exports all named exports |
| `export * as tools from "./tools.ns"` | Re-exports a namespace |
| `export * as default from "./tools.ns"` | Re-exports a namespace as the default |
| `export default expr` | Default-exports a value |
| `export default fn f() { ... }` | Default-exports a function declaration |

Rules:

- `pub`/`public` and `export` are equivalent export markers at item level.
- The formatter canonicalizes `public` to `pub` and redundant `pub export` to `export`.
- `export { ... }` may appear before or after the declaration it names.
- A private declaration cannot be exported, imported, or re-exported.
- A private type cannot leak through a public signature, alias, or field.
- Type-only exports and imports are erased in JS/TS output.
- A default export must be a value. A module has at most one default export.
- An explicit export takes precedence over `export *`; two `export *` sources with the same name conflict.

---

## 6. Declarations

### 6.1 Functions

```ns
fn add(a: i32, b: i32) -> i32 {
    return a + b
}

pub fn parse(input: str) -> Ast ! ParseError {
    // ...
}
```

| Part | Meaning |
|---|---|
| `fn` / `function` | Function declaration or function type |
| `name(...)` | Function name and parameters |
| `:` in a parameter | Parameter type annotation |
| `-> T` | Return type |
| `! E1 \| E2` | Error effect; the function is fallible |
| `<T>` | Generic parameter list |
| `{ ... }` | Function body |

A function without `->` returns `()`.

### 6.2 Structs, enums, constants, aliases

```ns
struct User {
    id: u64
    name: str
}

enum Status {
    Pending
    Active
    Suspended(reason: str)
}

const MAX: u32 = 1024
type UserId = u64
```

| Form | Meaning |
|---|---|
| `struct Name { field: T }` | Named product type |
| `struct Name(T)` | Tuple struct |
| `enum Name { Variant, Variant(T) }` | Tagged union |
| `const NAME: T = value` | Constant |
| `type Name = T` | Type alias |
| `foreign module name { ... }` | FFI declaration mapped by the manifest |

Struct fields and enum variants are newline-separated; commas are not required. Struct fields and variants inherit the declaration's visibility.

### 6.3 Generics

```ns
fn identity<T>(value: T) -> T { return value }

struct Box<T> {
    value: T
}
```

`<T>` introduces a generic parameter. Generic arguments may be inferred when unambiguous. v0.1 has no traits, typeclasses, or higher-kinded types.

---

## 7. Types

| Type syntax | Meaning |
|---|---|
| `i8`, `i16`, `i32`, `i64` | Signed fixed-width integer |
| `u8`, `u16`, `u32`, `u64` | Unsigned fixed-width integer |
| `usize`, `isize` | Pointer-sized integer |
| `f32`, `f64` | IEEE floating point |
| `bool` | Boolean |
| `str` | Immutable UTF-8 text |
| `bytes` | Mutable byte buffer |
| `()` | Unit |
| `Never` | Type of `panic`/`abort`; no normal value |
| `[T]` | Ordered sequence |
| `[T; N]` | Fixed-size array |
| `{K: V}` | Map |
| `(A, B)` | Tuple |
| `Option<T>` | Optional value |
| `Result<T, E>` | Value-style result |
| `Ptr<T>` | Manual allocation handle; native/Wasm only |
| `MutSlice<T>` | Mutable view over contiguous elements |
| `fn(A) -> B` | Function type |
| `Name<T>` | Generic named type |

There is no `null`; use `Option<T>`.

---

## 8. Statements, expressions, and patterns

### 8.1 Bindings and assignment

```ns
let count = 1          // immutable
let mut total = 0      // mutable
var mut other = 2      // alias form; mut is still required

total = total + 1      // assignment
```

| Syntax | Meaning |
|---|---|
| `let name = value` | Immutable local binding |
| `let mut name = value` | Mutable local binding |
| `var name = value` | Alias for `let name`; still immutable |
| `var mut name = value` | Mutable binding using the alias |
| `name: T = value` | Explicit type annotation |
| `name = value` | Assignment to a mutable binding or assignable location |

Same-line statements use `;`:

```ns
let x = 1; let y = x + 1
read(file); parse(data); save(result)
```

These are equivalent to writing each statement on its own line.

### 8.2 Control flow

```ns
if ready {
    run()
} else {
    wait()
}

while running {
    tick()
}

for item in items {
    print(item)
}
```

| Syntax | Meaning |
|---|---|
| `if condition { ... }` | Conditional expression; condition must be `bool` |
| `else { ... }` | Alternative branch |
| `while condition { ... }` | Loop while true |
| `for name in collection { ... }` | Iterate over a collection |
| `break` | Exit the nearest loop |
| `continue` | Skip to the next loop iteration |
| `defer statement` | Run at scope exit, in reverse order |
| `return expr` | Return from the function |
| `return` | Return `()` |

No parentheses are required around `if` or `while` conditions. There is no implicit truthiness.

### 8.3 Match

```ns
match value {
    Some(0) => "zero"
    Some(n) => "number"
    None => "none"
}
```

| Syntax | Meaning |
|---|---|
| `match expr { ... }` | Exhaustive pattern match expression |
| `pattern => expression` | Arm returning an expression |
| `pattern => { ... }` | Arm returning a block |
| `_ => ...` | Wildcard arm |

Patterns:

| Pattern | Meaning |
|---|---|
| `_` | Ignore the value |
| `name` | Bind the value |
| `0`, `"x"`, `true` | Literal pattern |
| `Type.Variant(...)` | Enum variant |
| `Type(...)` | Tuple/struct constructor |
| `(a, b)` | Tuple pattern |
| `Some(x)`, `None`, `Ok(x)`, `Err(e)` | Option/result patterns |

### 8.4 Other expression forms

| Syntax | Meaning |
|---|---|
| `f(a, b)` | Function call |
| `value.field` | Field access |
| `tuple.0` | Tuple element |
| `value[index]` | Indexing |
| `[a, b]` | Array/slice literal |
| `{"a": 1}` | Map literal |
| `User { id: 1 }` | Struct initializer |
| `(a + b)` | Grouping |
| `()` | Unit |
| `(a, b)` | Tuple value |
| `if ... { ... } else { ... }` | Expression form of conditional |
| `match ... { ... }` | Expression form of match |
| `spawn \|\| { ... }` | Start a scoped task |
| `panic("message")` | Programmer-bug failure |
| `abort("message")` | Immediate termination |

---

## 9. Errors and the `!` marker

### 9.1 Error effect

```ns
fn parse_age(input: str) -> i32 ! InvalidAge | ParseIntError {
    if input.is_empty() {
        return !InvalidAge("Age cannot be empty")
    }
    return input.to_int()!
}
```

| Syntax | Meaning |
|---|---|
| `-> T ! E` | Function can fail with error set `E` |
| `A \| B` | Error-set union |
| `!Error(args)` | Construct a failure value |
| `expr!` | Propagate a failure from the current function |
| `panic("...")` | Programmer bug; terminates the current task |
| `abort("...")` | Immediate termination without unwinding |

A fallible call without `!` is a compile-time error unless handled. Handle with `match` using `Ok(...)` and `Err(...)` patterns. A function without `!` is infallible.

---

## 10. Memory

GC is the default.

```ns
{@mode:system;
    {@memory:manual;
        let buffer = Memory.alloc<u8>(1024)!
        defer Memory.free(buffer)
    }
}
```

| Syntax | Meaning |
|---|---|
| `{@memory:auto; ... }` | Use the GC-managed policy for this block |
| `{@memory:manual; ... }` | Use manual memory for this block |
| `Ptr<T>` | Manual pointer; native/Wasm only |
| `MutSlice<T>` | Mutable slice view |
| `Memory.alloc<T>(count)!` | Allocate `count` elements; fallible |
| `Memory.free(ptr)` | Free a manual allocation |
| `ptr.read(i)` | Read through a manual pointer |
| `ptr.write(i, value)` | Write through a manual pointer |
| `defer Memory.free(ptr)` | Free at scope exit |

Precedence:

```text
block {@memory:...;}
  > file wrapper
  > manifest memory.files
  > manifest memory.default
```

JS builds reject manual-memory files. Manual pointers are not GC references and must not cross a GC boundary without an explicit boundary.

---

## 11. Modes, attributes, and scopes

### 11.1 Modes

| Mode | Meaning |
|---|---|
| `core` | Portable code; most restricted |
| `app` | Application/script capabilities |
| `web` | Browser, DOM, fetch, workers |
| `system` | Filesystem, networking, raw memory, FFI |

### 11.2 Attributes

| Syntax | Meaning |
|---|---|
| `@requires(system)` | Declares the minimum mode for an item/module |
| `@tags(fs, ffi)` | Declares capability tags |
| `@mode(system)` | Item-level mode attribute |
| `@memory(...)` | Reserved and invalid as an item attribute in v0.1; use a block wrapper |

### 11.3 Scoped blocks

```ns
{@mode:core;
    fn portable() { ... }
}

{@memory:manual;
    // manual-memory statements
}
```

| Syntax | Meaning |
|---|---|
| `{@mode:mode; ... }` | Scoped mode block; the header semicolon is required; inner statements may use optional `;` |
| `{@memory:policy; ... }` | Scoped memory block; the header semicolon is required; inner statements may use optional `;` |

A nested block may narrow capabilities. Widening requires an explicit requirement or boundary. Capability requirements are viral.

Mode selection order:

1. `--mode` CLI flag
2. `ASTRA_MODE` environment variable
3. Target-bound manifest mode
4. `build.defaultMode`
5. Least-privileged static inference

---

## 12. Concurrency

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

| Syntax | Meaning |
|---|---|
| `spawn \|\| { ... }` | Start a task owned by the current scope |
| `Channel.create<T>()` | Create a channel |
| `channel.send(value)!` | Send; fallible |
| `channel.receive()!` | Receive; fallible |
| `channel.close()` | Close explicitly |
| `task.join()` | Wait for a task result |
| `task.cancel()` | Request cooperative cancellation |

A `spawn` expression evaluates to `Task<T>`. The parent scope waits for children before exiting. Detached tasks are not allowed in the portable core.

---

## 13. FFI

```ns
foreign module math_engine {
    fn compute_matrix(data: [f64]) -> [f64]
}
```

| Syntax | Meaning |
|---|---|
| `foreign module name { ... }` | Foreign module declaration |
| `fn name(...) -> T` | Foreign function signature; no body |
| Manifest `ffi` entry | Maps the foreign module to ESM, C ABI, or Wasm imports |
| Ownership annotation | Required for foreign pointers; never inferred |

FFI is unsafe by default. Panics must not cross an FFI boundary silently.

> **Proposed V1 syntax, not part of v0.1:** inline `@script` blocks let a `.ns`
> file contain a bounded foreign implementation with a typed export surface. See
> [`archive/Design/V1/10-inline-script-interop.md`](./archive/Design/V1/10-inline-script-interop.md).
> The block is not accepted by the current parser.

---

## 14. Lookalikes and common mistakes

| Looks like | Actually |
|---|---|
| `=` | Assignment, not equality |
| `==` | Equality |
| `!` | Error/propagation marker, not boolean not |
| `not` | Boolean negation |
| `!=` | Not equal, not “not” plus assignment |
| `\|` | Error-set union only |
| `\|\|` | Logical or; also the task block marker after `spawn` |
| `&` | Invalid alone; use `&&` |
| `->` | Return/function result |
| `=>` | Match arm separator |
| `;` | Soft same-line separator; required after a scope annotation; not a comma |
| newline | Normal statement/item boundary |
| `;` at line end | Allowed but unnecessary |
| `%` | Remainder |
| `+%` | Explicit wrapping addition |
| `_` | Wildcard when alone; `_name` is an identifier |
| `#`, `/*`, `--` | Invalid comments in `.ns` |
| `++`, `--` | Not operators |
| `null`, `undefined` | Not source-language values |
| `var` | Alias for `let`, not “mutable” |
| `public` | Alias for `pub` |
| `export` | Export marker; equivalent to `pub` at item level |
| `private` | Hard block on exporting/importing |
| `import std.io` | Project-path namespace import binding `io` |
| `import "std:io"` | Side-effect import by string specifier |

---

## 15. Annotated example

```ns
// Module declaration
module app.main

// Project-path namespace import; binds `io` from the final segment
import std.io

// Named import from a string specifier
import { read } from "std:fs"

// Normal (module-local) constant; not exported yet
const MAX: u32 = 1024

// Private helper; cannot be exported or imported
private fn clamp(n: i32) -> i32 {
    if n < 0 {
        return 0
    }
    return n
}

// Public function with an explicit error effect
pub fn parse_age(input: str) -> i32 ! InvalidAge | ParseIntError {
    if input.is_empty() {
        return !InvalidAge("Age cannot be empty")
    }
    return input.to_int()!
}

// Normal function promoted to the export surface later
fn helper() -> i32 {
    return 1
}

// Named exports, a renamed export, and a default export
export { helper, parse_age as parse }
export { read } from "std:fs"
export default parse_age
```

---

## 16. Normative references

- [`spec-v0.1.md`](./spec-v0.1.md) — language contract
- [`archive/architecture.md`](./archive/architecture.md) — decisions D1–D25
- [`archive/Design/V1/01-language-design.md`](./archive/Design/V1/01-language-design.md) — V1 language surface
- [`Agent/project-context-glossary.md`](./Agent/project-context-glossary.md) — terminology
