# PV-03 — Semantic policies

**Status:** Unproven
**Pillar:** `math.integers` and `strings.model` are explicit manifest policies, not modes.

## Original promise

> Modes do not rewrite core syntax or core semantics. Semantic policies can change observable behavior, but only when explicitly set in the manifest and recorded in build metadata.
> — `Docs/archive/architecture.md` §3.4

## What we can currently show

- Both policies are specified in `Docs/spec-v0.1.md` and accepted by manifest validation.
- `integerPolicy` is plumbed through the compiler API and C backend.
- `strings.model` is not exposed in the compiler API request types; backend coverage is unaudited.
- No proof shows that changing a policy leaves the resolved mode and grammar untouched.

## Proof program

One source file, three build manifests.

`conformance/vision/pv03-policy.ns`:

```ns
module app.main

pub fn main() {
    let x: i32 = 2147483647
    println(x + 1)
    println("héllo😀".len())
}
```

Manifests:

| Fixture | `math.integers` | `strings.model` | Expected |
|---|---|---|---|
| A | `wrapping` | `utf8` | `-2147483648`, `10` |
| B | `safe-checked` | `utf8` | trap on overflow, policy recorded in build metadata |
| C | `wrapping` | `native` | `10` byte length still observable on every target |

## Gate

1. `astra explain` reports the same mode for A, B, and C — policies did not leak into capability selection.
2. Build metadata records the selected policy in all three runs.
3. Targets that cannot honor a policy reject explicitly instead of silently degrading.
4. `strings.model` is added to `CompileRequest` / compiler-client types if it is missing.

## Work implied

1. Audit every backend for both policies and record a coverage table.
2. Add the missing API field and pin it with a compiler test.
3. Add the three fixtures to conformance with expected stdout/metadata snapshots.

## Task list

- [ ] Audit every backend for `math.integers` and record a coverage table.
- [ ] Audit every backend for `strings.model` and record a coverage table.
- [ ] Add `stringsModel` to `CompileRequest`, `ProjectCheckRequest`, and `compiler-client` types if missing.
- [ ] Add fixtures A (wrapping/utf8), B (safe-checked/utf8), and C (wrapping/native).
- [ ] Verify `astra explain` reports the same mode for all three manifests.
- [ ] Verify build metadata records the selected policy in all three runs.
- [ ] Verify unsupported policy/target combinations reject explicitly.
- [ ] Record evidence and change status to `Proven`.

## Release code

`PV-03`. Release target: TBD.
