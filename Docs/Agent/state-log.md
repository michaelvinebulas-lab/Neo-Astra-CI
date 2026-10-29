# State Log

This is the agent's persistent memory. Update it at the end of every substantial task. Do not rewrite history; append what changed and keep the snapshot current.

---

## Current snapshot

| Field | Value |
|---|---|
| Last updated | 2026-10-29 |
| Current phase | Phase 7 - Backends |
| Current task | GC-001 **Phase 0 landed**: `NS6002` now forbids managed element types in `Memory.alloc<T>`/`region.alloc<T>` (checker rule + spec §8.4 rule line + §14.9 row; code confirmed by the user), completing the condition-2 decision from the approved `GC-001` design (all eleven `[P1]`-`[P11]` choices, amendments applied inline in `Docs/Design/GC-001-proposal.md`, both gates answered in §11). `Region.*` (fixed-block bump arena) and `NS6001` manual-scope enforcement stand. Next implementation step is GC-001 Phase A (block header + shape grammar + allocator split, no collection). Open sub-decisions: `[P9]`'s FFI signature (`11-c-abi.md`), the `Region.*`-scope question, per-case mode selection in the rejects corpus (infra, not decided), and the generic-forwarding instantiation gap (checker model, recorded) |
| Project status | D1-D25 confirmed; compiler frontend, cross-module type surfaces, multi-source compiler API, least-privileged mode inference, effect-provenance diagnostics, typed-HIR Nova interpreter, complete Nova CLI with project module loading/linking, complete NoPM stack, complete JavaScript ESM backend, complete TypeScript ESM backend, first C17 managed slice with hosted runtime, multi-module linking, generics, native libraries/ABI, single-threaded concurrency, path + installed package linking, managed arena memory, manual-memory alloc/free plus pointer access plus the `Region` bump arena, Tier-1 CI workflow, a full C conformance runner, and recorded Tier-1 CI runs on Linux/macOS/Windows; `@script` inline interop complete |
| Source code | `crates/astra-core` (lexer/parser/formatter/resolver/types/mode/checker incl. `Memory`/`Region` builtins), `crates/astra-capabilities` (mode inference and explanation), `crates/astra-nova` (module-aware typed-HIR interpreter and `nova` binary), `crates/astra-compiler-api` (checking and typed-module compile boundary incl. `compile_project` with `import_map`), `crates/astra-codegen-c` (C17 backend, freestanding runtime, managed arena + hosted shim, `Ptr`/manual memory, `NS_Region` bump arena, multi-module emission), `orchestrator/packages` (manifest, nopm, registry, compiler-client incl. `compileProject`, codegen-js, codegen-ts, CLI incl. `linkedPathPackages` and flavor-aware library emission), `.github/workflows/ci.yml` (Tier-1 matrix) |
| Git repository | Yes; initialized 2026-10-25 (baseline `2ce78ad`); `target/`, `dist/`, `node_modules/`, and `.tools/` are ignored |
| Blockers | None. `astra build --target native` links and runs through gcc (MSYS2 UCRT64), and Tier-1 runs are recorded on `voidgod79-debug/Neo-Astra-CI` (all three OSes green). clang alone remains unusable here without the MSVC/Windows SDK, so the MSVC and Apple toolchain paths are CI-verified only |
| Next action | Start GC-001 Phase A: block header + shape grammar (`LEAF`/`ARRAY`/`RECORD`/`VARIANT`) + manual/GC allocator split, **no collection yet** (proposal §7; `holds_managed_refs` from `NS6002` is the shape predicate's frontend twin). Open and recorded, not decided: the `Region.*`-scope design question (should `Region.new`/`region.alloc` require `@memory:manual`?), `[P9]`'s FFI push/pop signature (`11-c-abi.md`, gates only the FFI half), per-case mode selection in the negative-conformance runner (would let `NS6001`/`NS6002` be corpus-pinned), and generic forwarding past `NS6002` (checker checks bodies once against generic placeholders). WASM-001 stays deferred by design. The source mirror is synced at `1199998`; re-sync after each source-touching slice. Push to the `ci` remote after each slice so Tier-1 stays green |

---

### 2026-10-29 - GC-001 Phase 0: `NS6002` forbids managed element types in manual allocation

- Command: `cargo test -p astra-core checker::`, a throwaway inference probe (deleted after use), `cargo fmt --all -- --check`, `npm run ci` (final gate, green: `[ci] all checks passed`), mirror re-sync with verification (`fmt`/`clippy -D warnings`/`cargo test --workspace`/`tsc -b` all green in the stripped tree), then push to `ci` for Tier-1. The user confirmed the code number with "ok do it", so `NS6002` is assigned, not merely proposed.
- Result: condition 2's forbid is enforced ahead of Phase A: a managed element type in `Memory.alloc<T>` or `region.alloc<T>` is now a frontend error, with mode (`NS5004`) -> scope (`NS6001`) -> element type (`NS6002`) reporting order preserved - one program, one violated condition.
- What:
  - **Predicate** `holds_managed_refs` (checker): `str`, `bytes`, `[T]`, slices, and maps are managed (a scalar-payload map too - the buffer is the GC block); `FixedArray`, tuples, `Option`, and `Result` recurse; for `Named` types, `Ptr` and `Region` are never managed (raw address / manual-block handle, per §8.4), `Channel` and `Task` always are, and user structs/enums recurse through substituted fields and variant payloads; `Error`/`Unknown`/`Generic` stay silent because their own diagnostics already fired. This is exactly the inverse of proposal §5.5's shape predicate, so Phase A's tracing machinery can share it.
  - **Mechanism**: a `manual_alloc_spans` registration - `Memory.alloc` records its member span only after the mode and scope gates pass; `region.alloc` records for any `Region`-typed receiver in `check_member`; `check_call` then applies `NS6002` once `T` is bound in the `Ptr<T>` return type. That placement covers explicit type arguments and inference from the binding annotation alike (probe-verified: `let p: Ptr<str> = Memory.alloc(4)` reports `NS6002`, `Ptr<u8>` is accepted), and a callee rejected by an earlier gate types as `Error` and returns before the check can add a second condition. `Ptr<T>` as a *parameter* type remains expressible for the FFI boundary - what closes is only the compiler-visible way to *create* manual memory holding managed references.
  - **Spec**: §8.4 gains the rule line ("`T` MUST NOT contain GC-managed references ... such an allocation is a compile-time error"), §14.9 gains the `NS6002` row plus a paragraph stating the predicate and the check order.
  - **Tests** (56 checker tests green): `rejects_managed_element_types_in_manual_alloc` (7 simple managed forms; a struct/enum/`Wrap<str>` program asserting exactly three `NS6002`s; the region site needing system mode but no manual scope; the inferred-type-argument form), `accepts_pointer_free_element_types_in_manual_alloc` (11 pointer-free forms: scalars, `Ptr<U>`, structs of scalars/pointers, scalar enums/options, `Wrap<i32>`, scalar tuples, `[i32; 4]`, plus `region.alloc<i32>` outside any manual scope), and `ns6002_reports_only_after_mode_and_scope_hold`.
  - **Honest limitations, both recorded in `todo.md`**: (1) no negative-conformance case is possible - the rejects corpus runs at the default core mode, so `NS5004` always fires first (the same reason `NS6001` has no corpus case); per-case mode selection in the runner would close this and is an open infrastructure question, not decided. (2) Generic forwarding (`Memory.alloc<Wrap<T>>` inside a function whose own `T` is still a type parameter) is silent until the element type is concrete at the definition site - the checker validates bodies once against generic placeholders, so this is the existing instantiation model, not a new hole carved for `NS6002`.
  - Proposal §7's Phase 0 row and §11's implementation path now read "landed 2026-10-29" instead of "proposed".
- Verification: full local gate green (`npm run ci`), mirror re-synced to `1199998` "Initial commit" (93 files, 1 root, 0 forbidden paths, strip idempotent at 0 changed after `cargo fmt`, `fmt`/`clippy`/`cargo test --workspace` = 260 tests/`tsc -b` green in the cleaned tree; `npm ci` restored the mirror's absent `node_modules` so `tsc -b` could run at all). Pushed to `ci`; the run for this commit is polled and recorded by the next entry.

---

### 2026-10-29 - `GC-001` design decided: `[P1]`-`[P11]` verdicts applied, both gating conditions answered

- Command: probe tests (`cargo test -p astra-core --test zz_probe`, deleted after each run), reads of `Docs/Design/GC-001-proposal.md`, `parser.rs`, `checker.rs`, `emit.rs`; no gate needed for docs-only edits (last full `npm run ci` green on the `Region` slice commit `cd102a9`).
- Result: the proposal is no longer a proposal. All eleven choices are decided (user, 2026-10-29), the approved amendments are applied inline, and both conditions that gated implementation are answered with evidence. Phase 0 (`NS6002`) and Phase A are unblocked.
- What:
  - **Verdicts recorded in proposal §10** with reasons, verbatim from the user's table: `[P1]`-`[P8]`, `[P10]` approved (several with amendments), `[P9]` concept-approved with signature deferred to `11-c-abi.md`, `[P11]` split out for its own spec review.
  - **Amendments applied inline**, each marked "Amendment (2026-10-29)": `[P1]` re-snapshot after push growth / `&mut` calls / field stores, not just assignment (§5.1); `[P3]` hoisting must preserve `&&`/`||` short-circuit and branch-guarded evaluation (§5.3); `[P4]` document the long-non-looping-function starvation corner and specify `ASTRA_GC_STRESS=1` to collect inside every `astra_alloc` (§5.4); `[P5]` add `VARIANT(tag_offset, shapes[])` and require zeroed payloads or length-aware tracing (§5.5); `[P6]` record the per-collection sort cost and 32 KB budget as the revisit trigger, with ABI change as the approval-gated fallback (§5.6); `[P8]` record unbounded pinning as accepted for V1 with a release API follow-up, and state the returned-pinned / borrowed-not pair together (§5.8).
  - **Naming fix**: §5.1's `astra_roots_count` is now `astra_root_count`, matching the declaration.
  - **Mark stack**: made explicit as an approved addition - a heap-allocated `void*` worklist (no C-stack recursion, since R4 leaves no unwinding recovery), peak counted against the 32 KB native budget (§5.5, §8 item 7).
  - **Root-source statement** (§5.1): the V1 root set is frame slots + boundary pins + FFI registrations, and three candidates are stated as non-existent with probe codes - module-level statics (`NS2005`, declarations only), closure environments (`NS2002`, closure literals exist only as `spawn` operands and spawn is eager-inline), queued spawn arguments (no queue exists).
  - **Growth-then-collect verification test** added to §8 (item 4): push past capacity, force collect, read every element - the case `[P1]`'s amendment exists for.
  - **Condition 1 answered** (§11): tasks run on the caller's C stack; `spawn` is eager and inline (`ExprKind::Spawn` evaluates the block in the caller's frame, stores the result in a `Task` value; no thread/ucontext/setjmp/task queue anywhere in the runtime). `[P1]`'s single array is correct as written and is scoped per execution context so `CONC-001` extends it to one array per task.
  - **Condition 2 answered** (§11) by probe: `Memory.alloc<str>` (+ `p.write(0, "hello")`), `Memory.alloc<[u8]>`, `Memory.alloc<Channel<i32>>`, and `Memory.alloc<Rec>` with `struct Rec { name: str, ... }` are all **accepted today**; `Memory.alloc<Array<u8>>` is rejected only because `Array<T>` is not source syntax (`[T]` is), not by any managed-type rule. Verdict applied: **forbid** managed element types at `Memory.alloc<T>`/`region.alloc<T>`, predicate = inverse of `[P5]`'s shape predicate ("T contains no GC-managed references"), enforcement as proposed `NS6002` (Phase 0) plus one spec §8.4 rule line and a §14.9 row; `Ptr<T>` as a parameter type stays expressible for the FFI boundary. Linked to the deferred `MutSlice` follow-up: any future `Ptr<T>`-producing surface must use the same predicate.
  - **`NS6002`'s number is agent-proposed** (following the `NS6001` precedent in the `NS6xxx` range §14.3 reserves for Memory) and is recorded in `todo.md` as reversible.
  - **Q1 closed** (non-moving mark-sweep is fixed by design §4.1, not by this file); the borrowed-FFI question closed as "released at call end per §8.5; stashing violates §8.5", stated as a pair in §5.8.
- Verification: docs-only slice (`Docs/Design/GC-001-proposal.md`, `Docs/Agent/todo.md`, `Docs/Agent/state-log.md`, `Docs/Agent/c001-handoff.md`); probes were deleted after use; no source change, so the mirror is unaffected. Tier-1: run 15 (`40840682971`, `cd102a9`, the Region slice) and run 16 (`40851465887`, `70f66e1`, this decision-record commit) are both `completed/success` on ubuntu-latest, macos-latest, and windows-latest. The earlier "no run found" poll was a false negative: `credinput.txt` had been overwritten with credential-format text (protocol/host/path), so every API query 401'd inside the poll's `try/catch`; resolved by reading the `ci` credential from Windows Credential Manager (`git:https://github.com/voidgod79-debug/Neo-Astra-CI.git` via `CredRead`, used in-memory, never printed). The API's own `run_number` also corrected a stale label: `40690850075` is run 10, not run 11 as `c001-handoff.md` claimed (fixed in this slice).

---

### 2026-10-29 - `Region.*` manual-memory API landed (fixed-block bump arena)

- Command: `cargo test -p astra-core`, `cargo test -p astra-codegen-c`, `cargo test --workspace`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets`, `npm run ci` (final gate), then push to `ci` for Tier-1.
- Result: the spec §8.4 Region surface is implemented and green end-to-end, including a hosted compile-and-run through the bump allocator; GC-001 Phase A's recorded prerequisite (a decided Region strategy) is now satisfied.
- What:
  - **The decision came first (user):** fixed-block bump arena, chosen over a general free-list allocator because the spec's `region.free()` takes no argument - per-block free would invent language surface. Recorded in `03-runtime-and-memory.md` §5.2.
  - **Runtime** (`runtime.rs`): `NS_Region` is a header-plus-block handle (`base`, `cap`, `offset`) mapped to `NS_Region*` like `Channel`, so every allocation from one handle shares one bump cursor. `astra_region_new` reserves the block on the run arena (freed wholesale at exit if never freed explicitly); `astra_region_alloc` carves 8-byte-aligned (covered by the strict-aliasing-safe mask expression) and panics on exhaustion - the same V1 stance as `astra_alloc`'s OOM panic; `astra_region_free` releases the block and zeroes the header so a double-free is a no-op (spec §8.4 makes double-free a programmer bug).
  - **Emitter** (`emit.rs`, `ty.rs`): `Region.new(bytes)` is a static-path arm like `Memory.alloc`; `region.alloc<T>(n)`/`region.free()` are `method_c` receiver arms. `method_c` gained the full call expression as a parameter (one call site, threaded through) because `region.alloc<T>`'s element type lives on the call - the receiver `Region` carries no type arguments, unlike `Channel<T>` or `Ptr<T>`.
  - **Checker** (`checker.rs`): `Region` is a non-generic builtin type (`Region<T>` is a type-argument error, with its own hint); `Region.new` and the `region.alloc`/`region.free` members all carry the `NS5004` system-mode gate, mirroring `Memory.*` and `ptr.*`. The spec's scope rule names only `Memory.alloc`, so Region use needs system mode but no `@memory:manual` block - implemented literally per rule 10, with the question "should it require the scope too?" recorded as an open design question in `todo.md` rather than decided here.
  - **Conformance**: `conformance/rejects/region_api_requires_system` (`.ns` + `.codes` = `NS5004`) pins the gate backend-neutrally; reject counts corrected to 19 positive + **13** negative across `todo.md`, `c001-handoff.md`. Discovery was proven by temporarily writing a wrong expectation (`NS9999`) and confirming the runner failed on the missing code before restoring `NS5004`.
  - **GC-001 unblocked**: the proposal's §9 finding 2 (Region unimplemented) is marked resolved; Phase A can start once its `[P1]`-`[P11]` choices are approved.
- Tests: `checker::tests::region_api_type_checks_in_system_mode` (system-mode flow, annotation form, `Region<u8>` rejected), `checker::tests::rejects_region_api_outside_system_mode` (`Region.new`, `region.alloc` via a `Region` parameter, and `region.free` each give `NS5004` in core mode), `emit_c::region_bump_arena_lowers_and_runs` (structural + hosted run printing 20/22/100: two carvings are distinct, writes stay stable until `region.free()`), `emit_c::region_api_requires_system_mode_in_checker`, and the conformance rejects table (13 cases).
- Boundary: no spec change was needed - §8.4's table already carried the three signatures. No JS/TS/Nova backend change: manual memory is native-only and the frontend `NS5004` gate covers the other backends, exactly as for `Memory.*`/`ptr.*`. `! AllocError` stays nominal for the Region family (the frontend tracks no error effect, the C backend panics on failure), consistent with `Memory.alloc` until the fallible entry-point policy lands for both.

---

### 2026-10-29 - `C-001` spec gap closed: `Memory.alloc` outside `@memory:manual` is now `NS6001`

- Command: throwaway probe test, then `cargo test -p astra-core checker::`, then `npm run ci` (green twice, second run after the last test was added)
- Result: the probe confirmed the gap was real and user-visible; after the change the full local gate is green.
- What:
  - **Probe first, not assumption.** A temporary test printed ACCEPTED/REJECTED for `Memory.alloc` inside versus outside `@memory:manual` under system mode: outside came back **ACCEPTED**, so spec §8.4's "Memory.alloc in a GC-managed scope is a compile-time error" was genuinely unenforced, not merely absent from the code paths I had grepped. The probe file was deleted once the permanent test existed.
  - **The code is a decision, not a mechanical fix.** The spec declares `NS6xxx | Memory` in §14.3 but has never assigned a single code in it, and the existing codes mean other things (`NS5004` = "Manual memory requires system mode"; §14.3 forbids reusing a code for a different meaning). Approved by the user as `NS6001` "Manual allocation outside a manual-memory scope", added as a new spec §14.9 with a sentence recording why the mode condition reports first.
  - **Enforcement** (`check_member` in `crates/astra-core/src/checker.rs`): `Memory.alloc` reports `NS6001` when its span lies outside every recorded manual block, mirroring the existing mode gate (which runs first and bails with `Type::Error`).
  - **Span semantics fixed along the way.** `check_scope_annotation` used to record only the `@memory:manual;` header span, which can never contain the block's body - a containment test against it would have rejected everything. It now records the whole `{@memory:manual; ...}` block (`scoped.span`), which also makes an allocation made *after* a block closes correctly fail. Nova's `NV0001` still fires, because its tests assert the code and nothing pinned that span.
  - **Ordering proven, not assumed.** `run()` calls `check_mode_annotations()` before `check_functions()`, so an item-level `{@memory:manual; fn ...}` block is recorded before its bodies are type-checked; a test pins that so a future reorder fails loudly.
- Tests: `rejects_manual_alloc_outside_a_manual_scope` covers four cases - outside any block gives `NS6001`, allocation after a block closes gives `NS6001`, inside a statement-level block is accepted, inside an item-level block is accepted, and core mode gives `NS5004` **only** (one program, one violated condition).
- Boundary: only `alloc` is scoped because §8.4 names only `alloc`; `Memory.free` and `ptr.*` have no scope rule in the spec, so none was invented for them. There is no conformance reject case: the rejects runner uses `CheckOptions::default()` (core mode), where the mode condition fails first and `NS6001` is unreachable - manual memory has never had corpus coverage, since the corpus is a backend-neutral core-profile suite. Pinned in the checker instead, and the reason is recorded in `todo.md` so the absence does not look like an oversight.
- Verification: full `npm run ci` green locally, twice (the second run after the item-level test was added). Tier-1 run 13 on `46a417c` green on Ubuntu, Windows, and macOS. The source mirror was re-synced to `730b508` "Initial commit" (93 files, 1 root commit, 0 em dashes, no forbidden paths, strip idempotent at 0 changed); `checker.rs` is the only mirror-eligible file this slice touched, and in the stripped tree `tsc -b`, `cargo fmt --check`, `clippy -D warnings`, and `cargo test --workspace` (151 core tests, up from 150) are all green, with `cargo fmt --all` again needed after stripping to collapse the blank runs left where comments were removed.

---

### 2026-10-29 - `GC-001` design note: precise-roots proposal (documentation only)

- Command: read `03-runtime-and-memory.md` §4/§12/§14, `04` §6.2, `11-c-abi.md` §5, `Docs/spec-v0.1.md` §8, then verified each claim against `crates/astra-codegen-c/src/runtime.rs` and `src/emit.rs` line by line. No code was run or changed, so no gate was needed (last full `npm run ci` green immediately before `e112ac4`).
- Result: `Docs/Design/GC-001-proposal.md` written and committed. Nothing implemented, no spec text changed, no D-decision touched.
- What: the design baseline fixes the *what* (non-moving mark-sweep, precise roots from compiler-generated stack maps, task-local heaps, allocation-pressure triggering, a 32 KB runtime budget) but not the *how*, and it is silent on tracing through objects. The proposal supplies a mechanism and tags every non-authoritative choice `[P]` for approval, per `agent.md` rule 10 ("Do not invent decisions").
  - **Grounding pass** established the constraints from code, not from intent: managed memory is a per-run arena freed only at exit (`runtime.rs:234-318`, called from `emit_main`); manual and managed share that one allocator; values are by-value fat structs so only `data`/`keys`/`values` buffers are heap objects; `str`/`bytes` slices are **interior views** (`runtime.rs:374`, `431`, `460`, `504`); `args_c` emits arguments inline (`emit.rs:2185`), leaving already-computed subvalues in unnamed C temporaries under unspecified evaluation order; and there is no unwinding, so every return path is explicitly emitted and root release can be emitted rather than unwound.
  - **The mechanism proposed**: eager *value* snapshots (not addresses of locals) in one contiguous per-frame region of a per-task array, snapshotted at every managed assignment; type-directed payload extraction so a slot holds a heap block pointer (`str.data`, `arr.data`, tag-conditional for `Option`/`Result`); the invariant that every managed subvalue is materialised into a snapshotted temporary before any call; safepoints at function entry and loop back-edges so allocation never collects mid-expression; a block header plus a `LEAF`/`ARRAY`/`RECORD` shape grammar (necessary because V1 has no reflection, `01-language-design.md:698`, and the design says nothing about object tracing); interior roots resolved against a block directory sorted once per collection, which keeps `s.slice()` O(1) and avoids an ABI change; separate manual and GC allocators; and boundary-returned values pinned for the run so `11-c-abi.md` §5's "valid until process exit" contract stays literally true instead of being broken by collection.
  - **Why snapshots rather than a shadow stack of addresses**: no local has to be hoisted or zero-initialised, the table can `realloc` while frames are live without invalidating anything, and a stale snapshot is unobservable because collection cannot occur between an assignment and its adjacent snapshot store.
  - **Verification plan**: `ASTRA_GC_STRESS=1` collecting at every safepoint across the full gate, all 19 positive and 12 reject conformance cases, and every hosted `emit_c` run; poison-on-free (`0xDD`) as a use-after-free oracle; and a peak-live-blocks test that would *fail* against today's arena, since `astra_arena_live_count()` cannot fall mid-run today.
  - **Eleven decisions for approval**, `[P1]`-`[P11]`, each with the rejected alternative recorded (notably: collecting inside `astra_alloc` is also correct once the materialisation invariant holds, so the safepoint choice is controllability, not correctness; and materialising managed arguments would pin left-to-right argument evaluation, which the spec does not currently state - `[P11]`).
- Findings from the same pass, recorded as new `todo.md` items rather than folded into the proposal: spec §8.4's "Memory.alloc in a GC-managed scope is a compile-time error" appears unimplemented (`manual_memory_scopes` has no diagnostic reader), and `Region.new`/`region.alloc`/`region.free` have no implementation reference anywhere. The first is stated as static evidence needing a probe test, not as confirmed user-visible behaviour.
- Boundary: the note is explicitly non-authoritative; JS/TS backends and Nova are untouched (host GC); Wasm sequencing is a tagged decision, not an assumption; and no `.ns` syntax, type, or diagnostic changes.

---

### 2026-10-28 - `C-001` slice: `Ptr` pointer access (`ptr.read`/`ptr.write`)

- Command: `cargo test -p astra-core`, `cargo test -p astra-codegen-c`, `npm run ci`
- Result: 83 emit tests (up from 80 with three new ones), full local gate green
- What: spec §8.4's manual-memory pointer surface, minus the part the spec leaves open.
  - **Checking** (`builtin_method` in `astra-core/src/checker.rs`): `Ptr<T>` gains `read(index: usize) -> T` and `write(index: usize, value: T) -> ()`, mirroring how `Task`/`Channel` methods are declared. Neither is fallible, matching the spec's signatures; `write` types its value against `T`, so `p.write(0, true)` on a `Ptr<u8>` is a normal argument error rather than a reinterpretation.
  - **Mode gate**: raw pointer *usage* is what makes a file `@requires(system)` (06-tooling-and-workflow.md line 42), so `member_type` now applies the same `NS5004` gate to `ptr.read`/`ptr.write` that the `Memory` namespace applies to `alloc`. Without it, a core-mode function that merely received a `Ptr` parameter could read raw memory. This also let a dead duplicate dispatch block (`matches!(name, "Task"|"Channel"|"Ptr"|"MutSlice")`, unreachable behind the general `builtin_method` call) be deleted.
  - **Lowering** (`method_c` in `astra-codegen-c/src/emit.rs`): `Ptr<T>` already maps to `T*`, so `ptr.read(i)` is `(ptr)[i]` and `ptr.write(i, v)` is the statement `ptr[i] = v`, using the same `self.pre` statement-buffer pattern as `Channel.close`. Unchecked by design - a manual pointer carries no length, so there is nothing to bounds-check against, which is also what C gives us for free.
  - **Why no `as_slice`**: spec §8.4 lists `ptr.as_slice(count) -> MutSlice<T>`, but `MutSlice` appears exactly twice in the whole spec - once as a type name and once in that row. No methods, no field surface, no iteration or indexing contract. Implementing it would have meant inventing the language, so it is left open and documented in `todo.md` and the handoff.
  - **Spec divergence left open**: §8.4 declares `Memory.alloc` fallible (`Ptr<T> ! AllocError`), but the landed alloc is infallible (arena alloc, panics on OOM). Making it fallible is not mechanical: `!` propagates into the enclosing function's error set, and `emit_main` writes `(int)main()` with no notion of an error effect, so a fallible `main` - the natural home for a manual-memory block - has no specified native behavior. That is a policy decision (unwrap-and-panic entry, `alloc_or_panic`, or a native `NV0004`-style report), recorded in the handoff rather than invented.
- Tests: `manual_memory_pointer_access_runs` (emits, compiles through gcc, runs, and checks that `20 + 22` observed through the pointer prints `42`), `pointer_access_requires_system_mode_in_checker` (both `read` and `write` rejected with `NS5004` in core mode), `pointer_write_checks_the_element_type`, and the backend-neutral reject case `conformance/rejects/pointer_access_requires_system` (`.codes` = `NS5004`), which the dynamic rejects runner picks up and validates for a matching `.codes` file.
- Verification: `cargo test -p astra-core` (303 tests) and `-p astra-codegen-c` (83 + 1) green; full `npm run ci` green. Tier-1 run 8 on `296a4c4`: Ubuntu and Windows green, macOS came back `cancelled` after exactly 15 minutes with no steps logged (the run had sat `queued` for ~9 minutes first) - a macOS runner provisioning timeout, not a code failure. Re-running run 8 turned all three jobs green, so the slice is Tier-1 verified on Linux, macOS, and Windows. The source mirror was re-synced to `2cc2e51` "Initial commit" (93 files, 0 em dashes); the stripped tree needed `cargo fmt --all` because comment removal leaves blank runs at the old file heads, after which `tsc -b`, `cargo fmt --check`, `clippy -D warnings`, and `cargo test --workspace` are green in it.
- Boundary: the wasm backend implements no manual memory at all, so `ptr.*` is C-only today - consistent with the landed state, not a new gap. The Wasm half of "native or Wasm" is a `WASM-001` item. `ptr.as_slice` waits on a `MutSlice` design decision.

---

### 2026-10-28 - Tier-1 CI evidence recorded: all three OSes green (run `40677187427`)

- Command: `npm run ci`, `git push ci master`, GitHub Actions API (GCM token) for run/job/log queries
- Result: run `40677187427` on `bc5d1c9` = `completed/success` on `ubuntu-latest`, `macos-latest`, and `windows-latest`; full local gate green before each push
- What: the full-fidelity CI remote (`voidgod79-debug/Neo-Astra-CI`, created by the user, pushed from `C:\Projects\Neo Astra` with `master` as default) produced its first recorded Tier-1 runs. Reaching a fully green run took four pushes, each exposing the next real blocker:
  1. `2206f4e` → run `40655262547`: all three OSes failed in `npm test` — `codegen-ts` (and the stress suites) spawn `target/debug/astra-compiler`, but `scripts/ci.mjs` ran `npm test` before any cargo build, so a fresh runner hit ENOENT and reported `status: null`. Locally invisible because `target/` was warm. Also found that `npm test` covered only 4 files (49 tests) while `stress_compiler_api`/`stress_nopm` were in **no** npm script and never ran anywhere. Fix `1811ce1`: a `pretest` hook builds the binary (also repairs `npm run verify`) and `npm test` now runs 6 files (111 tests).
  2. `1811ce1` → run `40670278321`: TS tests passed (111/111 on Ubuntu), then `test:boundary` died with `E0463 can't find crate for std` — the workflow installed Rust with only rustfmt+clippy. Fix `fb51841`: `targets: wasm32-unknown-unknown` on the toolchain step.
  3. `fb51841` → run `40671917569`: **Ubuntu green** (first recorded Linux native verification, conformance corpus included); macOS failed `--emit staticlib` (`NS9008 No archiver found`) because `findArchiver` probed with `--version` and Apple's `ar` rejects GNU long options; Windows failed `--emit cdylib` with `LNK1181` because the runner's clang targets `windows-msvc` and link.exe treated `--export-all-symbols` as an input file. Fix `32b0ef5`: archiver candidates are now *attempted* with the real `rcs` operation (failures carry each candidate's stderr), and `compilerFlavor()` reads `-dumpmachine` — an MSVC triple writes a `.def` from the generated headers' `/* Exported function prototypes. */` section and links with `/DEF` + `/IMPLIB` (`libmathlib.lib`), while GNU-flavored Windows keeps the old flags.
  4. `32b0ef5` → run `40676108614`: **Windows green** (the MSVC path worked first try), but the cdylib test now asserted an import library on every platform, and ELF/Mach-O builds have none. Fix `bc5d1c9`: the existence check runs only on `win32`.
- Verification: run 5 (`40677187427`, `bc5d1c9`) `success` on all three jobs; each fix also passed the full local `npm run ci` first (56 boundary tests, 111 TS tests, fmt/clippy/`cargo test`). Run 6 (docs commit `7a644ce`) also `success` on all three OSes, so the gate is green on two consecutive pushes rather than a single lucky run. The source mirror was re-synchronized to `78c922b` "Initial commit" (93 files, 0 em dashes, comment-stripped `index.ts` still carries the `EXPORTS_MARKER` string literal the MSVC `.def` parser needs); it typechecks (`tsc -b`), and `cargo fmt --check`/`clippy -D warnings`/`cargo test --workspace` are green in the cleaned tree.
- Boundary: the MSVC `.def`/`/IMPLIB` path and the Apple `ar` path are CI-verified only — this host has neither a Windows SDK for clang nor Apple's toolchain, so local runs stay on gcc/MSYS2 (gnu flavor). The `ci` remote receives full history including `Docs/`, `tests/`, and `.github/`; the source-only mirror stays clean and guarded.

---

### 2026-10-28 - Correction: Tier-1 CI evidence needs a second, full-fidelity remote (not a push to the mirror)

- Command: `git ls-tree -r --name-only origin/master` (source mirror), doc reconciliation
- Result: docs corrected in `todo.md` and `c001-handoff.md`; mirror re-synced to `master = cae0c46` as commit `6ce9f39`
- What:
  - **Finding**: "recorded Linux/macOS CI evidence is blocked only on pushing to a remote" was wrong. The source-only mirror (`luminousvalentine-cell/Neo-Astra`) intentionally carries no `.github/`, `scripts/`, `tests/`, or `conformance/` — so pushing it never triggers the Tier-1 workflow (no workflow file exists on the remote), and the pre-push guard actively enforces those exclusions. The blocker is the mirror policy, not infrastructure timing.
  - **Decision (user: option A)**: create a **second, full-fidelity CI remote** that receives the whole repo (tests, conformance, `scripts/`, `.github/`) from `C:\Projects\Neo Astra`; the source mirror stays clean and is still guarded. Item 4 of the C-001 handoff moves from "waiting on a push" to "blocked on the CI remote not existing yet".
  - **Mirror sync**: re-snapshot from `cae0c46` (93 files, comment-stripped, em-dash-free), verified green in the cleaned tree (`cargo fmt --check`, `clippy -D warnings`, `cargo test --workspace`, `tsc -b`, 154 JS/TS tests — up one from the new codegen-js test), force-pushed as `6ce9f39` "Initial commit" through the pre-push guard.
- Verification: remote = single branch `master` (default), single commit, 93 files, 0 em dashes, no forbidden paths; hook blocks a simulated polluted push (main history) with the expected reasons.
- Boundary: the CI remote cannot be created from this host yet — `gh` is not authenticated, so repo creation stays with the user (same as the mirror repo). The Tier-1 workflow itself is written and correct; it needs the full-fidelity remote plus a first push to produce recorded runs.

---

### 2026-10-28 - `C-001` slices: non-path packages, Tier-1 CI workflow, managed arena, manual memory

- Command: `cargo test -p astra-codegen-c`, `cargo test -p astra-compiler-api`, package boundary `node --test`, `npm run ci`
- Result: 80 emission tests (up from 77 with the new arena/manual-memory tests), full `npm run ci` green (fmt, clippy, JS/TS tests, workspace Rust tests, conformance runner)
- What:
  - **Non-path package adapters**: `linkedPathPackages` now also resolves installed packages by joining `core.lock.json` (name → id) with `.astra/store/index.json` (id → root); git/registry `.ns` packages feed the same `deps/<name>/` discovery and import map. Foreign (non-Astra) installed packages are skipped, not fatal.
  - **Tier-1 CI workflow** (`.github/workflows/ci.yml`): runs `npm run ci` on ubuntu/macos/windows so the hosted compiler is present and the conformance corpus + native build execute on Tier-1 platforms.
  - **Managed arena (GC-001 managed-mode first step)**: `astra_alloc` records blocks; `astra_free`/`astra_realloc` keep the tracker in sync (array/map growth); emitted `main` calls `astra_runtime_shutdown` to release the run's arena at exit. `RUNTIME_HEADER` declares `astra_free`/`astra_runtime_shutdown`.
  - **Manual-memory first cut**: `Memory.alloc<T>(n) -> Ptr<T>` / `Memory.free(p)` checker builtins (system-mode gated — `NS5004` in core), `Ptr<T>` → `T*`, lowering to `astra_alloc`/`astra_free`; `NS8012` removed. Also fixed `emit_scoped_block`, which wrongly force-returned a scoped block's last statement (broke statement-position `@memory:manual`).
- Tests: `managed_arena_is_released_at_shutdown`, `allocation_heavy_program_runs_clean_after_shutdown`, `manual_memory_alloc_free_lowers_to_arena`, `manual_memory_requires_system_mode_in_checker`, and an installed-package CLI boundary test.
- Verification: full `npm run ci` green after each slice; arena + manual memory verified end to end through gcc.
- Boundary (honest handoff): `Ptr` dereference/indexing, precise mark-sweep GC with stack maps (`GC-001`), and OS-threaded concurrency (`CONC-001`) are design-gated and deliberately not invented here; recorded Linux/macOS CI evidence needs a push to a remote.

---

### 2026-10-28 - `C-001` slice: manifest path packages link into the C backend

- Command: `cargo test -p astra-compiler-api`, `cargo test -p astra-codegen-c`, `node --test --test-name-pattern "path package" orchestrator/tests/codegen-js.test.mjs`, `npm run ci`
- Result: 32 compiler-api tests (new import-map test), 77 emission tests, and an end-to-end `astra build --target native` fixture that links a path dependency and prints 42
- What:
  - **Import map**: `ProjectCompileRequest.import_map` (JSON `importMap`) maps source specifiers to canonical module keys; `resolve_project_target` consults it before identity/relative resolution, so `import { add } from "mathlib"` binds to `deps/mathlib/src/lib.ns`.
  - **CLI package discovery**: `linkedPathPackages` reads manifest `dependencies` with `source: "path"` (honours `targets` filtering), loads the package manifest (`nova.entry`), reuses `discoverProjectModules`, and re-keys modules under `deps/<sanitised name>/`; package modules are prepended so the importer stays the entry module.
  - **Include-guard fix**: `RUNTIME_HEADER` is wrapped in `#ifndef NS_RUNTIME_HEADER` — a dependency header re-including it previously produced conflicting anonymous-struct typedefs, latent because no test compiled a multi-module project as separate TUs (`assert_project_compiles` now does).
- Tests: compiler-api `compile_project_resolves_package_import_map`; CLI boundary test `astra build links a path package through a bare import`; `assert_project_compiles` pins multi-module TU compilation.
- Verification: full `npm run ci` green; the fixture binary prints `42` with exit code 0.
- Boundary: only `source: "path"` is wired; git/npm/cargo adapters (NoPM store) and package-of-package dependencies are not resolved; package-module diagnostics attribute spans to the entry file.

---

### 2026-10-27 - `C-001`/`CONC-001` slice: single-threaded concurrency and a full C conformance ledger

- Command: `cargo test -p astra-codegen-c`, `npm run ci`
- Result: 77/77 emission tests; all 19 corpus cases now lower, link, and run; full CI green
- What:
  - **Channel runtime**: `Channel<T>` maps to a bounded `NS_Channel*` (capacity 16) with value-copy messages, `astra_channel_new/send/receive/close`, and a shared `NS_ChannelError` payload. `send`/`receive` return `Result<_, ChannelError>` (`send` fails when closed, `receive` fails on a closed+empty channel); a full open channel or empty open channel panics with the JS/Nova deadlock message, and buffered messages stay receivable after `close`.
  - **Eager spawn/Task**: `spawn || { ... }` runs the block immediately (the V1 portable runtime is single-threaded) and stores its value in `NS_Task_<T>`; `Task.join()` returns the value and `Task.cancel()` marks it. Typedefs are emitted on demand like `Option`/`Result`.
  - **Typing/emission plumbing**: `ty.rs` special-cases `Named("Channel")`/`Named("Task")` before the generic-type reject, `call_c` unwraps the `TypeArguments` callee wrapper so `Channel.create<str>()` can use the call's own `Channel<str>` annotation, and `effect_error_type` recognizes channel `send`/`receive` so `!`/match materialize `ChannelError`.
  - **Corpus**: `channels` leaves `KNOWN_GAPS`; the C ledger is empty and `CONF-001`'s C runner now executes the whole suite.
- Tests: `channels_and_eager_spawn_run_the_corpus_semantics` (structure + hosted execution) and `spawn_returns_an_eager_task_that_joins`; the conformance test asserts every emitted case runs when a hosted compiler is present.
- Verification: full `npm run ci` green; `channels` output matches `sent / hello / world / closed send`.
- Boundary: no OS-thread scheduler, cancellation safepoints, task-local heaps, or cross-task transfer yet; the deadlock rule panics instead of yielding, matching the V1 single-threaded JS/Nova model.

---

### 2026-10-27 - `C-001` slice: `Array.get`, native library output, and provisional C ABI

- Command: `cargo test -p astra-codegen-c`, `node --test --test-name-pattern "staticlib|cdylib" orchestrator/tests/codegen-js.test.mjs`, `npm run ci`
- Result: 75/75 emission tests plus the conformance gate; add a staticlib and cdylib to the boundary suite; full CI green
- What:
  - **`Array<T>.get(i)`**: the checker already exposed it but the C emitter had no arm, so it fell through the generic method path. It now evaluates the receiver/index once, guards the bounds, and builds `NS_Option_T` (`some`/`value`) instead of using the panicking index path.
  - **Library emission**: `astra_codegen_c::EmitKind::{Executable, Library}` and `generate_project_with_kind` suppress the `int main(void)` wrapper while keeping `pub` functions external. `ProjectCompileRequest.library` (additive, serde-defaulted) and `CompilerClient.compileProject` carry it; the CLI forces the project path for library builds even with one module.
  - **Artifacts**: `astra build --emit staticlib|cdylib|library` (or target `emit` config). Objects are compiled per translation unit with the selected compiler; a static library is archived with the compiler's own `ar`/`llvm-ar`/`gcc-ar`; a shared library is linked with `-shared` (plus `--export-all-symbols` and an import library on Windows). No hand-written archive format.
  - **ABI**: new `Docs/Design/V1/11-c-abi.md` records symbol naming (`ns_<stem>_<name>`), per-type layouts, builtin error payloads, ownership (managed-until-exit, borrowed views), the static-vs-shared runtime linking rule, and a consumer example. It is explicitly provisional under design §3.2.
- Tests: `array_get_returns_bounded_option` (structure + hosted execution), `library_emission_omits_main_and_exports_public_functions` (no main, external symbol, header), and two CLI boundary tests that build a library, compile a hand-written consumer, and run it (static archive and shared library, self-skipping without a hosted toolchain).
- Verification: full `npm run ci` green; manual consumers printed `42 42` against both `libmathlib.a` and `libmathlib.dll`.
- Boundary: static archives exclude the runtime shim by design (consumers link `astra_host.c`); the ABI is provisional; GC/manual memory, concurrency, and package linking remain.

---

### 2026-10-27 - `C-001` slice: hosted runtime shim fix and native conformance execution

- Command: `cargo test -p astra-codegen-c`, `node --test --test-name-pattern native orchestrator/tests/codegen-js.test.mjs`, `npm run ci`
- Result: 73/73 emission tests plus the conformance gate; the gate now links and runs all 18 executable cases; full CI green
- What:
  - **Shim was not a translation unit**: `astra_host.c` used `NS_Bytes`, `NS_Map`, and the builtin error payloads without declaring them, and used `PRId*`/`INFINITY` without `<inttypes.h>`/`<math.h>` (51 gcc errors). `runtime::host_shim()` now prepends `RUNTIME_HEADER`, `HOST_SHIM` is documented as the body, and `GeneratedC::host_shim` returns the complete unit. Reusing the header's typedefs avoids the incompatible anonymous-struct redeclarations a naive prepend produced.
  - **Toolchain probing**: clang on this Windows host has no MSVC/SDK headers, so `gcc` (MSYS2 UCRT64) is the working compiler. The Rust helpers and `astra build --target native` now probe each candidate with a real hosted compile/syntax check (clang, clang, gcc, cc) instead of selecting the first binary that answers `--version`.
  - **Runtime conformance gate**: `crates/astra-codegen-c/tests/conformance.rs` links every emitted case against the shim, runs it, and compares stdout plus `RETURN=<code>` with the corpus `.expected` file, in addition to the existing freestanding compile check.
  - **Bug caught by the gate**: match-arm block statements were parked in `pre`; the next arm's flush emitted them into the wrong branch, so `99 => { break }` ran in the wildcard arm and `early_exit` printed `-1` instead of `3`. `emit_arm_body` now flushes `pre` inside the arm's braces, and `emit_block_tail_in_arm` flushes before writing the arm value.
- Tests: new `host_shim_is_a_complete_hosted_translation_unit` (asserts the shared declarations/includes and compiles the shim standalone) and `match_arm_statements_stay_inside_their_branch` (structure plus execution); new boundary test `astra build --target native links and runs a hosted binary` (self-skips when no hosted compiler is present).
- Verification: `npm run ci` green; manual `astra build --target native` produced `3 / -1 / -1` for the `early_exit` body.
- Boundary: this host now verifies native execution through gcc; Linux/macOS still need a Tier-1 CI run. Library output/ABI, package linking, GC/manual memory, and concurrency remain open.

---

### 2026-10-27 - `C-001` slice: generic function monomorphization

- Command: `cargo test -p astra-codegen-c`, `npm run ci`
- Result: 71/71 emission tests plus the static conformance gate; full CI green
- What: local generic functions are now monomorphized per call site, closing the last non-concurrency corpus gap for C.
  - **Substitution** (`ty.rs`): `TyCtxt` holds `subst` (type parameter to concrete type) with `set_subst`/`clear_subst`/`substitute`; `c_type` and `ident` resolve both `Type::Generic(name)` and bare `Type::Named(name, [])` through it. The checker represents type parameters as `Type::Generic`, which the emitter previously rejected with `NS8018`.
  - **Discovery** (`emit.rs`): a call to a local generic function unifies the declared parameter types with the call's argument types (`bind_type_params`, recursing through `Option`/`Result`/`Array`/`Slice`/`Map`/`Tuple`/named arguments; the active substitution is applied to the argument type first, so generic-to-generic calls work), registers a deduped instantiation in a `BTreeMap`, queues its body, and emits the mangled symbol (`ns_<fn>_<type args>`, e.g. `ns_identity_i32`). Uninferrable calls reject `NS8018`.
  - **Assembly**: `emit_program`/`emit_program_linked` emit constants, regular bodies, and `main` into a scratch buffer, drain the instantiation queue until no new ones appear, then write the runtime header, complete typedefs, prototypes (regular + instantiations), script prototypes, and the code. `emit_function`/`emit_fn_proto` gained `_named` variants; `collect_decls`/`prescan_fn` skip generic declarations; `mentions_unresolved` treats `Type::Generic` as a placeholder.
- Corpus: `generics` leaves the `KNOWN_GAPS` ledger in `crates/astra-codegen-c/tests/conformance.rs`; the static gate now emits 18 of 19 cases (`channels` remains, `CONC-001`).
- Tests: new `generic_functions_monomorphize_per_instantiation` pins the three instantiations (`i32`, `Str`, `bool`), the specialized bodies, and a generic-to-generic forwarded call; clang compiles the output.
- Verification: full `npm run ci` green immediately before this entry.
- Boundary: exported generic functions are not exposed in headers; cross-module generic calls are not instantiated in the importer; generic bodies that dispatch on `T` beyond type mapping (method calls on `T`, etc.) remain out of scope.

---

### 2026-10-27 - Docs: C-001 handoff + host-shim defect recorded

- Command: none (documentation only); tree at `47d2598`
- What: added `Docs/Agent/c001-handoff.md`, the transfer document for the C-001 workstream: repo orientation, C backend file/function map, the 70-test + static-conformance baseline, a feature inventory, architecture invariants (typedef ordering, `pre` flushing, fallible-call materialization, builtin nominals, cross-module types), per-item plans for everything still open (generics, package linking, GC-001, CONC-001, native library ABI, native verification/CI, WASM-001 with its deferral rationale), the slice workflow, environment quirks, known small gaps, open questions, and a quick index. `todo.md` now points at it from the C-001 section.
- Finding recorded while writing it: `astra_host.c` is written raw by the CLI, but `HOST_SHIM` uses `NS_Bytes`/`NS_Map` from `RUNTIME_HEADER` while typedefing only `NS_Str`/`NS_Array` — a latent hosted-build compile defect hidden by the missing-toolchain gate. Fix (prepend the runtime header or include a generated header) is the first item to verify on a Tier-1 runner.
- Next: generics monomorphization (handoff 6.A), per the user's direction; update `todo.md` as each item completes.

---

### 2026-10-27 - `C-001` slice: static conformance gate, bytes writes, and accessors

- Command: `cargo test -p astra-codegen-c`, `npm run ci`
- Result: 70/70 emission tests plus the new conformance gate; full CI green
- What:
  - **Static gate**: `crates/astra-codegen-c/tests/conformance.rs` lowers every `conformance/cases/*.ns` case and compiles the generated C freestanding with clang, behind an explicit `KNOWN_GAPS` ledger (`channels` -> CONC-001, `generics` -> C-001 monomorphization). On its first run it caught that `bytes_buffer` emitted invalid C and that `collections`/`options` could not be lowered at all.
  - **Bytes writes**: `buf[i] = v` fell through the generic array lvalue (`astra_array_index` over an `NS_Bytes*`, element size `int32_t`). It now calls the new bounds-checked `astra_bytes_set(&buf, i, (uint8_t)v)`; the guard keys on the indexed value's type (`bytes`), not the index expression's (`u8`).
  - **Accessors**: `Option::{unwrap, unwrap_or}` and `Result::{is_ok, is_err, unwrap, unwrap_or, error}` lower to one evaluated temporary plus a panic guard, with `unwrap_or`'s fallback kept inside the conditional so it stays lazy.
  - **Contextual `None`**: bare `None` is annotated `Option<Unknown>`, so `let`, `return`, and match-arm value positions now carry an expected type (`expr_with_expected`) that materializes the concrete Option. The match dispatchers (`match_literal`, `match_enum`, `match_struct`, `match_tuple`) thread the match's result type instead of hardcoding `Unit`. Prescan skips annotation types that still contain `Unknown`/`Never`/`Error`/Function placeholders, which previously rejected valid programs such as `find(false).unwrap_or(0)`.
- Tests: `test_bytes_indexing_and_methods` pins the write helper and `bytes.new`; the new `option_and_result_accessors_lower_to_checked_reads` compiles every accessor path with clang; the new `conformance.rs` gate emits 17 cases statically with 2 ledgered gaps.
- Verification: full `npm run ci` green immediately before this entry.
- Boundary: the new runtime helpers are still unexecuted on this host (native toolchain gate); `generics` is the only non-concurrency corpus gap left for C.

---

### 2026-10-27 - `C-001` slice: UTF-8 scalar views and `bytes.new`

- Command: `cargo test -p astra-codegen-c`, `npm run ci`
- Result: 69/69 in `astra-codegen-c`; full CI green
- What: the C backend gains the remaining spec §6.2 `str` members — `char_count`, `byte_at`, `char_at`, `bytes()`, `chars()` — plus the `bytes.new(length)` builtin, which is what the `strings_utf8` and `bytes_buffer` conformance cases exercise.
  - **Runtime** (`runtime.rs`): `astra_utf8_width` identifies a scalar's byte width; `astra_str_char_count` and `astra_str_char_at` walk UTF-8 scalars (invalid/truncated lead bytes count as one byte, defensively); `astra_str_bytes` copies the byte view into an `NS_Array` of `uint8_t`; `astra_str_chars` pushes each scalar as an `NS_Str` slice; `astra_bytes_new` calloc-allocates a zero-filled buffer.
  - **Emitter** (`emit.rs`): `byte_at` lowers to an `Option<u8>` ternary over the byte index, `char_at` to an `Option<str>` built from the scalar helper's verdict, and `char_count`/`bytes`/`chars` to single helper calls. `bytes.new(n)` is recognized only for the unshadowed builtin namespace: a local or imported `bytes` carries a type annotation, so it never reaches the `(None, "new")` arm.
- Tests: new `str_scalar_and_sequence_methods_lower_to_runtime_helpers` asserts every helper, both `Option` typedefs, and the allocation helper, and compiles the output with clang.
- Verification: full `npm run ci` green immediately before this entry.
- Boundary: the hosted shim (where the new helpers live) is not compiled on this Windows host — native linking still waits on the toolchain gate — so the helpers are reviewed and exercised only structurally here; `Channel.create` and other task/channel builtins remain rejected by `NS8011`.

---

### 2026-10-27 - `C-001` slice: `to_int`, `checked_*`, and `wrapping_*` integer methods

- Command: `cargo test -p astra-codegen-c`, `cargo test -p astra-nova --test conformance`, `npm run ci`
- Result: 68/68 in `astra-codegen-c`; full CI green
- What: the C backend now implements the spec §6.1 integer methods and §6.2 `str.to_int()`, closing the last builtin family (outside concurrency) that JavaScript and Nova already implement.
  - **Runtime** (`runtime.rs`): `astra_str_to_int_try` performs a strict decimal parse (`[+-]?[0-9]+`, no whitespace, int64 headroom for `INT32_MIN`) and reports failure instead of returning a sentinel; `NS_ParseIntError` and `NS_OverflowError` are shared stem-free tuple payloads (`{ NS_Str _0; }`) declared in the runtime header.
  - **Emitter** (`emit.rs`): `str.to_int` and the integer `checked_*`/`wrapping_*` family lower before the type-directed method dispatch (`integer_method_c`, per receiver width). Checked operations evaluate both operands once and use `__builtin_{add,sub,mul}_overflow` for overflow verdicts, or a zero panic plus a signed `MIN / -1` guard for division; wrapping operations reuse the signed `astra_{add,sub,mul}_{i8..i64}` helpers and C's defined unsigned wrap-around, with `wrapping_div` mapping `MIN / -1` to `MIN` to avoid undefined behavior. Division by zero panics in every mode, matching Nova and JavaScript.
  - **Fallible-call materialization** (`effect_error_type`): `to_int` and `checked_*` calls now materialize their `Result<base, E>` in `match`/`!` like `str.slice`, so all fallible builtin families share one path.
- New case: `conformance/cases/numeric_errors.ns` (19 positive cases now) pins strict parse success/failure/overflow, `checked_add` overflow, `checked_sub` success, and `wrapping_add` wrap-around; JavaScript, TypeScript, and Nova all pass.
- Tests: new `str_to_int_and_integer_checked_arithmetic_lower_to_results` asserts the parser helper, both Result typedefs, the overflow builtin, the division guards, and the signed wrap helper, and compiles the output with clang.
- Verification: full `npm run ci` green immediately before this entry.
- Boundary: C's native runner still waits on the toolchain gate, so the arithmetic verdicts are pinned structurally (clang-compiled) rather than executed here; float methods and `as`-casts remain outside this slice.

---

### 2026-10-27 - `C-001` follow-up: `expr!` propagation materializes fallible builtins

- Command: `cargo test -p astra-codegen-c`, `npm run ci`
- Result: 67/67 in `astra-codegen-c`; full CI green
- What: `propagate_c` typed its statement temporary from the enclosing function's Result when the inner expression had no `Result` annotation — which is always true for a fallible builtin call (annotated with its base type). `let part = s.slice(0, 1)!` inside `fn f(s: str) -> i32 ! SliceError` therefore declared the temp as `NS_Result_i32_SliceError` and assigned an `NS_Result_Str_SliceError`, which clang rejects.
- Fix: the materialization logic from `match_expr_c` is extracted as `materialized_result_type` (used by both `match` scrutinees and `!`), so propagation types the temp with the call's own `Result<base, E>`; the error early-return still uses the enclosing function's Result and the payload types agree because `!` propagates the effect into the function's error set.
- Tests: new `propagate_materializes_fallible_builtin_results` pins `NS_Result_Str_SliceError` in the emitted C and compiles it with clang.
- Verification: full `npm run ci` green immediately before this entry.
- Boundary: C's native runner still waits on the toolchain gate; cross-module fallible calls in `match`/`!` remain limited to local functions.

---

### 2026-10-27 - `C-001` slice: `str`/`bytes` slice semantics (spec 6.2/6.4)

- Command: `cargo test -p astra-codegen-c`, `cargo test -p astra-nova`, `npm run ci`
- Result: 66/66 in `astra-codegen-c`; full CI green (see Verification)
- What: `str.slice(start, end)` and `bytes.slice(start, end)` lowered with `end` passed as a *length* (`astra_str_slice(s, start, end)`), so every non-zero start returned the wrong range; `str.slice` also emitted a bare `NS_Str` although the checker's signature is fallible (`str ! SliceError`).
  - **Runtime** (`runtime.rs`): `astra_str_slice` becomes `astra_str_slice_try(NS_Str, start, end, NS_Str* out) -> bool` - byte offsets, `false` on reversed, out-of-bounds, or UTF-8-scalar-splitting ranges (the same conditions Nova fails with `Err(SliceError)`); `astra_bytes_slice(b, start, end)` keeps infallible panic-on-bad-range semantics (mirrors Nova). Both helpers now take end offsets.
  - **Types** (`ty.rs`): `SliceError` is a built-in nominal declared once in the runtime header (`NS_SliceError`, stem-free so every module shares one type); `user_typedef`/`ident` resolve it before the `NS8020` unknown-type rejection, and the derived container name is `NS_Result_Str_SliceError`.
  - **Emitter** (`emit.rs`): the `str.slice` arm evaluates receiver/start/end once into statement temporaries, calls `astra_str_slice_try`, and builds `Result<str, SliceError>` inline; the `bytes.slice` arm passes end offsets.
  - **Fallible-call matches**: the checker annotates a fallible call with its *base* type and tracks the error effect separately, so `match s.slice(..) { Ok .. Err }` arrived typed `str` and fell into the literal-match path (constructor patterns rejected `NS8021`). `match_expr_c` now materializes `Result<base, E>` exactly as the checker does for exhaustiveness; `E` comes from a new `effect_error_type` (built-in fallible members plus local `fn f() -> T ! E` declarations).
  - **Honesty fix**: the standalone `emit()` only checked rejects after `prescan`, so emission-time rejects were silently dropped (the empty literal match compiled and always returned the zero temp). Rejects are re-checked after `emit_program`, mirroring the linked path.
  - **Typedef ordering**: the runtime header and typedefs were written before function bodies, so a typedef ensured during body emission (`NS_Result_Str_SliceError`) was never declared. `emit_program`/`emit_program_linked` now emit the code into a scratch buffer and write the header, dependency includes, and complete typedefs first.
  - **JavaScript/TypeScript** (`generate.ts`): `bytes.slice` no longer falls through to the native `Uint8Array.slice` (which clamps and returns empty for reversed ranges); a new `$byteSlice` helper bounds-checks and panics, mirroring Nova.
- New case: `conformance/cases/slice_bounds.ns` (18 positive cases now) pins `str.slice` byte offsets (Ok at a non-zero start; `Err` for out-of-bounds, reversed, and UTF-8-splitting ranges) and `bytes.slice` end offsets; JavaScript, TypeScript, and Nova all pass.
- Tests: `test_bytes_indexing_and_methods` now asserts end-offset lowering (`b.slice(1, 3)` -> `astra_bytes_slice(v_b, (size_t)(1), (size_t)(3))`); new `str_slice_lowers_to_result_with_bounds_check` (named `Ok`/`Err` bindings compile under clang); new JS test `javascript backend bounds-checks bytes slices instead of clamping`.
- Verification: full `npm run ci` green immediately before this entry (fmt + clippy `-D warnings` clean).
- Boundary: C's native runner still waits on the toolchain gate, so `astra_str_slice_try`/`astra_bytes_slice` runtime behavior is pinned structurally (clang-compiled) rather than executed here; cross-module fallible calls in `match` scrutinees remain limited to local functions, and `str.to_int`/`checked_*` are still unimplemented in C.

---

### 2026-10-27 — `C-001` slice: cross-module struct/enum types (`NS8020` closure)

- Command: `cargo test -p astra-codegen-c`, `npm run ci`
- Result: 65/65 in `astra-codegen-c`; full CI green (see Verification)
- What: a type imported from a dependency now lowers to that module's qualified typedef (`NS_<dep stem>_<Type>`) everywhere instead of being rejected with `NS8020`.
  - **Type context** (`crates/astra-codegen-c/src/ty.rs`): `TyCtxt` gains a foreign-type table (`add_foreign`) and a single resolver `nominal(name)` covering local, imported, renamed, and namespace-dotted names; `struct_shape`/`enum_shape`/`enum_tag` wrap it for the emitter. `user_typedef` returns a foreign dependency's qualified name without re-emitting the typedef (the dependency's TU and header own it), and `ident(Type::Named)` takes the foreign prefix so container names (`NS_Option_lib_Point`) match across module boundaries.
  - **Emitter** (`emit.rs`): `install_foreign_types` builds the table per module from the HIR import tables — named imports (including renames) first, then `alias.Type` for namespace imports, then bare names carried by inferred annotations (first by sorted key; local declarations always win in `nominal`) — installed **before** `prescan` so signatures resolve. Construction (`struct_init_c`), tuple constructors, field access, `match_struct`/`match_enum`, nested pattern binding, and `fail` error construction all consult the resolver; struct/enum paths use the full dotted path so `lib.Point` resolves.
  - **Enum tag fix**: tag constants are now built from the owning module's prefix (`enum_tag`) instead of a bare `NS_<Owner>_TAG_<Variant>` — linked-mode enum construction and matches previously emitted tag names that did not match the prefixed declarations; this was latent and unpinned, and the new enum test pins it for both local and imported enums.
  - **Headers**: `emit_header` includes dependency headers, so exported signatures that reference `NS_lib_Point` resolve for consumers of the header alone.
- Tests: `project_rejects_cross_module_struct_type` is replaced by three positive tests — struct signature/literal with no re-emitted typedef and header dep-includes, enum construction + match cases using the dependency's tags, tuple-struct construction/signature — plus the existing `NS8019` default-import rejection stays. `TypeEnv::struct_of`/`enum_of` removed (superseded by the resolver).
- Verification: full `npm run ci` green immediately before this entry (fmt + clippy `-D warnings` clean).
- Boundary: cross-module types through *transitive* dependencies in field types, header self-containment for structurally nested foreign containers, and `NS8019` default imports remain open; GC (`GC-001`), concurrency (`CONC-001`), package linking, and the native CI gate remain open.

---

### 2026-10-27 — `JS-001` slice: statement-position control flow (`NS8001` closure)

- Command: `npm run ci`
- Result: pass (`[ci] all checks passed`); commit follows this entry
- What: the shared JavaScript/TypeScript emitter no longer lowers statement-position `if` and `match` to arrow IIFEs. `emitStatement`'s `Expr` case now routes `if` statements to a new `emitIfStatement` (plain `if`/`else if`/`else` statements, recursing through else-if chains) and `match` statements to a new `emitMatchStatement` (`else`-chained arms with the scrutinee temporary and `$failure` normalization shared with the value-mode path via `prepareMatchScrutinee`).
- Consequence: `break`, `continue`, and early `return` inside statement-position `if` and `match` blocks now emit as legal JavaScript bound to the enclosing loop or function — the documented JS conformance gap closes. Value-position `if`/`match`/block expressions still lower to IIFEs and keep rejecting non-local control flow with `NS8001`.
- Match detail: statement-position matches chain arms with `else if` so exactly one arm runs (arms no longer `return` out of an IIFE), and the non-exhaustive `$panic` fallback moved into a final `else` block so it is reachable only when no arm matches — the value-mode intent.
- Tests: the pinned `rejects non-local control flow inside expression blocks` test becomes two positive tests (early `return` from a statement-position `match`; `break`/`continue`/early `return` across statement-position `if` and `match` in a loop) plus one negative test keeping value-position control flow rejected with `NS8001`.
- New case: `conformance/cases/early_exit.ns` (17 positive cases now) passes on JavaScript, TypeScript, and Nova; `conformance/README.md` records no backend gap that blocks a case.
- Verification: full `npm run ci` green immediately before this entry (JS codegen suite 31/31, TS suite 9/9 incl. shared conformance, Nova `--test conformance` ok).

---

### 2026-10-27 — NOVA-004 Nova language gaps closed: str methods, `bytes`, and the Channel runtime

- Command: `cargo test -p astra-nova`, `npm run ci`
- Result: pass
- What: Nova now implements the spec §6 string method set beyond `len` — `char_count`, `bytes` (an `Array<u8>`), `chars`, `byte_at`/`char_at` (returning `Option`, `None` on any out-of-range index), and `slice(start, end)` on byte offsets that returns `Err(SliceError)` when the range is out of bounds, is reversed, or would split a UTF-8 scalar value.
- The `bytes` builtin resolves through `member()` as `bytes.new(length)` when unshadowed (mirroring the checker's guard: no local, constant, import, namespace, or struct owns the name). `bytes.new(n)` allocates a zero-filled buffer; indexed reads return `u8` and indexed writes convert like the JavaScript backend's `Uint8Array` write (`as u8` wrap), both bounds-checked with a `byte index … is out of bounds` panic. `len`, `is_empty`, `slice(start, end)` (panics on out-of-bounds ranges), and `for`-over-bytes (yielding `u8`) round out the builtin.
- Channels: `Channel.create<T>()` allocates a bounded (capacity 16) `ChannelValue`; `send` and `receive` return `Result` with a `ChannelError` payload (`send on a closed channel`, `receive on a closed channel`) and `close` is explicit. Because spawn stays eager and the runtime is single-threaded, a send to a full buffer or a receive from an empty open channel raises the same structured `channel deadlock: …` panic as the JavaScript backend instead of blocking (spec §9.2 backpressure).
- Conformance: the three `KNOWN_GAPS` entries (`bytes_buffer`, `channels`, `strings_utf8`) are removed — the ledger in `crates/astra-nova/tests/conformance.rs` is empty and its staleness check stays armed. Nova executes all 16 cases itself.
- New case: `conformance/cases/struct_mutation.ns` pins the `TS-001` `readonly` fix (`TS2540`) under `tsc --strict`; JavaScript, TypeScript, and Nova all pass it, so `conformance/README.md` now records no Nova and no TypeScript gap (JavaScript keeps the documented `NS8001` case-writing rule).
- Tests: 8 new Nova unit tests (utf8 string methods, str slice/index failure paths, bytes allocation/mutation/iteration, out-of-bounds write panic, eager channel execution, closed-channel send/receive errors, full/empty deadlock panics) — the Nova lib suite is 59 tests.
- Boundary: `bytes.slice` semantics are not pinned by the corpus — Nova panics on out-of-bounds ranges (bounds-checked per §6.4), JavaScript's native `Uint8Array.slice` clamps, and C uses `(start, len)` length semantics. Worth a future conformance pin; C's `astra_str_slice` `(start, len)` deviation remains C-backend scope.

---

### 2026-10-27 — `work/conf-001` merged into master

- Command: `git merge --no-edit work/conf-001`, `npm run ci`
- Result: pass (`[ci] all checks passed` at the merge commit)
- Merge: `78d8920` brings the expanded conformance corpus (15 positive + 11 negative cases), the Nova `KNOWN_GAPS` runner, the frontend reject runner (`crates/astra-core/tests/conformance_rejects.rs`), the `npm run test:boundary` script, and the conformance ledger docs onto master. The `ort` merge auto-resolved the shared `Docs/Agent/*` files with no conflicts.

---

### 2026-10-27 - C-001 multi-module project linking slice

- Command: `npm run ci`
- Result: pass (`[ci] all checks passed`: 548+ Rust tests incl. 63 in `astra-codegen-c` and 31 in `astra-compiler-api`, TypeScript unit tests, native/Wasm/boundary tests incl. 2 new C e2e tests, fmt and clippy clean)
- What: the C backend can now compile a multi-module project into one translation unit per module that links as a unit.
  - **Emitter** (`crates/astra-codegen-c/{emit,ty}.rs`): linked mode module-qualifies function symbols (`ns_<stem>_<name>`) and nominal type names (`NS_<stem>_<Type>`) so two modules declaring the same name cannot collide; standalone single-module output stays byte-identical (`ns_<name>`). Cross-module calls resolve through HIR import bindings (`collect_import_tables`: named imports → `(depKey, imported)`, namespace imports → `alias.fn(...)`); callers include `<dep stem>.h`. Default-import calls are rejected `NS8019`; cross-module struct/enum types `NS8020` (honest boundary). `module_key_stem` strips a trailing `.ns` so CLI file-path keys yield clean stems (`src/main.ns` → `src_main`, not `src_main_ns`).
  - **Compiler API** (`crates/astra-compiler-api/src/lib.rs`): `compile_project` JSON-RPC method returns `ProjectCompileResponse` (per-module `cFilename`/`hFilename`/`cSource`/`hSource`/`scriptSources` + shared `hostShim`); filename stem kept in step with the emitter. Relative import specifiers (`./lib`) now resolve against the importer key before type-checking via `check_module_graph_with_imports` (mirrors `check_project`), fixing `NS3012` on `compile_project`. `int main(void)` wrapper is emitted only for the entry module (last source).
  - **Client/CLI**: `CompilerClient.compileProject` mirrors `checkProject` (method `compile_project`); `astra build --target c` routes multi-module projects through it (writing one `.c`+`.h` per module, shared `astra_host.c`, and linking all translation units for `--target native`), while single-module builds keep the standalone `compile` path so `main.c`/`ns_main()` output is unchanged.
  - **Tests**: 8 new `emit_c.rs` project tests (cross-module call resolution, collision avoidance, qualified prototypes, entry-only `main`, namespace imports, `NS8019`/`NS8020` rejections); 3 new `compile_project` compiler-api tests (per-module artifacts, cross-module diagnostics, empty-sources `-32602`); 1 new boundary e2e test (`astra build --target c emits one qualified translation unit per module`).
- Verification: full `npm run ci` green immediately before this entry; commit follows.
- Boundary: cross-module struct/enum types (`NS8020`), default imports (`NS8019`), Cargo/npm package linking, GC (`GC-001`), concurrency (`CONC-001`), and the native CI gate remain open.

---

### 2026-10-27 - TS-001 complete: @script:ts pinned transpiler (slice 3)

- Command: `npm run ci`
- Result: pass (49 TypeScript unit tests, 46 native/Wasm/boundary tests, 548 Rust tests, fmt and clippy clean)
- Commit: `df4ffc9` "TS-001 slice 3: @script:ts emission with one pinned transpiler"
- What: the TypeScript backend now emits `@script:ts` blocks. The shared emitter gained an optional `transpileTypeScript` seam on `CodegenRequest`; in TS mode a `ts` script body is padded with leading blank lines so the pinned transpiler's source map reports `.ns`-absolute lines, transpiled, and emitted as a `.js` artifact plus `.d.ts`. The artifact's source map is composed from the transpiler's map by shifting past the two header lines with `sources`/`sourcesContent` pointing at the `.ns` source — no VLQ re-encode. If the transpiler produces no usable map, the artifact is emitted without one rather than an incorrect one.
- Pinning: `codegen-ts` pins `typescript` at exactly `7.0.2` (also pinned in the root devDependency so the lockfile cannot drift) and supplies `transpileTypeScriptBody`, which resolves the install, checks the pin via the pure `pinnedTypeScriptVersionError`, and spawns `tsc --ignoreConfig --noCheck --target es2022 --module esnext --moduleResolution bundler --sourceMap`. A missing install, a version mismatch, a non-zero `tsc` exit, or any TS diagnostic is a hard `NS7008` error with no best-effort fallback. The JS backend keeps rejecting `@script:ts`.
- Tests: `@script:ts` transpiles, type-checks under `tsc --strict`, runs, and matches the equivalent `@script:js` program; the artifact's composed map resolves to the `.ns` body line; a missing transpiler and a syntactically invalid body are hard `NS7008` errors emitting no artifact; the pin matches the installed compiler. The parity suite also gained a `safe-checked` integer policy program (5 parity programs total).
- TS-001 acceptance: all items complete; `Docs/Agent/todo.md` TS-001 section updated to reflect the three slices (`83ec555` shared-emitter TS mode, `6f0e80b` CLI wiring + test suite, `df4ffc9` `@script:ts`).

---

### 2026-10-27 — CONF-001 full gate run and backend findings

- Command: `npm run ci`, `cargo test --workspace`, `[Environment]::SetEnvironmentVariable("Path", ..., "User")`
- Result: pass
- CI failure: `npm run ci` stopped in Rust tests because the GNU toolchain could not find `dlltool.exe` for `getrandom`/`windows-sys`. The binary already existed at `C:\msys64\ucrt64\bin\dlltool.exe`; it was simply absent from `PATH`. Nothing needed downloading.
- Toolchain fix: appended `C:\msys64\ucrt64\bin` to the **user** `PATH` (one entry, verified) and to the working process. Verified `dlltool` 2.46.1, `gcc` 16.1.0, `ld`, and a compile/link/run smoke test. `npm run ci` now passes end to end without a per-command override. Restart terminals and editors to inherit the new `PATH`.
- TypeScript finding: the TS backend runs the shared corpus from `orchestrator/tests/codegen-ts.test.mjs`. The original `structs_enums` case failed `tsc` with `TS2540: Cannot assign to 'name' because it is a read-only property` — emitted struct fields are `readonly` even for `let mut` bindings. The shared case now constructs final values, and the gap is recorded in `conformance/README.md` for `TS-001`.
- Coverage: 15 positive cases pass on Nova, JavaScript, and TypeScript; 11 negative cases pass the frontend runner.

---

### 2026-10-27 — CONF-001 floats, booleans, and generics

- Command: `cargo test -p astra-nova --test conformance`, `node --test orchestrator/tests/codegen-js.test.mjs`
- Result: pass
- Added three positive cases: `floats` (arithmetic, comparison, and default formatting), `booleans` (`not`, `&&`, `||`, comparisons), and `generics` (generic function instantiated at `i32`, `str`, and `bool`).
- Pinned behavior: both Nova and JavaScript print whole-valued floats as `6.0`, not `6`. The case records `6.0` so neither backend drifts to JavaScript's default number formatting.
- Note: boolean negation is `not`; `!` is the error/propagation marker and is a parse error in expression position. The case documents that split.

---

### 2026-10-27 — CONF-001 negative conformance corpus

- Command: `cargo test -p astra-core --test conformance_rejects`, `cargo test -p astra-nova --test conformance`, `node --test orchestrator/tests/codegen-js.test.mjs`
- Result: pass
- Added `conformance/rejects/` with 11 rejection cases and matching `.codes` files: parse failure (`NS2001`), private export (`NS3001`), conflicting visibility (`NS3002`), duplicate export (`NS3003`), unknown export (`NS3005`), private type leak (`NS3010`), duplicate declaration (`NS3011`), type mismatch (`NS4001`), unknown name (`NS4002`), unhandled effect (`NS4011`), and mode requirement (`NS5001`).
- Runner: `crates/astra-core/tests/conformance_rejects.rs` runs parse → resolve → check and matches codes as a documented subset, so richer diagnostics remain allowed while every required code must appear. Orphan `.codes` files fail the test.
- Rationale: rejection happens before code generation, so one frontend runner pins the diagnostic contract for every backend, including the ones still in progress.

---

### 2026-10-27 — CONF-001 expanded on branch `work/conf-001`

- Command: `cargo test -p astra-nova --test conformance`, `node --test orchestrator/tests/codegen-js.test.mjs`
- Result: pass
- Corpus: 4 → 12 cases. Added `control_flow`, `integers`, `defer_order`, `collections`, `structs_enums`, `options`, `errors`, and `functions`.
- Nova runner: `crates/astra-nova/tests/conformance.rs` executes the corpus through `build_interpreter`, compares stdout plus `RETURN=<code>`, and keeps an explicit `KNOWN_GAPS` ledger (bytes builtin, full `str` methods, channels). A ledger entry that starts passing fails the test, so gaps cannot rot into silent skips.
- Finding: the suite was effectively JavaScript-only. Nova failed 3 of the 4 existing cases, and the JavaScript backend rejects `break`/`continue` and early `return` inside an `if` with `NS8001`. Cases now use single-exit loops and `match` expressions, and both gaps are documented in `conformance/README.md`.
- Verification: Nova conformance test green (9 cases executed, 3 ledgered gaps); JavaScript conformance test green; `npm run build` and the compiler binary build succeed on the branch.
- Not done: C backend runner (waits on `C-001` project linking), negative/diagnostic cases, and CI wiring for the Nova runner beyond `cargo test --workspace`.

---

### 2026-10-26 - js-interop merged into master: JS-001 complete

- Command: `npm run ci`
- Result: pass
- Merge: `js-interop` (`39437d0`) merged into `master` (`1119bae`), completing JS-001 on master: linked multi-module ESM, inline `@script:js` ESM artifacts, recoverable errors, eager tasks with bounded channels, UTF-8 string/byte operations, source maps, `.d.ts` artifacts, and the shared `conformance/` suite.
- Conflict resolution: `crates/astra-codegen-c/*` kept master's C backend; the JS emitter and `codegen-js.test.mjs` took js-interop's tested implementation; `checker.rs`, `astra-compiler-api`, the `astra build` CLI flow, and agent docs were hand-merged so both sides' features survive.
- Integration fixes: renamed `gen` bindings in `astra-compiler-api` (reserved keyword in edition 2024; the checkpointed C WIP had not compiled), removed duplicate `Type::Bytes`/`slice` checker arms (the spec-fallible `slice` with `SliceError` wins), ported master's `astra build --target c` e2e test into the merged suite, and ran `cargo fmt` over the C WIP.
- Verification: `npm run ci` green: 548 Rust tests, 41 TypeScript unit tests, 46 native/Wasm/boundary tests (28 codegen/build), fmt and clippy clean.
- Current boundary: `TS-001` next (groundwork in tree); `C-001` continues (multi-module project linking, cross-platform native CI gate).

---

### 2026-10-26 — C-001 Map methods, for-loops, and byte operations implemented

**What changed**

- **Map operations (`emit.rs`, `runtime.rs`, `checker.rs`)**:
  - Added `.insert(k, v)` emission: looks up prior entry via `astra_map_get`, captures prior value in `NS_Option_V`, and updates the map via `astra_map_set`.
  - Added `for`-over-map iteration: generates C `for` loops indexing keys with `astra_map_key_at(&map, idx)`.
  - Runtime support: implemented `astra_map_key_at(const NS_Map* m, size_t i)` in freestanding header and `HOST_SHIM`.
- **Full byte & string operations (`emit.rs`, `runtime.rs`, `checker.rs`)**:
  - Added byte indexing (`b[i]`) returning `u8` via `astra_bytes_get(b, i)`.
  - Added string and byte slicing (`s.slice(start, len)`, `b.slice(start, len)`) returning `NS_Str` and `NS_Bytes` via `astra_str_slice` and `astra_bytes_slice`.
  - Added comparison operators (`==`, `!=`, `<`, `<=`, `>`, `>=`) for `str` and `bytes` via `astra_str_eq`, `astra_str_cmp`, `astra_bytes_eq`, and `astra_bytes_cmp`.
  - Added `for`-in iteration over `bytes`, yielding `u8` elements via `astra_bytes_get`.
  - Added `.len()`, `.is_empty()`, and `.slice()` to `Type::Bytes` in `checker.rs` member resolution and added `Type::Bytes` to `expected_index` in `check_index` and `check_lvalue`.
- **Tests & Verification**:
  - Added `test_map_insert_method`, `test_for_over_map`, `test_for_over_array`, `test_bytes_indexing_and_methods`, `test_bytes_and_str_comparison`, and `test_for_over_bytes` to `crates/astra-codegen-c/tests/emit_c.rs`.
  - 50 `astra-codegen-c` tests pass (up from 44).
  - 142 `astra-core` tests pass (total 192 unit tests in suite).
  - Clippy clean across all crates with `-D warnings`.

---

### 2026-10-26 — JS-001 single-module slice implemented and verified; the full backend backlog remains open

**What changed**

- Verified the JavaScript backend suite on the `js-001` branch: the first single-module ESM slice works and produces runnable generated output.
- Confirmed the remaining backlog is intentionally still open and listed in the JS-001 acceptance block: project imports/re-exports, recoverable error effects, tasks/channels, source maps, `.d.ts` output, integer-policy coverage, and conformance work.
- The TypeScript target is still not started and remains separate from the current JS implementation work.

**Verification evidence**

- `node --test orchestrator/tests/codegen-js.test.mjs`: `16` passing tests, `0` failures
- `npm run build && node --test orchestrator/tests/codegen-js.test.mjs orchestrator/tests/codegen-ts.test.mjs`: `17` passing tests, `0` failures

---

### 2026-10-26 — JS-001 recoverable errors and task execution verified

**What changed**

- The JavaScript backend lowers recoverable failure values, `fail`, and `?` propagation through the typed `Propagate` and `Fail` paths.
- Task spawning and task join/cancel execution are covered by the JS backend helpers and pass in the active suite.
- The stale explicit-rejection regression was replaced with a real end-to-end execution test.

**Verification evidence**

- `node --test orchestrator/tests/codegen-js.test.mjs`: `16` passing tests, `0` failures

---

### 2026-10-26 — C-001 C17 backend slice and build pipeline complete

**What changed**

- **`defer` statement support (`emit.rs`)**:
  - Implemented `defer` emission in `emit_stmt`, `emit_block_body`, `emit_scoped_block`, `emit_return`, `emit_tail_expr`, and `emit_block_value`.
  - Added `emit_all_defers`, `emit_scope_defers`, and `emit_scope_defers_into_pre` helpers to execute deferred statements in strict LIFO reverse order across nested blocks and prior to function exits.
- **Entry point and `main` delegation (`emit.rs`)**:
  - Extended `find_main` to support both `()` and `i32` returns.
  - Implemented `emit_main` generating `int main(void)` which calls `ns_main()` and returns its code, avoiding function body duplication and unused function warnings.
- **CLI build pipeline integration (`orchestrator/packages/cli/src/index.ts`)**:
  - Wired `astra build --target c` and `--target native`.
  - Validated target backends (`js`, `c`, `native`), branching compilation to request target `c` from `astra-compiler-api` and emitting the generated translation unit and `astra_host.c` to `outDir`.
  - For `backend: "native"`, automatically invokes the discovered C toolchain (`clang`/`gcc`/`cc`) to produce the native binary.
- **Compiler client & boundary tests**:
  - Added `"c"` to `CompileRequest.target` in `@neo-astra/compiler-client`.
  - Added end-to-end test in `orchestrator/tests/codegen-js.test.mjs` verifying `astra build --target c` outputting `main.c` and `astra_host.c`.
  - Added tests in `crates/astra-codegen-c/tests/emit_c.rs`: `defer_runs_in_lifo_order`, `defer_runs_on_early_return`, and `end_to_end_native_execution`.
- **Verification evidence**:
  - `cargo test -p astra-codegen-c`: all 38 tests pass with `clang -std=c17 -ffreestanding` object file compilation and structural validation.
  - `npm run test:boundary`: all 30 boundary tests pass with 0 failures.
  - `npm test`: all 40 unit tests pass.
  - `cargo fmt --all --check`: clean formatting across all crates.
  - `cargo clippy --workspace --all-targets -- -D warnings`: 0 warnings, clean.
  - `npm run ci`: complete CI suite passes cleanly.

---

### 2026-10-26 — INTEROP-001 Slices A+B complete

**What changed**

- **Lexer bug fix** (`lexer.rs`): Replaced the `in_script_header: bool` flag with a
  `script_header_depth: u32` brace-depth counter. The previous flag was cleared by the
  `}` closing `@exports { … }`, so `@impl` was never recognised as the raw-source
  trigger. The depth counter starts at 1 when `{@script:` is entered, increments for
  inner `{`, and decrements for inner `}`; only reaching 0 exits header mode. All
  existing tests continued to pass.

- **Conformance tests added**:
  - `stress_parser.rs`: 13 new `@script` tests covering JS, C, multiple exports,
    braces in raw body, error-effect exports, `NS7001` (unknown language), `NS7003`
    (missing/empty `@exports`), and `NS7004` (missing `@end` / missing `@mode`).
  - `stress_formatter.rs`: 5 new tests — canonical layout assertions, verbatim
    raw-body preservation, idempotence (single block, multiple exports, mixed with
    functions).
  - `stress_checker.rs`: 11 new tests — no-target pass, `NS7002` target/language
    mismatch (js→native, c→js), `NS7005` non-boundary-safe export type (fn-value
    parameter), `NS7006` namespace collision, and namespace call through checker.

- **`todo.md` updated**: Five of eight INTEROP-001 implementation items are now `[x]`.
  Remaining open items: JS/TS ESM backend slice, C/C++ native slice, deferred post-V1
  work (Python, Wasm, Nova host adapter).

**Test evidence**: `cargo test --workspace` — all suites pass (0 failures).

---

## Completed

### 2026-10-23 — Architecture decisions

- Reviewed the original Neo Astra concept.
- Normalized naming to **Neo Astra**, `.ns`, and `astra`.
- Confirmed D1–D21 in `docs/architecture.md`.
- Defined the hybrid compiler split:
  - TypeScript orchestrator
  - Rust core engine
  - Modular web and low-level backends
- Defined modes, semantic policies, memory policy, error model, concurrency, FFI, packages, manifest rules, and strictness.
- Added the proposed repository layout.

### 2026-10-23 — Agent operating system

Created `Docs/Agent/`:

- `agent.md`
- `system-prompt.md`
- `edge-case-protocol.md`
- `project-context-glossary.md`
- `definition-of-done.md`
- `runbook.md`
- `io-schemas.md`
- `state-log.md`
- `todo.md`

Purpose: give future agents a stable role, boundaries, workflow, schemas, and completion gate.

### 2026-10-23 — V1 design package

Created `Docs/Design/V1/`:

- `README.md`
- `01-language-design.md`
- `02-compiler-design.md`
- `03-runtime-and-memory.md`
- `04-targets-and-backends.md`
- `05-manifest-and-packages.md`
- `06-tooling-and-workflow.md`
- `07-quality-security-and-delivery.md`

Scope: V1 as the first stable release, not the v0.1 implementation slice. The design covers the language contract, compiler split, runtime and memory, target backends, manifest/packages/FFI, tooling, and quality/security/delivery gates.

The V1 design is a proposal. It does not reopen D1–D24 and does not claim implementation.

### 2026-10-23 — v0.1 specification

Created `Docs/spec-v0.1.md`:

- Normative source, lexical, grammar, type, integer, string, error, memory, concurrency, mode, FFI, manifest, diagnostics, and conformance rules.
- Semantics matrix for JS/TS, native, and Wasm.
- Decision traceability table for D1–D24.
- Provisional choices and open questions for V1 freeze review.
- Grammar examples and complete end-to-end examples.

`SPEC-001` through `SPEC-012` are complete. The specification is a draft, not a claim that every target or runtime feature is implemented.

### 2026-10-23 — Keyword aliases (D22)

Added exact lexical aliases to the language contract:

- `fn` / `function`
- `let` / `var`
- `const` / `constant`
- `type` / `typedef`
- `struct` / `record`
- `mut` / `mutable`
- `pub` / `public`

Aliases are spelling options only. The canonical formatter normalizes them to the short form. `var` does not imply mutability; `let mut` or `var mut` is required. `record` is an alias for `struct`, not a separate type kind.

Updated `Docs/spec-v0.1.md`, `Docs/architecture.md`, the glossary, and the agent phase notes. `SPEC-013` is complete.

### 2026-10-24 — Declaration visibility and imports/exports (D23)

Added the optional declaration and module boundary system:

- Normal declarations are module-local by default and explicitly exportable.
- `pub`/`public` and `export` mark declarations as exported.
- `private` declarations cannot be exported, imported, or re-exported.
- Existing `import path` and `import path as alias` remain valid.
- JS/TS-style named, default, namespace, side-effect, re-export, and default-export forms are accepted.
- `export { ... }` can promote normal declarations, rename exports, and re-export from other modules.
- `export *`, `export * as namespace`, `export * as default`, and `export default` are defined.
- Type-only exports and imports are erased in JS/TS output; default exports must be values.
- Private declarations may not leak through public signatures, public aliases, or public fields.
- Visibility is compile-time only and never bypasses modes, capabilities, FFI allowlists, or package security.

Updated `Docs/spec-v0.1.md`, `Docs/architecture.md`, `Docs/Design/V1/01-language-design.md`, and the agent operating docs. `SPEC-014` is complete.

### 2026-10-24 — TypeScript orchestrator scaffold (SCAF-001)

Created a root npm workspace and the TypeScript orchestrator packages:

- `@neo-astra/cli` with `check`, `build`, `fmt`, `ast`, and `explain` entrypoints.
- `@neo-astra/manifest` with exactly-one-manifest discovery and JSON/TOML parsing.
- `@neo-astra/lsp` boundary stub.
- `@neo-astra/plugins` package-source and backend adapter contracts.
- `@neo-astra/codegen-js` and `@neo-astra/codegen-ts` backend stubs.
- Root build, test, and `astra` scripts; `tsconfig` project references.
- Manifest and CLI smoke tests.

Verification: `npm run build` passed; `npm test` passed 4/4; `astra --help` and `astra --version` ran successfully. `SCAF-001` is complete.

### 2026-10-24 — Rust core scaffold (SCAF-002)

Created the Rust workspace and shared compiler boundaries:

- `astra-core` with span, diagnostic, AST, HIR, IR, type, lexer, parser, and resolver modules.
- `astra-capabilities` with the mode vocabulary and source requirement shape.
- `astra-compiler-api` with protocol version, request/response types, and the compiler trait.
- `astra-codegen-c`, `astra-codegen-wasm`, and `astra-codegen-llvm` stubs.
- Workspace `Cargo.toml`, lockfile, and crate documentation.
- Unit tests for spans, diagnostics, modes, and protocol responses.

Verification: `cargo test --workspace` passed 6/6; `cargo fmt --all --check` passed; `cargo clippy --workspace --all-targets -- -D warnings` passed. `SCAF-002` is complete.

### 2026-10-24 — Bootstrap command (TOOL-001)

Added `scripts/bootstrap.mjs` and npm entrypoints:

- `npm run bootstrap` installs JavaScript and Rust dependencies.
- `npm run bootstrap:check` reports missing tools without installing.
- `npm run bootstrap:verify` installs, then builds, tests, fmt-checks, and runs clippy on both workspaces.
- Windows selects the GNU Rust toolchain when the MSVC linker is unavailable.
- Clang/LLVM is best-effort because its installer may require elevation.

### 2026-10-24 — Compiler API boundary (SCAF-003)

Implemented the versioned compiler boundary:

- JSON-RPC 2.0 request/response envelope with `apiVersion` `1.0`.
- `initialize`, `check`, `compile`, and `shutdown` commands; unsupported frontend commands return structured errors.
- Native `astra-compiler` binary reading stdin and writing stdout.
- `astra-wasm` cdylib with raw linear-memory `astra_alloc`, `astra_free`, `astra_compile`, and `astra_protocol_version` exports.
- Native and Wasm smoke tests in the orchestrator test suite.

Verification: `cargo test --workspace` passed 9/9; `npm run test:boundary` passed 2/2; formatting and clippy passed. `SCAF-003` is complete.

### 2026-10-24 — Build and CI automation (SCAF-004)

Added the local and CI verification path:

- `npm run ci` runs the TypeScript build/tests, native/Wasm boundary tests, Rust formatting, clippy, and Rust tests.
- `scripts/ci.mjs` orchestrates the checks cross-platform.
- `.github/workflows/ci.yml` runs on Ubuntu, Windows, and macOS with Node 24 and stable Rust.
- CI installs `rustfmt`, `clippy`, and `wasm32-unknown-unknown`.

Verification: `npm run ci` passed, including 4/4 TypeScript tests, 2/2 boundary tests, and 9/9 Rust tests. Phase 3 is complete.

### 2026-10-24 — Lexer and spans (LEX-001)

Implemented the first compiler frontend stage:

- UTF-8 byte spans, leading BOM handling, whitespace, and newline tokens.
- Exactly `//` comments; `/*`, `#`, and `--` produce `NS1001` and recover.
- Unicode identifiers with canonical keyword and alias mapping.
- Integer, float, string, and byte-string literals.
- Escape validation, byte-range checks, and unterminated-literal diagnostics.
- Longest-match operators, including `+%`, `-%`, `*%`, `/%`, `->`, and `=>`.
- Lexical diagnostics `NS1001`–`NS1005`.
- Compiler API `check` now reports lexical diagnostics before the parser stub.

Verification: 15/15 `astra-core` tests passed; `cargo test --workspace`, formatting, clippy, and the boundary smoke tests pass.

### 2026-10-24 — Syntax legend

Added `Docs/syntax-legend.md`:

- Source and comment rules.
- Identifiers, keywords, and aliases.
- Literals and escapes.
- Every operator and punctuation form, with special-role notes and precedence.
- Modules, imports, exports, and visibility.
- Declarations, types, statements, patterns, errors, memory, modes, concurrency, and FFI.
- Lookalike mistakes and an annotated example.

Added the legend to the root README, the specification's quick-reference links, and the agent read order. Documentation-only; link and UTF-8 checks pass.

### 2026-10-24 — Semicolon separator policy (D24)

Recorded D24:

- Newlines remain the normal statement and item boundaries.
- `;` is an optional soft separator for related statements/items on one line.
- Two constructs on one line must be separated by `;`.
- A trailing `;` is allowed but unnecessary.
- The `;` after a scope annotation is required.
- There is no automatic semicolon insertion.
- The formatter preserves explicit same-line `;` groups.

Updated the specification grammar, architecture, V1 language design, formatter rules, syntax legend, and decision traceability.

### 2026-10-24 — Parser and AST (PAR-001)

Implemented the recursive-descent parser and full AST:

- Module declarations, attributes, imports, exports, and visibility.
- Functions, structs, enums, constants, type aliases, and foreign modules.
- Composite types, function types, generics, and error sets.
- Statements, expressions, patterns, match arms, and failure construction.
- Scoped item/statement blocks and optional same-line `;` separators.
- Structured parser diagnostics `NS2001`–`NS2007` with recovery.
- Compiler API `check` now reports lexer and parser diagnostics before the resolver stub.

Verification: 29/29 `astra-core` tests passed; workspace tests, formatting, clippy, and boundary smoke tests pass.

### 2026-10-24 — Canonical AST formatter (FMT-001)

Implemented the core formatter:

- Deterministic, idempotent AST-based output.
- Canonical aliases, visibility, spacing, and 4-space indentation.
- Same-line `;` group preservation and required scope-header semicolons.
- Imports, exports, declarations, types, patterns, nested blocks, and match arms.
- JSON-RPC `format` method shared by native and Wasm boundaries.
- Comment preservation remains pending as `FMT-002`.

Verification: 36/36 `astra-core` tests passed; workspace tests, formatting, clippy, and boundary smoke tests pass.

### 2026-10-24 — Comment preservation (FMT-002)

Implemented comment trivia end to end:

- `LexResult` collects valid `//` comments with byte spans and trimmed text.
- `parse_with_comments` exposes comments without changing the AST contract.
- Formatter preserves leading, trailing, standalone, empty-body, module, field, variant, match-arm, and EOF comments exactly once.
- Comments inside inline expressions move to the nearest preserved syntax boundary.
- Canonical formatter idempotence holds with comments.
- Native JSON-RPC `format` preserves comments.

Verification: 47/47 `astra-core` tests passed; workspace tests, formatting, clippy, and boundary smoke tests pass.

### 2026-10-24 — Nova development runtime and NoPM (D25)

Recorded the environment layer:

- **Nova** is a development runtime that interprets typed `.ns` source directly. It reuses the Rust lexer, parser, resolver, and type checker; there is no second parser.
- Nova starts as a typed-HIR tree-walking interpreter with `nova run`, `nova repl`, `nova watch`, `nova test`, and `nova eval`.
- Production remains compiled through `astra build`. Nova may become a deployment runtime later, but it does not change the zero-VM production promise.
- **NoPM** is the universal package manager. It resolves path, Git, npm, and Cargo packages first, with later PyPI, NuGet, and OCI adapters.
- `core.lock.json` remains the single lockfile and records universal package IDs, integrity hashes, target variants, capabilities, lifecycle-script policy, and registry origin.
- The universal registry gateway at `registry.neovastra.dev` can host `nova:` packages and proxy upstream ecosystems. Local development does not require it.
- `core.json` / `core.toml` remains the only manifest, extended with `[nova]`, `[registry]`, and expanded `[packageSources]`.

Created `Docs/Design/V1/08-nova-and-nopm.md`; updated architecture, V1 design index, runtime, manifest/packages, tooling, agent docs, and the task queue. Nova and NoPM land after the type checker and before code generation.

### 2026-10-24 — Manifest formatting (FMT-004)

Implemented canonical manifest formatting:

- `@neo-astra/manifest` now exposes `parseManifestSource`, `formatManifestSource`, and `manifestFileForPath`.
- JSON manifests are emitted with canonical key order, two-space indentation, and a trailing newline; `packageSources` follows adapter order and dynamic maps are sorted.
- TOML manifests are formatted with Taplo; `#` comments are preserved and output is idempotent.
- `astra fmt` formats a project's root `core.json` / `core.toml` alongside its `.ns` files, or a manifest passed directly.
- `--check` and `--json` include manifest results.
- A syntax-invalid or duplicate-key manifest prevents all writes, including `.ns` files.
- BOM-prefixed manifests are normalized.
- The compiler client is created lazily, so manifest-only formatting does not require the native compiler binary.

Verification: `npm run ci` passed with 9/9 TypeScript tests, 12/12 boundary/CLI formatter tests, 47/47 `astra-core` tests, and 56 Rust tests total; Rust formatting and clippy passed.

### 2026-10-24 — Resolver (TYPE-001)

Implemented name, module, and export resolution in `astra-core`:

- `ModuleResolver` supports standalone source checks and strict resolution over a keyed module graph.
- Local value, type, and namespace symbol tables include declarations from top-level scoped blocks and `export default fn`.
- All project-path and JS/TS-style import forms resolve into bindings; named, default, namespace, re-export, star, and namespace-export forms are validated.
- Export surfaces retain value/type kinds, local/import/namespace/expression targets, and unresolved star sources.
- Visibility rules enforce private exports, duplicate exports, duplicate defaults, default value targets, and private-type non-leakage through function signatures, aliases, constants, struct fields, enum fields, foreign functions, and private namespaces.
- Structured diagnostics cover `NS3001`–`NS3013`, including duplicate declarations (`NS3011`), unresolved modules (`NS3012`), and cyclic named imports/re-exports (`NS3013`); conflicting visibility modifiers now report the spec-required `NS3002`.
- The compiler boundary now returns resolved HIR metadata for `check`/`compile` instead of the `NS9999` scaffold failure.
- HIR now retains source declarations for type checking and Nova, alongside resolved imports, symbols, and exports.

Verification: `npm run ci` passed with 9/9 TypeScript tests, 12/12 boundary/CLI tests, 73/73 `astra-core` tests, and 83 Rust tests total; Rust formatting and clippy passed.

### 2026-10-24 — Documentation reconciliation

Aligned the remaining design and agent docs with D25 and the completed formatter work:

- Spec §12 now includes `[nova]`, `[registry]`, universal package IDs, NoPM lockfile fields, and first/later adapters.
- Spec §16 now lists Nova, NoPM, and manifest formatting in the first slice.
- Architecture roadmap M1 now includes Nova and the path/Git/npm/Cargo package layer before code generation; M4 covers the registry and later adapters.
- Runbook Phase 6 is now “Nova and NoPM”; Phase 9 is FFI and concurrency.
- Tooling §9 documents the manifest formatter's JSON key order, TOML layout, duplicate-key rejection, BOM normalization, and trailing-newline rules.
- Tooling §14 uses `nopm add` / `nopm remove` instead of the removed `astra add` / `astra remove` surface.
- The state-log repository tree now distinguishes actual directories from planned `nopm`, `astra-nova`, and `registry` directories.

Verification: 22 Markdown files checked; all local links resolve.

### 2026-10-24 — Type checker (TYPE-002)

Implemented typed-HIR checking in `astra-core`:

- `checker::check_source`, `check_module`, and `check_resolved` run over parsed or resolved modules and return typed HIR plus expression annotations.
- Structural types cover primitives, tuples, arrays, slices, fixed arrays, maps, functions, named declarations, generics, `Option`, and `Result`.
- Local inference fills unannotated bindings and infers unambiguous generic-call arguments; function parameters, returns, and error effects remain explicit.
- Structs, enums, aliases, generic declarations, constructors, fields, methods, indexing, and lvalues are checked with arity and substitution validation.
- Exhaustiveness uses a recursive pattern matrix, so nested `Option`/`Result`, tuple, multi-field enum, and correlated boolean cases must genuinely be covered.
- Effect-tracked checking records propagated and handled error effects, validates declared error sets, rejects unhandled fallible calls, and accepts nominal failure construction.
- Mode checks cover `@requires`, `@tags`, scoped `@mode`, foreign-module system requirements, and scoped `@memory:manual`.
- The compiler API now runs parse → resolve → check for both `check` and `compile`; type and mode diagnostics stop program output.
- Generic struct initializers such as `Box<i32> { value: 1 }` now parse without ambiguity.
- Cleaned checker warnings and removed unused span/name data from resolved checker internals.

Verification: `npm run ci` passed with 9/9 TypeScript tests, 13/13 boundary/CLI tests, 109/109 `astra-core` tests, 120 Rust tests total, and clean Rust formatting and clippy.

---

## Active

### 2026-10-25 - Concurrent INTEROP-001 and C backend work (user)

- The user is actively implementing inline `@script` interop and the C backend.
- `ast::ItemKind::ScriptBlock` and `ScriptBlock` were added while `JS-001` was
  finishing. The checker, formatter, and resolver matches do not yet handle the
  new variant, so the workspace does not compile at this snapshot.
- `JS-001` work is committed in `2ce78ad`. Do not overwrite the active
  interop/C-backend files.
- Last green JS-001 verification before this concurrent change: 459 Rust tests,
  40 TypeScript unit tests, and 27 native/Wasm/boundary tests.

### 2026-10-24 - TYPE-003 project type surfaces (complete; boundary gap logged)

- Command: `cargo test --workspace`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `npm run ci`
- Result: pass
- Context: work began after the last logged handoff and was interrupted. The tree was left non-compiling: `checker.rs` imported `crate::type_surface`, but `lib.rs` did not declare the module, and `Checker::new` used `options` after moving it.
- Repair: declared `pub mod type_surface;`, captured `options.mode` before the struct move, collapsed a nested `if` for clippy, and renamed a test helper from the reserved word `private` to `unexported`.
- Implemented: `ModuleTypeSurface`, `module_type_surface`, `check_module_graph`, and `CheckOptions::module_surfaces` with imported-value and imported-type-definition checking.
- Verification: 123 `astra-core` tests (13 `type_surface`), 11 compiler API, 26 Nova library, 50 Nova CLI; 210 Rust tests total; `npm run ci` passes.
- Remaining: `astra-compiler-api` still checks a single module with `check_resolved`; Nova and the CLI do not use the graph checker yet.
- Reported gap (2026-10-24): project checking is unreachable across the compiler boundary because `CompileRequest` carries one `SourceUnit`. Imported symbols are `Type::Unknown` in the API, CLI, and Nova, so cross-module type errors pass silently. Filed as `TYPE-004` with acceptance criteria in `todo.md`; recorded as caveat 10 in `project-context-glossary.md` §8.
- Process defect recorded: an undeclared `src/type_surface.rs` left the build red while CI reported nothing, because unbuilt modules are invisible. Rules 7–8 added to `todo.md` and caveat 11 added to the glossary.

`NOVA-002` and `NOPM-001`–`NOPM-004` are complete; `TYPE-003` is complete and `TYPE-004` is the next task.

---

## Next

1. Finish `JS-001`: project-wide typed modules and import/re-export emission, recoverable error effects and `?`, tasks/channels, source maps, checked integer methods, and UTF-8 byte string operations.
2. Implement `TS-001` by reusing the JS typed-IR and emission infrastructure.
3. Implement the first `INTEROP-001` slice after both JS and TS backends exist.
4. Resolve the open questions in `Docs/spec-v0.1.md` §17 before V1 freeze, including inline-script acceptance, import-cycle policy, and TOML manifest key order.

See `todo.md` for task IDs and acceptance criteria.

---

## Known issues

| ID | Issue | Impact | Tracked by |
|---|---|---|---|
| KI-002 | A source file in `src/` that is not declared in `lib.rs` is never compiled | The file rots and the build can be red with no CI signal (this happened with `type_surface.rs`) | `todo.md` rules 7–8, glossary caveat 11 |

KI-001 (`CompileRequest` single-source, closed by `TYPE-004`) and the Nova
single-module limitation (closed by `NOVA-003`) are resolved. KI-002 is
prevented by making module declaration part of the completion gate.

---

## Blockers

None.

Known tensions to keep visible:

- UTF-8 core strings vs native JS strings
- Web Worker serialization cost
- Semantic policies vs modes
- D4 all-target architecture vs staged implementation
- Package ecosystem security

These are documented, not unresolved decisions.

---

## Locked decisions

D1–D25 are confirmed in `Docs/architecture.md`.

| ID | Short name |
|---|---|
| D1 | Configurable memory; GC default |
| D2 | Hybrid JS output |
| D3 | Hybrid compiler |
| D4 | Whole-system architecture |
| D5 | Custom syntax style |
| D6 | Integer policy |
| D7 | String policy |
| D8 | Effect-tracked errors |
| D9 | Structured concurrency |
| D10 | Polymorphic FFI |
| D11 | Ecosystem package sources |
| D12 | Neo Astra naming |
| D13 | Mode selection pipeline |
| D14 | Modes are capability layers |
| D15 | Least-privileged inference |
| D16 | Readability and AI-friendliness |
| D17 | `//` comments only |
| D18 | `core.json` or `core.toml` |
| D19 | TypeScript + Rust compiler split |
| D20 | Strictness policy |
| D21 | Memory precedence |
| D22 | Keyword aliases and canonical formatting |
| D23 | Declaration visibility, imports, and exports |
| D24 | Semicolon soft separators and scope-header separators |
| D25 | Nova development interpreter; NoPM universal package manager; extended manifest; production stays compiled |

Do not reopen these without an explicit user request.

---

## Environment

| Field | Value |
|---|---|
| Workspace | `C:\Projects\Neo Astra` |
| Platform | Windows |
| Shell | PowerShell |
| Source extension | `.ns` |
| CLI | `astra`, with `nova` and `nopm` |
| Manifest | `core.json` or `core.toml` |
| Lockfile | `core.lock.json` |
| Current docs | `Docs/architecture.md`, `Docs/spec-v0.1.md`, `Docs/Agent/*`, `Docs/Design/V1/*` |

---

## Repository files

```text
C:\Projects\Neo Astra\
  Cargo.toml
  Cargo.lock
  package.json
  package-lock.json
  tsconfig.json
  tsconfig.base.json
  Docs\
    architecture.md
    spec-v0.1.md
    Agent\
      agent.md
      system-prompt.md
      edge-case-protocol.md
      project-context-glossary.md
      definition-of-done.md
      runbook.md
      io-schemas.md
      state-log.md
      todo.md
    Design\
      V1\
        README.md
        01-language-design.md
        02-compiler-design.md
        03-runtime-and-memory.md
        04-targets-and-backends.md
        05-manifest-and-packages.md
        06-tooling-and-workflow.md
        07-quality-security-and-delivery.md
        08-nova-and-nopm.md
  orchestrator\
    README.md
    packages\
      cli\
      compiler-client\
      manifest\
      lsp\
      plugins\
      codegen-js\
      codegen-ts\
    tests\
      compiler-api.test.mjs
      format-cli.test.mjs
      manifest.test.mjs
  crates\
    README.md
    astra-core\
    astra-capabilities\
    astra-compiler-api\
    astra-nova\
    astra-wasm\
    astra-codegen-c\
    astra-codegen-wasm\
    astra-codegen-llvm
  scripts\
    bootstrap.mjs
    test-boundary.mjs
    ci.mjs
    README.md
  .github\
    workflows\
      ci.yml
```

Planned but not created: `orchestrator/packages/nopm/` and `registry/`.

The lexer, parser, formatter, resolver, type checker, typed-HIR Nova interpreter,
compiler client, CLI formatting, and manifest formatting are implemented; the
Nova CLI, NoPM, and real backends are not implemented yet.

---

## Verification

### 2026-10-23 — Agent docs

- Command: none
- Result: not run
- Reason: documentation-only task
- Manual check: files created under `Docs/Agent/`; links are relative to that folder.

### 2026-10-23 — V1 design docs

- Command: none
- Result: not run
- Reason: documentation-only task
- Manual check: files created under `Docs/Design/V1/`; relative links follow the folder structure; no compiler source was changed.

### 2026-10-23 — v0.1 specification

- Command: PowerShell relative Markdown link check
- Result: pass
- Evidence: all relative Markdown links under `Docs/` resolve.
- Manual check: `Docs/spec-v0.1.md` exists; contents and decision traceability are present; no compiler source was changed.
- Open gate: the spec is a draft. §17 questions must be resolved before V1 freeze, but they do not block Phase 3 scaffolding.

### 2026-10-23 — Keyword aliases (D22)

- Command: PowerShell relative Markdown link check and alias grammar presence check
- Result: pass
- Evidence: all relative Markdown links under `Docs/` resolve; all alias nonterminals and D22 are present in `Docs/spec-v0.1.md`.
- Manual check: D22 is recorded in `Docs/architecture.md`; aliases are exact lexical synonyms; formatter normalization is specified.

### 2026-10-24 — Declaration visibility and imports/exports (D23)

- Command: PowerShell relative Markdown link check and declaration grammar presence check
- Result: pass
- Evidence: links and UTF-8 checks pass; `private`, `export`, project-path and JS/TS import forms, re-export forms, default export, type-only erasure, `NS3001`–`NS3010`, and D23 are present in `Docs/spec-v0.1.md`.
- Manual check: private export rejection, normal-to-export promotion, and default-export value rules are specified; no compiler source was changed.

### 2026-10-24 — TypeScript orchestrator scaffold (SCAF-001)

- Command: `npm install`, `npm run build`, `npm test`, `npm run astra -- --help`, `npm run astra -- --version`
- Result: pass
- Evidence: workspace installed; TypeScript build passed; 4/4 tests passed; CLI help and version output are correct.
- Manual check: packages exist for CLI, manifest, LSP, plugins, and JS/TS codegen; the manifest package discovers exactly one `core.json` or `core.toml`; no Rust parser was introduced.

### 2026-10-24 — Toolchain installation

- Command: `winget install Rustlang.Rustup`, `rustup toolchain install stable-x86_64-pc-windows-gnu`, `rustup component add rustfmt clippy`, `cargo new/build/run`
- Result: pass
- Evidence: Rust 1.99.1, Cargo 1.99.1, rustfmt 1.10.0, clippy 0.1.99, `wasm32-unknown-unknown`, and `wasm32-wasip1` are installed; a temporary Cargo project built and ran successfully.
- Manual check: the GNU toolchain is the rustup default because the MSVC linker is unavailable without Visual Studio C++ Build Tools. The compiler core does not depend on MSVC.

### 2026-10-24 — Rust core scaffold (SCAF-002)

- Command: `cargo build --workspace`, `cargo test --workspace`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`
- Result: pass
- Evidence: all workspace crates built; 6/6 unit tests passed; formatting and clippy passed with warnings denied.
- Manual check: `astra-core` owns spans, diagnostics, AST, HIR, IR, and frontend boundaries; `astra-capabilities` owns the mode vocabulary; `astra-compiler-api` owns protocol types; backend crates are explicit stubs.

### 2026-10-24 — Bootstrap command (TOOL-001)

- Command: `npm run bootstrap:check`
- Result: pass for installed tools; clang/LLVM remains optional and missing
- Evidence: Node, npm, Git, CMake, rustup, cargo, rustc, rustfmt, and clippy are detected; `npm run bootstrap` installs missing dependencies and `npm run bootstrap:verify` builds and tests both workspaces.
- Manual check: Node.js 22+ is the only manual prerequisite; Windows falls back to the GNU Rust toolchain when the MSVC linker is unavailable.

### 2026-10-24 — Compiler API boundary (SCAF-003)

- Command: `cargo test --workspace`, `npm run test:boundary`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`
- Result: pass
- Evidence: 9/9 Rust tests passed; native JSON-RPC smoke test passed; Wasm linear-memory smoke test passed; formatting and clippy passed.
- Manual check: native and Wasm entrypoints share `astra-compiler-api`; unsupported frontend commands return structured JSON-RPC errors; no second parser exists.

### 2026-10-24 — Build and CI automation (SCAF-004)

- Command: `npm run ci`
- Result: pass
- Evidence: TypeScript build; 4/4 orchestrator tests; 2/2 native/Wasm boundary tests; Rust fmt, clippy, and 9/9 tests all passed.
- Manual check: `scripts/ci.mjs` is cross-platform; `.github/workflows/ci.yml` covers Ubuntu, Windows, and macOS; no target-specific grammar was introduced.

### 2026-10-24 — Lexer and spans (LEX-001)

- Command: `cargo test -p astra-core`, `cargo test --workspace`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `npm run test:boundary`
- Result: pass
- Evidence: 15/15 `astra-core` tests passed; workspace tests, formatting, clippy, and boundary smoke tests passed.
- Manual check: comments, Unicode identifiers, keywords/aliases, numeric/string literals, escapes, operators, newlines, BOM handling, and byte spans are covered; `check` reports lexical diagnostics before `NS9999`.

### 2026-10-24 — Syntax legend

- Command: PowerShell relative Markdown link and UTF-8 check
- Result: pass
- Evidence: `Docs/syntax-legend.md` is linked from the root README, spec quick-reference header, and agent read order; all relative links resolve; UTF-8 is valid.
- Manual check: the legend covers symbols, precedence, declarations, imports/exports, types, errors, memory, modes, concurrency, FFI, and lookalike mistakes.

### 2026-10-24 — Semicolon separator policy (D24)

- Command: PowerShell relative Markdown link and UTF-8 check
- Result: pass
- Evidence: D24 is present in architecture and the specification traceability table; semicolon rules appear in the grammar, syntax legend, and V1 formatter rules; all relative links resolve and UTF-8 is valid.
- Manual check: newlines remain the normal boundary; same-line constructs require `;`; scope headers keep a required `;`; no ASI.

### 2026-10-24 — Parser and AST (PAR-001)

- Command: `cargo test -p astra-core`, `cargo test --workspace`, `npm run ci`
- Result: pass
- Evidence: 29/29 `astra-core` tests passed; workspace tests, formatting, clippy, native/Wasm boundary tests, and `npm run ci` passed.
- Manual check: module/import/export/item grammar, functions, structs, enums, constants, type aliases, foreign modules, types, expressions, patterns, scoped blocks, semicolon boundaries, and parser recovery are covered.

### 2026-10-24 — Canonical AST formatter (FMT-001)

- Command: `cargo test -p astra-core`, `npm run ci`
- Result: pass
- Evidence: 36/36 `astra-core` tests passed; native format boundary test passed; workspace tests, formatting, clippy, and boundary smoke tests passed.
- Manual check: canonical aliases, indentation, nested blocks, match arms, same-line semicolon groups, scope headers, imports/exports, and types are covered. Comment preservation was still pending at this point and was completed in `FMT-002`.

### 2026-10-24 — Comment preservation (FMT-002)

- Command: `cargo test --workspace`, `npm run ci`
- Result: pass
- Evidence: 47/47 `astra-core` tests passed; native and Wasm boundary tests passed; workspace tests, formatting, clippy, and `npm run ci` passed.
- Manual check: leading, trailing, standalone, empty-body, module, field, variant, match-arm, expression-position, and EOF comments are preserved exactly once; formatted output with comments is idempotent.

### 2026-10-24 — CLI formatter client and wiring (FMT-003)

- Command: `npm run build`, `npm run test:boundary`, `npm run ci`
- Result: pass
- Evidence: `@neo-astra/compiler-client` speaks JSON-RPC 2.0 to the native binary; `astra fmt` formats individual files and recursive directories; `--check`, `--json`, and `--compiler` are covered; 8/8 native/Wasm and formatter boundary tests passed; 4/4 TypeScript tests passed; 47/47 `astra-core` tests and all workspace tests passed; Rust formatting and clippy passed.
- Manual check: comments and canonical aliases survive the CLI round trip; build directories are excluded; `--check` reports differences without writing; when any source has an error, no file is written; invalid UTF-8, missing binaries, malformed JSON-RPC responses, and source diagnostics produce structured errors.
- Scope: `.ns` source only. Manifest formatting remains `FMT-004`.

### 2026-10-24 — Nova and NoPM design (D25)

- Command: PowerShell relative Markdown link and UTF-8 check
- Result: pass
- Evidence: `Docs/Design/V1/08-nova-and-nopm.md` exists; D25 is recorded in architecture and the decision tables; all relative links resolve; UTF-8 is valid.
- Manual check: Nova reuses typed HIR and has no second parser; NoPM uses universal package IDs and `core.lock.json`; the registry gateway is optional for local development; the manifest remains exactly one of `core.json` or `core.toml`.

### 2026-10-24 — Manifest formatting (FMT-004)

- Command: `npm run ci`
- Result: pass
- Evidence: 9/9 TypeScript manifest/CLI tests; 12/12 native/Wasm and formatter boundary tests; 47/47 `astra-core` tests; 56 Rust tests total; Rust formatting and clippy passed.
- Manual check: JSON key order is canonical; TOML `#` comments survive repeated formatting; manifest-only projects do not require the compiler binary; invalid manifests stop all writes; BOM-prefixed manifests are normalized; direct `core.toml` formatting and `--check` work.

### 2026-10-24 — Resolver (TYPE-001)

- Command: `npm run ci`
- Result: pass
- Evidence: 9/9 TypeScript tests; 12/12 native/Wasm and formatter boundary tests; 73/73 `astra-core` tests; 83 Rust tests total; Rust formatting and clippy passed.
- Manual check: the 25 resolver tests cover symbol tables, direct and promoted exports, all import forms, strict module graphs, missing exports/modules, self-contained re-exports, star conflicts, explicit-export precedence, type-only star exports, default-export rules, duplicate declarations/bindings/exports, scoped items, generic parameters, private type/namespace leaks, and named cycles.
- Open gate: import-cycle acceptance remains proposal 16 in spec §17; the current resolver rejects named cycles with `NS3013`.

### 2026-10-24 — Type checker (TYPE-002)

- Command: `npm run ci`
- Result: pass
- Evidence: 9/9 TypeScript tests; 13/13 native/Wasm, type-diagnostic, and formatter boundary tests; 109/109 `astra-core` tests; 120 Rust tests total; Rust formatting and clippy passed.
- Manual check: positive and negative tests cover primitive/composite types, aliases, generic structs/enums/calls, struct construction and fields, arrays/maps/tuples/indexing, argument and return boundaries, unknown identifiers/types, mutability, all-path returns, recursive nested exhaustiveness, correlated tuple/enum patterns, effect handling and propagation, declared error sets, failure construction, mode requirements, capability tags, scoped mode narrowing, foreign-module requirements, and manual memory.
- Compiler API: `check` and `compile` now reject `NS4001`/`NS5001` inputs before producing a program; Rust API tests cover both paths.
- Open gates: capability inference (`MODE-001`) and the broader error-model task (`ERR-001`) remain separate; imported modules outside the module graph stay opaque by design.

### 2026-10-24 — Nova typed-HIR interpreter (NOVA-001)

- Command: `cargo test --workspace`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `npm run ci`
- Result: pass
- Evidence: 26/26 `astra-nova` tests; 110/110 `astra-core` tests; 147 Rust tests total including 11 compiler API tests; Rust formatting and clippy passed; `npm run ci` passed end to end with 9/9 TypeScript tests and 13/13 boundary/CLI tests.
- Manual check: the interpreter consumes `CheckResult` HIR and never re-parses source; tests cover hello-world output, recursion, structs, enums, tuple structs, arrays and bounds panics, insertion-ordered maps, mutable tuples, Unicode string semantics, `Option`, `Result`, matching, `!` propagation, deferred cleanup order, panic stacks, abort without defers, deterministic eager tasks with inferred `Task<T>` results, eager-task panic scoping, checked and wrapping integer behavior, constants, host stderr, imported-value rejection, missing entry points, and frontend diagnostic forwarding.
- Manual memory: the checker now exposes every `@memory:manual` span through `CheckResult.manual_memory_scopes`; Nova rejects those programs before execution with `NV0001`, even in system mode.
- Runtime diagnostics: stable `NV0000`-`NV0007` codes are documented in spec §14.7. Frontend diagnostics keep their original `NSxxxx` codes.
- Scope boundary: one checked module executes at a time. Imported runtime values and foreign modules fail clearly when reached; project-wide linking, FFI, channels, OS threads, and Web Workers remain later work.

### 2026-10-24 — Nova project linking design (NOVA-003A)

- Command: documentation review against `resolver.rs`, `hir.rs`, `checker.rs`, `interpreter.rs`, `value.rs`, `error.rs`, and spec §3.16/§12.5
- Result: pass
- Evidence: `Docs/Design/V1/09-nova-project-linking.md` defines canonical `ModuleId` values, a resolver `ModuleGraph` import map, a deterministic loader, module-aware runtime tables, imported function/constant/struct/enum/namespace resolution, module-aware diagnostics, and a staged implementation plan.
- Key decision: one `Interpreter` owns many `RuntimeModule` tables and a `current_module` context; it is not one interpreter per module.
- Key prerequisite: `TYPE-003` must resolve imported symbol types through target export surfaces. Per-module checking still treats imported types as `Unknown`, so multi-module execution is not fully type-safe until it lands.
- Open decisions: import-cycle policy (spec §17 proposal 16), anonymous default-expression retention in HIR, external `std:`/bare module handling, and Windows path/case normalization.

### 2026-10-24 — Nova CLI foundation (NOVA-002A)

- Command: `cargo test -p astra-nova`, `cargo fmt --all --check`, `cargo clippy -p astra-nova --all-targets -- -D warnings`, `npm run ci`
- Result: pass
- Evidence: 26 Nova library tests and 31 Nova CLI integration tests; 110 `astra-core` tests; 11 compiler API tests; 178 Rust tests total; Rust formatting and clippy passed; `npm run ci` passed end to end with 9/9 TypeScript tests and 13/13 boundary/CLI tests.
- Implementation: `crates/astra-nova/src/bin/nova/main.rs` provides `nova run`, `nova eval`, `nova version`, help, `--mode`, text diagnostics, and one-object JSON reports. Program stdout/stderr are captured in JSON mode.
- Diagnostics: frontend `NSxxxx` codes are preserved with paths and one-based line/column positions; runtime failures use `NVxxxx`; usage errors exit 2, source/input/runtime failures exit 1.
- Eval behavior: `nova eval` accepts a complete module or a bare expression/statement list, wrapping the latter in a temporary `main` block while still parsing through `astra-core`.
- Follow-ups: `nova repl`, `nova watch`, `nova test`, manifest `[nova].entry` discovery, packaged install entry, and project linking after `TYPE-003`.
- Developer entry: `npm run nova -- <command>` wraps the Rust binary.

### 2026-10-24 — Nova REPL, watch, and test runner (NOVA-002B)

- Command: `cargo test -p astra-nova`, `cargo fmt --all --check`, `cargo clippy -p astra-nova --all-targets -- -D warnings`, `npm run ci`
- Result: pass
- Evidence: 26 Nova library tests and 42 Nova CLI integration tests; 110 `astra-core` tests; 11 compiler API tests; 189 Rust tests total; Rust formatting and clippy passed; `npm run ci` passed end to end with 9/9 TypeScript tests and 13/13 boundary/CLI tests.
- REPL: line-oriented session persists declarations, `let` bindings, and assignments through a synthetic checked module rebuilt per input; errors do not end the session; `:help`, `:quit`, and JSON mode are supported.
- Watch: portable polling watcher for `nova watch` and `nova run --watch`; `--once` makes the path deterministic for tests.
- Test runner: recursive `*.test.ns` discovery, `@test` execution, `main` fallback, parameter validation, per-test output, summaries, and `--watch` reruns.
- Scope: manifest `[nova].entry` discovery, test filters/parallelism, project linking, and hot replacement remain.

### 2026-10-24 — Nova manifest entry discovery (NOVA-002C)

- Command: `cargo test -p astra-nova`, `cargo fmt --all --check`, `cargo clippy -p astra-nova --all-targets -- -D warnings`, `npm run ci`
- Result: pass
- Evidence: 26 Nova library tests and 50 Nova CLI integration tests; 110 `astra-core` tests; 11 compiler API tests; 197 Rust tests total; Rust formatting and clippy passed; `npm run ci` passed end to end.
- Implementation: `nova run` and `nova watch` now resolve `[nova].entry` from exactly one `core.json` or `core.toml` when no file is supplied. JSON and TOML manifests are supported.
- Validation: rejects missing/ambiguous manifests, missing `[nova].entry`, non-`.ns` entries, absolute paths, paths escaping the project root, and missing files.
- Boundary: full manifest validation and formatting remain owned by the TypeScript manifest package and `astra fmt`; Nova reads only the development entry path.

### 2026-10-24 — NoPM universal package model (NOPM-001A)

- Command: `npm run build`, `node --test orchestrator/tests/nopm.test.mjs`, `npm run ci`
- Result: pass
- Evidence: `@neo-astra/nopm` parses and canonically formats `nova:`, `npm:`, `cargo:`, `git+`, and `path:` IDs; structured descriptors; alias/spec validation; deterministic dependency graph ordering; cycle detection; missing/duplicate descriptor diagnostics; frozen graph outputs; 9 NoPM tests.
- Diagnostics: provisional `NP0001` invalid package ID, `NP0002` invalid dependency declaration or duplicate descriptor, `NP0003` missing descriptor, `NP0004` dependency cycle, `NP0005` conflicting root alias.
- Boundary: offline model only. No network, registry, lockfile writing, store, install commands, lifecycle scripts, or target/mode filtering yet.
- Verification: `npm run ci` passes with 18 TypeScript tests, 13 boundary tests, 110 `astra-core` tests, 11 compiler API tests, 26 Nova library tests, 50 Nova CLI tests, and 197 Rust tests total.

### 2026-10-24 — NoPM target/mode resolution and lockfile model (NOPM-001B)

- Command: `npm run build`, `node --test orchestrator/tests/nopm.test.mjs`, `npm run ci`
- Result: pass
- Evidence: target-aware package filtering by target name, backend, or triple; capability-derived mode checks; multi-target resolution; canonical `core.lock.json` creation, formatting, parsing, and validation; 6 additional NoPM tests (15 total).
- Diagnostics: added provisional `NP0006` target incompatibility, `NP0007` mode incompatibility, and `NP0008` invalid lockfile or missing integrity/commit.
- Lockfile rules enforced: version 1, canonical package IDs, sorted packages/dependencies, remote integrity, pinned Git commits, lifecycle-script flag, deterministic JSON key order, and trailing newline.
- Boundary: lockfile model only. File I/O, staleness checks, content-addressed store, adapters, and install commands remain.
- Verification: `npm run ci` passes with 24 TypeScript tests, 13 boundary tests, 110 `astra-core` tests, 11 compiler API tests, 26 Nova library tests, 50 Nova CLI tests, and 197 Rust tests total.

### 2026-10-24 — NoPM adapters, CLI, store, and registry client (NOPM-001C/NOPM-002/NOPM-003 partial)

- Command: `npm run build`, `node --test orchestrator/tests/nopm.test.mjs`, `npm run ci`
- Result: pass
- Evidence: 27 NoPM tests and 4 registry tests; 40 TypeScript tests total; 13 boundary tests; 197 Rust tests total; `npm run ci` passed end to end.
- Lockfile: added stable manifest hashing, staleness checks, missing-lockfile diagnostics, atomic reads/writes, and canonical formatting.
- Store: added content-addressed installation with integrity verification, tarball extraction, Git cache copying, store index generation, and path/remote package support.
- CLI: added `nopm lock`, `nopm tree`, `nopm install`, `nopm version`, and `nopm help`; `--locked`/`--frozen` verify the manifest hash and block stale installs.
- Adapters: local path, Git pinned to commits, npm via the local npm CLI with lifecycle scripts disabled, Cargo ranges/exact versions through crates.io, PyPI, NuGet, and OCI distributable layers.
- Registry: client and an in-process HTTP gateway are implemented for exact `nova:` packages, including publish, versions, search, yank, audit, Ed25519 signing/verification, and upstream mirroring. Production key management and federation hardening remain future work.
- Diagnostics: added `NP0009` stale lockfile, `NP0010` lockfile write failure, `NP0011` adapter/path/fetch/install failure, and `NP0012` CLI usage.

### 2026-10-24 — TYPE-003 build repair and core surfaces

- Command: `cargo test --workspace`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `npm run ci`
- Result: pass
- Evidence: 210 Rust tests total (123 `astra-core`, 11 compiler API, 26 Nova library, 50 Nova CLI); 13 new `type_surface` tests; 40 TypeScript tests; 13 boundary tests; `npm run ci` passed end to end.
- Repair: the tree did not compile because `checker.rs` imported an undeclared `crate::type_surface` module and `Checker::new` used `options` after moving it. Added the module declaration, captured `options.mode` first, fixed a clippy `collapsible_if`, and renamed a test helper from the reserved word `private`.
- Scope: core-level `check_module_graph` is available and tested; compiler API, Nova, and CLI wiring remain.

### 2026-10-25 — TYPE-004 multi-source compiler API

- Command: `cargo fmt --all --check`, `cargo test -p astra-compiler-api`, `npm run build`, `npm run test:boundary`, `npm run ci`
- Result: pass
- Repair: removed a duplicated trailing block in `crates/astra-compiler-api/src/lib.rs` that left an unmatched `)` after the test module; formatting then passed.
- API: added `ProjectCheckRequest`, `ProjectSourceUnit`, `ModuleCheckResult`, `ProjectCheckResponse`, and `check_project`; exposed the additive JSON-RPC `check_project` method without changing the single-source `check`/`compile` shapes.
- Semantics: sources are parsed up front, checked through `check_module_graph`, and diagnostics are attributed to module key/path. Responses preserve request order and include `program` plus `metadata.moduleOrder`, `target`, and `mode`.
- Client: `@neo-astra/compiler-client` now exposes and validates `checkProject()`.
- Tests: 7 compiler API unit tests and 3 boundary integration tests cover valid two-module projects, cross-module type/argument failures, module ordering, empty sources, and per-module parse errors.
- Verification: `npm run ci` passes with 40 TypeScript tests, 16 boundary tests, 123 `astra-core` tests, 18 compiler API tests, 26 Nova library tests, 50 Nova CLI tests, and 217 Rust tests total.

### 2026-10-25 — MODE-001 mode inference and ERR-001 effect provenance

- Command: `cargo fmt --all --check`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `npm run ci`
- Result: pass
- Evidence: 413 Rust tests (13 `astra-capabilities`, 24 compiler API, 135 `astra-core` library, 35 checker stress, 26 formatter, 26 lexer, 19 parser, 20 resolver, 26 Nova library, 53 Nova CLI, 36 interpreter stress); 40 TypeScript tests; 18 native/Wasm boundary tests; Rust formatting and clippy passed; `npm run ci` passed end to end.
- MODE-001: implemented `astra-capabilities` with `ModeInference`, `ModeReason`, `ModeReasonKind`, containment `join_modes`, `infer_module_mode`, `infer_module_mode_named`, `infer_project`, and `ModeInference::explain`.
- Inference inputs: module/item `@requires`, `@tags`, foreign modules, manual-memory scopes, console built-ins, scoped `@mode:` blocks, and a local call graph propagated to a fixed point including recursion.
- Inference joins: `core`/`app`/`web`/`system` use containment, not rank. `web` plus `system` is unsatisfiable and reports `NS5006`. Explicit module requirements narrower than the project requirement report `NS5001`.
- Compiler API: an omitted `mode` is inferred; an explicit `mode` remains authoritative and is still validated. Responses expose `mode`, `modeSource`, `requiredMode`, `inferredModes`, `modeReasons`, and `modeExplanation`.
- Nova: `NovaOptions::infer_mode` defaults to true; `run`, `eval`, `repl`, `watch`, and `test` resolve the mode before checking. `--mode` disables inference. JSON reports use the resolved mode. `resolve_source_mode` exposes the decision to library callers.
- ERR-001: `ErrorEffect` now carries a span; expression and lvalue results carry effect provenance; effects merge deterministically by nominal type while retaining the first site. Unhandled effects report `NS4011` at the call/failure site, and propagation reports `NS4012`/`NS4013` at the propagation site.
- Failure construction now rejects non-nominal, non-constructible, and ambiguous multi-variant enum errors; named fields validate types at the field value, and missing/duplicate fields use `NS4016`. Exhaustive `match` consumes the scrutinee effect; a non-exhaustive match reports `NS4010` only.
- Cross-module error effects are preserved through type surfaces. Imported type definitions are now registered before local signatures are resolved, so an imported type in `! E` resolves instead of becoming `NS4003` plus `NS4012`. Regression tests cover precise spans, duplicate/multiple effects, arguments and collections, match handling, malformed failures, cross-module effects, and generic function-value calls.
- Repairs: `None` now has an unconstrained `Option<Unknown>` payload so `None.unwrap_or(value)` works; stale parser/checker/interpreter stress tests were corrected for `_` bindings, enum syntax, expected `NS3002`, string lengths, and literal `\n` escapes.
- Docs: specification, architecture, tooling/Nova design, runbook, glossary, README, todo, compiler-client comments/types, and this state log updated.
- Next: implement `NOVA-003` project module loading and runtime linking.

### 2026-10-25 — INTEROP-001 inline `@script` design

- Command: documentation-only change; no build or test run required.
- Result: design complete, implementation pending.
- Proposal: added `Docs/Design/V1/10-inline-script-interop.md` for declaration-level `{@script:<language> as <name>; ... }` blocks.
- Syntax: required `@mode(...)`, typed `@exports { ... }`, verbatim `@impl` body, and an explicit `@end` line followed by the closing `}`. The body never enters the Nova parser or checker.
- Boundary: calls use a module-local namespace such as `math.add(...)`; exported signatures use the existing FFI type map; unsupported or unsafe signatures fail before code generation.
- Targets: JS/TS first on JS targets, C/C++ second on native through a C ABI, Python deferred behind an embedded runtime, Java deferred until a JVM backend. Nova rejects script calls with `NV0008` in V1.
- Diagnostics: provisional `NS7001`–`NS7008` for unknown languages, target mismatch, malformed blocks, unsafe exports, namespace collisions, manifest mappings, and link failures.
- Security: raw bodies are opaque and unsafe; `@mode` is trusted, not verified; `astra check`/`fmt` never execute foreign code; NoPM lifecycle rules are unchanged.
- Docs: updated the specification open questions, architecture D10 extension, V1 design baseline (`01`, `02`, `04`, `05`, `06`, `07`, `08`, `README`), runbook, glossary, syntax legend, root README, todo, and this state log.
- Next: keep `NOVA-003` as the next implementation task. The order is `JS-001` → `TS-001` → the first `INTEROP-001` slice; interop does not start until both JS and TS backends exist.

### 2026-10-25 - NOVA-003 project module loading and runtime linking

- Command: `cargo test --workspace`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `npm run ci`
- Result: pass
- Loader: `crates/astra-nova/src/project.rs` derives canonical `ModuleId`s from project-relative paths, validates root containment, rejects invalid/case-colliding ids, detects missing modules and cycles, loads UTF-8 sources once, checks dependencies first, and returns sorted/deduplicated module diagnostics.
- Resolver: `crates/astra-core/src/resolver.rs` adds `ModuleGraph` with an explicit `(importer, specifier) -> canonical target` map and specifier collection; `ModuleResolver::from_graph` is used by the project loader.
- Runtime: one `Interpreter` owns per-module function/constant/struct/enum/annotation tables. `FunctionValue`, constructors, enum namespaces, module namespaces, fault attribution, and panic frames carry module identity. Imported functions/constants, named/tuple structs, enums, namespace imports, named/star re-exports, and default functions link across modules.
- Type surface: `module_type_surface_with_surfaces` propagates re-exported values, namespace members, and imported type definitions; the checker builds imported tuple-struct constructors and resolves imported enum members.
- CLI: `nova run` and watch use the project loader, while `nova eval` remains a single in-memory module. Text and JSON diagnostics use module paths.
- Tests: 7 new project-loader integration tests and expanded project/runtime/type-surface/resolver tests cover multi-file loading, dotted paths, re-exports, namespaces, defaults, tuple structs/enums, constants, side-effect imports, cross-module panics/faults, manual-memory preflight, invalid UTF-8, case mismatches, root escapes, absolute paths, and deterministic diagnostics.
- Verification evidence: 456 Rust tests, 40 TypeScript tests, 18 native/Wasm boundary tests, formatting, clippy with warnings denied, and `npm run ci` all pass.
- Known boundary: anonymous default expressions remain `NV0003`; external/`std:` modules remain rejected until backend adapters exist; namespace enum/struct type checking is intentionally opaque where the surface cannot carry a nested type identity.

### 2026-10-25 - JS-001 first JavaScript ESM slice

- Command: `cargo test --workspace`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `npm run build`, `npm run test:boundary`, `npm run ci`
- Result: pass
- Compiler API: added `IntegerPolicy`, `ModuleIr`, and the typed `ir` payload on `compile`; added serde serialization to the AST, HIR, semantic types, and `TypeAnnotation`; `compile` now rejects non-JS/TS targets with `NS8001`.
- JS backend: implemented `orchestrator/packages/codegen-js` with serialized typed-module definitions and a readable ESM emitter. It supports functions, constants, locals, control flow, defer, calls, assignments, arrays, structural maps, tuples, structs, enums, options, results, match patterns, built-ins, checked collection/string/option/result methods, named/default exports, and tree-shaken local helpers.
- Integer semantics: wrapping helpers for fixed-width signed/unsigned integers, `BigInt` for `i64`/`u64`, and checked helpers for `safe-checked`. Floats are tagged objects so `f64`/`f32` rendering preserves `.0`; `f32` operations use `Math.fround`. The `arbitrary` policy and checked integer methods are explicitly unsupported.
- Representation: `Option` and `Result` are tagged objects; maps use a structural-key helper so key equality and insertion order match the language contract. Generated code has no framework dependency, no hidden global state, and no source re-parsing.
- CLI: `astra build [dir] [--target name] [--compiler path] [--json]` selects a target (`--target`, `ASTRA_TARGET`, `build.defaultTarget`), reads `nova.entry`, chooses target/build mode and `math.integers`, calls `compile`, emits one ESM module under `outDir`, and writes `{"type":"module"}` when absent.
- Diagnostics: backend failures use `NS8001` and `NS8003` with source locations; project imports, error propagation, tasks, arbitrary integers, and manual memory fail before emission.
- Tests: 11 new end-to-end codegen/build tests compile real `.ns` sources, execute generated ESM, check struct/enum/option/map/tuple/defer/float/byte-string rendering, named/default exports, verify wrapping and checked overflow behavior, reject unsupported constructs, and run `astra build` output through Node.
- Verification: 459 Rust tests, 40 TypeScript unit tests, 29 native/Wasm/boundary tests, `cargo fmt --check`, clippy with warnings denied, and `npm run ci` pass.
- Current boundary: `astra build` emits one module and rejects project imports with `NS8001`; recoverable error effects, `?`, `fail`, spawn/tasks/channels, source maps, `.d.ts`, checked integer methods, `arbitrary`, and full UTF-8 byte slicing remain.

### 2026-10-26 — C-001 full expression/pattern coverage, byte/string operations, map methods, inline scripts (@script:c/@script:cpp), and native build flags

- Command: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `npm test`, `node --test orchestrator/tests/codegen-js.test.mjs`
- Result: pass
- Map operations: added `.insert(k, v)` emitting `Option<V>` via `astra_map_get` and `astra_map_set`; added `for` loop over map keys via `astra_map_key_at(&map, idx)`.
- Byte and string operations:
  - Added byte indexing (`b[i]`) returning `u8` via `astra_bytes_get(b, i)`.
  - Added string and byte slicing (`s.slice(start, len)`, `b.slice(start, len)`).
  - Added string and byte comparisons (`==`, `!=`, `<`, `<=`, `>`, `>=`) via `astra_str_eq`, `astra_str_cmp`, `astra_bytes_eq`, `astra_bytes_cmp`.
  - Added `for` loop over bytes yielding `u8` via `astra_bytes_get`.
  - Added `.len()`, `.is_empty()`, `.slice()` to `Type::Bytes` in type checker.
- Pattern matching:
  - Added string and byte literal matching in `match` expressions (`astra_str_eq`, `astra_bytes_eq`) and identifier fallback bindings.
- Inline scripts (`INTEROP-001` Slice D):
  - Emits per-block translation units (`script_<name>.c` / `script_<name>.cpp`) with freestanding headers `<stdint.h>`, `<stdbool.h>`, `<stddef.h>`.
  - Generates C ABI prototypes in `main.c` and `extern "C"` declarations in C++ scripts.
  - Dispatches calls `math.add(...)` -> `add(...)`.
  - Rejects non-C/C++ script languages with `NS7002`.
  - Returns `c-script` artifacts in `astra-compiler-api`.
  - In `astra build`, CLI discovers and links all script units with `-lstdc++` when C++ is present.
- Native build flags / options in `astra build`:
  - `cflags` array passed to native compiler.
  - `optLevel`/`optimization` (`-O0`, `-O2`, `-O3`, etc.).
  - `triple`/`targetTriple` passed via `--target=<triple>`.
- Verification: 55 tests passing in `astra-codegen-c`, 142 in `astra-core`, 27 in `astra-compiler-api`, 51 in `astra-nova`, 40 in `orchestrator`, 12 in `codegen-js.test.mjs`, all clean with clippy zero warnings.

---

### 2026-10-25 - JS-001 recoverable errors and checked integer methods

- Command: `npm run ci`
- Result: pass
- Error effects: fallible functions now return `Ok(...)` for normal completion and a tagged `{ $failure: true, error }` sentinel for `!Error(...)` or a propagated `?`. A function-boundary `try`/`catch` converts a failure signal into the sentinel, `?` unwraps `Ok`, and `match` normalizes a failure sentinel to `Err` before pattern lowering.
- Failure construction: `!Error(...)` lowers through the existing struct/enum constructors; built-in nominal errors such as `ParseIntError` use the same tagged shape as the collection/string helpers.
- Checked integers: `checked_add`, `checked_sub`, `checked_mul`, and `checked_div` lower to a tree-shaken `$checkedInteger` helper that returns `Result` and `OverflowError` on overflow. Division by zero remains a panic, matching Nova.
- Tests: added execution tests for `!Error(...)`, `?` through a call chain, matching a failure sentinel, `to_int()` failure, and checked integer overflow/success. `orchestrator/tests/codegen-js.test.mjs` now has 12 codegen/build tests.
- Verification: 459 Rust tests, 40 TypeScript unit tests, 30 native/Wasm/boundary tests, `cargo fmt --check`, clippy with warnings denied, and `npm run ci` pass.
- Current boundary: project imports/re-exports, spawn/tasks/channels, source maps, `.d.ts`, the `arbitrary` integer policy, and full UTF-8 byte-slice/index operations remain.

### 2026-10-25 - JS-001 eager spawn and task scope

- Command: `npm run ci`
- Result: pass
- Tasks: `spawn || { ... }` now runs the body eagerly through a tree-shaken
  `$spawn` helper and returns a task value. `Task.join()` returns the task value,
  re-raises a captured panic, or returns a failure value for a cancelled task;
  `Task.cancel()` marks the task cancelled.
- Scope: a function containing spawn enters a task scope and checks unjoined
  tasks in a `finally` block. A task panic that was never joined fails the
  function after its side effects, matching Nova's scope-failure behavior for
  the supported single-scope case.
- Tests: added execution tests for eager ordering plus `join`, a joined-task
  panic, and an unjoined task panic failing the function scope. The codegen test
  file now has 15 tests.
- Verification: 459 Rust tests, 40 TypeScript unit tests, 33 native/Wasm/boundary
  tests, `cargo fmt --check`, clippy with warnings denied, and `npm run ci` pass.
- Current boundary: project imports/re-exports, channels and a real scheduler,
  source maps, `.d.ts`, the `arbitrary` integer policy, and full UTF-8
  byte-slice/index operations remain.

### 2026-10-26 - JS-001 line-level source maps

- Command: `npm run ci`
- Result: pass
- Source maps: `generateJavaScript` accepts optional source text and emits a
  version-3 map with the module path, embedded `sourcesContent`, and VLQ mappings
  anchored at each emitted top-level declaration.
- Build output: `astra build` writes `<module>.js.map`, rewrites `sources` to be
  relative to the output directory, and appends a `sourceMappingURL` comment to
  the generated module.
- Tests: the end-to-end build test now checks the `.map` file, source content,
  `file` field, non-empty mappings, and the sourceMappingURL comment.
- Verification: 459 Rust tests, 40 TypeScript unit tests, 33 native/Wasm/boundary
  tests, `cargo fmt --check`, clippy with warnings denied, and `npm run ci` pass.
- Current boundary: project imports/re-exports, channels and a real scheduler,
  the `arbitrary` integer policy, `.d.ts`, and full UTF-8 byte-slice/index
  operations remain.

### 2026-10-26 - JS-001 linked multi-module projects

- Command: `npm run ci`
- Result: pass
- Compiler API: `check_project` now accepts relative and dotted import
  specifiers, builds a canonical `ModuleGraph`, orders modules dependencies-first,
  and attaches a `ModuleIr` to each successful module. `integerPolicy` is carried
  into every module. Existing identity-key imports remain compatible.
- Core: added `check_module_graph_with_imports`; the original
  `check_module_graph` continues to treat raw specifiers as module identities.
- Codegen: emits named, default, and namespace imports plus named, namespace,
  default, and star re-exports, using caller-supplied output specifiers.
- CLI: `astra build` discovers local modules recursively from relative imports,
  calls `check_project`, emits one ESM module per source, rewrites relative
  specifiers, writes per-module source maps, and still writes the output
  `package.json`.
- Tests: a multi-module build test covers a library, a barrel with named/star/
  namespace re-exports, default import, namespace import, execution, and import
  rewriting. Compiler API coverage includes relative imports and project IR.
- Verification: 460 Rust tests, 40 TypeScript unit tests, 34 native/Wasm/boundary
  tests (16 codegen/build), `cargo fmt --check`, clippy with warnings denied, and
  `npm run ci` pass.
- Current boundary: external/package module linking, channels and a scheduler,
  the `arbitrary` integer policy, `.d.ts`, and full UTF-8 byte-slice/index
  operations remain.

### 2026-10-26 - INTEROP-001 JavaScript backend slice

- Command: `npm run ci`
- Result: pass
- Merged the `INTEROP-001` frontend snapshot (lexer/parser/AST/HIR, checker,
  resolver, formatter, capabilities, Nova rejection) with the `JS-001` backend
  work. Temporary script-block guards were replaced by the real frontend.
- Codegen: `@script:js` blocks emit a separate `*.js` ESM artifact under the
  output tree. The parent imports the namespace, wraps each declared export,
  validates `export` names with `NS7008`, converts `Option<T>` to/from
  `T | null`, and maps declared error effects to `Err` through try/catch.
- CLI: writes script artifacts and their source maps beside the parent module,
  rewrites map sources, and rejects output paths that escape `outDir`.
- Tests: inline script execution (ints, strings, `Option` arguments and returns),
  missing export diagnostics, exception-to-`Err` mapping, and rejection of
  `@script:ts` on the JavaScript target.
- Also implemented the `arbitrary` integer policy with `BigInt` arithmetic for
  all integer types, `usize`/`isize`, length values, index coercion, and explicit
  wrapping methods.
- Added `.d.ts` artifacts for generated modules and inline script modules,
  covering functions, constants, structs, enums, type aliases, re-exports,
  `Option`, `Result`, tuples, maps, and integer-policy-aware types.
- Verification: 485 Rust tests, 40 TypeScript unit tests, 39 native/Wasm/boundary
  tests (21 codegen/build), `cargo fmt --check`, clippy with warnings denied, and
  `npm run ci` pass.
- Current boundary: TypeScript-target script emission waits for `TS-001` or a
  pinned transpiler; C/C++ scripts wait for the native backend. Channels and a
  scheduler and full UTF-8 byte operations remain.

### 2026-10-26 - JS-001 complete: channels, UTF-8 string/byte operations, conformance suite

- Command: `npm run ci`
- Result: pass
- Checker: added the `Channel` builtin namespace (`Channel.create<T>()` with
  `send`/`receive` returning recoverable `ChannelError` results and infallible
  `close`), the `bytes` namespace (`bytes.new(n)`), spec §6 string operations
  (`char_count`, `bytes`, `chars`, `byte_at`, `char_at`, and `slice` with
  `SliceError`), `bytes` indexing by `usize` yielding `u8`, and the
  `ChannelError`/`SliceError` builtin error sets. Lvalue checks now annotate
  target types, so indexed writes (`array[i] = x`, `buffer[i] = u8`, including
  nested sequences) type-check and emit bounds-checked writes.
- Codegen: new tree-shaken helpers (`$channel*`, `$charCount`, `$strBytes`,
  `$strChars`, `$byteAt`, `$charAt`, `$strSlice`, `$bytesNew`, `$indexSet`);
  sequence-tagged index assignments lower to `$indexSet`; void fallible
  functions without an explicit `return` now end with `return $ok(undefined)`
  so callers can `match` them; `.d.ts` artifacts declare `AstraChannel<T>`
  (and `AstraTask<T>`) runtime shapes and map `Channel<T>`/`Task<T>`
  signatures to them.
- Semantics: channels are bounded (capacity 16) plain records; `send`/`receive`
  fail recoverably on a closed channel; because spawn stays eager in the V1
  single-threaded runtime, a full-channel send or empty-channel receive raises
  a structured deadlock panic instead of blocking. `len` on `str` remains the
  UTF-8 byte length; `slice` takes byte offsets and rejects out-of-range or
  scalar-splitting ranges.
- Conformance: new `conformance/` suite (`hello`, `strings_utf8`,
  `bytes_buffer`, `channels` with `.expected` stdout) plus
  `orchestrator/tests/conformance.mjs`; `codegen-js.test.mjs` runs every case
  and `TS-001` will reuse the same runner.
- Tests: 8 new checker unit tests, 5 new codegen execution tests (UTF-8 ops,
  bounded channels with closed errors, the deadlock panic, bytes buffers with
  an out-of-bounds panic, and `AstraChannel` declarations), and the shared
  conformance test.
- Housekeeping: the deprecated `js-001` branch and worktree were deleted after
  verifying full containment; their uncommitted TS-001 groundwork (codegen-ts
  emission work and `codegen-ts.test.mjs`) was preserved in this branch and is
  green here.
- Verification: 493 Rust tests, 41 TypeScript unit tests, 45 native/Wasm/boundary
  tests (27 codegen/build), `cargo fmt --check`, clippy with warnings denied,
  and `npm run ci` pass.
- Current boundary: TypeScript-target script emission waits for `TS-001`; C/C++
  scripts wait for the native backend. Channels are single-threaded in V1:
  blocking operations panic rather than yield.

---

## Handoff

```markdown
### Handoff
- Phase: 7 - Backends
- Task: `C-001` slices landed: `Array.get`; native library artifacts (`--emit staticlib|cdylib|library`) + provisional C ABI (`11-c-abi.md`); single-threaded `CONC-001` (eager spawn/Task, bounded Channel, ChannelError). C conformance ledger is empty (19/19 run).
- Status: Ready (commits `16afbcd` and `483caae`; full CI green immediately before each commit)
- Files changed: `crates/astra-codegen-c/src/{emit,ty,runtime,lib}.rs`, `crates/astra-codegen-c/tests/{emit_c,conformance}.rs`, `crates/astra-compiler-api/src/lib.rs` (`library` flag), `orchestrator/packages/compiler-client/src/index.ts`, `orchestrator/packages/cli/src/index.ts` (`--emit`, archiver discovery), `orchestrator/tests/codegen-js.test.mjs` (staticlib/cdylib consumer tests), `Docs/Design/V1/{11-c-abi.md,README.md}`, `conformance/README.md`, `Docs/Agent/{todo,state-log,c001-handoff}.md`
- Verification: `npm run ci` green: 77 `astra-codegen-c` tests, all 19 corpus cases link and run, TypeScript/boundary tests, fmt, clippy with warnings denied
- Blockers: none locally; Linux/macOS native CI run still outstanding
- Next action: package linking (6.B) reusing NoPM's resolver; GC/manual memory (6.C) from the memory design; full structured concurrency; Tier-1 native CI matrix; later WASM-001, FFI-001, PLAY-001, HARD-001
```


---

## Update protocol

When updating this file:

1. Change `Last updated`.
2. Update `Current snapshot`.
3. Append a dated entry under `Completed` or `Active`.
4. Update `Next`.
5. Update `Blockers`.
6. Add verification evidence.
7. Refresh `Handoff`.

Do not delete old entries unless they are factually wrong. If a decision changes, record the supersession instead of erasing history.
