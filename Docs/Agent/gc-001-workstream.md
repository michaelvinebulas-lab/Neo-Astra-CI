# GC-001 workstream board

> **Purpose.** Single source of truth for the `GC-001` (precise mark-sweep GC)
> workstream: phase status, open bugs, open decisions, the verification matrix,
> and the next slices. `Docs/Design/GC-001-proposal.md` is the approved design
> (what to build and why); this file is the live state (what is done, what is
> broken, what is next). `Docs/Agent/state-log.md` keeps the chronological
> history; `Docs/Agent/todo.md` keeps one queue line per slice that points here.

Last updated: 2026-10-30.

## 1. Phase status

| Phase | Scope | Status |
|---|---|---|
| 0 | `NS6002`: managed element types forbidden in `Memory.alloc`/`region.alloc` | **Landed** `cf1056a` (checker + spec §8.4/§14.9); Tier-1 run 18 |
| A | Block header (`NS_Gc_Head`), shape grammar (`NS_Gc_Shape`), compiler-emitted shape tables, manual/managed allocator split | **Landed** `a7c3b58` (Tier-1 run 20 red on Ubuntu, fixed by `753c54f`) |
| B | Root frames, type-directed snapshots, materialisation, safepoints, mark-sweep collector under `ASTRA_GC_STRESS=1`, stress leg in CI | **Landed** `3e9893b`, `0233ce2`, `18fc3fa`, follow-ups `1b6fa85` + `463ff54`; collection still **off by default**. Verified by Tier-1 run 27 (`97e1475`): green on Linux, macOS and Windows |
| C | Collection on by default (`bytes_since_gc` threshold), re-point arena-accounting tests | **Done** (2026-10-30) - `astra_gc_maybe_collect` collects once 1 MiB (`ASTRA_GC_DEFAULT_THRESHOLD`) of managed bytes have been allocated since the last collection; the direct-runtime arena-accounting harnesses never reach a safepoint, so they needed no re-pointing |
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
| `[P9]` FFI root registration signature (`astra_gc_push`/`pop` placeholders) | proposal §5.9, §10 Q5; landed in `11-c-abi.md` §5.1 | **Decided 2026-10-30** (user approved the recommendation): scoped handle triple `astra_gc_scope_enter` / `astra_gc_root(scope, payload)` / `astra_gc_scope_exit(scope)`, chosen over counted push/pop because the emitter owns the pairing and a script author cannot unbalance a handle. Landed as `11-c-abi.md` §5.1 before any call site, per the §5.9 trigger; the emitter emits it with the first `@script:c` boundary that passes managed values into a foreign frame (none does today) |
| §5.8 pinning scope: which returns pin? | proposal §5.8, `11-c-abi.md` §5 | **Decided 2026-10-30**: exported (`pub`) non-generic functions in **library** builds (implemented in S2). Executable builds have no foreign consumer, and private helpers never cross the boundary, so neither pins |
| `Region` scope rule: should `Region.new`/`region.alloc` require `@memory:manual`? | todo.md, spec §8.4 | **Decided 2026-10-30**: no mode change — spec §8.4 names only `Memory.alloc`, matching the implementation; `Region` is scoped allocation whose blocks are released by `region.free()`. A clarification line landed in spec §8.4, recording the hazard too: a region that is never freed leaks its blocks, and the GC never reclaims manual memory |
| Per-case mode selection in the negative-conformance runner | todo.md | **Decided 2026-10-30**: implement it, as test infrastructure only — a runner-parsed comment directive in the case file (no language surface). Unblocks `NS6001`/`NS6002` negative-conformance cases; own infra slice, not on the Phase C critical path |
| Generic forwarding past `NS6002` | todo.md, proposal §9 | **Recorded** - checker validates bodies once against generic placeholders; follows the existing instantiation model |

## 4. Verification matrix (proposal §8)

| Item | State |
|---|---|
| 1. Stress mode over the gate + corpus | **Done** - `scripts/ci.mjs` stress leg: `emit_c` + `conformance` with `ASTRA_GC_STRESS=1` |
| 2. Poison-on-free oracle | **Done** - `swept_payloads_are_poisoned_for_the_use_after_free_oracle`: the fill itself is asserted on a still-owned block (pattern + full extent) and both free paths (explicit and sweep) are asserted to run it via the poison counter. Reading freed memory back proved non-portable (macOS unmaps fully freed regions - run 30 - or reclaims them zero-filled - run 31), so the test verifies the oracle without it. Debug builds only; `#define NDEBUG` skips the fill |
| 3. Bounded live set while total grows | **Done** - `stress_collection_keeps_the_live_set_bounded` (stress mode) and `default_collection_keeps_the_live_set_bounded` (Phase C default mode: no collection below the threshold, bounded live peak past it) |
| 4. Growth-then-collect (push realloc) | **Done** - `roots_survive_stress_collection_at_every_allocation` |
| 5. Interior-root (`bytes` slice after owner reassigned) | **Done** - `interior_slice_keeps_its_block_alive_under_stress`; harness-level interior/VARIANT test |
| 6. Boundary pinning test | **Done (returned half)** - `library_pins_values_returned_across_the_c_boundary` returns an array to C, collects, and reads it; `only_library_exports_pin_managed_returns` covers an `Err` payload and asserts executable builds/private helpers never pin. The borrowed-not-pinned half is the §5.8 pair stated in `11-c-abi.md` §5 and asserted by the pass-back/no-new-pin check; FFI frame registration remains S4 |
| 7. Runtime budget vs `03` §12 (32 KB) | **Done** (2026-10-30) - static GC state is 116 B of globals (4 pointers, 10 `size_t`, 1 `int`; +24 B shared arena bookkeeping), ~0.4% of the budget; per-collection transients (directory 16 B/live block, mark stack 8 B/entry, roots 8 B/slot, pins 8 B/pin) are freed at the end of the collection. Shape tables are emitted per-program data, not runtime state |
| 8. Tier-1 across Linux/macOS/Windows | **Done for Phase B** - run 27 (`97e1475`) green on all three OSes (runs 23-25 caught the channel and map root bugs first); re-verified locally 2026-10-29 (full `npm run ci`, 96 `emit_c` tests, 19/19 conformance cases executed under `ASTRA_GC_STRESS=1`) |

## 5. Next slices (in order)

- **S1 - finish the stress fixes. Done** (`1b6fa85`, `463ff54`; run 27 green on
  all three OSes).
- **S2 - §5.8 boundary pinning. Done** (2026-10-29; see the landing record).
  Library-mode exported functions pin managed returns (including `Ok`/`Err`
  payloads) into a monotone run-scoped set; `11-c-abi.md` §5 now states the
  returned-pinned / borrowed-not pair. Covered by two `emit_c` tests, one of
  which runs a hand-written C consumer across an explicit `astra_gc_collect()`.
- **S3 - §8.2 poison oracle. Done** (2026-10-30; see the landing record).
  Freed managed payloads are filled with `0xDD` in debug builds on both free
  paths (sweep and explicit), so a stale read of a collected block fails
  loudly instead of returning plausible-looking data. The stress leg reruns
  over it.
- **S4 - §5.9 FFI signature. Done** (2026-10-30): the scoped-handle triple
  (`astra_gc_scope_enter`/`astra_gc_root`/`astra_gc_scope_exit`) is landed as
  `11-c-abi.md` §5.1 before any call site, per the §5.9 trigger. The emitter
  does not emit registration yet: the first `@script:c` boundary that passes
  managed values into a foreign frame emits it, and the §7 boundary-rooting
  test lands with that slice.
- **S5 - Phase C. Done** (2026-10-30, Tier-1 run 33 green on all three OSes;
  see the landing record). `astra_gc_alloc`
  bumps `bytes_since_gc`; `astra_gc_maybe_collect` - already emitted at
  function entry and loop back-edges - collects once the threshold (fixed 1
  MiB constant, runtime-only, not language surface) is reached. Stress mode is
  unchanged. No emitter change was needed and no arena-accounting harness
  needed re-pointing: they call the runtime directly and never reach a
  safepoint.
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

### Phase B - roots, materialisation, collector (`3e9893b`, `0233ce2`, `18fc3fa`, `1b6fa85`, `463ff54`)

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
  (`463ff54`); both were the same helper class and both have deterministic
  live-count regression tests.
- Limits, still true: §5.9 FFI registration and §8.2 poison and §8.7 budget are
  not done (§5.8 boundary pinning landed later as S2); hoisting gaps remain
  inside short-circuit/`while` conditions; republishing is statement-level;
  slots are not cleared on scope exit (dead locals stay alive until the frame
  returns, as §5.1 designs); manual memory is unchecked.

### S2 - §5.8 boundary pinning (landed `25763da`, 2026-10-30; records written 2026-10-29)

- Runtime: a monotone pin set (`astra_gc_pins`) alongside the frame root array.
  `astra_gc_pin(payload)` appends (NULL ignored), the collector marks every pin
  after the frame roots, and `astra_runtime_shutdown` releases the array.
  `astra_gc_pin_count` is the observability hook the tests assert on. Pinning is
  deliberately unbounded for V1 per the §5.8 amendment.
- Emitter: `Emitter::library` (true iff `emit_project` runs with
  `emit_main = false`) plus `pin_returns`, set per function by
  `returns_across_abi` — exported (`pub`), non-generic, library mode. Every
  value-carrying return path materialises the value into a fresh temporary and
  pins `TyCtxt::root_exprs` of the declared return type before the frame exit:
  tail expression (plain and fallible), explicit `return`, `fail`, and the
  `expr!` error branch. Materialising first means each sub-expression is
  evaluated exactly once; `Option`/`Result` guards yield `NULL` for inactive
  payloads, which the pin call ignores. Because the result is materialised, a
  fallible return pins the `Ok` value and the `Err` payload through the same
  type-directed expressions.
- Why library mode only: an executable has no foreign consumer, and pinning
  every `pub` return there would make a loop that calls a linked module's
  function grow the live set without bound once Phase C turns collection on.
- `11-c-abi.md` §5: the allocator paragraph now describes the Phase B collector,
  the `str`/`bytes` and array/map promises say "because returned managed values
  are pinned for the run (GC-001 §5.8)", and the returned-pinned /
  borrowed-not-pinned pair is stated together.
- Tests: `library_pins_values_returned_across_the_c_boundary` (hosted consumer
  returns an array, calls `astra_gc_collect()`, reads all three elements, asserts
  exactly one pin, and checks that passing the value back into an exported
  function adds none) and `only_library_exports_pin_managed_returns` (exported
  array + constructed `Err` across a collection in C; private helper and
  executable build emit no pin calls).
- Evidence: full local gate green (`npm run ci` → `[ci] all checks passed`,
  98 `emit_c` tests including the stress leg) and the conformance corpus green
  with `ASTRA_GC_STRESS=1`.
- Limits: an exported function that returns a *foreign* struct/enum type pins
  nothing for its inner fields (the shape is not in this module's `TypeEnv`),
  the same limitation `root_exprs` already has for locals of foreign nominal
  types; a pinned pointer can still be invalidated by `astra_gc_realloc` if the
  exporter later grows the same buffer (aliasing, not collector loss); release
  is post-V1.

### S3 - §8.2 poison oracle (2026-10-30)

- Runtime: `astra_gc_poison` fills the payload extent with `0xDD` before both
  managed free paths - the collector's sweep and the strict `astra_gc_free` -
  behind `#ifndef NDEBUG`, so release builds that define `NDEBUG` skip the
  fill. Manual blocks are deliberately *not* poisoned: they carry no size
  header, and spec §8.4 already leaves their use-after-free undefined (the
  debug double-free detection there is a separate MAY).
- Test: `swept_payloads_are_poisoned_for_the_use_after_free_oracle` verifies
  two portable things instead of reading freed memory (runs 30/31 showed that
  is not portable: macOS unmaps a fully freed tiny region, or reclaims the
  page and returns it zero-filled): the fill itself on a still-owned block
  via the `astra_gc_poison_payload` debug hook (every byte `0xDD`, full
  payload extent), and that both free paths - the strict `astra_gc_free` and
  the collector's sweep - run that fill, counted by `astra_gc_poison_count`.
  Runs under `ASTRA_GC_STRESS=1`; failure diagnostics print to stderr.
- Effect on the verification plan: §8 items 2-6 are now all done; §8.7 (the
  budget measurement) and the per-slice Tier-1 runs remain.

### S5 - Phase C: collection on by default (2026-10-30)

- Runtime: `astra_gc_alloc` and `astra_gc_realloc` bump a new
  `astra_gc_bytes_since_gc` counter; `astra_gc_maybe_collect` - which the
  emitter already emits at function entry (after `frame_enter`) and at loop
  back-edges (`while`, `for`, `continue`) - now collects when that counter
  reaches `ASTRA_GC_DEFAULT_THRESHOLD`, a fixed 1 MiB constant reset at every
  collection and at shutdown. The threshold is a runtime constant, not
  language surface: tuning it (or making it adaptive to the surviving heap)
  is post-V1. `ASTRA_GC_STRESS=1` is unchanged (collect at every safepoint
  and allocation).
- No emitter change and no re-pointed tests: the arena-accounting harnesses
  call the runtime directly and never reach a safepoint, and the whole gate -
  19 positive + 13 reject conformance cases plus every hosted harness -
  passed with default collection on, which the stress leg had already made
  safe.
- Test: `default_collection_keeps_the_live_set_bounded` - the §8.3 proof in
  default mode: below the threshold nothing collects (16 x 4 KiB), past it a
  2000-iteration loop's safepoints keep the live peak bounded while the total
  keeps growing (the assertion that fails against the old arena), and the one
  rooted survivor is untouched. Stress mode skips the gating phase.
- Budget (§8.7): static GC state is 116 B of globals (4 pointers, 10 `size_t`,
  1 `int`, plus 24 B of shared arena bookkeeping) - ~0.4% of `03` §12's 32 KB
  native budget. Per-collection transients (directory 16 B per live block,
  mark stack 8 B per entry, root array 8 B per frame slot, pins 8 B per pin)
  are freed at the end of the collection; shape tables are emitted per-program
  data, not runtime state.
- Verification-plan status: §8 items 1-7 are now all done; only §8 Tier-1
  (the per-slice runs) remains, per slice.
