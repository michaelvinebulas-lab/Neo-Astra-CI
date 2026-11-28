# PV-01 — One semantic contract

**Status:** Proven (2026-11-28)
**Pillar:** Same observable behavior on JS/TS, native, Wasm, and Nova.

## Original promise

> The hardest problem is not parsing source code or generating JavaScript. The hardest problem is defining **one semantic contract** that native, Wasm, and JavaScript can all honor.
> — `Docs/archive/architecture.md`

> A feature belongs in the portable core only if all targets can implement the same observable behavior.
> — `Docs/archive/architecture.md` §3.3

## What we can currently show

Conformance runners exist for JS, TypeScript, C, and Nova, and the C runner executes every non-gap case. A single end-to-end proof program now pins the dangerous edges together and diffs all targets (see `conformance/vision/` and the evidence below). Direct Wasm execution is still out of scope; the Wasm path is represented by the playground compiler bridge plus the shared JS backend.

Known risk areas from the founding matrix:

| Edge | Divergence if unspecified |
|---|---|
| Integer overflow | JS `Number` vs native wrapping vs C undefined behavior |
| UTF-8 `str.len()` | UTF-16 code units vs bytes vs scalar count |
| Error propagation | generated `try`/`catch` vs value+tag |
| Map iteration | insertion order vs hash order |
| Division by zero | panic vs JS `Infinity` vs C UB |

## Proof program

`conformance/vision/pv01-contract.ns` is the proof program. It keeps the three
founding edges together and adds an explicit `parse_age` definition so the error
effect is portable:

```ns
fn show(label: str, value: i32) {
    println(label)
    println(value)
}

fn parse_age(input: str) -> i32 ! ParseIntError {
    return input.to_int()!
}

pub fn main() -> i32 {
    let x: i32 = 2147483647
    show("overflow", x + 1)      // wrapping: -2147483648

    let s = "héllo😀"
    println("len")
    println(s.len())             // UTF-8 bytes: 10

    println("parse")
    match parse_age("nope") {
        Ok(age) => show("age", age)
        Err(error) => println("bad age")
    }
    return 0
}
```

`conformance/vision/pv01-contract.expected` freezes the stdout. The build
manifests live in `conformance/vision/pv01/`: `core.json` declares the package,
and `build.json` declares the js/ts/native targets, `nova.entry`,
`math.integers: "wrapping"`, and `strings.model: "utf8"`. The source uses
edition `0.1` deliberately so postfix `!` is valid.

## Gate

`conformance/vision/run-pv01.mjs` copies the canonical source and the frozen
manifests into a fresh temporary project, then runs:

```sh
astra build . --target js && node dist/js/runner.mjs
astra build . --target ts && tsc --ignoreConfig --noCheck ... && node dist/ts/runner.mjs
astra build . --target native && ./dist/native/main
nova run src/main.ns
```

The ESM artifacts export `main`, so the JS/TS runners import the module and call
it; stdout follows the conformance `RETURN=<code>` convention. All four outputs
must match `pv01-contract.expected` exactly after CRLF/trailing-whitespace
normalization. A fifth check compiles the same source through the playground
Wasm bridge (`orchestrator/packages/playground/src/bridge.ts`), generates JS
with the shared `@neo-astra/codegen-js` backend, and diffs that against
`dist/js/src/main.js` (ignoring the `sourceMappingURL` line). It exercises the
code path the browser worker will use; the worker UI itself is a PLAY-001 item.

Run it with:

```sh
node conformance/vision/run-pv01.mjs
```

It writes `conformance/vision/pv01-report.md` and exits non-zero on divergence.

## Evidence (2026-11-28)

All four targets produced byte-identical stdout:

```text
overflow
-2147483648
len
10
parse
bad age
RETURN=0
```

JS, TS, native, and Nova all pass; the playground bridge + shared JS backend
parity check passes. Full captured output and exact commands are in
`conformance/vision/pv01-report.md`. No stdout divergences were found, so there
is nothing to classify as a backend bug, missing policy plumbing, or spec
ambiguity.

## Work implied

1. Add the proof program to `conformance/vision/` with a driver that captures stdout per target.
2. Where outputs differ, classify: backend bug, missing policy plumbing, or spec ambiguity.
3. Do not “fix” drift by weakening the assertion; fix the backend or record a spec change explicitly.

## Task list

- [x] Add `conformance/vision/pv01-contract.ns` with integer overflow, UTF-8 length, and error-effect cases.
- [x] Add the wrapping/utf8 build manifests (`conformance/vision/pv01/`) and freeze their expected values.
- [x] Capture stdout from JS, TS, native, and Nova into one diff report (`conformance/vision/pv01-report.md`).
- [x] Classify every divergence as backend bug, missing policy plumbing, or spec ambiguity — none occurred.
- [x] Add a playground-worker parity check against the native CLI output (via the Wasm bridge and shared `codegen-js`; the browser worker UI itself remains a PLAY-001 gap recorded in `Docs/todo.md`).
- [x] Record evidence in this file and change status to `Proven`.
- [x] Move any uncovered gaps into `Docs/todo.md` under `PV-01`.

## Release code

`PV-01`. Release target: TBD.
