# Neo Astra Language Specification v0.2 (skeleton)

**Status:** Non-normative skeleton for the `0.2.0` cleanup train
**Date:** 2026-11-08
**Normative source today:** [`spec-v0.1.md`](spec-v0.1.md)
**Decision sources:** [`Report/Syntax-v0.2.md`](Report/Syntax-v0.2.md),
[`Report/RFC-A-error-model.md`](Report/RFC-A-error-model.md),
[`Report/RFC-B-generics.md`](Report/RFC-B-generics.md)
**Freeze gate:** Phases 0-5 of the v0.2 plan must be green before this file
becomes normative.

This file records the intended v0.2 grammar, edition rules, deprecations, and
diagnostics. It is **not** a second normative specification. Until the Phase 6
freeze, `spec-v0.1.md` remains the only normative language contract. Where this
skeleton and `spec-v0.1.md` disagree, `spec-v0.1.md` wins for every shipped
edition.

## 0. Scope

### 0.1 What `0.2.0` is

`0.2.0` is a **syntax cleanup**, not a feature release. It:

- deprecates same-semantics spellings and gives canonical replacements;
- makes ambiguous grammar explicit (turbofish, struct literals versus blocks,
  `fail` versus calls, scope blocks);
- adds the edition switch so existing v0.1 projects keep their syntax;
- adds `astra migrate` as the only rewrite path;
- keeps the existing AST shape used by parser, checker, C codegen, Nova, and the
  JS/TS backends.

### 0.2 What `0.2.0` is not

These are a separate feature train and must not be pulled into the cleanup
freeze:

- `if let` / `while let`;
- ranges and slicing syntax;
- bitwise operators;
- `char` and new scalar literals;
- lambdas (`fn(x) => expr`);
- a complete integer/float suffix table;
- error-set polymorphism or `Task.join()` redesign;
- a `Result<T, E>` effect bridge.

### 0.3 AST freeze

The cleanup reuses the existing AST:

| v0.2 spelling | Existing AST |
|---|---|
| `fail Error(...)` / `fail Error { ... }` | `ExprKind::Fail` + `FailArguments` |
| `expr?` | `ExprKind::Propagate` |
| `value.method::<T>()` | `ExprKind::TypeArguments` |
| `Box::<T> { ... }` | `StructInit.path.arguments` |
| `@mode(core) { ... }` / `@memory(manual) { ... }` | `ScopeAnnotation` + `ScopedItems` / `ScopedStatements` |
| canonical aliases | canonical `Keyword` variants and `Visibility` |

The cleanup must not add AST nodes. Feature work in 0.2 adds nodes later and
must update every backend explicitly.

## 1. Editions

### 1.1 Accepted values

- `edition = "0.1"` selects the v0.1 syntax.
- `edition = "0.2"` selects the cleanup syntax.
- A missing `edition` field means `"0.1"`.
- Any other value is rejected with `NS2008`, including `"2026"`, with a hint to
  use `"0.1"` or `"0.2"`.

The edition is declared in `core.toml` and is passed to `check`, `fmt`, `ast`,
`explain`, and build commands. It is not inferred from source syntax.

### 1.2 Compatibility rule

- v0.1 source keeps parsing and formatting with v0.1 spellings.
- v0.2 source may still contain deprecated spellings; they are accepted unless
  `astra check --canonical-syntax` or `astra fmt --canonical` is active.
- `astra migrate` is the only command that rewrites source between editions.
- The formatter is edition-aware. It must not silently rewrite v0.1 source to
  v0.2-only spellings.
- v0.1 remains normative until this specification is promoted.

### 1.3 Strictness

- `astra check --canonical-syntax` is a deny lint. It reports every deprecated
  form with a replacement, but never rewrites.
- `astra fmt --check` checks formatting only.
- `astra fmt --canonical` is a check-only canonical-source mode for v0.2. It
  errors on deprecated spellings and never rewrites.
- `astra migrate` rewrites; it is idempotent and supports `--dry-run`, `--check`,
  and `--json`.

## 2. Lexical changes

### 2.1 `fail`

`fail` is a keyword under edition 0.2 and an identifier under edition 0.1. The
v0.1 expression form `!Error(...)` remains valid only under edition 0.1.

### 2.2 `?`

`?` is a postfix error-propagation token under edition 0.2. Postfix `!` remains
valid only under edition 0.1; under 0.2 it is rejected with a `?` fix.

### 2.3 `::` and `::<`

- `::` is a single token and is legal only immediately before `<`.
- `:: <` with whitespace is an error (`NS2011`).
- A standalone `::` is an error (`NS2011`).
- `::` is not a path separator; paths use `.`.
- In type position, `<`/`>` delimit type arguments. In expression position, a
  bare `<` is always comparison.

### 2.4 Tuple indexes

After a postfix `.`, a digit begins an integer-only tuple index. It never starts
a float:

```ns
t.0.1       // two tuple indexes
1.0         // float
t.0.1.2     // three tuple indexes
```

A float token in tuple-index position is rejected with `NS2014` and a targeted
hint. `.5` is not a valid expression; it belongs to the later range work.

### 2.5 Comments

`///` has no special documentation meaning in v0.2. It is an ordinary `//`
comment. No doc-comment trivia is emitted.

### 2.6 Deprecated lexical spellings

| Canonical | Deprecated alias | Status |
|---|---|---|
| `fn` | `function` | accepted; omitted from canonical docs |
| `let` | `var` | accepted; `var mut` is still just `let mut` |
| `const` | `constant` | accepted |
| `type` | `typedef` | accepted |
| `struct` | `record` | accepted |
| `mut` | `mutable` | accepted |
| `pub` | `public` | accepted |

Aliases are spelling options, not semantic modes. They remain reserved words;
deprecation does not make them usable as identifiers. Contextualizing them is
optional later work.

`/%` is deprecated but keeps its existing overflow semantics. The formatter
preserves `/%`; migration does not rewrite it to `/`, because that would change
behavior. Strict canonical mode reports it and asks for a manual decision.

`spawn || { ... }` is deprecated. Both spellings are accepted; the formatter
emits `spawn { ... }`.

## 3. Grammar

This grammar is the canonical v0.2 surface. Deprecated aliases and old forms are
accepted by the parser but omitted from this grammar. The parser may accept the
deprecated forms only where section 5 says so.

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
visibility           = "pub" | "private" ;

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
named_import         = "import" "{" [ import_specifier_list ] "}" "from" module_specifier ;
default_import       = "import" identifier "from" module_specifier ;
default_named_import = "import" identifier "," "{" [ import_specifier_list ] "}"
                       "from" module_specifier ;
import_specifier     = ( identifier | "default" ) [ "as" identifier ] ;
import_specifier_list = import_specifier { "," import_specifier } [ "," ] ;
module_specifier     = string_literal ;

export_decl    = export_list | export_all | export_default ;
export_list    = "export" "{" [ export_specifier_list ] "}"
                 [ "from" module_specifier ] ;
export_specifier = ( identifier | "default" )
                   [ "as" ( identifier | "default" ) ] ;
export_specifier_list = export_specifier { "," export_specifier } [ "," ] ;
export_all     = "export" "*" [ "as" ( identifier | "default" ) ]
                 "from" module_specifier ;
export_default = "export" "default" ( expression | function_decl ) ;

path           = identifier { "." identifier } ;

attribute      = "@" identifier [ "(" [ attribute_args ] ")" ] ;
attribute_args = expression { "," expression } [ "," ] ;
```

Newlines remain item and statement boundaries. The optional `;` in `program` and
`item` does not make newlines insignificant. The Phase 0 conflict-check grammar
is newline-insensitive, so it requires `;` after constants; that is a check
restriction, not a source rule.

`@mode` and `@memory` are reserved scope introducers at item and statement
position. An attribute cannot use those two names.

### 3.2 Functions

```ebnf
function_decl  = "fn" identifier
                 [ type_params ]
                 "(" [ parameter_list ] ")"
                 [ "->" type [ "!" error_set ] ]
                 block ;

type_params    = "<" identifier { "," identifier } [ "," ] ">" ;
parameter_list = parameter { "," parameter } [ "," ] ;
parameter      = identifier ":" type ;

error_set      = type_path { "|" type_path } ;
```

A function without `->` returns `()`. A `!` with no error types is invalid. The
error set is a disjoint union of nominal error types.

### 3.3 Types

```ebnf
type           = type_path
               | "[" type "]"
               | "[" type ";" integer_literal "]"
               | "{" type ":" type "}"
               | "(" [ type_list ] ")"
               | "fn" "(" [ type_list ] ")" [ "->" type [ "!" error_set ] ] ;

type_path      = identifier { "." identifier } [ type_args ] ;
type_args      = "<" type { "," type } [ "," ] ">" ;
type_list      = type { "," type } [ "," ] ;
```

`Option<T>` and `Result<T, E>` are ordinary generic types. `Result<T, E>` is a
library/FFI value type, not the source-level error mechanism.

`_` is not a type argument in v0.2. Wildcard type arguments are rejected with
`NS4018`; omit the whole `::<...>` list when inference should choose it.

### 3.4 Structs and enums

```ebnf
struct_decl    = "struct" identifier [ type_params ]
                 ( "{" [ field_list ] "}" | "(" [ type_list ] ")" ) ;
field_list     = field { "," field } [ "," ] ;
field          = identifier ":" type ;

enum_decl      = "enum" identifier [ type_params ]
                 "{" [ variant_list ] "}" ;
variant_list   = variant { "," variant } [ "," ] ;
variant        = identifier [ "(" [ variant_fields ] ")" ] ;
variant_fields = variant_field { "," variant_field } [ "," ] ;
variant_field  = [ identifier ":" ] type ;
```

Commas are required between v0.2 fields and variants. A trailing comma is
allowed. v0.1 newline-separated fields remain accepted under edition 0.1.

### 3.5 Constants, aliases, and foreign modules

```ebnf
const_decl     = "const" identifier ":" type "=" expression ;
type_alias     = "type" identifier [ type_params ] "=" type ;

foreign_module = "foreign" "module" identifier "{" { foreign_fn } "}" ;
foreign_fn     = "fn" identifier "(" [ parameter_list ] ")"
                 [ "->" type [ "!" error_set ] ] ;
```

A foreign function has no body. Its mapping is supplied by the manifest.
`?` and `fail` are rejected in constant initializers.

### 3.6 Statements and blocks

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

let_stmt       = "let" [ "mut" ] identifier [ ":" type ] "=" expression ;
assignment     = assignable "=" expression ;
expression_stmt = expression ;
return_stmt    = "return" [ expression ] ;
break_stmt     = "break" ;
continue_stmt  = "continue" ;
defer_stmt     = "defer" statement ;
```

`return fail X(...)` and `return fail X { ... }` are rejected as redundant
(`NS4014`): `fail` already diverges and has type `Never`.

### 3.7 Control flow

```ebnf
if_expr        = "if" expression block [ "else" ( if_expr | block ) ] ;
while_stmt     = "while" expression block ;
for_stmt       = "for" identifier "in" expression block ;

match_expr     = "match" expression "{" { match_arm } "}" ;
match_arm      = pattern "=>" ( expression | block ) ;
```

`if` and `while` conditions must be `bool`; there is no implicit truthiness.
A block-bodied match arm may omit the trailing comma.

### 3.8 Patterns

```ebnf
pattern        = "_"
               | identifier
               | literal_pattern
               | type_path [ "(" [ pattern_list ] ")" ]
               | type_path "." identifier [ "(" [ pattern_list ] ")" ]
               | "(" pattern { "," pattern } [ "," ] ")" ;

pattern_list   = pattern { "," pattern } [ "," ] ;
```

`Ok(...)` and `Err(...)` are constructor patterns over fallible expressions. The
`Err` payload follows RFC A A.8:

- `Err(e)` binds the declared error side;
- `Err(_)` covers all error members;
- member-specific patterns such as `Err(InvalidAge { message })` are allowed;
- a match must cover both `Ok` and `Err`;
- if member-specific `Err` patterns are used, every declared error member must be
  covered or a final `_`/binding arm must be present.

### 3.9 Expressions

```ebnf
expression     = assignment_expr ;
assignment_expr = logical_or_expr [ "=" assignment_expr ] ;
logical_or_expr = logical_and_expr { "||" logical_and_expr } ;
logical_and_expr = equality_expr { "&&" equality_expr } ;
equality_expr  = comparison_expr { ( "==" | "!=" ) comparison_expr } ;
comparison_expr = additive_expr
                [ ( "<" | "<=" | ">" | ">=" ) additive_expr ] ;
additive_expr  = multiplicative_expr
                 { ( "+" | "-" | "+%" | "-%" ) multiplicative_expr } ;
multiplicative_expr = unary_expr
                 { ( "*" | "/" | "%" | "*%" | "/%" ) unary_expr } ;
unary_expr     = ( "-" | "not" ) unary_expr | postfix_expr ;
postfix_expr   = primary_expr
                 { call_suffix | index_suffix | field_suffix | try_suffix } ;

call_suffix    = [ turbofish ] "(" [ expression_list ] ")" ;
index_suffix   = "[" expression "]" ;
field_suffix   = "." ( identifier | integer_literal ) ;
try_suffix     = "?" ;

primary_expr   = literal
               | path_expr [ turbofish ]
               | "(" expression ")"
               | "[" [ expression_list ] "]"
               | "{" [ map_entry_list ] "}"
               | struct_expr
               | block
               | if_expr
               | match_expr
               | spawn_expr
               | fail_expr
               | panic_expr
               | abort_expr ;

path_expr      = identifier { "." identifier } ;
struct_expr    = path_expr [ turbofish ]
                 "{" [ field_init_list ] "}" ;
turbofish      = "::<" type_list ">" ;

expression_list = expression { "," expression } [ "," ] ;
field_init_list = field_init { "," field_init } [ "," ] ;
field_init      = identifier ":" expression ;
map_entry_list  = map_entry { "," map_entry } [ "," ] ;
map_entry       = expression ":" expression ;
assignable      = identifier { "." identifier | "[" expression "]" } ;

literal         = integer_literal
                | float_literal
                | string_literal
                | byte_literal
                | "true"
                | "false"
                | "()" ;
literal_pattern = literal ;
```

Comparisons are non-associative: `a < b < c` and `a < b > (c)` are syntax
errors. Write `(a < b) && (b < c)` when that is intended.

A turbofish may appear:

- on a call: `identity::<i32>(5)`;
- on a struct initializer: `Box::<i32> { value: 1 }`;
- on a unit enum variant: `None::<i32>`.

It may not stand alone on an ordinary value or function path
(`identity::<i32>`), follow an index expression (`xs[0]::<T>()`), or appear twice
in one postfix chain. The parser accepts the unit-variant shape; the checker
rejects the others with `NS2010`/`NS2011`.

The canonical enum-variant spelling is `None::<i32>`. Edition 0.2 rejects
`Option.None::<i32>` and `Option::<i32>.None`; migration rewrites the former to
`None::<i32>` when the target is unambiguous.

A struct initializer is not allowed unparenthesized in the head of `if`,
`while`, `for`, or `match`. Write `if x == (User { id: 1 }) { ... }`. This keeps
the block and struct-initializer decision out of lookahead.

### 3.10 `fail`, `spawn`, and panic forms

```ebnf
fail_expr      = "fail" type_path
                 ( "(" [ expression_list ] ")"
                 | "{" [ field_init_list ] "}" ) ;
panic_expr     = "panic" "(" expression ")" ;
abort_expr     = "abort" "(" expression ")" ;

spawn_expr     = "spawn" block
               | "spawn" "||" block ;  (* deprecated; formatter emits "spawn" *)
```

`fail E(...)` and `fail E { ... }` have type `Never`. The named form is valid
for nominal error types with fields. `fail E(...)` requires `E` to be in the
enclosing function's declared error set (`NS4012`). `return fail ...` is
redundant (`NS4014`).

### 3.11 Operator precedence

From highest to lowest:

1. Postfix: call, turbofish call/constructor, index, field access, `?`
2. Unary: `-`, `not`
3. `*`, `/`, `%`, `*%`, `/%`
4. `+`, `-`, `+%`, `-%`
5. `<`, `<=`, `>`, `>=` (non-associative)
6. `==`, `!=`
7. `&&`
8. `||`
9. Assignment

`&&` and `||` short-circuit. `?` binds with the postfix group, so
`a.b(c)?.d` parses as `((a.b(c))?).d`.

## 4. Scopes

### 4.1 Canonical scope blocks

```ebnf
scoped_item_block = "@" scope_kind "(" identifier ")"
                    "{" { item [ ";" ] } "}" ;
scoped_stmt_block = "@" scope_kind "(" identifier ")"
                    "{" { statement [ ";" ] } "}" ;
scope_kind        = "mode" | "memory" ;
```

The canonical forms are `@mode(core) { ... }` and `@memory(manual) { ... }`.
They are valid at item and statement positions respectively. The new form has
no mandatory semicolon.

### 4.2 Deprecated v0.1 scope blocks

```ebnf
scoped_item_block = "{" "@" scope_kind ":" identifier ";"
                    { item [ ";" ] } "}" ;
scoped_stmt_block = "{" "@" scope_kind ":" identifier ";"
                    { statement [ ";" ] } "}" ;
```

The old forms are accepted under both editions for compatibility, but are
deprecated under edition 0.2 and omitted from canonical docs. The formatter emits
the canonical form under edition 0.2 and preserves the old form under edition
0.1. `astra migrate` rewrites old scopes to canonical scopes.

## 5. Edition-gated and deprecated forms

| Form | edition 0.1 | edition 0.2 | Canonical action |
|---|---|---|---|
| `function`, `var`, `constant`, `typedef`, `record`, `mutable`, `public` | accepted, canonical formatter normalizes | accepted but deprecated | canonical spelling |
| `!Error(...)` expression | accepted | rejected (`NS2015`) | `fail Error(...)` |
| postfix `expr!` | accepted | rejected (`NS2015`) | `expr?` |
| `{ @mode:name; ... }` | accepted | accepted but deprecated | `@mode(name) { ... }` |
| `path<T>(...)` heuristic | accepted | bare `<` is comparison; `NS2010` | `path::<T>(...)` |
| `spawn || { ... }` | accepted | accepted but deprecated | `spawn { ... }` |
| `/%` | accepted | accepted but deprecated; semantics preserved | manual: `/` or explicit overflow API |
| `///` | comment | ordinary comment | `//` if docs are not intended |

Do not rewrite `/%` to `/` in the formatter or migration. Do not rewrite
deprecated aliases in a v0.1 project unless migration is explicitly requested.

## 6. Diagnostics

The v0.2 front-end reserves `NS2008`-`NS2019`. `NS2001`-`NS2007` remain the
existing parse codes, and `NS4010`-`NS4019` remain the type/effect codes.

| Code | Meaning | Notes |
|---|---|---|
| `NS2008` | Unsupported or invalid edition | `core.toml`; hint to `"0.1"`/`"0.2"` |
| `NS2009` | Deprecated canonical alias | applies to the seven aliases; message names the replacement |
| `NS2010` | Expression-position type arguments require `::<...>` | v0.2 bare `<` is comparison |
| `NS2011` | Malformed turbofish | `:: <`, standalone `::`, second turbofish |
| `NS2012` | Deprecated `spawn ||` | formatter emits `spawn` |
| `NS2013` | Deprecated `/%` | no automatic rewrite; semantics differ from `/` |
| `NS2014` | Invalid tuple index | float token after `.`; targeted hint |
| `NS2015` | Removed v0.1 spelling under edition 0.2 | postfix `!`, old scope block |
| `NS2016` | Canonical-syntax lint family | reserved aggregate; details may use `NS2009`-`NS2015` |
| `NS2017`-`NS2019` | Reserved for v0.2 front-end follow-ups | do not assign outside this block |

Type/effect diagnostics used by the closed decisions:

| Code | Meaning |
|---|---|
| `NS4010` | Non-exhaustive match (including missing `Ok`/`Err`/error member) |
| `NS4012` | Error is not declared by the function |
| `NS4013` | Invalid error propagation |
| `NS4014` | Invalid control flow (`return fail`, and existing cases) |
| `NS4017` | Invalid pattern, arity, or error-member pattern |
| `NS4018` | Invalid type argument or arity (including wildcard arguments) |

The untracked `Docs/error-code-ledger.md` is a reference snapshot. These
reservations live here and in the implementation when Phase 3 starts; Phase 0
does not rewrite the ledger.

## 7. Canonical formatter rules

- v0.1 input stays v0.1-valid. Aliases may canonicalize only where the v0.1
  grammar permits the canonical spelling.
- v0.2 input uses `fail`, `?`, `::<...>`, and `@mode(...)` / `@memory(...)`.
- `/%` stays `/%` under both editions.
- `spawn || { ... }` becomes `spawn { ... }`.
- `///` stays an ordinary comment; the formatter does not create doc trivia.
- Trailing commas are preserved or normalized according to the formatter's
  canonical list policy, but they must remain valid in every list.
- `format(parse(x)) == x` for canonical sources.

## 8. Migration

`astra migrate` is the only rewrite path. It:

- rewrites the seven aliases to canonical spellings;
- rewrites `!Error(...)` / `!Error { ... }` in expression position to `fail`,
  never touching `-> T ! E` or `!=`;
- rewrites postfix `!` to `?`;
- rewrites old scope blocks to `@mode(...)` / `@memory(...)`;
- rewrites confident `path<T>(` calls to `path::<T>(`; ambiguous matches are
  reported for manual review;
- rewrites `spawn ||` to `spawn`;
- leaves `/%` unchanged with a warning;
- changes `edition` only with an explicit flag;
- supports `--dry-run`, `--check`, and `--json`, and is idempotent.

## 9. Phase 0 grammar check

The scratch crate `tools/grammar-v0.2` encodes the ambiguity set with canonical
LR(1) tables and Pager merging. It is not a shipped dependency and is not a
workspace member.

```text
cargo test --manifest-path tools/grammar-v0.2/Cargo.toml
```

The four tests cover:

- zero conflicts for the focused v0.2 grammar;
- a known dangling-else grammar still reports a conflict (the checker is not
  silently accepting everything);
- coverage of `fail`, `spawn`/`spawn ||`, `::<`, `mode`/`memory`, struct
  initializers, and both scope spellings;
- optional trailing commas at every list site.

The focused grammar omits operator-precedence layers and control-flow
expressions; they do not participate in the ambiguity set. It models one
postfix suffix per expression; chained postfix forms are covered by the parser
suite and the full EBNF in section 3.9.

## 10. Freeze checklist

- [ ] Phase 0 decisions closed; grammar check green.
- [ ] Edition plumbing accepts only `"0.1"`/`"0.2"` and defaults to `"0.1"`.
- [ ] Lexer and parser handle both editions; v0.1 tests unchanged.
- [ ] Edition-aware formatter round-trips both editions.
- [ ] Canonical lint catches every deprecated form.
- [ ] `astra migrate` is idempotent and reports ambiguous rewrites.
- [ ] Conformance and the 10-20 program corpus pass.
- [ ] `cargo fmt --all --check`, workspace clippy, `cargo test --workspace`,
      `npm test`, native parity, and the canonical lint are green.
- [ ] Only then promote this file to normative and update the ledger, roadmap,
      state, and examples.
