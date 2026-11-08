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

## B.8 Unresolved questions

1. Should `::` double as a general path separator later?
2. Partial or wildcard arguments (`Map.new::<str, _>()`)?
3. Type arguments on enum variants: `Option.None::<i32>` versus `None::<i32>`.
4. Trailing comma inside `::<...>`: allowed or not?
5. Should explicit type arguments be permitted on builtins through the same rule?

## Lead review notes (2026-11-08)

- **Grammar conflict:** the draft has `[turbofish]` in both `call_suffix` and
  `struct_expr`, but `path_expr` no longer has it. That is the right shape:
  turbofish belongs to the call/constructor, not to a standalone path. Keep it
  out of `path_expr` so a turbofish cannot appear twice or without a call.
- **Missing `struct_expr`:** the original snippet's `primary_expr` omitted it;
  add it explicitly.
- **Comparison EBNF is still associative:** the draft says comparisons are
  non-associative but the v0.1 `comparison_expr = additive { ... }` still
  allows chains. Rewrite it as an optional single comparison, not a loop.
- **`fail` named initializers:** if error types support named construction,
  `fail TypeName { field: value }` must be added to RFC A's grammar.
- **Turbofish on enum variants:** decide before freeze. Recommended:
  `None::<i32>` is a constructor call, not a path; `Option.None::<i32>` should
  be rejected or normalized to one spelling.
- **Whitespace:** `:: <` is an error; keep that explicit in the lexer.
- **Trailing comma:** allow it, matching the general list rule.
