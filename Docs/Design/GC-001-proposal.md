# GC-001 — precise roots and non-moving mark-sweep (proposal)

> **Status: PROPOSAL. This file is not authoritative.**
>
> It changes no spec text, no design-baseline document, and no D-decision. Per
> `Docs/Agent/agent.md` rule 10 ("Do not invent decisions"), every choice here
> that is not already fixed by D1-D25, `Docs/spec-v0.1.md`, or
> `Docs/Design/V1/` is tagged **[P]** and listed in §10 for approval. Nothing
> in this file is implemented.
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

## 5. Proposed mechanism

### 5.1 [P1] Eager value snapshots, one contiguous region per frame

Per task, a growable array of `void*` slots:

```c
void**  astra_roots;        /* slot values, not addresses */
size_t  astra_root_count;   /* high-water mark of the live region */
```

- Function entry: `size_t _rf = astra_gc_frame_enter(N);` reserves `N` slots
  and zeroes them (so a slot for a local not yet assigned reads as `NULL`).
  `N` comes from a cheap pre-pass over the function body that assigns each
  managed local and temporary a fixed slot index.
- **Snapshot on assignment**: immediately after every C store of a value that
  holds managed data (`v_x = ...`, parameter binding, emitter temporary),
  emit `astra_roots[_rf + k] = <root expr>;`.
- Return: `astra_roots_count = _rf;` before every `return`, emitted by
  `emit_return` on all paths (ordinary, fallible, unit).

Why snapshots rather than the more usual "array of addresses of locals":

- No local needs to be hoisted to the top of the function, and no local needs
  zero-initialisation for safety — the table itself is the zeroed storage.
- `astra_roots` may `realloc` while frames are live; stored *values* survive
  that, stored *addresses* would not.
- A stale snapshot is never observable, because snapshots are updated at every
  assignment and collection never runs between an assignment and its snapshot
  store (both are adjacent emitted statements).

Cost: one extra store per managed assignment, and `memset(N)` per frame.

### 5.2 [P2] Root extraction is type-directed

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

### 5.3 [P3] The emitter invariant: materialise managed subvalues before calls

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

Note: materialising also pins left-to-right argument evaluation for managed
arguments. `Docs/spec-v0.1.md` §2 (expressions) does not currently state an
evaluation order for call arguments; if it should, that is a separate spec
decision and is listed in §10 as [P11].

### 5.4 [P4] Where collection runs: explicit safepoints

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

### 5.5 [P5] Object model: block header + compiler-emitted shape

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

Shape grammar (proposed, minimal):

- `LEAF` — bytes only (`str`/`bytes` buffers, arrays of scalars).
- `ARRAY(elem)` — payload is `bytes / sizeof(elem)` elements of `elem`;
  the size division must be exact, so the element size lives in the shape.
- `RECORD({offset, shape}[])` — fixed pointer-bearing fields (structs, enum
  payloads, and the fields of a buffer element such as `Array<u8>` inside an
  `Array<Array<u8>>`).
- Shapes compose; a variant wrapper adds the tag handling already present in
  §5.2's table.

This is the part the design baseline does **not** cover: §4.1 fixes precise
*roots* and says nothing about tracing *through objects*. Hence [P5].

### 5.6 [P6] Interior pointers from `str`/`bytes` slices

A slice view (`runtime.rs:374`) does not point at a block start, so
`payload - sizeof(NS_Gc_Head)` is wrong for it.

Proposed: maintain a directory of live blocks; at collection time, sort it once
(`O(k log k)` per collection) and resolve each root by binary search — exact
match if the root is a base pointer, otherwise containment. This keeps slicing
`O(1)` and keeps precision (only real pointers are followed).

Rejected alternatives:

- copy-on-slice: makes `s.slice()` `O(len)` and changes current behaviour cost
  for no correctness gain;
- widening `NS_Str` with a base pointer: an ABI change to `11-c-abi.md` §2;
- conservative scanning: contradicts §4.1 outright.

### 5.7 [P7] Split manual from managed allocation

Today `Memory.free` unlinks from the same list the collector would trace. Once
GC exists they must be separate owners:

- `Memory.alloc` → `astra_manual_alloc`: plain `malloc`, never traced, never
  collected, freed only by `Memory.free` (spec §8.4 rules, spec 1315-1322).
- Managed values → `astra_gc_alloc`: header + payload, traced by §5.5.

Debug builds should assert the two families are not crossed — the spec already
allows double-free detection (§8.4, spec 1321).

### 5.8 [P8] C-boundary values are pinned for the run

`11-c-abi.md` §5 promises a value returned across the boundary stays valid
until process exit, and adds "(allocated/static data) — not until the next
collection, because there is none". Rather than break that promise, every
managed value returned from an exported symbol is added to a permanent
run-scoped root set. §5's parenthetical then needs one sentence changed
("because they are pinned for the run"); the guarantee itself is unchanged.

The rejected alternative — releasing boundary values at the next collection —
would silently invalidate the documented consumer contract.

### 5.9 [P9] FFI frames register roots explicitly

§4.3 already requires this. Proposal: expose a small runtime API for hand-written
`@script:c` blocks — `astra_gc_push(void*)` / `astra_gc_pop(n)` — and have the
inline-script boundary emit it around calls, since foreign frames are opaque to
the emitter. The exact signature is an ABI question for `11-c-abi.md`.

---

## 6. What this proposal does not change

- Any `.ns` syntax, type, or diagnostic (no new surface).
- JS/TS backends (host GC) and Nova (host memory).
- `Memory`/`ptr` API signatures; finalizers and weak references stay absent.
- The single-threaded task model: one task, one heap, one root array. Task-local
  heaps become meaningful only when `CONC-001` adds OS threads.

---

## 7. Sequencing

| Phase | Content | Note |
|---|---|---|
| A | Header + shapes, allocator split, no collection yet | Pure refactor; arena keeps its current behaviour and its leak tests |
| B | Frame regions, snapshots, materialisation (§5.1-5.3), safepoints behind `ASTRA_GC_STRESS=1` (collect at every safepoint) | Default off; §8 proves correctness |
| C | Turn collection on by default; re-point `astra_arena_live_count`-based tests at the new accounting | |
| D | Wasm: needs a managed allocator in linear memory before any of this can run there | Sequencing decision, [P10] |

No new manifest key is proposed for V1; if a `memory.gc` switch is wanted it is
a spec change, not an implementation detail (see §10).

---

## 8. Verification plan

1. **Stress mode.** `ASTRA_GC_STRESS=1` collects at every safepoint. Run the
   full gate (`npm run ci`), the 19 positive + 12 reject conformance cases, and
   every hosted `emit_c` compile-and-run under it. This is the standard way to
   find a missing root slot.
2. **Use-after-free oracle.** Poison block payloads on free (`0xDD`) in debug
   builds; any live read of a poisoned byte fails the test.
3. **Proof that collection actually happens.** Today `astra_arena_live_count()`
   cannot fall during a run. Add a test that allocates in a loop and asserts the
   live-block peak stays bounded while total allocations grow — the first test
   in this repo that would fail against today's arena.
4. **Interior-root test.** Take a slice whose base becomes unreachable, collect,
   and read through the slice.
5. **Boundary pinning test.** Return an array to a C consumer, collect, then
   read it (§5.8).
6. **Budget.** Measure GC runtime against `03` §12's 32 KB native budget.
7. **Tier-1.** All of the above on Linux, macOS, and Windows via the CI remote.

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
   1308-1310) have no implementation reference anywhere in `crates/`,
   `orchestrator/packages/`, or `conformance/`.
3. **Fallible `Memory.alloc`** needs an entry-point error policy first; recorded
   in `Docs/Agent/c001-handoff.md`.

Items 1 and 2 are recorded in `Docs/Agent/todo.md` alongside this proposal.

---

## 10. Decisions requiring approval

| ID | Decision | If rejected |
|---|---|---|
| [P1] | Snapshots (values) rather than address-based shadow stacks | Needs local hoisting + zero-init, and breaks across `realloc` of the root array |
| [P2] | Type-directed payload extraction, one slot set per managed local | Alternative: conservative per-type "whole struct is a root", which mislabels `str.data` bytes as pointers |
| [P3] | Materialise managed subvalues before calls (§5.3) | Alternative: suspend collection mid-expression; architecturally awkward (no post-expression hook) |
| [P4] | Safepoints at function entry + loop back-edges, not inside `astra_alloc` | Collecting inside `astra_alloc` is also correct once [P3] holds |
| [P5] | Block header + `LEAF`/`ARRAY`/`RECORD` shape grammar | Nothing else can trace objects without reflection |
| [P6] | Sorted block directory for interior-root resolution | Copy-on-slice, or ABI change to `NS_Str` |
| [P7] | Separate manual and GC allocators | `Memory.free` and the collector would fight over one list |
| [P8] | Pin boundary-crossing values instead of releasing them | Breaks `11-c-abi.md` §5 as written |
| [P9] | `astra_gc_push/pop` API for FFI frames | §4.3 requires *some* answer; the API shape is an ABI decision |
| [P10] | Native first, Wasm in a later phase | Doing both at once doubles the surface of a change that is hard to debug |
| [P11] | Whether call-argument evaluation order becomes normative (surfaced by [P3]) | Spec change; needs its own review |

**Open questions carried forward** (`03` §14): is non-moving mark-sweep enough
for V1 (Q1)? how are FFI roots registered (Q5, answered here only as a
proposal)? plus one new: does a value handed to a foreign module by *borrowed*
ownership become pinned like a boundary return (§5.8), or is it released when
the call ends? Spec §8.5 says borrowed pointers are valid only for the call,
which suggests released — but the two rules must be stated together.

---

## 11. References

- `Docs/spec-v0.1.md` §8 (memory), §8.4 (manual API and rules), §8.5 (FFI ownership)
- `Docs/Design/V1/03-runtime-and-memory.md` §4, §12, §14
- `Docs/Design/V1/04-targets-and-backends.md` §6.2
- `Docs/Design/V1/11-c-abi.md` §2, §5, §6
- `Docs/Agent/project-context-glossary.md` §6 (D1-D25)
- `Docs/Agent/todo.md` `GC-001` entry; `Docs/Agent/c001-handoff.md`
- Implementation: `crates/astra-codegen-c/src/runtime.rs`, `.../src/emit.rs`,
  `crates/astra-codegen-c/tests/emit_c.rs`
