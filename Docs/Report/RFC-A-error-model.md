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
fail_expr     = "fail" type_path "(" [ arg_list ] ")" ;

(* handling *)
match_arm     = pattern "=>" ( expr | block ) ;
pattern       = ... | "Ok" "(" pattern ")" | "Err" "(" pattern ")" ;
```

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
2. **D2 (recommended):** no dedicated `error` item; use ordinary nominal\n   structs/enums and let the checker restrict failure constructibility.
3. **Pattern shape for `Err(e)`:** is `e` the union type, and can patterns destructure (`Err(InvalidAge(msg))`)?
4. **Error-set polymorphism:** can a generic function propagate whatever `f` throws?
5. **Tasks:** how does a failure inside `spawn { ... }` surface?
6. **`Result<T,E>` bridging:** names for conversions to and from the effect model.
7. **Does `?` convert between error types?**

## Lead review notes (2026-11-08)

- **D1 is closed:** `return fail X(...)` is rejected. The principal already chose
  this.
- **D2 recommendation: no new `error` item.** Use ordinary nominal structs and
  enums; the checker restricts which nominal types are constructible as failures
  and rejects ambiguous multi-variant enum construction. This avoids adding a
  second type-declaration form.
- **D3 must be settled before freeze.** Define whether `Err(e)` binds the whole
  error union or a destructured variant, and how exhaustiveness works.
- **D4:** keep error-set polymorphism out of v0.2 and say so.
- **D5:** do not change `Task.join()` to `T ! E` in this RFC; the existing
  `TaskError` design is separate and needs its own decision.
- **D6:** bridge names are open; recommend `Result.from_effect(...)` and
  `result.into_effect()?`.
- **D7:** no implicit error-type conversion; subset checking only.
- The grammar must also allow the named-initializer form
  `fail TypeName { field: value }` if existing nominal error types can be
  constructed that way. Add it or explicitly reject it.
