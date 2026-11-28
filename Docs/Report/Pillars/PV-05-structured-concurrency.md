# PV-05 — Structured concurrency

**Status:** Partial
**Pillar:** Scoped tasks and channels; no detached tasks or shared mutable state in the portable core; threads and Web Workers.

## Original promise

> Structured concurrency uses scoped tasks and channels.
> — `Docs/archive/Design/V1/README.md` hard invariant 10

> The portable core does not allow shared mutable state between tasks.
> — `Docs/archive/Design/V1/01-language-design.md` §9.3

## What we can currently show

- Native: OS thread per spawn, bounded channels, cooperative cancellation, per-task heaps, deadlock watchdog.
- JS/TS: eager spawn, task scopes, channels, message passing.
- Web Worker mapping is not demonstrated; browser behavior is unverified.
- No proof shows identical results and cancellation semantics across native and browser targets.

## Proof program

`conformance/vision/pv05-tasks.ns`:

```ns
module app.main

pub fn main() {
    let channel = Channel.create<str>()
    spawn || {
        channel.send("hello")!
    }
    let message = channel.receive()!
    println(message)
}
```

`conformance/vision/pv05-detached.ns` attempts to keep a task past its scope and must be rejected.

## Gate

1. Native and JS/TS print the same `hello` with no ordering nondeterminism.
2. A browser run shows the task actually executing on a Web Worker, not cooperatively interleaved on the main thread.
3. The detached-task case is rejected at compile time with the documented diagnostic.
4. Cancellation at scope exit is deterministic on both targets.

## Work implied

1. Add a browser conformance runner that can observe worker creation.
2. Document the Web Worker mapping in the spec if it is currently implicit.
3. Record any capability that exists only on native as a preview, not portable.

## Task list

- [ ] Add the tasks/channels fixture and the detached-task rejection fixture.
- [ ] Compare native and JS/TS stdout for the passing fixture.
- [ ] Add a browser runner that observes Web Worker creation.
- [ ] Verify scope-exit cancellation is deterministic on both targets.
- [ ] Document the Web Worker mapping in the spec if it is currently implicit.
- [ ] Mark native-only concurrency capabilities as preview in public docs.
- [ ] Record evidence and change status to `Proven`.

## Release code

`PV-05`. Release target: TBD.
