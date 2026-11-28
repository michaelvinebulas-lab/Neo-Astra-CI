# PV-01 — One semantic contract

**Status:** Partial
**Pillar:** Same observable behavior on JS/TS, native, Wasm, and Nova.

## Original promise

> The hardest problem is not parsing source code or generating JavaScript. The hardest problem is defining **one semantic contract** that native, Wasm, and JavaScript can all honor.
> — `Docs/archive/architecture.md`

> A feature belongs in the portable core only if all targets can implement the same observable behavior.
> — `Docs/archive/architecture.md` §3.3

## What we can currently show

Conformance runners exist for JS, TypeScript, C, and Nova, and the C runner executes every non-gap case. There is no single end-to-end program that pins the dangerous edges together and diffs all targets.

Known risk areas from the founding matrix:

| Edge | Divergence if unspecified |
|---|---|
| Integer overflow | JS `Number` vs native wrapping vs C undefined behavior |
| UTF-8 `str.len()` | UTF-16 code units vs bytes vs scalar count |
| Error propagation | generated `try`/`catch` vs value+tag |
| Map iteration | insertion order vs hash order |
| Division by zero | panic vs JS `Infinity` vs C UB |

## Proof program

`conformance/vision/pv01-contract.ns` (to be created when this proof is scheduled):

```ns
module app.main

fn show(label: str, value: i32) {
    println(label)
    println(value)
}

pub fn main() {
    // Integer policy: wrapping (manifest default).
    let x: i32 = 2147483647
    show("overflow", x + 1)

    // UTF-8 byte length, not UTF-16 code units.
    let s = "héllo😀"
    println("len")
    println(s.len())          // expected: 10

    // Nominal error effect, propagated explicitly.
    let parsed = parse_age("nope")!
    println(parsed)
}
```

`core.json` sets `math.integers: "wrapping"` and `strings.model: "utf8"` in the build manifest.

## Gate

All four commands produce byte-identical stdout and identical diagnostic codes/spans:

```sh
astra build --target js --project . && node dist/js/main.js
astra build --target ts --project . && node dist/ts/main.js
astra build --target native --project . && ./dist/native/main
nova run src/main.ns
```

A fifth check runs the same source through the browser playground worker and diffs its generated JS against `dist/js/main.js`.

## Work implied

1. Add the proof program to `conformance/vision/` with a driver that captures stdout per target.
2. Where outputs differ, classify: backend bug, missing policy plumbing, or spec ambiguity.
3. Do not “fix” drift by weakening the assertion; fix the backend or record a spec change explicitly.

## Release code

`PV-01`. Release target: TBD.
