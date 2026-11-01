# CONC-001 — OS-threaded structured concurrency (proposal)

**Status:** **Approved 2026-11-01** (see 10. Verdict). Decided, not yet implemented;
implementation is sequenced last in V1.
**Date:** 2026-10-31
**Depends on:** GC-001 (single-heap collector, Phase C on by default), spec §9
**Related:** `Docs/spec-v0.1.md` §9, `Docs/Design/V1/03-runtime-and-memory.md`,
`Docs/Design/V1/04-targets-and-backends.md`, `Docs/Agent/gc-001-workstream.md`

---

## 0. Why this is a proposal, not an implementation

V1 ships a single-threaded eager model: `spawn` evaluates its body inline,
`Task<T>` stores the result, channels are bounded FIFO queues with deadlock
panics on full/empty open channels, and the collector has one global root
array. Spec §9 already promises OS threads on native and Web Workers on web,
so a second slice is owed. The four decision areas below each change observable
semantics or the runtime contract, so they are written up as options with
recommendations rather than decided silently. Implementation follows approval.

## 1. Current model (V1 baseline)

- `spawn || { ... }` runs the block eagerly and stores its value in a `Task`.
- `Task<T>` exposes `.join()` (returns the stored value) and `.cancel()`.
- `Channel.create<T>()` creates a bounded (capacity 1, or as given) channel.
- `send`/`receive` block on a full/empty open channel; a blocked operation on
  a channel with no possible progress panics (deadlock) rather than spinning.
- `close` is explicit; sending to a closed channel fails.
- The GC root array (`astra_roots`) is global and single-threaded; `spawn`
  never moves work off-thread.
- Conformance case `channels` pins the single-threaded eager semantics.

## 2. Decision area A — Threading model

**Question:** what does `spawn` actually do on native?

- **A1 — Eager inline (status quo).** `spawn` evaluates inline; `Task` is a
  value container. No parallelism. Zero runtime change. Keeps JS/Nova/C
  identical. *Cons:* no real parallelism; spec §9.4 "OS thread pool" is
  aspirational.
- **A2 — OS thread per spawn.** Each `spawn` spawns an OS thread that runs
  the block; `join` joins it; the enclosing scope waits for children. True
  parallelism; matches §9.4 literally. *Cons:* thread creation cost, per-task
  stacks, and the collector must become thread-aware (area C).
- **A3 — Fixed thread pool.** A runtime-owned pool of N worker threads;
  `spawn` enqueues a task. Amortizes thread cost; bounds concurrency. *Cons:*
  pool sizing policy, queue backpressure, still needs thread-aware GC.

**Recommendation:** A2 for the native target (simplest correct model; pool
sizing is a perf tuning concern, not a semantic one). A1 remains the model for
JS/Wasm-fallback and Nova, which cannot spawn OS threads; the spec already
distinguishes these (§9.4). The backend reports which model it implements in
build metadata.

## 3. Decision area B — Blocking-wait semantics

**Question:** when a `send`/`receive` blocks, what does the calling thread do,
and what happens to the conformance corpus?

- **B1 — Busy-wait with deadlock panic (status quo).** A blocked operation
  spins briefly then panics if no progress is possible. Deterministic and
  testable, but burns CPU and cannot make progress with real threads.
- **B2 — Park the thread.** `send`/`receive` park the OS thread on a
  condition variable; `close` or a successful counterpart unparks it. No CPU
  spin; deadlock is still detected by a separate timeout or by the scope's
  join (which never returns). *Cons:* deadlock detection becomes "the program
  hangs" rather than "panics", which changes the `channels` conformance
  case.
- **B3 — Park with watchdog panic.** Park as B2, but a runtime watchdog
  panics if a scope's children are blocked past a threshold (default 5 s,
  configurable). Preserves a panic oracle for tests while avoiding spin.

**Recommendation:** B3. It keeps a deterministic failure signal (the
conformance suite can still assert a panic) while avoiding busy-wait, and the
threshold is a runtime constant, not a language rule. The `channels`
conformance case gains an `ASTRA_CHANNEL_DEADLOCK_TIMEOUT` env override for
tests. Conformance re-pinning: the case's expected output changes from an
immediate deadlock panic to a watchdog panic; the case is updated in the same
slice, not deleted.

## 4. Decision area C — Task-local heaps vs single-heap collector

**Question:** how does the GC coexist with multiple OS threads?

- **C1 — Stop-the-world global collector (GC-001 unchanged shape).** One
  global root array and one heap; collection stops all worker threads. Simplest
  reuse of GC-001. *Cons:* pause latency grows with thread count; worker
  threads must reach safepoints.
- **C2 — Per-task heaps.** Each task owns a heap; values never cross task
  boundaries except by message copy (§9.3). No cross-task tracing; collection
  is per-task and never stops other tasks. *Cons:* messages are copied
  (already required by §9.3); shared immutable values need a separate
  immortal/global heap.
- **C3 — Global heap, per-task root sets.** One heap, one root array per
  task; collection traces all task root sets. Intermediate: reuses GC-001's
  heap, adds root-set registry.

**Recommendation:** C2, because §9.3 already forbids shared mutable state and
requires message copy, so per-task heaps cost nothing in the language model and
buy independent collection. A small global/immortal heap holds interned
strings and type descriptors. This is the largest runtime change in the slice
and is the reason CONC-001 is sequenced last: it builds on GC-001 Phase C's
safepoint hook, which already exists.

## 5. Decision area D — Cancellation safepoints

**Question:** when does `task.cancel()` take effect?

- **D1 — Immediate (cooperative at next allocation).** Cancellation is
  checked at every GC safepoint and channel operation; a cancelled task's next
  safepoint throws/cancels. Matches "cooperative" in §9.1.
- **D2 — Checkpoint-only.** Cancellation is checked only at explicit
  `task.join()` and channel operations, not at allocations. Cheaper, but a
  tight CPU loop never cancels.
- **D3 — Language-level `cancel` points.** A `cancel` keyword/point the
  programmer inserts. Most explicit; changes the language surface.

**Recommendation:** D1. It reuses the GC-001 safepoint hook (`astra_gc_maybe_collect`
and its per-allocation threshold) and needs no new syntax. Tight loops that
never allocate or touch a channel will not cancel promptly; that limitation is
documented, not a language violation (§9.1 says "MAY be cancelled
cooperatively").

## 6. Conformance impact

- `channels` is re-pinned: same expected output except the deadlock case, which
  becomes a watchdog panic (area B3). A new env override gates the timeout in
  tests.
- A new case `task_parallel` (optional, gated behind a `conformance: threaded`
  directive if added) exercises two tasks producing distinct values; deferred
  until the threaded backend exists, recorded as a gap otherwise.
- Nova and JS keep the eager model; their gap ledgers record `task_parallel`
  as not applicable rather than failing.

## 7. Implementation slices (after approval)

1. Runtime: per-task heaps + global immortal heap; task spawn/join on OS
   threads (native only); channel parking with watchdog.
2. Collector: move GC-001's single root array into a per-task structure;
   immortal heap is a permanent root set.
3. Safepoints: extend `astra_gc_maybe_collect` to check the cancellation flag.
4. Spec §9.4 target mapping updated to state which targets implement threads.
5. Conformance: re-pin `channels`, add `task_parallel` behind a directive.
6. CI: threaded tests run on Tier-1 only (OS-threaded); JS/Nova/Wasm keep
   eager tests.

## 8. Open questions (for approval)

1. Default thread-pool size when A3 is preferred over A2.
2. Watchdog threshold default and whether it is user-configurable.
3. Whether `task.cancel()` on an already-completed task is a no-op or an
   error.
4. Whether per-task heaps require a copy at every `send` or only for managed
   values (scalars can move by value).
5. Whether the Web target gains real Web Workers in this slice or stays on the
   eager model.

## 9. Decision summary

If approved, CONC-001 adds (as a proposal, not code): per-task OS threads on
native, parked channel operations with a watchdog panic, per-task heaps with a
global immortal heap, and cancellation at GC safepoints. JS, Nova, and the
Wasm fallback retain the eager single-threaded model. No language syntax
changes.

---

## 10. Verdict (2026-11-01, principal)

All four areas decided, plus the five operational specifications. The rationale
below is the principal's, recorded verbatim in substance.

**A = A2 - OS thread per spawn, with A1 as the Web/Nova fallback.** A2 is the
strategic sweet spot for v0.1: true hardware parallelism without a scheduling
queue in the compiler. Thread lifetimes are bounded strictly to structured
scopes, which enforces spatial determinism. A3 stays a pure performance
optimization that can be layered on later. The multi-compiler manifest carries
this per target: a native machine-code target spawns a dedicated OS thread; the
JS/TS and Nova targets slide back to the eager inline model (A1), which is the
ecosystem separation already designed in D13.

**B = B3 - park with watchdog panic.** Losing the automated deadlock panic
contract (B2) would cripple diagnostic reliability and replace it with flaky,
uninformative CI timeouts. B3 keeps the language's safety profile: threads park
on an OS condition variable (no CPU burn) and a lean runtime watchdog monitors
task scopes; a block past the threshold raises a structured panic, so the
existing `channels` conformance case is re-pinned without changing the semantic
shape of the diagnostics.

**C = C2 - per-task heaps plus a small global/immortal heap.** Since D9 and
spec 9.3 mandate copy-on-send isolation and outlaw shared mutable state, C2 is
the only logical architecture: a global stop-the-world collector (C1 or C3)
would impose thread-lock latency that defeats the point of native concurrency.
Each task gets an independent lightweight heap, so collection scales linearly
with core count and never pauses other threads. Strings and shared static
descriptors live in a thread-safe permanent global heap.

**D = D1 - cooperative cancellation at GC safepoints.** The cancellation token
is checked inside `astra_gc_maybe_collect` (already executed at allocation
boundaries) and during blocking channel transactions, giving safe cancellation
for free. It sidesteps fragile thread-interruption signals and adds no language
keyword (D3), keeping the grammar intact.

### 10.1 Operational specifications

1. **Thread pool size:** not applicable in v0.1. A2 is structured
   OS-thread-per-spawn; thread management maps to OS scaling limits.
2. **Watchdog threshold:** 5 s default, treated as a runtime implementation
   constant, not a visible spec feature; immediately overridable via
   `ASTRA_CHANNEL_DEADLOCK_TIMEOUT` so the conformance suite stays fast.
3. **`cancel()` on a completed or dead task:** idempotent no-op - returns
   immediately, no error flag.
4. **Copying on `send`:** managed values only. Primitives and scalars move by
   value; managed heap objects are deep-copied into the destination task's
   private heap, honouring C2's isolation boundary.
5. **Web Workers:** deferred in v0.1. Web/JS targets run the eager A1 profile;
   parallel tasks for web are recorded as a gap alongside the deferred Wasm
   slice (GC-001 S6 / Phase D), not as a conformance failure.

### 10.2 Status

Decided. Spec 9 is updated with the decided target mapping, watchdog, and
copy-on-send rules, each carrying an implementation-pending note. Implementation
stays the **last** V1 work item, after NOPM-005, NOPM-006, and TOOLCHAIN-001a/b,
and lands in the order given by 7: runtime, collector, safepoints, spec, then
conformance and CI.