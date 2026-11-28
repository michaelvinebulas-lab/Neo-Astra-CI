# PV-01 evidence report

Generated: 2026-11-28T17:22:48.060Z
Source: `conformance/vision/pv01-contract.ns`; expectation: `conformance/vision/pv01-contract.expected`.
Manifests: `conformance/vision/pv01/{core,build}.json` (edition 0.1, `math.integers=wrapping`, `strings.model=utf8`).

## Frozen expectation

```text
overflow
-2147483648
len
10
parse
bad age
RETURN=0
```

## Captured stdout

### js — PASS

`astra build . --target js && node dist/js/runner.mjs`

```text
overflow
-2147483648
len
10
parse
bad age
RETURN=0
```

### ts — PASS

`astra build . --target ts && tsc --ignoreConfig --noCheck ... && node dist/ts/runner.mjs`

```text
overflow
-2147483648
len
10
parse
bad age
RETURN=0
```

### native — PASS

`astra build . --target native && ./dist/native/main`

```text
overflow
-2147483648
len
10
parse
bad age
RETURN=0
```

### nova — PASS

`nova run src/main.ns`

```text
overflow
-2147483648
len
10
parse
bad age
RETURN=0
```

## Playground worker parity

PASS

playground bridge + codegen-js output matches dist/js/src/main.js

## Divergence classification

No stdout divergences were observed.

## Verdict

PROVEN — all four targets and the playground bridge agree.
