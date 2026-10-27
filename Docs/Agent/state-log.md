# State Log

This is the agent's persistent memory. Update it at the end of every substantial task. Do not rewrite history; append what changed and keep the snapshot current.

---

## Current snapshot

| Field | Value |
|---|---|
| Last updated | 2026-10-27 |
| Current phase | Phase 7 - Backends |
| Current task | `C-001` numeric builtins slice (`to_int`, `checked_*`, `wrapping_*`) complete, commit follows; `C-001` str/bytes slice semantics + fallible-call `match`/`!` materialization (`84ac11d`, `9d1173e`); `C-001` cross-module struct/enum types slice (`NS8020` closure) (`12bfd86`); `JS-001` statement-position control-flow slice (`NS8001` closure) (`c8555c6`); `NOVA-004` Nova gaps closed (`f0a62e6`); `work/conf-001` merged into master (`78d8920`); `C-001` project-linking slice (`a0dea19`); `TS-001` [x] complete (`83ec555`, `6f0e80b`, `df4ffc9`); TS struct-mutation fix (`49a84c8`) |
| Project status | D1-D25 confirmed; compiler frontend, cross-module type surfaces, multi-source compiler API, least-privileged mode inference, effect-provenance diagnostics, typed-HIR Nova interpreter, complete Nova CLI with project module loading/linking, complete NoPM stack, complete JavaScript ESM backend, complete TypeScript ESM backend (shared emitter TS mode, `astra build --target ts`, `@script:ts` pinned transpiler), first C17 managed slice (`astra-codegen-c`) with freestanding runtime & `astra build --target c` pipeline, C multi-module project linking (module-qualified symbols, cross-module calls and struct/enum types, `compile_project`), `@script` inline interop frontend complete |
| Source code | `crates/astra-core` (lexer/parser/formatter/resolver/types/mode/checker), `crates/astra-capabilities` (mode inference and explanation), `crates/astra-nova` (module-aware typed-HIR interpreter and `nova` binary), `crates/astra-compiler-api` (checking and typed-module compile boundary incl. `compile_project`), `crates/astra-codegen-c` (C17 backend, freestanding runtime, host shim, multi-module emission), `orchestrator/packages` (manifest, nopm, registry, compiler-client incl. `compileProject`, codegen-js, codegen-ts, CLI) |
| Git repository | Yes; initialized 2026-10-25 (baseline `2ce78ad`); `target/`, `dist/`, `node_modules/`, and `.tools/` are ignored |
| Blockers | Hosted native executable linking on Windows requires MSVC/Windows SDK headers for libc (`stdio.h`), or a Tier-1 host with standard C library headers in CI |
| Next action | C-001 remainder (GC/manual memory, concurrency, package linking, native CI gate); later backlog WASM-001, GC-001, FFI-001, CONC-001, PLAY-001, HARD-001, INTEROP-001 native slice |

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
- Task: C-001 multi-module project linking slice implemented (module-qualified symbols, cross-module calls, `compile_project` JSON-RPC method, `CompilerClient.compileProject`, `astra build --target c` multi-module path, `NS3012` relative-import fix); TS-001 complete (`83ec555`/`6f0e80b`/`df4ffc9`); TS struct-mutation fix committed (`49a84c8`)
- Status: Ready (this slice commits next; CI green immediately before commit)
- Files changed: `crates/astra-codegen-c/src/{emit,ty}.rs` (module qualification, `collect_import_tables`, `module_key_stem` `.ns` strip), `crates/astra-codegen-c/tests/emit_c.rs` (8 project tests), `crates/astra-compiler-api/src/lib.rs` (`compile_project` artifacts, relative-import resolution via `check_module_graph_with_imports`, 3 tests), `orchestrator/packages/compiler-client/src/index.ts` (`compileProject` + parse), `orchestrator/packages/cli/src/index.ts` (multi-module C build path), `orchestrator/tests/codegen-js.test.mjs` (multi-module C e2e), `Docs/Agent/{todo,state-log}.md`
- Verification: `npm run ci` green: 63 `astra-codegen-c` tests, 31 `astra-compiler-api` tests, TypeScript unit + boundary tests, fmt clean, clippy with warnings denied
- Blockers: Windows native linking needs MSVC/Windows SDK (`stdio.h`) or a Tier-1 CI host
- Next action: Nova gaps (bytes builtin, str byte_at/char_at/slice/char_count/chars/bytes, Channel runtime), JS IIFE control-flow (NS8001); then C-001 remainder (GC-001, CONC-001, cross-module types NS8020, native CI gate); later backlog WASM-001, FFI-001, PLAY-001, HARD-001, INTEROP-001 C slice
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
