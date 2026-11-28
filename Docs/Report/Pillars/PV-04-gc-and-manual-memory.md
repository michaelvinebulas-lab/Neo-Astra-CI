# PV-04 — GC + manual memory

**Status:** Partial
**Pillar:** Managed core by default; explicit manual memory in `system`; JS rejects manual memory.

## Original promise

> GC by default, manual memory by choice.
> — `Docs/archive/Design/V1/03-runtime-and-memory.md`

> Portable code must not be able to observe when memory is reclaimed.
> — `Docs/archive/architecture.md` §6.2

## What we can currently show

- C17 backend has a managed arena, precise root frames, mark-sweep GC with stress mode, boundary pinning, `Memory.alloc`/`free`, `Region`, and a provisional C ABI.
- JS/TS reject manual-memory scopes through `manualMemoryScopes` in `ModuleIr`.
- No proof shows identical observable behavior between GC and manual versions, and no cross-target memory gate exists.

## Proof program

`conformance/vision/pv04-managed.ns`:

```ns
module app.main

pub fn main() {
    let values = [1, 2, 3]
    println(values[1])
}
```

`conformance/vision/pv04-manual.ns` (illustrative; pin exact syntax against spec §8):

```ns
@requires(system)
module app.manual

pub fn main() {
    {@memory:manual;
        let buf = Memory.alloc<u8>(4)!
        buf[0] = 7
        println(buf[0])
        Memory.free(buf)
    }
}
```

`conformance/vision/pv04-js-reject.ns` is the same manual source built for `js`.

## Gate

1. Managed and manual programs print the same results on their allowed targets.
2. `js` and `ts` builds of the manual program fail with the documented diagnostic and no emitted artifacts.
3. The GC stress run produces identical output to the normal run.
4. A managed function cannot hold a raw pointer into a manual region without an explicit boundary (`NS6002` family).

## Work implied

1. Pin the illustrative syntax against `spec-v0.1.md` §8 before writing the fixture.
2. Add a memory behavior matrix to conformance: managed/manual × native/Wasm/JS × stress.
3. Verify that `ASTRA_GC_STRESS=1` is part of CI for this proof.

## Release code

`PV-04`. Release target: TBD.
