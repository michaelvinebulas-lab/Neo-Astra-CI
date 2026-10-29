# GC-001 — precise roots and non-moving mark-sweep (approved design)

> **Status: APPROVED with amendments, 2026-10-29.** The eleven choices
> `[P1]`-`[P11]` that this file originally carried for approval have been
> decided (see §10 for the verdicts and reasons); the approved amendments are
> applied inline in the sections they change and marked "Amendment". Two items
> are not fully closed: `[P9]`'s exact signature is deferred to
> `11-c-abi.md` (concept approved), and `[P11]` is split out for its own spec
> review (until then, left-to-right managed-argument evaluation is an
> implementation detail, not normative). The two conditions the approval was
> gated on (task execution model; `Ptr<T>` with managed element types) are
> answered in §11 with evidence.
>
> This file still changes no spec text by itself: the `[P5]` VARIANT shape is
> internal runtime machinery, but the `[P11]` split-out and the `NS6002`
> managed-element-type rule proposed in §11 are spec changes that follow their
> own approval path.
>
> Written 2026-10-29 under the `GC-001` backlog item in `Docs/Agent/todo.md`.

---

## 1. What is already decided (citations, not proposals)

| Source | Text |
|---|---|
| D1 (`Docs/Agent/project-context-glossary.md:325`) | GC by default; memory policy configurable by manifest/file/block |
| `Docs/spec-v0.1.md` §8.1 (1243) | Default policy is GC-managed; no user-visible finalizers; no manual `free`; bounds-checked collections; no raw pointers in `core` |
| `Docs/spec-v0.1.md` §8.4 (1318) | "`Ptr<T>` is not a GC reference" |
| `03-runtime-and-memory.md` §4.1 (107-112) | GC by default; **non-moving mark-sweep**; **precise roots from compiler-generated stack maps**; no finalizers; no weak references |
| `03` §4.2 (116-124) | Task-local heaps on native/Wasm; each task owns its heap; no cross-task heap references; avoids stop-the-world pauses |
| `03` §4.3 (126-133) | Collection triggered by allocation pressure; each task collects its own heap; a task may pause during its own collection; **stack maps identify roots precisely**; FFI frames require explicit root registration |
| `04-targets-and-backends.md` §6.2 (201) | C output rule: "Emit explicit GC roots or stack maps" |
| `03` §12 (382) | GC runtime budget: under 32 KB native |
| `03` §8 (286) / `08-nova-and-nopm.md:141` | JS/TS use the host GC; Nova uses interpreter host memory — **neither needs this work** |
| `01-language-design.md:698` | Runtime reflection is a V1 non-goal |

What these decide is the *what*. They do not decide *how* a C backend exposes
roots, how objects are traced, or what happens at the C boundary. Those are the
subject of this proposal.

---

## 2. Where the implementation is today

All line numbers are current as of `41690c8`.

- **Managed memory is an arena, not a collector.**
  `crates/astra-codegen-c/src/runtime.rs:234-318` records every `astra_alloc`
  block in a linked list; `astra_free` unlinks one; `astra_runtime_shutdown`
  (called from `emit_main`, `emit.rs:1195`) frees everything at exit. The
  comment at `runtime.rs:238` already says a future precise collector is meant
  to reuse this bookkeeping.
- **Manual and managed share one allocator.** `Memory.alloc` lowers to the same
  `astra_alloc` (`emit.rs:2143`) that backs arrays and strings, and
  `Memory.free` unlinks from that same list (`runtime.rs:259-277`).
- **Values are by-value fat structs; only the buffers are heap objects.**
  `NS_Array {void* data; size_t len; size_t cap;}`, `NS_Map`, `NS_Channel`,
  `NS_Str {const char* data; size_t len;}`, `NS_Bytes` (`runtime.rs:23-39`).
  The separately allocated `data`/`keys`/`values` buffers
  (`runtime.rs:321-337`, `582-583`) are the blocks the collector would trace.
- **Slices are interior views.** `out->data = s.data + start` for `str`
  (`runtime.rs:374`) and for `bytes` (`431`, `460`, `504`). A root may therefore
  point *into* a block, not at its start.
- **Most allocation happens inside runtime helpers**, not in emitted code:
  `astra_array_new*`, array growth, `astra_str_bytes`, `astra_bytes_new`,
  `astra_map_new`, `astra_channel_new`.
- **Nothing ever collects mid-run.** `astra_arena_live_count()` only falls via
  explicit `Memory.free` (`tests/emit_c.rs:955-977`).
- **Frames are simple.** `emit_let` writes `    {ty} v_{name} = {expr};`
  (`emit.rs:1404`); temporaries are `fresh_id` C locals; there is no unwinding
  (`11-c-abi.md` §6: no C++-style unwinding, `astra_panic` terminates), so every
  return path is explicitly emitted (`emit_return`, `emit.rs:1407`) and there
  are no landing pads to keep consistent.
- **`args_c` emits arguments inline** (`emit.rs:2185-2190`), so already-computed
  subvalues live only in compiler temporaries, and C argument evaluation order
  is unspecified. This matters in §5.3.

---

## 3. Requirements the mechanism must satisfy

- **R1 — Precision.** No conservative scanning: it contradicts §4.1/§4.3.
- **R2 — Root completeness at collection time.** At the instant a collection
  runs, every live managed value in every frame of the current task must be
  reachable from a published root set. The values are C locals whose addresses
  and liveness the C compiler owns, so they must be published *by the emitter*
  before collection is possible.
- **R3 — Interior roots.** `str`/`bytes` slices must be traceable without
  changing the documented `NS_Str`/`NS_Bytes` ABI (`11-c-abi.md` §2) and without
  making slicing copy.
- **R4 — No unwinding.** Root release must be emitted on each explicit return.
- **R5 — Manual is not managed.** `Ptr<T>` must never keep a GC object alive
  (spec §8.4: "`Ptr<T>` is not a GC reference").
- **R6 — Boundary contract stays true.** `11-c-abi.md` §5 promises values
  returned to C consumers are valid until process exit.

---

## 4. The central difficulty

`astra_alloc` is called from deep inside runtime helpers. If a collection can
run there, then *at that moment* the emitter's frame state must already be
correct — including values the caller computed but has not yet stored, because C
may hold them in unnamed temporaries during argument evaluation. Getting that
wrong is not a leak; it is a use-after-free.

Everything below follows from one invariant that makes that safe:

> **I1. Whenever collection can run, every live managed value sits in a named C
> local whose slot in the published root table has been updated.**

---

## 5. The mechanism (as approved)

### 5.1 [P1] Eager value snapshots, one contiguous region per frame — approved with amendment

Per execution context, a growable array of `void*` slots:

```c
void**  astra_roots;        /* slot values, not addresses */
size_t  astra_root_count;   /* high-water mark of the live region */
```

The landed runtime is single-threaded and eager: `spawn` evaluates its block
inline in the caller's C frame and stores the result in a `Task` value
(`emit.rs` `ExprKind::Spawn`; no threads, `ucontext`, or task queue exist
anywhere in the runtime), so there is exactly one execution context and
exactly one root array today. The array is nonetheless scoped *per execution
context* so that `CONC-001` OS-threaded tasks become one array per task — an
extension, not a redesign (see §11, condition 1).

- Function entry: `size_t _rf = astra_gc_frame_enter(N);` reserves `N` slots
  and zeroes them (so a slot for a local not yet assigned reads as `NULL`).
  `N` comes from a cheap pre-pass over the function body that assigns each
  managed local and temporary a fixed slot index.
- **Snapshot on assignment**: immediately after every C store of a value that
  holds managed data (`v_x = ...`, parameter binding, emitter temporary),
  emit `astra_roots[_rf + k] = <root expr>;`.
- Return: `astra_root_count = _rf;` before every `return`, emitted by
  `emit_return` on all paths (ordinary, fallible, unit).

**Amendment (2026-10-29): values survive realloc of the root array; addresses
don't — so the snapshot set is not "assignment only".** A managed local's
payload pointer can also change under operations that are not spelled as
assignments, and each such operation must re-snapshot every local whose
payload pointer it can move:

- array/map **push growth** (`astra_array_push` may replace `arr.data` — every
  live local aliasing that array must be re-snapshotted after the push);
- **`&mut` calls** that may realloc through a mutable reference;
- **field stores** into a managed struct or collection element.

Where the emitter cannot cheaply prove which locals alias the grown buffer,
re-snapshotting every managed local of the current frame after the operation
is the correct over-approximation; the store is O(live managed locals), once
per growth, not per element.

Root sources, stated completely for V1: the root set at any collection is
(a) the current task's frame slots (this section), (b) the permanent boundary
pin set (§5.8), and (c) explicit FFI registrations (§5.9). Three candidates
that do not exist in the language and therefore cannot be root sources
(probed 2026-10-29, reject codes recorded in §11): **module-level statics**
(the module level accepts declarations only — `let`/`static` at top level is
`NS2005`), **closure environments** (closure literals exist only as `spawn`
operands — `let f = || ...` is `NS2002` — and spawn evaluates its block
eagerly inline, so no closure value with a captured environment ever exists),
and **queued spawn arguments** (there is no queue; a `Task<T>` stores an
already-computed value, which is an ordinary managed local of the receiving
frame and is snapshotted like any other).

Why snapshots rather than the more usual "array of addresses of locals":

- No local needs to be hoisted to the top of the function, and no local needs
  zero-initialisation for safety — the table itself is the zeroed storage.
- `astra_roots` may `realloc` while frames are live; stored *values* survive
  that, stored *addresses* would not.
- A stale snapshot is never observable, because snapshots are updated at every
  assignment and collection never runs between an assignment and its snapshot
  store (both are adjacent emitted statements).

Cost: one extra store per managed assignment, and `memset(N)` per frame.

### 5.2 [P2] Root extraction is type-directed — approved

(Amendment none: the verdict recorded that the alternative — conservative
scanning — violates R1 and design §4.1, so there was nothing to change.)

A collector marks *objects*, so a slot must hold a heap block pointer, not the
local's own value. For each managed local the emitter emits the payload pointer
it owns:

| Value type | Slot contents |
|---|---|
| `str`, `bytes` | `s.data` (empty views are `NULL` or a 1-byte block; both safe) |
| `Array<T>`, `Map<K,V>` | `arr.data` / `keys` and `values` |
| `Channel<T>` | the `NS_Channel*` itself (the buffer is traced from it) |
| struct | payload pointers of each managed field, one slot each |
| `Option<T>` / `Result<T,E>` / tagged enum | `tag ? <payload root> : NULL` |
| scalars, `f64`, `bool`, `Unit` | no slot |
| `Ptr<T>`, `MutSlice<T>` | **never a root** (R5) |

This parallels `tcx.c_type` and lives in the same crate.

### 5.3 [P3] The emitter invariant: materialise managed subvalues before calls — approved with amendment

`args_c` currently emits arguments inline, so `foo(a, b())` can hold `a`'s
managed payload in an unnamed C temporary while `b()` allocates. To satisfy I1
without restricting where collection runs:

- Before evaluating a call (or any expression containing calls), a subexpression
  whose type holds managed data is evaluated into an emitter temporary first
  (the existing `pre`/`fresh_id` mechanism), and that temporary is snapshotted.
- Scalars stay inline — no cost where nothing can be collected wrongly.

This uses the existing `pre` buffer, which is why it is preferred over the
alternative "suspend collection while mid-expression" scheme: the emitter has a
hook *before* an expression but no hook *after* one, so bracketing a call with
`suspend`/`resume` statements would require new plumbing.

**Amendment (2026-10-29): hoisting must preserve short-circuit and branch
semantics.** Hoisting is only legal in positions whose evaluation is
unconditional. A managed subvalue under the right operand of `&&` or `||` must
not be evaluated (and therefore not snapshot) above the guard — `cond &&
side_effecting_call()` must not run the call when `cond` is false, and the same
applies under `if`/`match` arms and conditional expressions. The hoisting pass
distributes over short-circuit operators: only the operands that actually
execute publish their temporaries, which means the temporaries for the right
operand live inside the guard's own emitted branch. This constrains *where*
materialisation may fire, not *whether* it is sound: any expression that can
allocate while holding an unhoisted managed value is still an error under I1.

Note: materialising also pins left-to-right argument evaluation for managed
arguments. `Docs/spec-v0.1.md` §2 (expressions) does not currently state an
evaluation order for call arguments; per the `[P11]` verdict that is split out
for its own spec review, and until that review concludes, left-to-right
behaviour for managed arguments is an implementation detail of this mechanism,
not a normative rule.

### 5.4 [P4] Where collection runs: explicit safepoints — approved with amendment

`astra_alloc` allocates, records, and bumps `bytes_since_gc`; it never
collects. `astra_gc_maybe_collect()` is emitted:

1. at function entry, after `frame_enter`; and
2. at loop back-edges (`while`/`for`).

If `bytes_since_gc` exceeds the threshold it honours a pending request and
collects the current task's heap.

This keeps "triggered by allocation pressure" (§4.3) while making the set of
collection points small and auditable: a missed safepoint costs delayed
collection, never corruption. With §5.3 in place, collecting inside
`astra_alloc` would also be *correct*, so this is a controllability choice, not
a correctness one.

**Amendments (2026-10-29):**

- **Document the starvation corner.** A long function with no loop — one that
  allocates heavily but never reaches a back-edge — never collects until it
  returns and the caller's next safepoint is reached. This is accepted: it
  costs delay (peak memory), never correctness, and matches "a missed
  safepoint costs delay, not corruption". The behaviour is documented rather
  than mitigated in V1.
- **Debug mode that collects inside every `astra_alloc`.** `ASTRA_GC_STRESS=1`
  (already the stress knob in §7/§8) is specified to collect at *every*
  allocation, not only at safepoints. This exists to continuously test the
  claim that collecting inside `astra_alloc` is also correct under `[P3]` —
  if that claim were ever wrong, the stress runs would fail first, which is
  exactly what a debug mode is for.

### 5.5 [P5] Object model: block header + compiler-emitted shape — approved with amendment

Mark-sweep needs, from a payload pointer alone: the block's extent, and how to
find the pointers inside it. V1 has no runtime reflection (§1), so the shape
must be emitted by the compiler — and the C backend already monomorphises
generics, so every concrete shape is known at emit time.

```c
typedef struct NS_Gc_Head {
    uint32_t marks;
    size_t   bytes;              /* payload extent */
    const NS_Shape* shape;
} NS_Gc_Head;                    /* immediately precedes the payload */
```

Shape grammar (approved, minimal):

- `LEAF` — bytes only (`str`/`bytes` buffers, arrays of scalars).
- `ARRAY(elem)` — payload is `bytes / sizeof(elem)` elements of `elem`;
  the size division must be exact, so the element size lives in the shape.
- `RECORD({offset, shape}[])` — fixed pointer-bearing fields (structs, enum
  payloads, and the fields of a buffer element such as `Array<u8>` inside an
  `Array<Array<u8>>`).
- **`VARIANT(tag_offset, shapes[])`** — Amendment (2026-10-29): tagged enums
  get a first-class shape instead of being approximated by a `RECORD` plus a
  conditional. The tag is read at `tag_offset`; `shapes[tag]` is the payload
  shape for that variant (or `NULL` for a payload-less variant). This replaces
  the vague "a variant wrapper adds the tag handling" sentence the proposal
  originally carried with a shape the mark loop can actually execute.
- Shapes compose.

**Amendment (2026-10-29): zeroed payloads or length-aware tracing.** Spare
capacity and struct padding must never be read as pointers. Two rules make
that true, and the allocator chooses one per block:
  1. **Zero on alloc** (default): a freshly allocated payload is zeroed, so
     unused capacity reads as `NULL` slots and tracing them is harmless; or
  2. **Length-aware shapes**: `ARRAY` shapes that can carry spare capacity
     (array/map buffers, whose `cap` may exceed `len`) carry the live element
     count at a known offset, and the mark loop traces only `len` elements.
The runtime already leans this way (`cap`-sized buffers exist precisely to
grow), so option 2 is the intended path for collection-bearing buffers and
option 1 covers fixed-size records. A trace must never consult uninitialized
bytes.

**Mark stack (approved addition).** Marking uses an explicit, heap-allocated
mark stack (a growable `void*` worklist), not C-stack recursion: a deeply
nested `Array<Array<...>>` must not be able to overflow the C stack during
collection, and the design's no-unwinding rule (R4) leaves no recovery path
if it did. Its peak size is counted against the 32 KB native GC budget
(`03` §12) alongside the root array and block directory; exceeding the budget
in measurement is a revisit trigger, not a silent overrun.

This is the part the design baseline does **not** cover: §4.1 fixes precise
*roots* and says nothing about tracing *through objects*. Hence [P5].

### 5.6 [P6] Interior pointers from `str`/`bytes` slices — approved

A slice view (`runtime.rs:374`) does not point at a block start, so
`payload - sizeof(NS_Gc_Head)` is wrong for it.

Approved: maintain a directory of live blocks; at collection time, sort it once
(`O(k log k)` per collection) and resolve each root by binary search — exact
match if the root is a base pointer, otherwise containment. This keeps slicing
`O(1)` and keeps precision (only real pointers are followed).

**Recorded with the approval (2026-10-29):** the per-collection sort cost and
the 32 KB native budget (`03` §12) are the things to revisit first if this
strains — the directory grows with the live block count, and the sort runs on
every collection. If the budget cannot hold, the fallback is an ABI change
(widening `NS_Str`/`NS_Bytes` with a base pointer, per `11-c-abi.md` §2),
which would need its own approval; it is not to be taken quietly.

Rejected alternatives:

- copy-on-slice: makes `s.slice()` `O(len)` and changes current behaviour cost
  for no correctness gain;
- widening `NS_Str` with a base pointer: an ABI change to `11-c-abi.md` §2;
- conservative scanning: contradicts §4.1 outright.

### 5.7 [P7] Split manual from managed allocation — approved

(One list with two owners would be a bug, and spec §8.4 already says
`Ptr<T>` is not a GC reference.)

Today `Memory.free` unlinks from the same list the collector would trace. Once
GC exists they must be separate owners:

- `Memory.alloc` → `astra_manual_alloc`: plain `malloc`, never traced, never
  collected, freed only by `Memory.free` (spec §8.4 rules, spec 1315-1322).
- Managed values → `astra_gc_alloc`: header + payload, traced by §5.5.

Debug builds should assert the two families are not crossed — the spec already
allows double-free detection (§8.4, spec 1321).

### 5.8 [P8] C-boundary values are pinned for the run — approved with amendment

`11-c-abi.md` §5 promises a value returned across the boundary stays valid
until process exit, and adds "(allocated/static data) — not until the next
collection, because there is none". Rather than break that promise, every
managed value returned from an exported symbol is added to a permanent
run-scoped root set. §5's parenthetical then needs one sentence changed
("because they are pinned for the run"); the guarantee itself is unchanged.

The rejected alternative — releasing boundary values at the next collection —
would silently invalidate the documented consumer contract.

**Amendment (2026-10-29): unbounded pinning is accepted for V1, with a
release API as a follow-up.** Pinning is monotone — a long-running program
that exports many values pins all of them for the process lifetime. That is
accepted for V1 rather than designed away; a follow-up (a
`astra_gc_unpin`-style release API, and/or a stable handle indirection) is
recorded as post-V1 work, not part of this implementation.

**The pinning pair, stated together (2026-10-29):** values *returned* to a
foreign module are pinned for the run (this section); values *borrowed* by a
foreign module are not — a borrowed pointer is valid only for the call
(spec §8.5) and is released when the call ends, exactly like any other
non-root. A foreign module that stashes a borrowed pointer for later use is
violating §8.5, and the collector treating the stashed value as dead is the
correct consequence, not a defect. The two rules are documented as a pair so
neither can be read without the other.

### 5.9 [P9] FFI frames register roots explicitly — concept approved, signature deferred

§4.3 already requires this. The concept is approved: expose a small runtime
API for hand-written `@script:c` blocks — a push/pop root registration pair —
and have the inline-script boundary emit it around calls, since foreign frames
are opaque to the emitter. The names used in this document (`astra_gc_push` /
`astra_gc_pop`) are placeholders: **the exact signature stays open as an ABI
decision for `11-c-abi.md`** and must be settled there before the FFI half is
implemented. The mechanism (§5.1-§5.4) does not depend on the signature.

---

## 6. What this proposal does not change

- Any `.ns` syntax, type, or diagnostic (no new surface).
- JS/TS backends (host GC) and Nova (host memory).
- `Memory`/`ptr` API signatures; finalizers and weak references stay absent.
- The single-threaded task model: one execution context, one heap, one root
  array. `spawn` evaluates inline in the caller's frame (no threads, no task
  queue), so task-local heaps and one-root-array-per-task become meaningful
  only when `CONC-001` adds OS threads — the per-execution-context scoping in
  §5.1 makes that an extension rather than a redesign.

---

## 7. Sequencing

| Phase | Content | Note |
|---|---|---|
| 0 | `NS6002`: forbid managed element types in `Memory.alloc<T>`/`region.alloc<T>` (§11, condition 2) | Small checker+spec slice; lands before Phase A so the footgun class is closed before any allocator split makes it observable |
| A | Header + shapes, allocator split, no collection yet | Pure refactor; arena keeps its current behaviour and its leak tests |
| B | Frame regions, snapshots, materialisation (§5.1-5.3), safepoints behind `ASTRA_GC_STRESS=1` (collect at every safepoint) | Default off; §8 proves correctness |
| C | Turn collection on by default; re-point `astra_arena_live_count`-based tests at the new accounting | |
| D | Wasm: needs a managed allocator in linear memory before any of this can run there | Sequencing decision, [P10] |

Both conditions that gated implementation (§11) are answered, so Phase 0 and
Phase A may start. The `[P9]` signature remains deferred to `11-c-abi.md`, but
only the FFI half of the work depends on it.

No new manifest key is proposed for V1; if a `memory.gc` switch is wanted it is
a spec change, not an implementation detail (see §10).

---

## 8. Verification plan

1. **Stress mode.** `ASTRA_GC_STRESS=1` collects at every allocation (§5.4
   amendment — not only at safepoints, so the "collecting inside `astra_alloc`
   is correct under `[P3]`" claim is continuously tested). Run the full gate
   (`npm run ci`), the 19 positive + 13 reject conformance cases, and every
   hosted `emit_c` compile-and-run under it. This is the standard way to find
   a missing root slot.
2. **Use-after-free oracle.** Poison block payloads on free (`0xDD`) in debug
   builds; any live read of a poisoned byte fails the test.
3. **Proof that collection actually happens.** Today `astra_arena_live_count()`
   cannot fall during a run. Add a test that allocates in a loop and asserts the
   live-block peak stays bounded while total allocations grow — the first test
   in this repo that would fail against today's arena.
4. **Growth-then-collect test.** Build an array past its capacity (forcing a
   push-growth realloc that moves `arr.data`), then force a collection, then
   read every element. This is the case the `[P1]` amendment exists for: the
   snapshot must have been re-published after the growth, or the collector sees
   a stale payload pointer and the read fails under the poison oracle.
5. **Interior-root test.** Take a slice whose base becomes unreachable, collect,
   and read through the slice.
6. **Boundary pinning test.** Return an array to a C consumer, collect, then
   read it (§5.8). A companion case passes a *borrowed* pointer, lets the call
   end, collects, and asserts the borrow is not kept alive (the §5.8 pair).
7. **Budget.** Measure GC runtime against `03` §12's 32 KB native budget — the
   root array, the block directory, **and the mark stack** (§5.5) all count
   against it.
8. **Tier-1.** All of the above on Linux, macOS, and Windows via the CI remote.

---

## 9. Adjacent findings made while writing this

Recorded because they are honest gaps, not part of this proposal:

1. **Spec §8.4 rule "Memory.alloc in a GC-managed scope is a compile-time error"
   was unimplemented - now enforced as `NS6001`.** A probe test confirmed the
   gap was user-visible (system mode accepted `Memory.alloc` with no manual
   block anywhere); the checker now rejects it with `NS6001` "Manual allocation
   outside a manual-memory scope", the first code in the `NS6xxx` Memory range
   that §14.3 declares but had never populated, added here as spec §14.9.
   Making it testable required fixing what is recorded: scope spans now cover
   the whole `{@memory:manual; ...}` block instead of only its header line, so
   containment can see the body and can also reject an allocation made after a
   block closes. Only `alloc` is scoped, because §8.4 names only `alloc`.
2. **`Region.new` / `region.alloc` / `region.free`** (spec §8.4, spec
   1308-1310) **were unimplemented; since resolved - the Region API is landed
   with a fixed-block bump-arena strategy (user decision), so this
   proposal's manual/managed allocator split now has a decided second §8.4
   surface to align with.** See the Region entry in `Docs/Agent/todo.md`.
3. **Fallible `Memory.alloc`** needs an entry-point error policy first; recorded
   in `Docs/Agent/c001-handoff.md`.

Items 1 and 2 are recorded in `Docs/Agent/todo.md` alongside this proposal.

---

## 10. Decision record (decided 2026-10-29)

| ID | Decision | Verdict | Reason |
|---|---|---|---|
| [P1] | Snapshots (values) rather than address-based shadow stacks | **Approve with amendment** | Values survive realloc of the root array; addresses don't. Amendment: re-snapshot after any operation that can change a managed local's payload pointer (push growth, `&mut` calls, field stores), not only after assignment — applied in §5.1 |
| [P2] | Type-directed payload extraction, one slot set per managed local | **Approve** | The alternative is conservative scanning, which violates R1 and design §4.1 |
| [P3] | Materialise managed subvalues before calls (§5.3) | **Approve with amendment** | The emitter has a pre-hook but no post-hook, so suspend/resume is the harder path. Amendment: hoisting must preserve short-circuit and branch semantics (`&&`, `||`, branch-guarded evaluation) — applied in §5.3 |
| [P4] | Safepoints at function entry + loop back-edges, not inside `astra_alloc` | **Approve with amendment** | Auditable, and a missed safepoint costs delay, not corruption. Amendments: document that a long non-looping function never collects, and specify `ASTRA_GC_STRESS=1` to collect inside every `astra_alloc` to test the claim that this is also correct under `[P3]` — applied in §5.4 |
| [P5] | Block header + shape grammar | **Approve with amendment** | Without reflection, compiler-emitted shapes are the only way to trace objects. Amendments: add `VARIANT(tag_offset, shapes[])`, and require zeroed payloads or length-aware tracing so spare capacity and padding are never read as pointers — applied in §5.5 |
| [P6] | Sorted block directory for interior-root resolution | **Approve** | Keeps slicing O(1) with no `NS_Str` ABI change. Recorded: the per-collection sort cost and the 32 KB budget are the things to revisit; the fallback if it fails is an ABI change, which would need its own approval — applied in §5.6 |
| [P7] | Separate manual and GC allocators | **Approve** | One list with two owners would be a bug, and spec §8.4 already says `Ptr<T>` is not a GC reference |
| [P8] | Pin boundary-crossing values instead of releasing them | **Approve with amendment** | Preserves the `11-c-abi.md` §5 contract. Amendment: record that unbounded pinning is accepted for V1 and add a release API as a follow-up — applied in §5.8, together with the returned-pinned / borrowed-not pair |
| [P9] | `astra_gc_push/pop` API for FFI frames | **Approve concept, defer signature** | §4.3 requires an answer and this is a reasonable one; the exact signature is an ABI decision for `11-c-abi.md` and stays open — applied in §5.9 |
| [P10] | Native first, Wasm in a later phase | **Approve** | Wasm needs a managed allocator in linear memory first, so this is sequencing, not a real choice |
| [P11] | Whether call-argument evaluation order becomes normative (surfaced by [P3]) | **Split out, not decided here** | It is a spec change that needs its own review; `[P3]`'s left-to-right behaviour for managed args is an implementation detail until that review happens |

Open questions (`03` §14), status after the decisions:

- **Q1 — is non-moving mark-sweep enough for V1? Closed: yes, and not by this
  file** — design §4.1 already fixes non-moving mark-sweep as the V1 design;
  the proposal never contested it.
- **Q5 — how are FFI roots registered?** Answered as a concept by `[P9]`
  (push/pop registration, signature deferred to `11-c-abi.md`).
- **Borrowed values across FFI** (the question this file originally added):
  **answered** — borrowed values are not pinned; they are released at call end
  per spec §8.5, and a foreign module that stashes a borrowed pointer is
  violating §8.5. The pair (returned = pinned, borrowed = not) is stated
  together in §5.8 as required.

---

## 11. Conditions before implementation — answered (2026-10-29)

The approval was gated on two conditions. Both are answered here with evidence;
neither blocks Phase 0/A any more.

### Condition 1 — task execution model: tasks run on the caller's stack

**Answered: the caller's C stack; the single root array in `[P1]` works as
written.** `spawn` is eager and inline: the C backend evaluates the spawn block
in the caller's own frame and stores the result in a `Task` value (the emitter
comment says "run the block now and store its value in a Task"), and the
runtime contains no thread, `ucontext`, `setjmp`, or task queue of any kind. A
`Task<T>` is therefore just a value holding an already-computed result, and its
payload is an ordinary managed local of the receiving frame.

To keep the design robust rather than merely correct-for-now, `[P1]`'s root
array is scoped **per execution context** (§5.1): there is exactly one today,
and `CONC-001` OS-threaded tasks then add one array per task without redesign.

### Condition 2 — `Ptr<T>` and managed types: forbid (with `NS6002` proposed)

**Answered by probe (2026-10-29): the hazard is fully expressible today, so
the decision is to forbid managed element types in manual allocation.**

What the probe established (system mode, `@memory:manual`):

| Allocation | Result |
|---|---|
| `Memory.alloc<str>(4)` + `p.write(0, "hello")` | **ACCEPTED** — a managed pointer is written into manual memory |
| `Memory.alloc<[u8]>(4)` | **ACCEPTED** |
| `Memory.alloc<Channel<i32>>(2)` | **ACCEPTED** |
| `Memory.alloc<Rec>(2)` with `struct Rec { name: str, size: i32 }` | **ACCEPTED** |
| `Memory.alloc<Pair>(2)` with scalar-only `Pair` | ACCEPTED (the legitimate case) |
| `Memory.alloc<Array<u8>>(4)` | Rejected — but only because `Array<T>` is not source syntax (`[T]` is, per spec §5; `Array<T>` is the backend spelling), **not** by any managed-type rule |

Since `[P5]`/`[P7]` make `Ptr<T>` never a root (spec §8.4), every accepted row
above is a dangling read waiting to happen once collection runs: the manual
copy outlives a collection that cannot see it. The choice is the offered one:
**forbid**, not document-as-footgun. The predicate is exactly the inverse of
`[P5]`'s shape predicate — "T contains no GC-managed references" (`str`,
`bytes`, `[T]`, `Map`, `Channel`, `Task`, and structs/enums/tuples containing
them) — so it costs no new analysis: the shape machinery already computes it.

Scope of the rule: the allocation sites `Memory.alloc<T>` and
`region.alloc<T>`. `Ptr<T>` as a parameter type stays expressible (FFI can
still hand one over the boundary, which is unsafe by contract under spec
§8.5); what closes is the compiler-visible way to *create* manual memory that
holds managed references.

Implementation path (Phase 0, §7): a checker rule with proposed code
**`NS6002`** "Manual allocation of a GC-managed element type" — the second
code in the `NS6xxx | Memory` range that spec §14.3 reserves — plus one rule
line under spec §8.4. The code number follows the `NS6001` precedent and is
flagged as proposed in `Docs/Agent/todo.md`; it is reversible.

Linked to the deferred pointer-access follow-up: `ptr.as_slice`/`MutSlice` and
any future `Ptr<T>`-producing surface must apply the same predicate, so the
forbid is written as a predicate over "manual allocation of T", not as a
one-off check on two builtins.

---

## 12. References

- `Docs/spec-v0.1.md` §8 (memory), §8.4 (manual API and rules), §8.5 (FFI ownership)
- `Docs/Design/V1/03-runtime-and-memory.md` §4, §12, §14
- `Docs/Design/V1/04-targets-and-backends.md` §6.2
- `Docs/Design/V1/11-c-abi.md` §2, §5, §6
- `Docs/Agent/project-context-glossary.md` §6 (D1-D25)
- `Docs/Agent/todo.md` `GC-001` entry; `Docs/Agent/c001-handoff.md`
- Implementation: `crates/astra-codegen-c/src/runtime.rs`, `.../src/emit.rs`,
  `crates/astra-codegen-c/tests/emit_c.rs`
