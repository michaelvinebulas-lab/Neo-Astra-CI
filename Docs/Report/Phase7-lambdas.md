# Phase 7 slice 5: lambdas

Status: implemented for edition `0.2`. Edition `0.1` is unchanged.

## Syntax

```astra
let doubled = (fn(x) => x * 2)(21)          // 42
let typed = (fn(x: i32, y: i32) => x + y)(1, 2)
let block = (fn(value) => { println(value); value })(9)
```

Lambda parameters may carry an optional `: Type` annotation. Unannotated
parameters infer their type from the corresponding argument expression using
the existing `let` inference rules. `=>` introduces either an expression or a
block body.

## First-slice boundary

A lambda must be the callee of an immediate call: `(fn(x) => body)(arg)`.
A bare lambda value (`let f = fn(x) => ...`) is rejected with a parser error.
This keeps the first slice free of first-class function values, which the C
backend does not support; higher-order functions and stored closures are
deferred.

## Desugaring (implementation decision)

No AST node or backend change is needed. The parser rewrites an immediate
lambda call into a block:

```
(fn(x, y) => body)(a, b)
```

becomes

```
{
    let x = a
    let y = b
    body
}
```

Annotated parameters keep their `: Type` on the generated `let`. The rewrite
happens before checking, so the checker, formatter, free-name analysis,
migration, capabilities, LSP, interpreter, and C/JS/TS backends all reuse the
existing block/let machinery.

## Capture and mutation rules

The generated block introduces a new scope nested inside the surrounding
scope. Captured locals are ordinary lexical references read from that scope;
the lambda cannot outlive the call, so no move/borrow transfer is created.
Mutating a captured local follows the existing rule: the outer binding must be
`mut`. A lambda parameter is immutable unless its enclosing function binding is
already mutable; `fn(mut x) => ...` is not part of this slice.

Consequence: the formatter canonicalizes lambda calls to block form. This is
intentional and mirrors index-range slicing canonicalizing to `.slice(...)`.

## Edition gate and release boundary

Part of the `1.1.2` additive Phase 7 boundary. Edition `0.1` keeps treating
`fn` in expression position as a parse error.

## Tests

Parser tests cover inference, annotations, block bodies, arity errors, and the
immediate-call requirement. Checker/capability tests confirm captured reads and
mutation work. Formatter tests confirm idempotent canonical block output. C,
JS, TS, and Nova parity programs execute representative lambdas. A conformance
case joins the edition-`0.2` corpus.

## Deferred

- First-class/stored closures and higher-order functions.
- `fn(mut x) => ...` and explicit move/capture lists.
- Recursive lambdas and generic lambda parameters.
