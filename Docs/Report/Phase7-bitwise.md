# Phase 7 slice 3: bitwise operators

Status: implemented for edition `0.2`. Edition `0.1` is unchanged.

## Syntax

```astra
let mask = 0b1100 & 0b1010   // 0b1000
let bits = 0b1000 | 0b0010   // 0b1010
let flip = 0b1010 ^ 0b1111   // 0b0101
let shifted = 1 << 4         // 16
let back = 256 >> 4          // 16
```

Operators: `&` (and), `|` (or), `^` (xor), `<<` (shift left), `>>` (shift
right). They are integer-only and return the left operand's type.

## Precedence (tightest last)

| Level | Operators |
|---|---|
| logical or | `||` |
| logical and | `&&` |
| equality / comparison | `== != < <= > >=` |
| bitwise or | `\|` |
| bitwise xor | `^` |
| bitwise and | `&` |
| shifts | `<< >>` |
| additive | `+ - +% -%` |
| multiplicative | `* / % *% /%` |

`|` in an error set (`-> T ! ErrorA | ErrorB`) is parsed by the signature
parser before expression parsing and is never a bitwise operator. `||` remains
logical or in every context.

## Representation and edition gate

`BinaryOp` gains `BitAnd`, `BitOr`, `BitXor`, `Shl`, and `Shr`. No new AST node
is needed. `&` and `^` are lexed as new punctuation only under edition `0.2`;
edition `0.1` keeps its existing "unexpected character" diagnostic. `<<` and
`>>` are recognized by the parser as two adjacent `<` / `>` tokens so nested
generic closers (`Option<Result<i32, str>>`) keep lexing and parsing correctly.

## Semantics

- Both operands must be integers of the same type; mixed widths are rejected
  with `NS4006`. Floats and `bool` are not valid bitwise operands.
- Results are truncated to the left operand's width (two's complement for
  signed types), matching the existing wrapping-integer policy.
- Shift counts must be non-negative integers. Shifting by at least the type's
  bit width yields zero; negative counts panic at runtime.
- `!` remains logical/not and bitwise complement is not added in this slice.

## Backends

- C emits `&`, `|`, `^`, `<<`, and `>>` directly on the primitive integer type.
- JS/TS emit the same operators; wrapping coercion (`$i32`, etc.) already
  truncates results, and arbitrary-precision mode uses `BigInt.asIntN` /
  `asUintN`.
- Nova computes on `i128` values and normalizes to the declared integer type.

## Tests

Parser unit tests cover precedence, the error-set `|` distinction, and the
edition `0.1` rejection. Checker tests cover integer-only operand rules.
C/JS/TS/Nova tests execute representative expressions, including shifts and
mixed logical/bitwise precedence.

## Deferred

- Bitwise complement (`!` as `~`), `&=`/`|=`/similar compound assignments, and
  shifts by unsigned-only counts.
