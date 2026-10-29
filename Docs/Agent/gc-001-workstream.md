# GC-001 workstream board

> **Purpose.** Single source of truth for the `GC-001` (precise mark-sweep GC)
> workstream: phase status, open bugs, open decisions, the verification matrix,
> and the next slices. `Docs/Design/GC-001-proposal.md` is the approved design
> (what to build and why); this file is the live state (what is done, what is
> broken, what is next). `Docs/Agent/state-log.md` keeps the chronological
> history; `Docs/Agent/todo.md` keeps one queue line per slice that points here.

Last updated: 2026-10-29.

## 1. Phase status

| Phase | Scope | Status |
|---|---|---|
| 0 | `NS6002`: managed element types forbidden in `Memory.alloc`/`region.alloc` | **Landed** `cf1056a` (checker + spec §8.4/§14.9); Tier-1 run 18 |
| A | Block header (`NS_Gc_Head`), shape grammar (`NS_Gc_Shape`), compiler-emitted shape tables, manual/managed allocator split | **Landed** `a7c3b58` (Tier-1 run 20 red on Ubuntu, fixed by `753c54f`) |
| B | Root frames, type-directed snapshots, materialisation, safepoints, mark-sweep collector under `ASTRA_GC_STRESS=1`, stress leg in CI | **Landed** `3e9893b`, `0233ce2`, `18fc3fa`, follow-ups `1b6fa85` + `463ff54`; collection still **off by default**. Verified by Tier-1 run 27 (`97e1475`): green on Linux, macOS and Windows |
| C | Collection on by default (`bytes_since_gc` threshold), re-point arena-accounting tests | **Blocked** on §2 and §3 below |
| D | Wasm managed allocator in linear memory | Deferred by design (proposal §7, `[P10]`) |

## 2. Open bugs

| Bug | State |
|---|---|
| macOS-only stress failure in the `collections` corpus case (panic before output) | **Fixed** `463ff54`: the helper roots the key buffer across the value allocation (same class as the channel handle bug). Regression test: `map_growth_keeps_the_key_buffer_across_the_value_allocation` (deterministic live-count + lookup under stress). Verified: Tier-1 run 27 (`97e1475`) green on all three OSes |
| `channels` corpus case under stress on Windows/macOS (empty messages) | **Fixed** `1b6fa85`: `astra_channel_new` freed the handle while allocating the message buffer. Regression test: `channel_creation_keeps_the_handle_across_the_data_allocation`. Verified: run 27 green (run 25 had fixed Windows; macOS then exposed the map bug above) |

**Rule learned (applies to every runtime helper):** a helper that allocates more
than one managed block must root the intermediate block(s) with
`astra_gc_temp_enter`/`astra_gc_publish`/`astra_gc_temp_exit`, because the stress
hook collects inside every `astra_gc_alloc` and the language cannot publish a
value that has not been returned yet. Current multi-block helpers: map growth,
channel creation. Single-block helpers are safe by construction.

## 3. Open decisions (owner: user unless noted)

| Decision | Where | Status |
|---|---|---|
| `[P9]` FFI root registration signature (`astra_gc_push`/`pop` placeholders) | proposal §5.9, §10 Q5; must land in `11-c-abi.md` §5 | **Open** - proposal §5.9 now states the trigger: approved as an `11-c-abi.md` change before any `@script:c` boundary emits registration calls. Needed before the FFI half and before Phase C claims boundary safety |
| `Region` scope rule: should `Region.new`/`region.alloc` require `@memory:manual`? | todo.md, spec §8.4 names only `Memory.alloc` | **Open** - spec question, not blocking GC |
| Per-case mode selection in the negative-conformance runner | todo.md | **Open** - infrastructure; would let `NS6001`/`NS6002` be corpus-pinned |
| Generic forwarding past `NS6002` | todo.md, proposal §9 | **Recorded** - checker validates bodies once against generic placeholders; follows the existing instantiation model |

## 4. Verification matrix (proposal §8)

| Item | State |
|---|---|
| 1. Stress mode over the gate + corpus | **Done** - `scripts/ci.mjs` stress leg: `emit_c` + `conformance` with `ASTRA_GC_STRESS=1` |
| 2. Poison-on-free oracle | **Not implemented** - next slice (S3) |
| 3. Bounded live set while total grows | **Done** - `stress_collection_keeps_the_live_set_bounded` |
| 4. Growth-then-collect (push realloc) | **Done** - `roots_survive_stress_collection_at_every_allocation` |
| 5. Interior-root (`bytes` slice after owner reassigned) | **Done** - `interior_slice_keeps_its_block_alive_under_stress`; harness-level interior/VARIANT test |
| 6. Boundary pinning test | **Blocked** on S2 (pinning) and S4 (`[P9]` for the borrowed half) |
| 7. Runtime budget vs `03` §12 (32 KB) | **Not measured** - root array, directory, mark stack |
| 8. Tier-1 across Linux/macOS/Windows | **Done for Phase B** - run 27 (`97e1475`) green on all three OSes (runs 23-25 caught the channel and map root bugs first) |

## 5. Next slices (in order)

- **S1 - finish the stress fixes. Done** (`1b6fa85`, `463ff54`; run 27 green on
  all three OSes).
- **S2 - §5.8 boundary pinning.** Managed values returned from exported symbols
  are pinned for the run (a permanent root set), and the `11-c-abi.md` §5
  parenthetical changes from "because there is none" to "because they are
  pinned for the run". Test: return an array to C, collect, read it; borrowed
  pointers are *not* pinned (the §5.8 pair). Needs the exported-symbol surface
  the emitter already knows (`pub` functions in library mode).
- **S3 - §8.2 poison oracle.** Fill freed payloads with `0xDD` in debug builds
  so a live read of a collected block fails loudly; rerun the stress leg.
- **S4 - §5.9 ABI proposal.** Draft the push/pop signature options in
  `11-c-abi.md` §5 for user approval (single value vs counted push; returned
  frame handle vs scope stack; emitter owns pairing). Blocks the FFI half only.
- **S5 - Phase C.** `bytes_since_gc` threshold at safepoints, collection on by
  default, re-point `astra_arena_live_count` tests, record the budget (§8.7).
- **S6 - Phase D (Wasm).** Deferred; needs a managed allocator in linear memory.

## 6. Landing records

### Phase 0 - `NS6002` (`cf1056a`)

Checker predicate `holds_managed_refs` (inverse of the shape predicate) rejects
managed element types in `Memory.alloc`/`region.alloc`; ordering mode
(`NS5004`) → scope (`NS6001`) → element type (`NS6002`). Spec §8.4 rule line,
§14.9 row. Limits: no negative-conformance case is possible (corpus runs at
core mode, so `NS5004` always fires first); generic forwarding is silent until
the element type is concrete at the definition site.

### Phase A - object model and allocator split (`a7c3b58`)

`NS_Gc_Head {marks, bytes, shape}` before every managed payload; `NS_Gc_Shape`
grammar `LEAF`/`ARRAY(elem_size, elem)`/`RECORD({offset, shape}[])`/
`VARIANT(tag_offset, shapes[])` with builtins `NS_SHAPE_LEAF`, `NS_SHAPE_STR`,
`NS_SHAPE_ARRAY_OF_STR`. The `Gc_` infix avoids a collision with user types named
`Shape` (which emit `NS_Shape`). `astra_gc_alloc` zero-allocates and records;
`astra_gc_realloc` re-heads and zeroes grown tails; `astra_gc_free` is strict.
`astra_manual_alloc/free` plus counters serve `Memory.alloc`/`Memory.free` and
the `Region` block (the `Region` handle stays a managed LEAF block so
`region.free()`'s double-free no-op survives). `astra_bytes_new` moved from
untracked `calloc` onto a tracked LEAF block. Shapes are emitted by
`TyCtxt::shape_ref` (deduped, dependency-ordered, static const per TU) and
threaded through array/map/channel/`str_chars` allocations. Limits: nothing read
the shapes yet at that point; manual free stays unchecked; the enum tag is
assumed `int`-sized for the future mark loop; shape tables are per-TU statics.

### Phase B - roots, materialisation, collector (`3e9893b`, `0233ce2`, `18fc3fa`, `1b6fa85`)

- Runtime (`3e9893b`): growable root array of payload pointers;
  `frame_enter/exit`, `publish` (grows on demand, idempotent per declaration
  site), `temp_enter/exit`; sorted block directory with interior-pointer
  resolution; explicit mark stack tracing by shape; sweep; `ASTRA_GC_STRESS`
  collecting at every allocation and safepoint; shutdown releases roots and the
  mark stack.
- Emitter (`0233ce2`): `TyCtxt::root_exprs` (§5.2 table) next to `shape_ref`;
  publishes parameters, `let` bindings, for-loop iterable copy and binding,
  match/pattern bindings; republishes a local after an assignment or a mutating
  method call; safepoints at entry and loop back-edges; frame exit on every
  return path.
- Materialisation (`18fc3fa`): managed subvalues hoisted into published
  temporaries before call/method arguments, array/map/struct/tuple literals and
  binary operands; disabled in short-circuit right operands and `while`
  conditions. CI stress leg added.
- Follow-ups: channel handle rooting (`1b6fa85`) and map-growth key rooting
  (this slice); both were the same helper class and both have deterministic
  live-count regression tests.
- Limits, still true: §5.8 pinning and §5.9 FFI registration are not
  implemented; §8.2 poison and §8.7 budget are not done; hoisting gaps remain
  inside short-circuit/`while` conditions; republishing is statement-level;
  slots are not cleared on scope exit (dead locals stay alive until the frame
  returns, as §5.1 designs); manual memory is unchecked.
