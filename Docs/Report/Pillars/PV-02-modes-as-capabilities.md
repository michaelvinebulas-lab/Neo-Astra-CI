# PV-02 — Modes as capabilities

**Status:** Partial
**Pillar:** One grammar, compile-time capability layers, least-privileged inference, explainable selection.

## Original promise

> Modes add or remove capabilities. They do not change core syntax or core semantics.
> — `Docs/archive/Design/V1/01-language-design.md` §10.4

> Fluid at development time, deterministic at build time.
> — `Docs/archive/architecture.md` §4.5

## What we can currently show

- `core`, `app`, `web`, `system` are implemented with least-privileged inference and `astra explain`.
- Native-order side-effect rejection (`NS5007`) is verified.
- Public matrix: `core` stable, `app` partial, `web`/`system` preview, `high` planned.
- No single proof shows the same library consumed under two modes with a visible reason tree and a rejected widening.

## Proof program

`conformance/vision/pv02-core-lib.ns`:

```ns
module lib.util

pub fn classify(n: i32) -> str {
    if n > 0 { return "positive" }
    return "other"
}
```

`conformance/vision/pv02-web-app.ns`:

```ns
@tags(fetch)
module app.web
import { classify } from "lib.util"

pub fn main() {
    println(classify(3))
}
```

`conformance/vision/pv02-system-app.ns`:

```ns
@tags(fs)
module app.system
import { classify } from "lib.util"

pub fn main() {
    println(classify(-1))
}
```

A third file attempts a silent widening: a `@requires(core)` module that imports a `system` function without an explicit boundary.

## Gate

1. `astra check --project .` resolves `web` and `system` modes from the same `lib.util` without changing its source.
2. `astra explain` prints a reason tree naming the exact source tags that forced each mode.
3. The widening case fails with the documented mode diagnostic (`NS5001`/`NS5006` family), not a silent build.
4. The generated JS for `pv02-web-app.ns` contains no system capability imports.

## Work implied

1. Build the three-file fixture and an expected-explanation snapshot.
2. Verify that `app` and `web`/`system` claims in public docs match the checker, or narrow the claims.
3. Record any capability that is accepted but not enforced as a gap under this code.

## Task list

- [ ] Add the core library, web consumer, system consumer, and widening fixture.
- [ ] Run `check_project` and confirm the same library resolves under both modes.
- [ ] Snapshot `astra explain` reason trees for web and system.
- [ ] Verify the widening case fails with the documented `NS5001`/`NS5006` diagnostic.
- [ ] Verify generated JS for the web consumer contains no system capability imports.
- [ ] Reconcile the public mode matrix with what the checker actually enforces.
- [ ] Record evidence and change status to `Proven`.

## Release code

`PV-02`. Release target: TBD.
