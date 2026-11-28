# RFC B - Generic calls (reviewer draft)

**Status:** Reviewer proposal, non-normative  
**Date:** 2026-11-08  
**Target:** [`Syntax-v0.2.md`](Syntax-v0.2.md) Phase 1  
**Normative source today:** [`spec-v0.1.md`](../spec-v0.1.md)

## B.1 Summary

- Type arguments in **type position** keep `<...>`: `Box<T>`, `Option<Option<i32>>`.
- Type arguments in **expression position** use turbofish `::<...>`: `Memory.alloc::<u8>(1024)`.
- `::` exists only before `<`. It is not a path separator (paths still use `.`).
- In expression position, a bare `<` is always comparison. No lookahead heuristic.
- Type arguments may be omitted when inferable.

## B.2 Grammar (EBNF)

```ebnf
type_args      = "<" type { "," type } [ "," ] ">" ;
turbofish      = "::" type_args ;

(* postfix chain *)
postfix_expr   = primary_expr { call_suffix | index_suffix | field_suffix | try_suffix } ;
primary_expr   = path_expr | ... | struct_expr ;
path_expr      = IDENT { "." IDENT } ;
call_suffix    = [ turbofish ] "(" [ arg_list ] ")" ;
struct_expr    = type_path [ turbofish ] "{" [ field_init_list ] "}" ;

(* types are unchanged *)
type_path      = IDENT { "." IDENT } [ type_args ] ;
```

Lexer notes:

- `::` is a single token, legal only when followed by `<`.
- `>>` and `>=` must be split by the parser when closing nested type arguments (`Option<Option<i32>>`, `Box<Vec<i32>>=...`). This matters in Phase 4 when `>>` becomes a bitwise shift.
- Whitespace between `::` and `<` is not allowed.

Comparisons are non-associative. The expression grammar uses an optional single
comparison, not a repeating loop:

```ebnf
comparison_expr = additive_expr
                [ ( "<" | "<=" | ">" | ">=" ) additive_expr ] ;
```

Thus `a < b < c` and `a < b > (c)` are syntax errors; write
`(a < b) && (b < c)` when that is intended. `::<` is lexed before `<`, so
`f::<T>(x)` never enters comparison parsing.

## B.3 Precedence

The turbofish binds to the call suffix or struct initializer immediately after it, tighter than `()`:
`a.b::<T>(x)` is a call to `a.b` with type argument `T`. It is not a standalone suffix, so
`xs[0]::<T>` and `(f)::<T>` are errors.

## B.4 Struct literals versus blocks

- A `{` after a type path (optionally with turbofish) in expression position begins a struct initializer: `User { id: 1 }`, `Box::<i32> { value: 1 }`.
- **Restriction (Rust-style):** a struct initializer is not allowed unparenthesized in the head of `if`, `while`, `for`, or `match`. Write `if x == (User { id: 1 }) { ... }`. This resolves the `if x == User { ... } { ... }` ambiguity without lookahead.

## B.5 Positive examples

```ns
let buf = Memory.alloc::<u8>(1024)?
let ch  = Channel.create::<str>()
let id  = identity::<i32>(5)
let n   = value.convert::<f64>()
let b   = Box::<i32> { value: 1 }
let c: Box<i32> = Box { value: 1 }      // inferred from annotation
let ok  = a < b && b > c                // comparisons, untouched
let d: {str: Option<i32>} = {}          // type position uses <...>
```

## B.6 Negative examples

| Source | Diagnostic |
|---|---|
| `Memory.alloc<u8>(1024)` | in expression position use `::<u8>`; suggest the fix |
| `a < b > (c)` | comparison chain; comparisons are non-associative |
| `identity::<i32>` (no call) | turbofish must be followed by a call or struct initializer |
| `x::<T>(1)` where `x` is a local value | turbofish on a non-generic path |
| `xs[0]::<T>()` | turbofish must follow an identifier |
| `f:: <T>()` | no whitespace between `::` and `<` |
| `let v = Vec.new()` (not inferable) | cannot infer type argument; use `Vec.new::<T>()` |

Comparisons are made non-associative so that `a < b > c` can never be silently misread.

## B.7 Migration from the v0.1 heuristic

- v0.1 rule: `<` after a path is type arguments if followed by a type list and a call.
- Under `edition = "0.1"` the parser keeps the heuristic. Under `edition = "0.2"` it emits the diagnostic above with a machine-applicable fix.
- `ns fix` rewrites `path<T>(` to `path::<T>(` using the old heuristic and a parse of the file. Cases where the old heuristic was already ambiguous are reported for manual review, not rewritten.

## B.8 Phase 0 decisions (closed 2026-11-08)

1. **General `::` paths:** rejected. `::` is legal only immediately before
   `<`; paths continue to use `.`. A standalone `::` or `:: <` is an error.
2. **Wildcard type arguments:** rejected in `0.2.0`. `_` is not a type argument.
   When arguments are only partly inferable, omit the whole `::<...>` list and
   let inference handle it, or name every argument. `Map.new::<str, _>()` is
   invalid; write `Map.new()` when the whole list is inferable.
3. **Trailing comma:** allowed inside `::<...>` and every other comma-separated
   list, including type, parameter, field, variant, argument, array, map, and
   struct-initializer lists. The Phase 0 grammar uses a separate optional
   trailing comma so the LR(1) decision points stay disjoint.
4. **Enum variants:** the canonical spelling is the unqualified variant name
   with the turbofish on the variant: `None::<i32>`. `Option.None::<i32>` is
   rejected in edition 0.2 and migrated to `None::<i32>`; `Option::<i32>.None`
   is not valid. The turbofish binds to the constructor or variant, never to
   the enum path.
5. **Builtins:** explicit type arguments on builtins use the same turbofish
   rule. There is no builtin-only spelling.
6. **Whitespace:** `:: <` is an error. The lexer must produce a single `::`
   token and the parser must require `<` immediately after it.
7. **Support for non-call turbofish:** a turbofish may appear on a unit enum
   variant (`None::<i32>`) or before a call or struct initializer. It may not
   stand alone on an ordinary value or function path (`identity::<i32>`), and
   it may not follow an index expression (`xs[0]::<T>()`). The parser accepts the
   unit-variant shape; the checker rejects the others with the turbofish
   diagnostic.

The v0.1 heuristic remains in edition 0.1 only: `<` after a path is
type-argument syntax when the lookahead proves a type list followed by a call.
In edition 0.2, a bare `<` is always comparison, and the migration command
rewrites confident `path<T>(` calls to `path::<T>(`. Ambiguous matches are
reported for manual review, never guessed.

## B.9 Lead review notes (2026-11-08)

- The grammar conflict is resolved: turbofish belongs to the call/constructor
  suffix, not to `path_expr`.
- `struct_expr` is explicit in the primary-expression grammar.
- Comparisons are non-associative in the EBNF, not only in prose.
- `fail TypeName { field: value }` is accepted; see RFC A A.8.
- The Phase 0 conflict check in `tools/grammar-v0.2` covers turbofish, struct
  initializers versus blocks, `fail` versus calls, both scope spellings, and
  trailing commas with zero LR(1) conflicts.
