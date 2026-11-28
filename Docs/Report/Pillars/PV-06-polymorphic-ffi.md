# PV-06 — Polymorphic FFI

**Status:** Partial
**Pillar:** One `foreign module` declaration, target-specific manifest bindings, C ABI base layer, explicit error mapping.

## Original promise

> FFI is polymorphic and manifest-resolved.
> — `Docs/archive/Design/V1/README.md` hard invariant 11

> This is the polymorphic FFI model: one source contract, target-native bindings.
> — `Docs/archive/architecture.md` §10.1

## What we can currently show

- Native C ABI with symbol naming, layouts, library artifacts, and ownership rules (provisional).
- Inline `@script:js`, `@script:ts`, `@script:c`, `@script:cpp` with typed export surfaces and boundary validation.
- Manifest `ffi` mapping exists in the design, but target coverage is partial.
- No proof calls the same foreign module from two targets with one source declaration.

## Proof program

`conformance/vision/pv06-ffi.ns`:

```ns
foreign module math_lib {
    fn add(a: i32, b: i32) -> i32
}

module app.main

pub fn main() {
    println(math_lib.add(2, 3))
}
```

Manifest mapping:

```json
{
  "ffi": {
    "math_lib": {
      "native": { "source": "./libmath.a", "type": "c-abi" },
      "typescript": { "source": "./math-bridge.js", "type": "esm" }
    }
  }
}
```

## Gate

1. `astra build --target native` prints `5` using the C ABI binding.
2. `astra build --target ts` prints `5` using the ESM binding, with no source change.
3. A target with no mapping fails with `NS7001`-range diagnostics, never a silent fallback.
4. Error propagation across the boundary is explicit and tested with a fallible foreign function.

## Work implied

1. Verify manifest FFI mapping is actually wired into both backends; if not, implement it.
2. Pin ownership and string encoding at the boundary with a second fixture.
3. Keep `@script` as a separate, narrower proof — it is an escape hatch, not the FFI contract.

## Task list

- [ ] Verify the manifest `ffi` mapping is wired into the native backend.
- [ ] Verify the manifest `ffi` mapping is wired into the TS backend.
- [ ] Add the foreign-module fixture and native C ABI binding.
- [ ] Add the TS ESM binding and compare stdout with native.
- [ ] Add a missing-mapping rejection test for `NS7001`-range diagnostics.
- [ ] Add a fallible foreign function to pin error mapping at the boundary.
- [ ] Keep `@script` interop as a separate, narrower proof.
- [ ] Record evidence and change status to `Proven`.

## Release code

`PV-06`. Release target: TBD.
