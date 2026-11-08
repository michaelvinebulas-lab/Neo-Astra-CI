# RFC A - Error model (reviewer draft)

**Status:** Reviewer proposal, non-normative  
**Date:** 2026-11-08  
**Target:** [`Syntax-v0.2.md`](Syntax-v0.2.md) Phase 1  
**Normative source today:** [`spec-v0.1.md`](../spec-v0.1.md)

## A.1 Summary

- Fallibility is an effect on the function type: `-> T ! E1 | E2`.
- `fail Type(args)` constructs and raises a failure.
- Postfix `?` propagates a failure to the caller.
- `Ok(...)` / `Err(...)` patterns are the handling vocabulary in `match`.
- `!` survives only in type position (error effect) and in `!=`. It never appears in expression position.
- `Result<T, E>` is a library/FFI value type, not a source-level error mechanism.

## A.2 Grammar (EBNF)

```ebnf
(* types *)
ret_type      = "->" type [ "!" error_set ] ;
error_set     = type_path { "|" type_path } ;
fn_type       = "fn" "(" [ type_list ] ")" [ ret_type ] ;

(* expressions *)
expr          = assign_expr ;
assign_expr   = or_expr [ "=" assign_expr ] ;
(* ... or_expr down to unary_expr as in v0.1 ... *)
unary_expr    = ( "-" | "not" ) unary_expr | postfix_expr ;
postfix_expr  = primary_expr { call_suffix | index_suffix | field_suffix | try_suffix } ;
try_suffix    = "?" ;

primary_expr  = ... | fail_expr ;
fail_expr     = "fail" type_path
                ( "(" [ arg_list ] ")" | "{" [ field_init_list ] "}" ) ;

(* handling *)
match_arm     = pattern "=>" ( expr | block ) ;
pattern       = ... | "Ok" "(" pattern ")" | "Err" "(" error_pattern ")" ;
error_pattern = "_" | identifier | pattern ;  (* constructor forms only; struct-field patterns deferred *)
```

`field_init_list` is the same comma-separated list used by struct
initializers, with an optional trailing comma. Both `fail Type(args)` and
`fail Type { field: value }` construct errors; the named form is accepted for
nominal error types with fields.

## A.3 Precedence and typing

- `?` is a postfix operator at level 1, same as call, index, and field access: `a.b(c)?.d` parses as `((a.b(c))?).d`.
- `fail` is a primary expression whose operand is a constructor call, so `fail X(1) + 2` is a type error, not a precedence puzzle.
- `fail E(...)` has type `Never`, so it is valid anywhere an expression is expected (`None => fail NotFound("x")`).
- `e?` where `e : T ! E1` evaluates to `T`. The enclosing function must declare an error set `S` with `E1 subset S`. Otherwise it is a compile error that names the missing error types.
- `fail E(...)` requires `E` in the enclosing function's declared set.
- `defer` statements run on the failure path before control leaves the function.
- `?` and `fail` are rejected in `const` initializers.

## A.4 Positive examples

```ns
fn parse_age(input: str) -> i32 ! InvalidAge | ParseIntError {
    if input.is_empty() {
        fail InvalidAge("Age cannot be empty")
    }
    input.to_int()?
}

fn load(path: str) -> Config ! IoError | ParseError {
    let text = read(path)?
    parse_config(text)?
}

fn lookup(m: {str: i32}, k: str) -> i32 ! NotFound {
    match m.get(k) {
        Some(v) => v,
        None => fail NotFound(k),
    }
}

fn main() {
    match parse_age("42") {
        Ok(n) => print(n),
        Err(e) => print("bad age"),
    }
}
```

## A.5 Negative examples

| Source | Diagnostic |
|---|---|
| `return fail X("a")` | redundant `return`; `fail` already diverges |
| `return !X("a")` | removed v0.1 syntax; replace with `fail X("a")` |
| `let n = s.to_int()!` | removed v0.1 syntax; replace with `?` |
| `fail 5` | operand of `fail` must be an error-type constructor |
| `fail X()` in a function with no `!` | function does not declare `X` |
| `x?` where `x` is infallible | `?` applied to a non-fallible expression |
| `let n = parse_age(s)` | fallible call must be propagated with `?` or handled with `match` |
| `const A: i32 = f()?` | `?` not allowed in const context |

## A.6 Migration

| v0.1 | v0.2 |
|---|---|
| `return !E(args)` | `fail E(args)` |
| `expr!` | `expr?` |
| `-> T ! A \| B` | unchanged |

The formatter/codemod can do both rewrites mechanically, because `!` was never boolean not in v0.1.

## A.7 Unresolved questions

1. **D1 (closed):** reject `return fail X()` as redundant.
2. **D2 (closed):** no dedicated `error` item; use ordinary nominal
   structs/enums and let the checker restrict failure constructibility.
3. **D3 (closed):** `Err(e)` binds the declared error side and `Err(_)`
   discards it. Member-specific patterns are allowed in the existing
   constructor form, e.g. `Err(InvalidAge(...))`. Struct-field patterns such as
   `Err(InvalidAge { message })` are deferred because the v0.1 pattern grammar
   has no struct patterns; adding them is a feature gap, not cleanup.
   Exhaustiveness covers every declared error member or a wildcard/binding. See
   A.8.
4. **D4 (closed):** error-set polymorphism is out of v0.2. Generic functions
   declare concrete error sets.
5. **D5 (closed):** `Task.join()` is unchanged in this RFC; task failure
   surfacing needs a separate decision.
6. **D6 (closed):** no `Result<T, E>` bridge ships in `0.2.0`. Reserved later
   spellings are `Result.from_effect(expr)` and `result.into_effect()?`.
7. **D7 (closed):** no implicit error-type conversion; only declared-set
   subset checking.

## A.8 Phase 0 decisions (2026-11-08)

These decisions close the blocking error-model questions for the v0.2 cleanup
freeze. They are language decisions; implementation lands with the error-model
phase, not in the `0.2.0` syntax-only cleanup.

### `fail Type { ... }`

The named-initializer form is **accepted**:

```ns
fail InvalidAge { message: "too young", minimum: 18 }
```

`fail Type(args)` remains the positional form. The AST already represents both
through `ExprKind::Fail` and `FailArguments::{Tuple, Struct}`. `return fail ...`
remains rejected as redundant. The grammar in A.2 has been updated.

### D3 - `Err` pattern shape

- A fallible expression's error side is the function's declared error set: a
  disjoint union of nominal error types.
- `Err(e)` binds the whole error side. If the error set has one member, `e` has
  that member's type. If it has several, `e` is an opaque union value; v0.2.0
  does not add union syntax to type annotations.
- `Err(_)` discards the whole error side.
- Member-specific patterns are allowed in the existing constructor/variant
  pattern forms, e.g. `Err(InvalidAge(...))` and
  `Err(ParseIntError(code))`. The pattern's head type must name a member of the
  scrutinee's declared error set; otherwise the checker reports `NS4017`. A
  variant pattern may select one of that member's variants.
- Struct-field patterns such as `Err(InvalidAge { message })` are deferred.
  They need a pattern AST addition, so they are a feature gap, not part of the
  AST-free cleanup.
- Exhaustiveness:
  - `Ok` and `Err` must both be covered; a missing side is `NS4010`.
  - If any `Err` arm uses a member-specific constructor pattern, every member
    of the error set must be covered by a member-specific arm or by a final
    `_`/binding arm; a missing member is `NS4010`.
  - A single `Err(_)` or `Err(e)` covers all error members.
  - Nested constructor payloads must be exhaustive for the member type
    (`NS4010`/`NS4017`).
- The checker must preserve the declared error set when checking `Err`
  patterns. The current `result_parts` fallback to `Type::Unknown` for
  multi-error effects is an implementation gap, not the language rule.
- No AST change is needed for constructor member patterns: the existing
  `PatternKind::Constructor` and `PatternKind::Variant` nodes represent them.
  Named-field patterns are the only deferred form.

### D6 - `Result<T, E>` bridge

No bridge is part of the `0.2.0` cleanup release. `Result<T, E>` remains a
library/FFI value type and is never an implicit source-level error path. If a
bridge is added later, the reserved spellings are:

- `Result.from_effect(expr)` to capture a fallible expression as a
  `Result<T, E>`;
- `result.into_effect()?` to enter the effect model from a `Result`.

The later RFC must define effect capture, conversion of multi-member error sets,
and the `Result` error type's relationship to declared error sets.

### D4, D5, D7

- **D4:** no error-set polymorphism in v0.2. A generic function declares a
  concrete error set; a caller may propagate only its subset.
- **D5:** `Task.join()` keeps its current `TaskError` design. Spawned-task
  failure surfacing is not redesigned in this RFC.
- **D7:** no implicit error-type conversion. `?` and `fail` use subset checking
  against the enclosing function's declared set.

### Grammar check

The Phase 0 conflict check lives in `tools/grammar-v0.2`. It uses canonical
LR(1) tables with Pager merging and covers `fail Type(...)` versus
`fail Type { ... }`, turbofish, struct initializers versus blocks, both scope
spellings, and optional trailing commas. Its four tests are green with zero
conflicts. The focused grammar omits operator-precedence layers and
control-flow expressions; those do not participate in the ambiguity set.
