# Neo Astra Language Specification v0.1

**Status:** Draft specification  
**Date:** 2026-10-24  
**Language:** Neo Astra  
**Edition:** 0.1  
**Source extension:** `.ns`  
**CLI:** `astra`  
**Manifest:** `core.json` or `core.toml`  
**Lockfile:** `core.lock.json`  
**Decision baseline:** D1–D25

**Normative references:**

- [`architecture.md`](./architecture.md)
- [`Design/V1/README.md`](./Design/V1/README.md)
- [`Agent/definition-of-done.md`](./Agent/definition-of-done.md)

**Quick reference:** [`syntax-legend.md`](./syntax-legend.md)

---

## Contents

- [0. Scope and conventions](#0-scope-and-conventions)
- [1. Source model](#1-source-model)
- [2. Lexical structure](#2-lexical-structure)
- [3. Grammar](#3-grammar)
- [4. Types](#4-types)
- [5. Integer semantics](#5-integer-semantics)
- [6. String and byte semantics](#6-string-and-byte-semantics)
- [7. Errors](#7-errors)
- [8. Memory](#8-memory)
- [9. Concurrency](#9-concurrency)
- [10. Modes and capabilities](#10-modes-and-capabilities)
- [11. FFI](#11-ffi)
- [12. Manifest and build](#12-manifest-and-build)
- [13. Standard library surface](#13-standard-library-surface)
- [14. Diagnostics](#14-diagnostics)
- [15. Conformance](#15-conformance)
- [16. v0.1 implementation scope](#16-v01-implementation-scope)
- [17. Provisional choices and open questions](#17-provisional-choices-and-open-questions)
- [18. Appendix — complete examples](#18-appendix--complete-examples)
- [19. Change control](#19-change-control)
- [20. Decision traceability](#20-decision-traceability)

---

## 0. Scope and conventions

### 0.1 Purpose

This document defines the language contract for Neo Astra v0.1. It is the first specification intended to be implemented directly by the lexer, parser, type checker, interpreter, and JavaScript backend.

It is more precise than the architecture review and narrower than the full V1 design. **v0.1** is the first implementation slice; **V1** is the first stable release. Where this document and the confirmed architecture decisions conflict, the confirmed decisions win.

### 0.2 Normative keywords

The keywords **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative.

- **MUST** — required for conformance.
- **MUST NOT** — forbidden.
- **SHOULD** — recommended; deviation requires a documented reason.
- **MAY** — optional.

### 0.3 Implementation status does not change requirements

The v0.1 implementation slice is JavaScript-first. Native, Wasm, and FFI behavior is defined here so later backends do not need to rewrite the language contract.

A target that does not implement a feature MUST reject it explicitly. It MUST NOT silently emulate different semantics.

### 0.4 One semantic contract

Every target MUST preserve the observable behavior defined in this specification, except where a target-specific boundary is explicitly defined.

---

## 1. Source model

### 1.1 Encoding

- `.ns` source MUST be valid UTF-8.
- A UTF-8 BOM MAY be accepted, but the canonical formatter MUST remove it.
- Source spans are byte offsets into the UTF-8 encoding.

### 1.2 Files

- Neo Astra source files use the `.ns` extension.
- A module MAY span multiple files.
- A project MUST contain exactly one manifest file: `core.json` or `core.toml`.

### 1.3 Comments

- The only valid comment form is `//` followed by text until the end of the line.
- The canonical formatter MUST preserve every valid `//` comment exactly once. Leading and trailing comments keep their attachment; a comment inside an inline expression MAY move to the nearest item, statement, field, variant, or match-arm boundary.
- `/* ... */`, `#`, `--`, and every other comment form MUST be a syntax error in `.ns` source.
- `core.toml` uses standard TOML `#` comments because it is a configuration file, not Neo Astra source.
- `core.json` has no comments.

### 1.4 Whitespace and newlines

- Space, tab, carriage return, and line feed are whitespace.
- A newline terminates a statement or item when the parser is at a statement or item boundary.
- A semicolon is an optional soft separator between statements or items on the same line.
- Two statements or items on the same line MUST be separated by `;`. A trailing `;` at the end of a line is allowed but not required.
- Semicolons are never inserted automatically; there is no JavaScript-style ASI.
- A newline is the normal boundary. Newlines inside parentheses, brackets, and braces are insignificant unless the grammar says otherwise.
- The semicolon after a scope annotation is required because the annotation is a block-header directive, not a statement.
- The canonical formatter MUST produce deterministic whitespace.
- The canonical formatter SHOULD preserve explicit same-line `;` groups.

### 1.5 Identifiers

```text
identifier = identifier_start { identifier_continue }
identifier_start = Unicode_letter | "_"
identifier_continue = Unicode_letter | Unicode_digit | "_"
```

Identifiers are case-sensitive.

Canonical style:

- `snake_case` for values and functions
- `PascalCase` for types, enum variants, and modules
- `SCREAMING_SNAKE_CASE` for constants

### 1.6 Keywords

The following words are reserved:

```text
abort    as         break      const      continue
default  defer      else       enum       export
false    fn         for        foreign    from
if       import     in         let        match
module   mut        not        panic      private
pub      return     spawn      struct     true
type     while
```

`defer` is reserved for scoped cleanup. Its v0.1 behavior is defined in §8.4.

#### Keyword aliases

The following aliases are accepted as exact lexical synonyms. Both spellings are reserved; the canonical formatter emits the canonical spelling.

| Canonical | Accepted alias | Scope |
|---|---|---|
| `fn` | `function` | Function declarations and function types |
| `let` | `var` | Variable bindings; mutability is still controlled by `mut` |
| `const` | `constant` | Constant declarations |
| `type` | `typedef` | Type alias declarations |
| `struct` | `record` | Struct declarations |
| `mut` | `mutable` | Mutable bindings |
| `pub` | `public` | Public item visibility |

Aliases are a spelling option, not a semantic mode. `var` does not mean “mutable” by itself; use `let mut` or `var mut`. The canonical formatter MUST normalize aliases to the canonical column.

`record` is a spelling alias for `struct`; it does not introduce value semantics or a separate type kind.

---

## 2. Lexical structure

### 2.1 Literals

#### Integer literals

```text
0
42
1_000_000
0xFF
0b1010
```

- Integer literals have no implicit type.
- The expected type determines the integer type.
- If no expected type exists, the literal defaults to `i32`.
- An integer literal that does not fit its expected type is a compile-time error.

#### Floating-point literals

```text
1.0
1e9
1.5e-3
```

Floating-point literals default to `f64` unless the expected type is `f32`.

#### Boolean literals

```text
true
false
```

#### String literals

```text
"hello"
"héllo😀"
"line\nbreak"
```

Supported escapes:

| Escape | Meaning |
|---|---|
| `\\` | Backslash |
| `\"` | Quote |
| `\n` | Line feed |
| `\r` | Carriage return |
| `\t` | Tab |
| `\0` | NUL |
| `\u{...}` | Unicode scalar value |

A string literal MUST be valid UTF-8. Invalid escapes are syntax errors.

#### Byte literals

```text
b"hello"
```

A byte literal produces `bytes` and MUST contain only bytes representable by the source escape rules.

#### Byte values

```text
0xFFu8
```

An integer literal with a `u8` suffix is a byte value.

### 2.2 Operators and punctuation

```text
+  -  *  /  %  +%  -%  *%  /%
== != <  <= >  >=
&& || !  not
=  .  ,  :  ;  ->  =>
(  )  [  ]  {  }
@  |  _
```

`!` has two roles:

- Prefix failure construction: `!InvalidAge("...")`
- Postfix propagation: `parse_age(input)!`

The parser distinguishes them by position. Boolean negation uses `not`, not `!`.

`;` is a soft separator. It may separate statements or items on the same line. A newline is the normal boundary, so a trailing semicolon is allowed but not required. The semicolon after a scope annotation is required.

### 2.3 Comments and documentation

Documentation uses `//` with a convention:

```ns
/// Public API documentation.
pub fn parse(input: str) -> Ast ! ParseError
```

`///` is a normal `//` comment with documentation meaning. It is not a separate syntax.

### 2.4 Lexical diagnostics and examples

| Code | Meaning |
|---|---|
| `NS1001` | Invalid comment form |
| `NS1002` | Invalid escape sequence or byte-literal value |
| `NS1003` | Unterminated string or byte literal |
| `NS1004` | Invalid UTF-8 source |
| `NS1005` | Invalid identifier, malformed literal, or unexpected character |

Valid source:

```ns
// A line comment is valid.

fn main() {
    let message = "hello\nworld"
    print(message)
}
```

Invalid source:

```ns
/* block comments are not valid in .ns */

# TOML-style comments are only valid in core.toml

fn main() {
    print("unterminated)
}
```

Each invalid form MUST produce a structured `NS1xxx` diagnostic with a source span. The parser MUST NOT silently reinterpret an invalid comment form as code.

---

## 3. Grammar

This grammar is a normative baseline. The parser MAY be more permissive only where this document explicitly allows it.

### 3.1 Program and items

```ebnf
program        = { item [ ";" ] } ;
item           = attribute* item_core | scoped_item_block ;

item_core      = module_decl
               | import_decl
               | export_decl
               | declaration_item ;

declaration_item     = declaration_modifier* declaration_core ;
declaration_modifier = visibility | "export" ;
visibility           = pub_kw | "private" ;

declaration_core     = function_decl
                     | struct_decl
                     | enum_decl
                     | const_decl
                     | type_alias
                     | foreign_module ;

module_decl    = "module" path ;

import_decl    = path_import
               | side_effect_import
               | namespace_import
               | named_import
               | default_import
               | default_named_import ;

path_import          = "import" path [ "as" identifier ] ;
side_effect_import   = "import" string_literal ;
namespace_import     = "import" "*" "as" identifier "from" module_specifier ;
named_import         = "import" "{" [ import_specifier { "," import_specifier } ] "}" "from" module_specifier ;
default_import       = "import" identifier "from" module_specifier ;
default_named_import = "import" identifier "," "{" [ import_specifier { "," import_specifier } ] "}" "from" module_specifier ;
import_specifier     = ( identifier | "default" ) [ "as" identifier ] ;
module_specifier     = string_literal ;

export_decl    = export_list | export_all | export_default ;
export_list    = "export" "{" [ export_specifier { "," export_specifier } ] "}" [ "from" module_specifier ] ;
export_specifier = ( identifier | "default" ) [ "as" ( identifier | "default" ) ] ;
export_all     = "export" "*" [ "as" ( identifier | "default" ) ] "from" module_specifier ;
export_default = "export" "default" ( expression | function_decl ) ;

path           = identifier { "." identifier } ;

function_kw    = "fn" | "function" ;
let_kw         = "let" | "var" ;
const_kw       = "const" | "constant" ;
type_kw        = "type" | "typedef" ;
struct_kw      = "struct" | "record" ;
mut_kw         = "mut" | "mutable" ;
pub_kw         = "pub" | "public" ;

attribute      = "@" identifier [ "(" [ attribute_args ] ")" ] ;
attribute_args = expression { "," expression } ;

scoped_item_block = "{" scope_annotation ";" { item [ ";" ] } "}" ;

scope_annotation = "@mode" ":" identifier
                 | "@memory" ":" identifier ;
```

### 3.2 Functions

```ebnf
function_decl  = function_kw identifier
                 [ type_params ]
                 "(" [ parameter_list ] ")"
                 [ "->" type [ "!" error_set ] ]
                 block ;

type_params    = "<" identifier { "," identifier } ">" ;
parameter_list = parameter { "," parameter } ;
parameter      = identifier ":" type ;

error_set      = error_type { "|" error_type } ;
error_type     = type_path ;
```

A function without `->` returns `()`.

A function with `!` but no error types is invalid.

### 3.3 Types

```ebnf
type           = type_path
               | "[" type "]"
               | "[" type ";" integer_literal "]"
               | "{" type ":" type "}"
               | "(" [ type { "," type } ] ")"
               | function_kw "(" [ type_list ] ")" [ "->" type [ "!" error_set ] ] ;

type_path      = identifier { "." identifier } [ type_args ] ;
type_args      = "<" type { "," type } ">" ;
```

`Option<T>` and `Result<T, E>` are ordinary generic types in the standard library.

### 3.4 Structs and enums

```ebnf
struct_decl    = struct_kw identifier [ type_params ]
                 ( "{" { field_decl } "}" | "(" [ type_list ] ")" ) ;
field_decl     = identifier ":" type ;

enum_decl      = "enum" identifier [ type_params ] "{" { variant } "}" ;
variant        = identifier [ "(" [ variant_fields ] ")" ] ;
variant_fields = variant_field { "," variant_field } ;
variant_field  = [ identifier ":" ] type ;
```

Struct fields and enum variants are separated by newlines. There is no comma requirement.

A tuple struct, such as `struct InvalidAge(str)`, has positional fields and may be used as an error constructor.

### 3.5 Constants and aliases

```ebnf
const_decl     = const_kw identifier ":" type "=" expression ;
type_alias     = type_kw identifier [ type_params ] "=" type ;
```

### 3.6 Foreign modules

```ebnf
foreign_module = "foreign" "module" identifier "{" { foreign_fn } "}" ;
foreign_fn     = function_kw identifier "(" [ parameter_list ] ")" [ "->" type [ "!" error_set ] ] ;
```

A foreign function has no body. Its mapping is supplied by the manifest.

### 3.7 Statements

```ebnf
block          = "{" { statement [ ";" ] } "}" ;

statement      = let_stmt
               | assignment
               | expression_stmt
               | return_stmt
               | break_stmt
               | continue_stmt
               | defer_stmt
               | if_expr
               | while_stmt
               | for_stmt
               | match_expr
               | scoped_stmt_block ;

let_stmt       = let_kw [ mut_kw ] identifier [ ":" type ] "=" expression ;
assignment     = assignable "=" expression ;
expression_stmt = expression ;
return_stmt    = "return" [ expression ] ;
break_stmt     = "break" ;
continue_stmt  = "continue" ;
defer_stmt     = "defer" statement ;

scoped_stmt_block = "{" scope_annotation ";" { statement [ ";" ] } "}" ;
```

`scoped_stmt_block` uses the same `scope_annotation` form as `scoped_item_block`, but contains statements.

### 3.8 Control flow

```ebnf
if_expr        = "if" expression block [ "else" ( if_expr | block ) ] ;
while_stmt     = "while" expression block ;
for_stmt       = "for" identifier "in" expression block ;

match_expr     = "match" expression "{" { match_arm } "}" ;
match_arm      = pattern "=>" ( expression | block ) ;
```

`match` is an expression and may be used in a `let` initializer.

`if` and `while` do not require parentheses. Their condition expression MUST have type `bool`; there is no implicit truthiness.

`break` and `continue` are valid only inside a loop body. `defer` may appear anywhere in a block and captures the current scope for cleanup at exit.

### 3.9 Patterns

```ebnf
pattern        = "_"
               | identifier
               | literal_pattern
               | type_path "(" [ pattern_list ] ")"
               | type_path "." identifier [ "(" [ pattern_list ] ")" ]
               | "(" pattern { "," pattern } ")" ;
```

Patterns support:

- Wildcard `_`
- Binding identifiers
- Literals
- Enum variants
- Struct/tuple constructors
- `Ok(...)` and `Err(...)` for fallible expressions

### 3.10 Expressions

```ebnf
expression     = assignment_expr ;
assignment_expr = logical_or_expr [ "=" assignment_expr ] ;
logical_or_expr = logical_and_expr { "||" logical_and_expr } ;
logical_and_expr = equality_expr { "&&" equality_expr } ;
equality_expr  = comparison_expr { ( "==" | "!=" ) comparison_expr } ;
comparison_expr = additive_expr { ( "<" | "<=" | ">" | ">=" ) additive_expr } ;
additive_expr  = multiplicative_expr { ( "+" | "-" | "+%" | "-%" ) multiplicative_expr } ;
multiplicative_expr = unary_expr { ( "*" | "/" | "%" | "*%" | "/%" ) unary_expr } ;
unary_expr     = ( "-" | "not" ) unary_expr | postfix_expr ;
postfix_expr   = primary_expr { "." ( identifier | integer_literal )
                              | type_args
                              | "(" [ argument_list ] ")"
                              | "[" expression "]"
                              | "!" } ;
primary_expr   = literal
               | identifier
               | "(" expression ")"
               | "[" [ expression_list ] "]"
               | "{" [ map_entry { "," map_entry } ] "}"
               | struct_init
               | block
               | if_expr
               | match_expr
               | spawn_expr
               | fail_expr
               | panic_expr
               | abort_expr ;

argument_list   = expression { "," expression } ;
expression_list = expression { "," expression } ;
type_list       = type { "," type } ;

map_entry       = expression ":" expression ;
struct_init     = type_path "{" [ field_init { "," field_init } ] "}" ;
field_init      = identifier ":" expression ;
assignable      = identifier { "." identifier | "[" expression "]" } ;

literal         = integer_literal
                | float_literal
                | string_literal
                | byte_literal
                | "true"
                | "false"
                | "()" ;
literal_pattern = literal ;
pattern_list    = pattern { "," pattern } ;
```

`if_expr` and `match_expr` are expression forms of the control-flow constructs. A block used as an expression MUST end in an expression or be empty, in which case it evaluates to `()`.

**Evaluation order (D27).** The arguments of a call — including constructor and `fail` calls — are evaluated strictly left-to-right before the callee is invoked. Backends MUST NOT reorder call arguments; a program MAY rely on their side effects occurring in source order, and that order is portable across all backends.

Everywhere else — the operands of a binary operator, the elements of an array literal, the fields of a struct initializer, and the entries of a map literal — evaluation order is **target-native** and carries no portability guarantee. `&&` and `||` short-circuit the right operand when the result is already determined. To keep the portability trap out of systems code, the frontend's side-effect detection pass (system mode) rejects an expression that combines multiple state-changing subexpressions in a target-native-order position, such as `a() + b()`, and instructs the developer to hoist the calls into named bindings.

> Implementation status: the call-argument rule is enforced by every backend and pinned by the `arg_order` conformance case. The system-mode side-effect rejection is the next frontend slice; until it lands, target-native positions evaluate in target order without a diagnostic.

`fail_expr`:

```ebnf
fail_expr      = "!" type_path ( "(" [ argument_list ] ")" | "{" [ field_init { "," field_init } ] "}" ) ;
panic_expr     = "panic" "(" expression ")" ;
abort_expr     = "abort" "(" expression ")" ;
```

### 3.11 Spawn

```ebnf
spawn_expr     = "spawn" "||" block ;
```

### 3.12 Operator precedence

From highest to lowest:

1. Postfix: call, index, field access, error propagation `!`
2. Unary: `-`, `not`
3. `*`, `/`, `%`, `*%`, `/%`
4. `+`, `-`, `+%`, `-%`
5. `<`, `<=`, `>`, `>=`
6. `==`, `!=`
7. `&&`
8. `||`
9. Assignment

`&&` and `||` short-circuit.

### 3.13 Grammar examples

Modules, imports, constants, aliases, and generics:

```ns
module app.main
import std.io as io

pub const MAX: u32 = 1024
pub type UserId = u64

fn identity<T>(value: T) -> T {
    return value
}
```

The same declarations using long-form aliases:

```ns
public function add(a: i32, b: i32) -> i32 {
    return a + b
}

public constant MAX: u32 = 1024
public typedef UserId = u64

public record User {
    id: UserId
}

function main() {
    var mut total = 0
    total = add(1, 2)
}
```

The canonical formatter rewrites the long forms to `pub`, `fn`, `const`, `type`, `struct`, `mut`, and `let`.

Loops, tuples, maps, member access, and defer:

```ns
fn count(items: [str]) -> usize {
    let mut n = 0

    for item in items {
        n = n + 1
    }

    while n < 10 {
        n = n + 1

        if n == 5 {
            continue
        }

        if n == 8 {
            break
        }
    }

    return n
}

fn demo() {
    let pair = (1, "a")
    let scores = { "a": 1, "b": 2 }

    defer print("done")

    if pair.0 == 1 {
        print(scores["a"])
    }
}
```

Patterns and optional values:

```ns
fn classify(value: Option<i32>) -> str {
    match value {
        Some(0) => "zero"
        Some(n) => "number"
        None => "none"
    }
}
```

Scoped statement block:

```ns
fn local_scope() {
    {@mode:core;
        let value = 42
        print(value)
    }
}
```

### 3.14 Ambiguity resolution

The grammar is parsed with these deterministic rules:

1. A `{` after a type path is a struct initializer; a `{` at a statement or expression position is a block or map literal according to context.
2. `<` after a path in expression position is parsed as type arguments when it is immediately followed by a type argument list and a call; otherwise it is comparison.
3. Prefix `!` followed by a type path and `(` or `{` is failure construction. Postfix `!` at the end of a postfix expression is propagation.
4. `@mode:` and `@memory:` are scope annotations inside `{ ... }`. `@mode(...)`, `@requires(...)`, `@tags(...)`, and `@export_as(...)` are item attributes. `@memory(...)` is reserved and invalid in v0.1; use a file or block wrapper.
5. `else` binds to the nearest unmatched `if`.
6. `|` is only an error-set separator in v0.1; it is not a bitwise operator.
7. `defer` binds the next statement.
8. Newlines terminate statements only at statement boundaries. Inside `(...)`, `[...]`, and `{...}`, newlines are whitespace.
9. A `match` arm body is the single expression or block after `=>`; the next arm starts at the next pattern.
10. Keyword aliases are lexed as keywords, never as identifiers. The formatter normalizes them to their canonical spelling.
11. At item position, `export` followed by `{`, `*`, or `default` is an export declaration; `export` followed by a declaration is a declaration modifier.
12. `private`, `pub`, and `public` are declaration modifiers only; they are invalid on statements and expressions.
13. `from` is only valid in an import or export declaration.
14. `default` is only valid after `export` or inside an import/export specifier.
15. `import` followed by an identifier and then `from` is a default import; followed by `,` it is a default-plus-named import; otherwise it is a project-path import.
16. `import` followed by `*`, `{`, or a string literal selects the namespace, named, or side-effect form respectively.
17. `;` is an optional soft separator after an item or statement. If two items or statements share a line, `;` is required between them. A newline is the normal separator. The `;` after a scope annotation is required.

A parser MUST reject an ambiguous construct with a clear diagnostic rather than guessing silently.

### 3.15 Declaration visibility

A declaration is a function, struct, enum, constant, type alias, or foreign module.

| Form | Meaning | Exportable | Importable |
|---|---|---|---|
| No modifier | Normal declaration; module-local by default | Yes, via an export declaration | Only after export |
| `pub` / `public` | Public declaration | Yes | Yes |
| `export` | Exported declaration | Yes | Yes |
| `private` | Private declaration | No | No |

Rules:

- A normal declaration is visible throughout its module. It is not visible to other modules until it is exported.
- A public or exported declaration is part of its module's export surface.
- `export { ... }` MAY export a normal or public declaration. It MUST NOT export a private declaration.
- A private declaration is visible only inside its declaring module. It cannot be imported, re-exported, or named by an export declaration.
- `pub`/`public` and `export` are equivalent export markers. Using both is allowed but redundant; the canonical formatter emits `export`.
- `private` MUST NOT be combined with `pub`, `public`, or `export`.
- Duplicate visibility markers, such as `pub public`, and duplicate `export` modifiers are `NS3002`.
- Struct fields and enum variants inherit the visibility of their declaration. v0.1 has no field-level or variant-level visibility.
- A private declaration MUST NOT appear in the signature of a public or exported declaration, in a public type alias, or in a public struct field or enum variant field. Such a leak is `NS3010`.
- A private implementation detail may be used inside the body of a public declaration.
- Visibility is compile-time only. It does not change runtime behavior, memory behavior, or capabilities.
- Importing or re-exporting a declaration does not bypass mode or capability requirements.

Canonical formatter rules:

| Source | Canonical output |
|---|---|
| `public fn` | `pub fn` |
| `pub export fn` | `export fn` |
| `export pub fn` | `export fn` |
| `private fn` | `private fn` |
| `fn` | `fn` |

Diagnostics:

| Code | Meaning |
|---|---|
| `NS3001` | Private declaration cannot be exported |
| `NS3002` | Conflicting visibility modifiers |
| `NS3003` | Duplicate export name |
| `NS3004` | Duplicate default export |
| `NS3005` | Unknown export target |
| `NS3006` | Duplicate import binding |
| `NS3007` | Imported name is not exported |
| `NS3008` | Conflicting re-exports |
| `NS3009` | Default export target is not a value |
| `NS3010` | Private declaration leaks through a public API |
| `NS3011` | Duplicate top-level declaration |
| `NS3012` | Imported module cannot be resolved |
| `NS3013` | Cyclic named import or re-export |

### 3.16 Imports and exports

#### Imports

| Form | Meaning |
|---|---|
| `import path` | Namespace import using the final path segment as the alias |
| `import path as alias` | Namespace import using the project path syntax and an explicit alias |
| `import "specifier"` | Side-effect import |
| `import * as alias from "specifier"` | Namespace import |
| `import { name } from "specifier"` | Named import |
| `import { name as local } from "specifier"` | Renamed named import |
| `import fallback from "specifier"` | Default import |
| `import fallback, { name } from "specifier"` | Default plus named import |
| `import { default as local } from "specifier"` | Default import through a named specifier |

Rules:

- `path` is the dotted project-module syntax, such as `std.io` or `app.util`.
- `module_specifier` is a string literal resolved by the package and module rules in §12.4–§12.5, such as `"./thing.ns"`, `"std:fs"`, or `"some-package"`.
- `import path` binds the final path segment as the alias. For example, `import std.io` is equivalent to `import std.io as io`.
- `import path as alias` is equivalent to `import * as alias from "resolved-path"`.
- If the implicit alias collides with another binding, it is `NS3006`.
- A named import binds an exported name. Importing a missing or private name is `NS3007`.
- A default import requires the target module to have a default export. A missing default is `NS3007`.
- In an import specifier, `default` MUST be aliased, as in `import { default as local } from "specifier"`. A bare `import { default }` is `NS3006`.
- A namespace import exposes named exports and the default export under `default`.
- A side-effect import evaluates the module for initialization and binds no names.
- Imported names are normal declarations in the importing module. They may be re-exported.
- Duplicate local binding names are `NS3006`.
- Imports MUST NOT bypass modes, capabilities, FFI allowlists, or package security rules.

#### Exports

| Form | Meaning |
|---|---|
| `export fn name(...)` | Export a function declaration |
| `export function name(...)` | Same, using the long-form alias |
| `export { local }` | Export a normal or public declaration |
| `export { local as name }` | Export under a different name |
| `export { local as default }` | Export a value as the default export |
| `export { name } from "specifier"` | Re-export a named export |
| `export { name as alias } from "specifier"` | Re-export under a different name |
| `export * from "specifier"` | Re-export all named exports |
| `export * as namespace from "specifier"` | Re-export a namespace |
| `export * as default from "specifier"` | Re-export a namespace as the default export |
| `export default expression` | Default-export a value |
| `export default fn name(...) { ... }` | Default-export a function declaration |

Rules:

- A module's export surface is the union of exported declarations, export lists, re-exports, and at most one default export.
- An export declaration may appear before or after the declaration it names. Resolution is module-wide.
- `export { local }` promotes a normal declaration into the module export surface. It does not change the declaration's internal name or local visibility.
- `export { privateName }` is `NS3001`.
- Exporting an unknown local name is `NS3005`.
- Without `from`, `default` is not a valid local name; `export { default }` is `NS3005`.
- Two different declarations exporting the same name are `NS3003`.
- Re-exporting the same local binding under its existing name is redundant and allowed; the formatter MAY drop the redundant entry. Exporting it under a new alias is allowed.
- A module MUST NOT have more than one default export; a second one is `NS3004`.
- An export specifier whose exported name is `default`, such as `export { local as default }`, counts as the default export.
- `export *` re-exports named exports but not the default export.
- A namespace export, such as `export * as ns from \"specifier\"`, exposes named exports and the default export under `default`.
- `export { default as alias } from \"specifier\"` re-exports the default under a name.
- `export { default } from \"specifier\"` re-exports the default and counts toward the one-default rule.
- `export * as default from \"specifier\"` exports a namespace as the default and counts toward the one-default rule.
- An explicit export takes precedence over `export *`. If two `export *` sources provide the same name, it is `NS3008`.
- Re-exporting a missing or private name is `NS3007`.
- `export default` MUST NOT expose a private declaration. If the expression is an identifier resolving to a private declaration, it is `NS3001`.
- `export default` MUST target a value. A type alias or other type-only declaration is `NS3009`.
- Any export whose exported name is `default` MUST target a value.
- `export` is a compile-time visibility declaration. It does not add a runtime wrapper, allocation, or serialization step.
- A type alias is type-only. Exporting it is allowed for type checking, but it creates no runtime export; JS/TS output erases it.
- Importing a type-only export is erased in JS/TS output.
- Type-only exports remain type-only through `export { ... }` and `export *`.
- `export type Foo = ...` is a type-alias declaration with an export modifier. It is not TypeScript's `export type { ... }` form.
- A struct or enum is both a type and a value when the target gives it a runtime representation.
- Re-exporting a capability-bearing module requires a compatible mode. Capabilities remain viral through exports.

Example:

```ns
module app.main

import std.io as io
import std.math
import { read, write as writeFile } from "std:fs"
import fallback from "./thing.ns"
import "./effects.ns"

fn helper() -> i32 {
    return 1
}

private fn secret() -> i32 {
    return 2
}

pub fn public_api() -> i32 {
    return helper()
}

export fn exported() -> i32 {
    return public_api()
}

export { helper, public_api as api }
export { read } from "std:fs"
export * from "./shared.ns"
export * as tools from "./tools.ns"
export default exported
```

Normal declarations can be promoted anywhere at item position:

```ns
fn makenum() -> i32 {
    return 42
}

fn astra() {
    // ...
}

export { makenum, astra }
```

Invalid:

```ns
private fn secret() -> i32 {
    return 2
}

export { secret } // NS3001
```

---

## 4. Types

### 4.1 Primitive types

| Type | Meaning | Required behavior |
|---|---|---|
| `i8`, `i16`, `i32`, `i64` | Signed fixed-width integer | Wrapping by default |
| `u8`, `u16`, `u32`, `u64` | Unsigned fixed-width integer | Wrapping by default |
| `f32`, `f64` | IEEE floating point | IEEE semantics |
| `bool` | Boolean | `true` or `false` |
| `str` | Immutable UTF-8 text | Byte length contract |
| `bytes` | Mutable byte buffer | Bounds-checked |
| `usize` | Pointer-sized unsigned integer | Target-sized |
| `isize` | Pointer-sized signed integer | Target-sized |
| `Never` | Type of `panic`/`abort` | No normal value |
| `()` | Unit | No value |

`i128`/`u128` are not part of v0.1.

### 4.2 Composite types

- `[T]` — ordered sequence of `T`
- `[T; N]` — fixed-size array
- `{K: V}` — map from `K` to `V`
- `(A, B)` — tuple
- `struct` — named product type
- `enum` — tagged union
- `Option<T>` — optional value
- `Result<T, E>` — value-style result
- `Ptr<T>` — manual allocation handle; native/Wasm only
- `MutSlice<T>` — mutable view over contiguous elements
- `fn(A) -> B` — function type

### 4.3 Option and null

There is no `null`.

```ns
let name: Option<str> = None
let name: Option<str> = Some("Astra")
```

`Option<T>` MUST be used for optional values. JavaScript `null` and `undefined` are not part of the source type system; they may appear only at an explicit web boundary.

### 4.4 Result

`Result<T, E>` is a library type for value-style handling.

```ns
let result: Result<i32, ParseError> = Ok(42)
```

The preferred error mechanism for fallible functions is the `!` effect described in §7.

### 4.5 Generics

v0.1 supports monomorphized generics:

```ns
fn identity<T>(value: T) -> T {
    return value
}

struct Box<T> {
    value: T
}
```

v0.1 does not support typeclasses, traits, higher-kinded types, or specialization.

### 4.6 Inference

- Local variable types MAY be inferred.
- Function parameter and return types MUST be explicit in public APIs.
- Generic arguments MAY be inferred when unambiguous.
- No implicit numeric conversion is allowed.
- No implicit truthiness is allowed.
- No implicit stringification is allowed.
- No implicit `Option` or error unwrapping is allowed.

### 4.7 Exhaustiveness

`match` MUST be exhaustive. A missing case is a compile-time error.

### 4.8 Semantics matrix

| Feature | Portable core rule | JS/TS | Native | Wasm |
|---|---|---|---|---|
| Integers | Fixed-width; wrapping by default | `number` / `bigint` plus helpers | Machine integers | Wasm integers |
| Checked maths | `math.integers: safe-checked` | Checked helpers | Checked intrinsics | Checked helpers |
| Strings | UTF-8 byte contract | Native/encoded plus helpers | Pointer + length | Pointer + length |
| Arrays | Ordered, bounds-checked | `Array` plus checks | Bounds checks | Bounds checks |
| Maps | Insertion-ordered in v0.1 | `Map` | Ordered map | Ordered map |
| Null | None; use `Option<T>` | `null` only at boundary | Tag or niche | Tag or niche |
| Errors | Effect-tracked `!` | Generated control flow | Tag/register | Tag/register |
| Memory | GC by default; manual on native/Wasm | Host GC; reject manual | GC or manual | GC or manual |
| Concurrency | Scoped tasks and channels | Web Workers | OS thread pool | Threads or workers |
| FFI | Manifest-resolved | ESM imports | C ABI | Wasm imports |

---

## 5. Integer semantics

### 5.1 Default policy

The default integer policy is `wrapping`.

A fixed-width integer operation wraps modulo 2^N, where N is the bit width.

```ns
let x: i32 = 2147483647
let y = x + 1 // -2147483648 under the default policy
```

### 5.2 Manifest policy

```json
{
  "math": {
    "integers": "wrapping"
  }
}
```

Allowed values:

- `wrapping` — wrapping arithmetic; default
- `safe-checked` — overflow traps at runtime
- `arbitrary` — arbitrary-precision behavior where supported

The selected policy MUST be recorded in build metadata.

### 5.3 Explicit operations

```ns
let a = x +% y          // wrapping addition
let b = x.wrapping_add(y)
let c = x.checked_add(y)! // checked addition
```

Rules:

- `+%`, `-%`, `*%`, `/%` are explicit wrapping operations.
- `.wrapping_add`, `.wrapping_sub`, `.wrapping_mul`, `.wrapping_div` are explicit APIs.
- `.checked_add` and related APIs return a fallible result typed `T ! OverflowError`.
- Explicit operations override the project policy.
- `safe-checked` MUST NOT silently turn explicit wrapping operations into checked operations.
- `arbitrary` MUST NOT silently change explicit fixed-width operations.

### 5.4 Division by zero

Integer division by zero:

- MUST be a runtime panic in `wrapping` and `safe-checked` modes.
- MUST be an explicit error or arbitrary-precision behavior in `arbitrary` mode if the target supports it.

The compiler MUST NOT generate undefined behavior for division by zero.

---

## 6. String and byte semantics

### 6.1 Canonical model

`str` is an immutable UTF-8 byte sequence.

```ns
let s = "héllo😀"
```

### 6.2 Required operations

| Operation | Result |
|---|---|
| `s.len()` | UTF-8 byte length |
| `s.char_count()` | Unicode scalar value count |
| `s.is_empty()` | `true` if byte length is zero |
| `s.bytes()` | Byte iterator or byte view |
| `s.chars()` | Unicode scalar value iterator |
| `s.byte_at(i)` | `Option<u8>` |
| `s.char_at(i)` | `Option<str>` |
| `s.slice(start, end)` | `str ! SliceError` |

`s.slice` uses byte offsets. It MUST fail if the range is out of bounds or splits a UTF-8 scalar value.

### 6.3 Representation policy

```json
{
  "strings": {
    "model": "utf8"
  }
}
```

Allowed values:

- `utf8` — native/Wasm pointer + length; JS may use encoded helpers
- `native` — JS/TS may use native strings, but must preserve the observable contract

A target MAY change representation. It MUST NOT change `len()`, `char_count()`, or byte semantics.

### 6.4 Bytes

`bytes` is mutable and bounds-checked.

```ns
let buf = bytes.new(16)
buf[0] = 0xFF
```

Out-of-bounds indexing is a runtime panic in managed code and a checked error in manual code where the API says so.

---

## 7. Errors

### 7.1 Error effect

A fallible function declares an error set after `!`:

```ns
fn parse_age(input: str) -> i32 ! InvalidAge | ParseIntError {
    if input.is_empty() {
        return !InvalidAge("Age cannot be empty")
    }
    return input.to_int()!
}
```

Rules:

- `!` in a return type introduces an error effect.
- The error set is a union of nominal error types.
- A function without `!` is infallible.
- A fallible function MUST declare every error it can produce.
- The compiler MUST reject a call to a fallible function that does not propagate or handle the error.
- Each error effect carries the source span of the failure construction or
  propagating call. Diagnostics for an unhandled effect MUST point at that span,
  not at the whole enclosing function.
- Effects are merged deterministically by nominal error type. Duplicate effects
  of the same type retain their first construction/propagation site.
- An imported struct or enum error type MUST resolve through that module's
  exported type surface. An imported function's declared error set is compatible
  with the same nominal type declared by the importing function.

### 7.2 Failure construction

```ns
return !InvalidAge("Age cannot be empty")
```

`!Type(args)` constructs a failure value of the declared error type.

The error type MUST be nominal and constructible:

- A struct error accepts positional or named construction matching its fields.
- An enum error MAY be constructed positionally only when it has exactly one
  variant; an ambiguous multi-variant enum constructor is `NS4012`.
- Non-nominal, unresolved, and non-constructible error types are `NS4012`.
- Named initializers validate declared fields, report missing or duplicate
  fields as `NS4016`, and report a wrong field type at the field value as
  `NS4001`.

### 7.3 Propagation

```ns
let age = parse_age(input)!
```

The postfix `!`:

- If the call succeeds, evaluates to the success value.
- If the call fails, returns the error from the current function.
- Is only valid if the current function declares a compatible error effect.

A fallible call without `!` is a compile-time error unless it is explicitly handled.
Propagation is checked at the propagation site against the enclosing function's
declared error set: an undeclared error is `NS4012`, and an invalid `!` target is
`NS4013`.

### 7.4 Handling

A fallible expression MAY be handled with `match`:

```ns
match parse_age(input) {
    Ok(age) => print(age)
    Err(error) => print(error)
}
```

`Ok` and `Err` are patterns for the error-effect value at a handling site. This is not a requirement to wrap every successful return in `Ok(...)`.

Rules:

- Handling MUST cover all error variants.
- Error values MUST NOT be silently discarded.
- A `match` over a fallible expression MUST be exhaustive over the error set.
- A `match` handles the scrutinee's effect only structurally: it consumes the
  effect when the patterns exhaust the `Ok`/`Err` shape. A non-exhaustive match
  reports `NS4010` and MUST NOT also report `NS4011` for the same scrutinee.
- Effects raised inside an arm body still propagate or require handling.

### 7.5 Panic and abort

```ns
panic("invariant violated")
abort("unrecoverable")
```

- `panic` reports a programmer bug and terminates the current task.
- `abort` terminates immediately without unwinding.
- `panic` and `abort` have type `Never` and may appear where any type is expected.
- Panics MUST NOT cross an FFI boundary silently.
- Panics MUST produce a clean message and, where supported, a stack trace.

### 7.6 Target representations

| Target | Representation |
|---|---|
| Native | Value + tag or error register |
| Wasm | Value + tag or multi-value |
| JS/TS | Generated control flow or `try`/`catch` at boundaries |
| FFI | Explicit error mapping |

A target MUST NOT use an invisible exception path that changes source semantics.

---

## 8. Memory

### 8.1 Default

The default memory policy is GC-managed memory.

Managed code:

- Has no user-visible finalizers.
- Has no manual `free`.
- Uses bounds-checked collections.
- Cannot create raw pointers in `core`.

### 8.2 Manual memory

Manual memory is available only when:

- The resolved mode grants manual memory. `system` grants it; another mode MAY grant it only through an explicit manifest capability.
- The target is native or Wasm.
- The file or block policy is `manual`.

JavaScript builds MUST reject manual-memory files.

### 8.3 Precedence

```text
block {@memory:...;}
  > file wrapper
  > manifest memory.files
  > manifest memory.default
```

Example manifest:

```json
{
  "memory": {
    "default": "auto",
    "files": {
      "src/io.ns": "manual"
    }
  }
}
```

### 8.4 Manual API

```ns
{@mode:system;
    {@memory:manual;
        let buffer = Memory.alloc<u8>(1024)!
        defer Memory.free(buffer)

        // use buffer
    }
}
```

Minimum v0.1 manual API:

| Operation | Signature |
|---|---|
| `Memory.alloc<T>(count: usize)` | `Ptr<T> ! AllocError` |
| `Memory.free(ptr: Ptr<T>)` | `()` |
| `ptr.read(index: usize)` | `T` |
| `ptr.write(index: usize, value: T)` | `()` |
| `ptr.as_slice(count: usize)` | `MutSlice<T>` |
| `Region.new(bytes: usize)` | `Region ! AllocError` |
| `region.alloc<T>(count: usize)` | `Ptr<T> ! AllocError` |
| `region.free()` | `()` |

Rules:

- `Memory.alloc` is fallible.
- `Memory.free` is explicit.
- `defer` runs in reverse order at scope exit.
- `Memory.alloc` in a GC-managed scope is a compile-time error.
- `Region.new` and `region.alloc` are scoped allocation and do not require the manual-memory mode (the mode gate applies to `Memory.alloc`/`Memory.free`): a region's blocks are released by `region.free()`, and a region that is never freed leaks them — the GC never reclaims manual memory.
- The element type `T` of `Memory.alloc<T>` and `region.alloc<T>` MUST NOT contain GC-managed references (`str`, `bytes`, `[T]`, a map, `Channel`, `Task`, or a struct, enum, or tuple containing them); such an allocation is a compile-time error.
- `Ptr<T>` is not a GC reference.
- `ptr.read` and `ptr.write` are unchecked except in an explicit debug-checked mode.
- Manual pointers MUST NOT be captured by GC-managed closures without an explicit boundary.
- Double-free is a programmer bug; debug builds MAY detect it.
- Use-after-free is undefined behavior in manual mode.

### 8.5 FFI ownership

- Borrowed pointers are valid only for the call.
- Owned pointers transfer according to the foreign declaration.
- The compiler MUST NOT infer foreign ownership.
- FFI declarations MUST state ownership.

---

## 9. Concurrency

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
- The parent scope MUST wait for children before exiting.
- A child panic fails the parent scope.
- Detached tasks are not allowed in the portable core.
- A task MAY be cancelled cooperatively.
- CPU tasks and I/O readiness are distinct concepts.

### 9.2 Channels

- `Channel.create<T>()` creates a channel.
- `send` and `receive` are fallible.
- A channel is bounded by default.
- A bounded channel applies backpressure: `send` blocks while the buffer is full, and `receive` blocks while the buffer is empty.
- `close` is explicit.
- Sending to a closed channel fails.
- Cancellation MUST interrupt blocked channel operations.

### 9.3 Isolation

- Shared mutable state is not allowed between tasks in the portable core.
- Messages are copied or serialized.
- Immutable values MAY be shared.
- `SharedArrayBuffer` is an explicit `web` opt-in.
- Native tasks use isolated heaps or synchronized message queues.

### 9.4 Target mapping

| Target | Mapping |
|---|---|
| Native | OS thread pool |
| Web/JS | Web Workers + `postMessage` |
| Wasm | Threads when available; worker fallback |
| Async I/O | Separate I/O runtime |

### 9.5 Task results

`spawn` returns `Task<T>`.

- `task.join()` waits for completion and returns the task result.
- A child error is propagated explicitly.
- `task.cancel()` requests cooperative cancellation.
- The enclosing scope joins remaining children automatically.

The exact `TaskError` shape belongs in the standard library specification. It MUST preserve the child error.

---

## 10. Modes and capabilities

### 10.1 Built-in modes

| Mode | Purpose | Typical targets |
|---|---|---|
| `core` | Portable libraries and shared code | All |
| `app` | Applications and scripts | Native, Wasm, JS |
| `web` | Browser, DOM, fetch, workers | JS, `wasm32-web` |
| `system` | Filesystem, networking, raw memory, FFI | Native, `wasm32-wasi` |

### 10.2 Selection pipeline

1. `--mode` CLI flag
2. `ASTRA_MODE` environment variable
3. Target-bound mode in the manifest
4. `build.defaultMode`
5. Least-privileged inference from the static call graph

The first four entries are explicit selections. An explicit mode is authoritative:
inference MUST NOT replace it. The checker still validates source tags, scoped
blocks, and imported requirements against it. When no explicit selection exists,
the compiler infers the least-privileged built-in mode.

The resolved mode, its source (`explicit` or `inferred`), the inferred
requirement, the inferred mode of each project module, and the explanation tree
MUST be recorded in build/compiler metadata. The inferred mode is part of the
build cache key.

### 10.3 Source requirements

```ns
@requires(system)
@tags(fs, ffi)
module io
```

- `@requires` declares the minimum mode.
- `@tags` declares capabilities.
- Requirements are viral.
- A caller that does not declare a compatible requirement is a compile-time error.

### 10.4 Scoped mode blocks

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

Rules:

- A nested block MAY narrow capabilities.
- A nested block MUST NOT silently widen capabilities beyond the parent context.
- Widening requires an explicit enclosing requirement or boundary.
- A `core` file cannot contain a reachable `system` block without upgrading its requirement.
- The semicolon after the annotation is required.

### 10.5 Inference

The compiler infers the least-privileged mode from:

- Module and item `@requires` annotations
- `@tags` capability declarations
- Foreign modules and manual-memory scopes
- Capability-bearing built-ins, such as console output
- The local static call graph, including recursion and mutual recursion
- Scoped `@mode:` blocks

Built-in mode requirements use containment, not numeric rank: `app` contains
`core`, `web` contains `core` and `app`, and `system` contains `core` and `app`.
`web` and `system` are incomparable. A project that needs both has no single
built-in mode and MUST fail with `NS5006`.

Rules:

- Explicit annotations win.
- Inference is static only.
- Dynamic calls and dynamic imports require explicit declarations.
- A scoped block MAY narrow its parent requirements. Widening beyond an explicit
  enclosing requirement is `NS5002`; an inferred enclosing scope is upgraded
  when no explicit requirement exists.
- Requirements propagate through calls to a fixed point, including recursive
  cycles.
- A project's inferred requirement is the containment join of all module
  requirements. A single module's explicit requirement narrower than another
  module's requirement is `NS5001`, because a project build has one mode.
- Inference MUST be explainable with `astra explain` and in compiler metadata.
- The compiler MUST NOT silently choose between incompatible modes.

### 10.6 Explanation

`astra explain` MUST produce a reason tree. Compiler responses expose the same
information as `mode`, `modeSource`, `requiredMode`, `inferredModes`,
`modeReasons`, and `modeExplanation` metadata, so tooling can render it before
the dedicated CLI command lands.

```text
Resolved mode: system (via manifest target: native)
  ├── src/main.ns -> uses app capabilities
  └── src/io.ns   -> forces system mode
       └── line 42: @requires(system) due to raw pointer usage
```

---

## 11. FFI

### 11.1 Foreign module declaration

```ns
foreign module math_engine {
    fn compute_matrix(data: [f64]) -> [f64]
}
```

### 11.2 Manifest mapping

```json
{
  "ffi": {
    "math_engine": {
      "native": {
        "source": "./native/libmath.so",
        "type": "c-abi",
        "ownership": "borrowed",
        "threading": "safe",
        "errors": "result-tag"
      },
      "typescript": {
        "source": "./npm-package/index.js",
        "type": "esm",
        "ownership": "borrowed",
        "threading": "single-threaded",
        "errors": "exception"
      }
    }
  }
}
```

### 11.3 Rules

- A required foreign module MUST have a mapping for the active target.
- A missing mapping is a compile-time error.
- FFI is unsafe by default.
- FFI declarations MUST state ownership, threading, and error behavior.
- Foreign exceptions MUST be caught and mapped.
- Foreign pointers MUST NOT be captured by GC-managed values without an explicit boundary.
- C++ requires a C shim.
- Wasm imports MUST be allowlisted by capability.

### 11.4 Type mapping

| Neo Astra | JS/TS | C ABI |
|---|---|---|
| `i32`, `i64`, `f64` | `number` / `bigint` | fixed-width / `double` |
| `str` | `string` | `const char*` + length |
| `bytes` | `Uint8Array` | pointer + length |
| `[T]` | `Array<T>` | pointer + length |
| `Option<T>` | `T \| null` at boundary | tagged or nullable |
| `T ! E` | mapped exception/result | error tag + payload |

Conversions MUST be explicit.

---

### 11.5 Generic instantiation exports (FFI-001)

A generic function is not callable across the C boundary by default
(`11-c-abi.md` §3). A module may explicitly export concrete instantiations of
a generic function with the `@export_as` attribute:

```ns
@export_as("ns_demo_identity_i32", T = i32)
@export_as("ns_demo_identity_str", T = str)
export fn identity<T>(value: T) -> T {
    return value
}

@export_as("ns_demo_first_i32_str", A = i32, B = str)
export fn first<A, B>(a: A, b: B) -> A {
    return a
}
```

Rules:

- `@export_as(...)` is an item attribute (§3.1) and MAY be repeated on a
  single declaration; each occurrence lists one instantiation.
- It MAY only appear on a generic function declaration that is exported
  (`export`/`pub`). Applying it to a non-generic function, or to a non-exported
  function, is `NS7010`.
- The first argument is a string literal naming the exported C symbol. It is
  used verbatim (the compiler does not mangle it) and MUST be a valid C
  identifier, unique within the artifact. An invalid identifier, or a
  collision with another `@export_as` symbol, is `NS7013`.
- The remaining arguments are `TypeParameter = type` bindings. Every type
  parameter of the declaration MUST be bound exactly once, in any order; a
  missing, extra, or duplicated binding is `NS7011`.
- Every bound type MUST be a concrete type that is boundary-safe for the
  active target's C ABI (§11.4): primitives, structs, enums, and composites of
  those; no type parameter and no type that the C backend cannot represent. A
  non-boundary-safe binding is `NS7012`.
- Two attributes on the same declaration that name the same symbol or bind the
  same type list are `NS7013`.
- Each listed instantiation is emitted as a concrete, externally visible
  function with exactly the given symbol and the substituted signature,
  monomorphized exactly as an internal call site is. It obeys the declaration's
  mode and capability requirements.
- The attribute does not change the function's generic status: internal call
  sites still monomorphize per call site, and the generic symbol itself remains
  non-exported.
- Without `@export_as`, a generic function remains outside the C
  boundary, exactly as before.
- The symbol named by the attribute is part of the ABI contract
  (`11-c-abi.md` §3).

Conversions at the boundary remain explicit (§11.4).

---

## 12. Manifest and build

### 12.1 Manifest files and rules

Two manifests exist, split by purpose (D26):

- `core.json` / `core.toml` — the **package manifest**: what the package is
  and what it depends on. Exactly one of the two MUST exist.
- `build.json` / `build.toml` — the **build manifest**: how the package is
  built, run, and which toolchain it asks for. Optional; absent means the
  documented defaults in §12.2.

Rules:

- Exactly one of `core.json` or `core.toml` MUST exist.
- At most one of `build.json` or `build.toml` MAY exist.
- Both formats MUST use the same semantic schema per file.
- Every key belongs to exactly one manifest. A key placed in the wrong file
  MUST be rejected with a diagnostic naming the file it belongs in.
- Unknown keys MUST be rejected.
- Duplicate keys MUST be rejected.
- Wrong value types MUST be rejected.
- Paths MUST NOT escape the project root unless explicitly allowed.
- Required FFI mappings MUST exist.
- `astra check` MUST validate both manifests before building.
- Each manifest MUST be canonically formattable without changing semantics.
- `astra fmt` MUST preserve TOML `#` comments and reject duplicate keys.

### 12.2 Sections

`core.*` (required) — package identity and dependency information:

```json
{
  "package": {
    "name": "my-app",
    "version": "0.1.0",
    "edition": "2026"
  },
  "dependencies": {},
  "packageSources": {},
  "registry": {},
  "ffi": {},
  "capabilities": []
}
```

`capabilities` declares the package's privilege set and `ffi` its foreign
contract; both are properties of the package, so they live here.

`build.*` (optional) — build directives and the compiler guide:

```json
{
  "build": { "defaultTarget": "native" },
  "targets": {
    "native": { "backend": "native", "outDir": "dist/native" }
  },
  "nova": { "entry": "src/main.ns" },
  "math": { "integers": "wrapping" },
  "strings": {},
  "memory": {},
  "toolchain": { "c": "auto" }
}
```

- `build` selects the default target and mode.
- `targets` configures each target: `backend`, `mode`, `module`/`outDir`,
  and emission options.
- `nova` configures the development entry point and watch roots.
- `math`, `strings`, and `memory` are the semantic policies that guide
  codegen.
- `toolchain` names the build toolchain policy (`auto` = resolve from the
  environment, or a pinned toolchain id); the resolved toolchain is recorded
  in the lockfile (§12.3).

Defaults when `build.*` is absent (D26): `defaultTarget` = `native`;
`defaultMode` omitted, so the mode is inferred least-privileged (§5); `targets.native`
= `{ backend: "native", outDir: "dist/native" }`; `nova.entry` =
`src/main.ns`; `math.integers` = `wrapping`; `toolchain.c` = `auto`.

### 12.3 Lockfile

- The lockfile is `core.lock.json` and serves both manifests.
- It records resolved packages, source-qualified package IDs, target scoping, capabilities, integrity hashes, lifecycle-script policy, compiler metadata, and the resolved toolchain.
- CI SHOULD verify that the lockfile is current.
- A release build SHOULD use `--locked` or `--frozen`.
- The same lockfile serves Nova and compiled builds.

### 12.4 Package sources

NoPM is the package manager for both Nova and compiled builds. Package IDs are
source-qualified, such as `npm:react@19`, `cargo:serde@1`, `git+https://...#ref`,
or `path:../local`.

Initial adapters:

- Local path
- Git
- npm
- Cargo
- PyPI
- NuGet
- OCI

The hosted universal registry
gateway is optional; local development MUST work without it.

Rules:

- Lifecycle scripts are disabled by default.
- Remote packages require integrity hashes.
- Git dependencies are pinned to commits.
- A package adapter MUST NOT execute arbitrary install scripts without explicit opt-in.
- A package that is incompatible with the selected target MUST fail with a clear diagnostic.

### 12.5 Module resolution

- Dotted paths such as `std.io` resolve in the project module namespace.
- Relative specifiers such as `"./thing.ns"` and `"../shared.ns"` resolve from the importing file.
- `std:` specifiers resolve to built-in modules, such as `"std:fs"`.
- Bare specifiers such as `"some-package"` resolve through the manifest dependency map.
- A package adapter MAY expose npm, Cargo, or other ecosystem packages, but the adapter MUST declare the mapping.
- A module specifier MUST NOT escape the project root unless the manifest explicitly allows it.
- Resolution is deterministic and MUST be recorded in build metadata.
- Unresolved modules, ambiguous mappings, and disallowed escapes are compile-time errors.

---

## 13. Standard library surface

### 13.1 Core

- `str`, `bytes`
- arrays, maps, tuples
- math
- `Option`, `Result`
- iterators
- formatting
- time and duration
- channels
- scoped tasks

### 13.2 Web

- DOM
- `fetch`
- storage
- events
- Web Workers
- browser-specific APIs

### 13.3 System

- filesystem
- networking
- processes
- environment
- threads
- raw memory
- OS-specific APIs

A module MUST NOT use an API its resolved mode does not provide.

---

## 14. Diagnostics

### 14.1 Requirements

- Every error MUST have a stable diagnostic code.
- Every diagnostic MUST include a file and source span.
- Diagnostics MUST be deterministic.
- Diagnostics MUST be available as JSON.
- Warnings MUST NOT hide errors.

### 14.2 JSON shape

```json
{
  "code": "NS4001",
  "severity": "error",
  "message": "Type mismatch: expected i32, found str.",
  "file": "src/main.ns",
  "span": {
    "start": 42,
    "end": 58
  },
  "mode": "core",
  "target": "js",
  "hint": "Convert the value explicitly.",
  "related": [
    {
      "file": "src/main.ns",
      "span": { "start": 20, "end": 24 },
      "message": "The value was declared as str here."
    }
  ]
}
```

Each entry in `related` MUST contain `file`, `span`, and `message`.

### 14.3 Code ranges

| Range | Area |
|---|---|
| `NS1xxx` | Lexical |
| `NS2xxx` | Parser |
| `NS3xxx` | Resolver |
| `NS4xxx` | Types |
| `NS5xxx` | Modes and capabilities |
| `NS6xxx` | Memory |
| `NS7xxx` | FFI and packages |
| `NS8xxx` | Backends and targets |
| `NS9xxx` | Tooling and configuration |

Codes are stable and MUST NOT be reused for different meanings.

### 14.4 Parser diagnostics

| Code | Meaning |
|---|---|
| `NS2001` | Expected token or delimiter |
| `NS2002` | Invalid syntax |
| `NS2003` | Invalid type |
| `NS2004` | Invalid pattern |
| `NS2005` | Invalid or unexpected item |
| `NS2006` | Missing newline or `;` boundary |
| `NS2007` | Invalid modifier |

### 14.5 Type and effect diagnostics

| Code | Meaning |
|---|---|
| `NS4001` | Type mismatch |
| `NS4002` | Unknown identifier |
| `NS4003` | Unknown type |
| `NS4004` | Wrong argument count |
| `NS4005` | Value is not callable |
| `NS4006` | Invalid operand types |
| `NS4007` | Missing return on a reachable path |
| `NS4008` | Reserved |
| `NS4009` | Assignment to an immutable binding |
| `NS4010` | Non-exhaustive match |
| `NS4011` | Unhandled error effect |
| `NS4012` | Error is not declared by the function |
| `NS4013` | Invalid error propagation |
| `NS4014` | Invalid control flow |
| `NS4015` | Invalid member or index access |
| `NS4016` | Invalid, missing, or duplicate struct field |
| `NS4017` | Invalid pattern or pattern arity |
| `NS4018` | Invalid type argument or arity |

### 14.6 Mode and capability diagnostics

| Code | Meaning |
|---|---|
| `NS5001` | Mode requirement is not satisfied |
| `NS5002` | Mode widening is not allowed |
| `NS5003` | Capability requirement is not satisfied |
| `NS5004` | Manual memory requires system mode |
| `NS5005` | Invalid capability or mode annotation |
| `NS5006` | No single built-in mode satisfies a project requirement |
| `NS5007` | System mode: two or more state-changing calls in a target-native-order position (binary operands, array elements, struct fields, map entries) - hoist the calls into bindings |

### 14.7 Nova runtime diagnostics

Nova reports parse, resolve, type, mode, and capability failures with the same
`NSxxxx` codes as `astra check`. Runtime failures use a separate stable range:

| Code | Meaning |
|---|---|
| `NV0000` | Frontend diagnostics were returned unchanged |
| `NV0001` | Manual memory is unsupported by the first Nova engine |
| `NV0002` | Missing entry point |
| `NV0003` | Unsupported runtime feature or host adapter |
| `NV0004` | Unhandled error escaped the entry point |
| `NV0005` | Panic, including the interpreted call stack |
| `NV0006` | Abort without unwinding |
| `NV0007` | Nova internal invariant failure |

`NV` codes are development-runtime diagnostics. They do not replace `NS` source
diagnostics and do not appear in compiled target output.

### 14.8 NoPM package diagnostics

NoPM uses a separate provisional range for package IDs, dependency graphs, and
lockfiles:

| Code | Meaning |
|---|---|
| `NP0001` | Invalid source-qualified package ID |
| `NP0002` | Invalid dependency declaration or duplicate descriptor |
| `NP0003` | Missing package descriptor |
| `NP0004` | Dependency cycle |
| `NP0005` | Conflicting root dependency alias |
| `NP0006` | Package is incompatible with the selected target |
| `NP0007` | Package capability is incompatible with the selected mode |
| `NP0008` | Invalid lockfile, missing integrity, or unpinned Git dependency |
| `NP0009` | Missing or stale lockfile |
| `NP0010` | Lockfile write failure |
| `NP0011` | Package adapter, path, fetch, or installation failure |
| `NP0012` | NoPM command-line usage error |

These codes are provisional until the V1 NoPM diagnostics table is frozen.

### 14.9 Memory diagnostics

The `NS6xxx` range declared in §14.3 covers memory-policy violations:

| Code | Meaning |
|---|---|
| `NS6001` | Manual allocation outside a manual-memory scope |
| `NS6002` | Manual allocation of a GC-managed element type |

`Memory.alloc` used where the file or block policy is not `manual` is rejected
with `NS6001`, which is how the rule in §8.4 that "`Memory.alloc` in a
GC-managed scope is a compile-time error" is reported. A program whose mode
does not grant manual memory reports `NS5004` instead: the mode condition is
checked first, so one program reports the single condition it actually
violates.

An allocation whose element type `T` holds GC-managed references is rejected
with `NS6002`, reporting the §8.4 rule that `T` MUST NOT contain them. The
conditions are checked in order - mode (`NS5004`), then scope (`NS6001`), then
element type (`NS6002`) - so the single reported condition remains the one the
program actually violates first. The predicate is the inverse of what a
precise collector traces: `str`, `bytes`, `[T]`, maps, `Channel`, `Task`, and
structs, enums, or tuples containing them; scalars, `Ptr<U>`, `Region`, and
composites built only from pointer-free types are allowed.

### 14.10 FFI diagnostics

The `NS7xxx` range (§14.3) covers FFI. The `@script` block diagnostics are
`NS7001`–`NS7008` (see `Design/V1/10-inline-script-interop.md` §6). The
generic instantiation export diagnostics (§11.5) are:

| Code | Meaning |
|---|---|
| `NS7010` | `@export_as` on a non-generic or non-exported function |
| `NS7011` | `@export_as` binding list does not bind every type parameter exactly once |
| `NS7012` | `@export_as` bound type is not C-boundary-safe |
| `NS7013` | `@export_as` symbol is not a valid C identifier, or duplicates another symbol or instantiation |

---

## 15. Conformance

### 15.1 Required areas

Every v0.1 implementation MUST have conformance coverage for:

- Lexical rules and comments
- Keyword aliases and formatter normalization
- Declaration visibility, private export rejection, and private API leakage
- Import forms and export lists
- Default exports and re-exports
- Type-only export/import erasure and default-export value rules
- Import/export conflict diagnostics
- Parser and formatter round-trip
- Primitive types
- Integer wrapping
- Checked integers
- String byte length
- Unicode scalar count
- Byte operations
- Arrays and bounds
- Maps and iteration order
- Option and Result
- Error propagation
- Error handling
- Panic behavior
- Mode selection
- Source tags
- Scoped mode blocks
- Capability inference
- Memory policy precedence
- Manual memory rejection on JS
- Task scoping
- Channel send/receive/close
- Cancellation
- FFI type mapping
- FFI errors
- Manifest validation
- Lockfile validation
- JSON diagnostics

### 15.2 Case format

A conformance case SHOULD include:

```text
tests/conformance/<area>/<case>/
  main.ns
  core.json
  expected.json
```

`expected.json` records:

- Targets
- Expected exit status
- Expected stdout or behavior
- Expected diagnostics
- Target-specific notes

### 15.3 Target requirements

Target tiers are defined in [`Design/V1/README.md`](./Design/V1/README.md).

- Tier 1 targets MUST pass all portable cases.
- Tier 2 targets MUST pass their documented subset.
- Unsupported features MUST fail with a clear diagnostic, not a silent fallback.

### 15.4 Target-specific cases

| Target | Required cases |
|---|---|
| JS/TS | ESM named/default/namespace import and export mapping, native `str` boundary, no manual memory, Web Worker messaging, JSON diagnostics |
| Native | C ABI mapping, pointer width, GC/manual memory, thread-pool tasks, FFI ownership |
| Wasm | Import allowlist, linear memory, thread availability/fallback, boundary serialization |
| Manifest | JSON/TOML schema parity, lockfile freshness, unknown-key rejection |
| Tooling | `astra check`, `astra build`, `astra run`, `astra fmt`, `astra ast`, `astra explain` |

---

## 16. v0.1 implementation scope

The first runnable implementation slice is JavaScript-first.

### 16.1 Required for the first slice

- Lexer
- Parser
- AST
- Resolver
- Type checker
- Mode and capability checks
- Error-effect checks
- Nova typed-HIR tree-walking interpreter (development runtime)
- JavaScript ESM backend
- CLI: `astra check`, `astra build`, `astra run`, `astra fmt`, `astra ast`, `astra explain`
- CLI: `nova run`, `nova repl`, `nova watch`, `nova test`
- CLI: `nopm install`, `nopm lock`, `nopm add`, `nopm remove`
- Browser playground
- JSON diagnostics
- Manifest validation and canonical formatting
- NoPM universal resolver with path, Git, npm, and Cargo adapters
- Core conformance suite

### 16.2 Defined but not required for the first slice

- Native C backend
- Wasm backend
- Native GC runtime
- Manual-memory runtime
- FFI runtime
- NoPM adapters for PyPI, NuGet, and OCI
- The hosted universal registry gateway
- Native thread pool
- Web Worker code generation
- LSP
- Incremental compilation

These features are still normative in this specification. A target that does not implement them MUST reject them explicitly.

### 16.3 JavaScript output contract

The JavaScript backend MUST emit readable ESM.

- No framework dependency.
- No mandatory runtime beyond small, tree-shaken helpers.
- No generated code that re-parses `.ns` source.
- No hidden global state.
- Value-bearing public/exported declarations map to ESM exports; normal and private declarations remain module-local.
- Type-only exports and imports are erased.
- Default exports map to ESM `export default`.
- Native JavaScript strings MAY be used at an explicit web boundary, but the core `str` contract MUST be preserved.
- Manual memory MUST be rejected.
- Output SHOULD include source maps.
- Generated identifiers SHOULD remain recognizable and deterministic.

The backend MAY specialize generated code, but specialization MUST NOT change observable semantics.

---

## 17. Provisional choices and open questions

The following choices are normative for v0.1 but should be reviewed before V1 freeze.

### 17.1 Provisional choices

1. Newlines are the normal statement/item boundaries; semicolons are optional soft separators for same-line constructs and are required after scope annotations.
2. `defer` is available for manual scoped cleanup in v0.1.
3. Maps are insertion-ordered.
4. `str.slice` uses byte offsets.
5. `Channel.create<T>()` creates a bounded channel.
6. Error handling uses `Ok`/`Err` patterns over fallible expressions.
7. `Task<T>` is the return type of `spawn`.
8. `@requires`, `@tags`, and `@export_as` are item/module attributes.
9. Scoped annotations use `{@mode:name; ...}` syntax.
10. `core.json` schema URL is not yet finalized.

### 17.2 Open questions

1. Exact map iteration order guarantee.
2. Whether `defer` is in V1 or v0.1 only.
3. Exact `Task<T>` join/error shape.
4. Channel default capacity.
5. Exact schema domain and manifest `$schema` URL.
6. Whether generic `type` aliases are monomorphized or erased.
7. Whether `match` arms require blocks for multi-statement bodies.
8. Whether `if` remains an expression in all contexts or becomes statement-only.
9. Whether `str` supports direct index syntax.
10. Whether `safe-checked` panic behavior is catchable.
11. Whether `arbitrary` integers are available in `core`.
12. Whether FFI ownership can be inferred from C headers.
13. Whether the first slice includes `astra test`.
14. Whether the formatter should preserve blank lines.
15. Whether TOML manifests should adopt the JSON manifest key order or remain layout-only.
16. Whether import cycles are allowed or rejected statically.
17. Whether struct fields and enum variants may have their own visibility.
18. Whether `export default` may expose non-function values.
19. Whether package-root exports are declared in the manifest or inferred from the entry module.
20. Whether `export *` should remain portable or be restricted on some targets.
21. Whether type-only imports and exports need explicit `import type`/`export type` syntax.
22. Whether declaration-level inline `@script` blocks should be part of the language, and how their raw-body terminator, typed export surface, and target matrix are defined. The current proposal is [`Design/V1/10-inline-script-interop.md`](./Design/V1/10-inline-script-interop.md).

---

## 18. Appendix — complete examples

### 18.1 Hello world

```ns
module app.main

fn main() {
    print("hello, Neo Astra")
}
```

### 18.2 Structs and matching

```ns
module app.main

struct User {
    id: u64
    name: str
}

enum Status {
    Pending
    Active(reason: str)
}

fn describe(status: Status) -> str {
    match status {
        Status.Pending => "pending"
        Status.Active(reason) => reason
    }
}

fn main() {
    let user = User { id: 1, name: "Astra" }
    print(user.name)
    print(describe(Status.Pending))
}
```

### 18.3 Errors

```ns
module app.main

struct InvalidAge(str)

fn parse_age(input: str) -> i32 ! InvalidAge | ParseIntError {
    if input.is_empty() {
        return !InvalidAge("Age cannot be empty")
    }
    return input.to_int()!
}

fn main() {
    match parse_age("42") {
        Ok(age) => print(age)
        Err(error) => print("invalid age")
    }
}
```

### 18.4 Mode boundary

```ns
module app.main

{@mode:core;
    fn parse(input: str) -> i32 ! ParseIntError {
        return input.to_int()!
    }
}

{@mode:system;
    fn read_config(path: str) -> bytes ! IoError {
        return fs.read(path)!
    }
}
```

### 18.5 Concurrency

```ns
module app.main

fn main() {
    let channel = Channel.create<str>()

    spawn || {
        channel.send("hello from a task")!
    }

    let message = channel.receive()!
    print(message)
}
```

### 18.6 Foreign module

```ns
module app.main

foreign module math_engine {
    fn compute_matrix(data: [f64]) -> [f64]
}

fn main() {
    let result = math_engine.compute_matrix([1.0, 2.0, 3.0])
    print(result.len())
}
```

---

## 19. Change control

This specification is normative for v0.1.

Changes follow this order:

1. Identify affected confirmed decisions.
2. If a confirmed decision must change, update `architecture.md` first.
3. Update this specification.
4. Update the V1 design if the change affects it.
5. Add or update conformance cases.
6. Record the change in `Agent/state-log.md`.

A change that exists only in source code is not a language change.

---

## 20. Decision traceability

| ID | Confirmed decision | Specification home |
|---|---|---|
| D1 | Configurable memory; GC default | §8 |
| D2 | Hybrid JS fidelity | §4.8, §7.6, §9.4 |
| D3 | TypeScript orchestrator plus Rust core | §0.3, §16 |
| D4 | Whole-system architecture | §0.3, §16 |
| D5 | Custom syntax with Rust/JS bareness and C/C++ low-level capability | §1–§3 |
| D6 | Fixed-width integers with selectable policy | §5 |
| D7 | UTF-8 canonical strings | §6 |
| D8 | Effect-tracked explicit errors | §7 |
| D9 | Structured concurrency | §9 |
| D10 | Polymorphic FFI | §11 |
| D11 | Ecosystem package sources | §12.4 |
| D12 | Neo Astra naming, `.ns`, `astra` | Header, §1 |
| D13 | Mode selection pipeline | §10.2 |
| D14 | Modes are capability layers | §10 |
| D15 | Least-privileged inference | §10.5 |
| D16 | Readability, determinism, AI-friendliness | §1.4, §3, §14 |
| D17 | `//` comments only | §1.3, §2.4 |
| D18 | `core.json` or `core.toml`; `core.lock.json` | §12 |
| D19 | TypeScript + Rust compiler split | §0.3, §16 |
| D20 | Strictness policy | §4.6, §7, §14 |
| D21 | Memory precedence | §8.3 |
| D22 | Keyword aliases with canonical formatting | §1.6, §3.1–§3.7 |
| D23 | Declaration visibility, imports, exports, and canonical ESM mapping | §3.15, §3.16, §16.3 |
| D24 | Semicolon soft separators and scope-header separators | §1.4, §2.2, §3.1, §3.7, §3.14 |
| D25 | Nova development interpreter and NoPM universal packages; production stays compiled | §16.1, `Docs/Design/V1/08-nova-and-nopm.md` |

D3, D4, and D19 are architecture-level decisions. This document consumes their consequences but does not replace the compiler design in `Docs/Design/V1/02-compiler-design.md`. D25 is a tooling/runtime decision; it does not change the language contract.

### 20.1 Review result

- Every confirmed decision maps to a normative section.
- The normative language rules are consistent with D1–D25.
- Open questions are recorded as proposals in §17, not hidden assumptions.
- Phase 3 scaffolding is unblocked after the specification is accepted.
