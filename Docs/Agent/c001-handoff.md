# C-001 handoff — C backend state, architecture, and remaining work

> **Purpose.** This is the transfer document for the `C-001` workstream: where
> the C backend stands, how it is built, what is left, and how to pick up
> without re-deriving context. It complements `Docs/Agent/todo.md` (task queue)
> and `Docs/Agent/state-log.md` (chronological history); when they disagree with
> this document, the task queue and spec win.
>
> **Snapshot:** `master` at `cf1056a` plus this docs entry (2026-10-29),
> working tree clean, full `npm run ci` green at the `NS6002` slice. Since the
> previous snapshot this work recorded the `GC-001` design decision (all eleven
> `[P1]`-`[P11]` choices approved with amendments applied inline in
> `Docs/Design/GC-001-proposal.md`, both implementation-gating conditions
> answered in §11) and landed its Phase 0: `NS6002` forbids managed element
> types in `Memory.alloc<T>`/`region.alloc<T>` (spec §8.4 rule line, §14.9 code
> row; code confirmed by the user). Tier-1 runs are green on Linux, macOS, and
> Windows through run 18 (`40858504156`, `cf1056a`); `40690850075` is run 10,
> not run 11, per the API's own `run_number`.
>
> **Remaining (honest handoff):** precise mark-sweep GC with compiler stack maps
> (`GC-001`, 6.C; design approved 2026-10-29 and Phase 0 (`NS6002`) landed -
> next is Phase A: block header + shape grammar + allocator split, no
> collection yet; only `[P9]`'s FFI push/pop signature stays open as a
> `11-c-abi.md` ABI decision), the unspecced `MutSlice` surface behind
> `ptr.as_slice` (6.C), the fallible `Memory.alloc`/`region.alloc` entry-point
> policy (6.C), and OS-threaded structured concurrency (`CONC-001`, 6.D). All
> are design-gated; see the section notes before starting. `Ptr` pointer
> access, the `Region` API, `NS6001`, and recorded Linux/macOS/Windows CI
> evidence are closed (6.C, 6.F).

---

## 1. Resume in five minutes

```powershell
cd "C:\Projects\Neo Astra"
git log --oneline -3          # expect cd102a9 Implement the spec 8.4 Region API ...
git status --porcelain        # expect empty
cargo test -p astra-codegen-c # 70 emit/unit tests + the static conformance gate
npm run ci                    # full gate; green is the baseline
```

Then read section 5 (feature inventory), section 7 (architecture invariants)
and section 6.A (the generics design) before touching code.

---

## 2. Project orientation

- Product: **Neo Astra**, language file extension `.ns`; CLIs `astra` / `nova` /
  `nopm`.
- Authoritative documents: `Docs/spec-v0.1.md` and the D1–D25 decisions
  (recorded in `Docs/Agent/state-log.md`). Design docs live in `Docs/Design/V1/`.
- Exactly one manifest (`core.json` / `core.toml`) and one lockfile
  (`core.lock.json`).
- Backends consume the **same serialized typed module** produced by the
  frontend — they must never re-parse `.ns` source.
- TypeScript backends must pass `tsc --strict --noEmit`.
- `Docs/Agent/definition-of-done.md` defines "done"; `Docs/Agent/edge-case-protocol.md`
  defines how edge cases are recorded.

### Repo map

| Path | Contents |
|---|---|
| `crates/astra-core` | lexer, parser, formatter, resolver, types, mode inference hook, checker, typed HIR |
| `crates/astra-capabilities` | mode inference / explanation |
| `crates/astra-nova` | reference typed-HIR interpreter + `nova` binary |
| `crates/astra-compiler-api` | checking / typed-module compile boundary, `compile_project` |
| `crates/astra-codegen-c` | **C17 backend** (this workstream), freestanding runtime, host shim |
| `crates/astra-codegen-js` / `codegen-wasm` / `codegen-llvm` | other backends (Wasm/LLVM are scaffolds) |
| `orchestrator/packages` | manifest, nopm, registry, compiler-client, codegen-js, codegen-ts, CLI |
| `conformance/` | shared behavior corpus (`cases/`, `rejects/`) + README policy |
| `Docs/Agent/` | `todo.md`, `state-log.md`, `runbook.md`, this file |

---

## 3. C backend file map

`crates/astra-codegen-c/src/`:

| File | Role |
|---|---|
| `lib.rs` | public API: `CModule`, `ProjectEntry`, `generate`, `generate_project`, `GeneratedC`, `BackendError` |
| `emit.rs` | the emitter (~3.5k lines): prescan, program assembly, statements/expressions, matches, methods, projects, headers, scripts |
| `ty.rs` | `TypeEnv` (declared shapes), `TyCtxt` (C type mapping, typedef registry, rejects, foreign table), `to_type` / `path_to_type`, `sanitize` |
| `runtime.rs` | `RUNTIME_HEADER` (freestanding typedefs + `extern` prototypes) and `HOST_SHIM` (hosted implementation written as `astra_host.c`) |

`crates/astra-codegen-c/tests/`:

| File | Role |
|---|---|
| `emit_c.rs` | 70 emission tests; each parses/checks a snippet through `astra-core`, generates C, asserts structure, and (when clang exists) compiles with `-std=c17 -ffreestanding -Wall -Wno-unused-variable -c` |
| `conformance.rs` | **static conformance gate**: lowers every `conformance/cases/*.ns` and clang-compiles it; `KNOWN_GAPS` ledger for cases C cannot lower yet |

---

## 4. Current snapshot

- `master = 483caae`; tree clean; `npm run ci` green.
- `astra-codegen-c`: **77** emission tests + 1 conformance gate.
- Corpus: **19 positive** cases, **13 negative** rejects (including
  `pointer_access_requires_system`, which pins the `NS5004` gate on raw pointer
  usage backend-neutrally, and `region_api_requires_system`, which pins the
  same gate on the `Region` API).
- C emits, compiles, links, **and runs** **all 19** cases when a hosted-capable
  compiler is present; the C `KNOWN_GAPS` ledger is empty.
- Native execution is verified on this Windows host through **gcc**
  (`C:\msys64\ucrt64\bin`): `astra build --target native` links `main.c` +
  `astra_host.c` (+ `@script:c`/`.cpp` TUs) and the binary runs. Clang alone
  still cannot build hosted C here (no MSVC/Windows SDK), so both the test
  harness and the CLI probe for hosted capability instead of preferring clang.
- **Library artifacts** `--emit staticlib|cdylib|library` are implemented and
  verified with hand-written C consumers; the contract is recorded in
  `Docs/Design/V1/11-c-abi.md` (provisional under design §3.2).
- **Concurrency (first slice):** eager `spawn`/`Task<T>`, bounded
  `Channel<T>` (capacity 16), `ChannelError`, and deadlock panics ship and run
  the `channels` case. No OS-thread scheduler or task-local heaps yet.
- Linux/macOS native builds: the Tier-1 CI workflow (`.github/workflows/ci.yml`,
  3 OSes) runs from the full-fidelity remote `voidgod79-debug/Neo-Astra-CI` -
  the source-only mirror still cannot trigger it (no `.github/`, `scripts/`,
  `tests/`, or `conformance/`, enforced by the pre-push guard). Recorded:
  run `40677187427` is green on Linux, macOS, and Windows.

### Commits that built the current state (newest first)

| Commit | Content |
|---|---|
| `d0d7bf9` | `Ptr` pointer access: `ptr.read`/`ptr.write` (spec §8.4), `NS5004` gate on raw pointer usage, `as_slice` left open (unspecced `MutSlice`) |
| `bc5d1c9` | Tier-1 run `40677187427` green on all three OSes (cdylib import-library check is win32-only) |
| `32b0ef5` | macOS `ar` candidates attempted for real instead of `--version`-probed; MSVC-flavored cdylib links with a generated `.def` + `/IMPLIB` |
| `fb51841` | CI installs the `wasm32-unknown-unknown` target for `test:boundary` |
| `1811ce1` | `pretest` builds `astra-compiler` before `npm test`; orphaned stress suites join the gate (49 → 111 tests) |
| `399bee0` | `Memory.alloc`/`Memory.free` first cut (system-gated, `Ptr<T>`→`T*`, arena lowering, `NS8012` removed) |
| `79ef931` | NoPM-installed (git/registry) packages link via lockfile + store index |
| `3cafffc` | Tier-1 CI workflow (Linux/macOS/Windows `npm run ci`) |
| `bd734d5`/`623c4b8` | Managed arena allocator released at exit (`astra_runtime_shutdown`) |
| `ed5ac5e`/`2e80554` | Manifest path-package linking (import map) + include-guard fix + docs |
| `4040aa5` | Docs: record C-001 library and concurrency slices |
| `483caae` | Single-threaded concurrency (eager spawn/Task, bounded Channel, ChannelError); C corpus runs 19/19 |
| `16afbcd` | `Array.get`, native library artifacts (`--emit staticlib\|cdylib`), provisional C ABI |
| `d8e4efa` | Hosted shim fix (`host_shim()` TU), hosted-compiler probing, native conformance execution, match-arm `pre` fix |
| `a8e41f2` | Docs: record generics monomorphization completion |
| `90cdba1` | Generic function monomorphization (`ns_<fn>_<types>` per call site) |
| `8696b07` | C-001 handoff document + host-shim defect finding |
| `47d2598` | Docs: reconcile INTEROP-001 C/C++ slice status |
| `7b86025` | Static conformance gate; bytes writes (`astra_bytes_set`); `Option`/`Result` accessors; contextual `None` |
| `731ec32` | UTF-8 views (`char_count`, `byte_at`, `char_at`, `bytes`, `chars`) and `bytes.new` |
| `c33fcfd` | `str.to_int`, integer `checked_*` / `wrapping_*` |
| `9d1173e` | `expr!` propagation materializes fallible builtins |
| `84ac11d` | `str`/`bytes` slice end-offset + `SliceError` Result semantics; JS/TS `bytes.slice` no longer clamps; typedef-order and reject-check fixes |
| `12bfd86` | Cross-module struct/enum types (`NS8020` closure) |
| `c8555c6` | JS/TS statement-position `if`/`match` (NS8001 closure) |
| `f0a62e6` | NOVA-004: Nova str/bytes/Channel gaps |
| `a0dea19` | C multi-module project linking |
| `49a84c8` | TS struct-mutation (`readonly`) fix |

---

## 5. What the C backend supports today

**Types.** `i8`–`i64`, `u8`–`u64`, `f32`/`f64` (printing/arithmetic), `bool`,
`str` (`NS_Str`), `bytes` (`NS_Bytes`), `usize`/`isize` where mapped, `Unit`,
`Option<T>`, `Result<T, E>`, arrays, maps, tuples, named structs, tuple structs,
enums with payloads. Fixed arrays, function values and slices are rejected.

**Expressions.** Literals (integer/float/bool/str/`b"..."`/unit), identifiers,
unary/binary (incl. `+%` `-%` `*%` `/%`), assignment (incl. bounds-checked bytes
writes), calls (local, imported named, namespace `alias.fn`, enum variant
constructors, script exports, builtins), indexing (arrays/bytes), member access
(struct fields, enum variants, module functions), `expr!` propagation, array /
map / tuple literals, struct init, block expressions, value-position `if` and
`match`, `fail`, `panic`, `abort`, and eager `spawn`.

**Statements.** `let` (typed/untyped), expression statements, `if`/`else`,
`while`, `for` over arrays / maps / bytes, `return`, `break`/`continue`, `defer`,
scoped blocks, statement-position `if`/`match`.

**Methods.**

| Receiver | Methods |
|---|---|
| `str` | `len`, `is_empty`, `slice`, `to_int`, `char_count`, `byte_at`, `char_at`, `bytes`, `chars` |
| `bytes` | `len`, `is_empty`, `slice`, index read/write; `bytes.new(n)` |
| `Option` | `is_some`, `is_none`, `unwrap`, `unwrap_or` |
| `Result` | `is_ok`, `is_err`, `unwrap`, `unwrap_or`, `error` |
| arrays | `len`, `push`, `get` |
| maps | `len`, `contains`, `get`, `insert` |
| integers | `checked_add/sub/mul/div`, `wrapping_add/sub/mul/div` |
| `Channel<T>` | `send`, `receive`, `close` (bounded, capacity 16) |
| `Task<T>` | `join`, `cancel` |

**Failible model.** `str.slice`, `str.to_int`, `checked_*`, user error-set
functions. Division by zero panics in every arithmetic mode.

**Multi-module.** One TU + one header per module; module-qualified symbols
(`ns_<stem>_<name>`, `NS_<stem>_<Type>`); cross-module calls and cross-module
struct/enum types; entry module emits `int main(void)`; headers include
dependency headers. Default imports are rejected (`NS8019`).

**Generics.** Local generic functions are monomorphized per call site
(`ns_<fn>_<type args>`, e.g. `ns_identity_i32`), including generic-to-generic
calls; see section 6.A for the boundaries.

**Concurrency (first slice).** `spawn || { ... }` is eager: it runs the block
immediately and stores the result in `NS_Task_<T>`; `join()` returns it and
`cancel()` marks it. `Channel<T>` maps to `NS_Channel*` (capacity 16, value
copies); `send`/`receive` return `Result<_, ChannelError>`, `close` is
infallible, and a full/empty open channel panics with a deadlock message.
No OS-thread scheduler, task-local heaps, or cancellation safepoints yet.

**Library output.** `astra build --emit staticlib|cdylib|library` (or the target's
`emit` key) compiles per-module objects and archives/links them with the
platform toolchain (`ar`/`llvm-ar`/`gcc-ar`, `-shared`). Static archives exclude
the runtime shim; shared libraries include it. `EmitKind::Library` suppresses
`int main(void)`. The symbol/layout/ownership contract is in
`Docs/Design/V1/11-c-abi.md` (provisional).

**Inline scripts.** `@script:c` / `@script:cpp` → one TU per block, C ABI
prototypes, `extern "C"` for C++, CLI compiles and links script TUs
(`-lstdc++` for C++). Native link verified end to end on this host.

**Diagnostics currently emitted by the backend.** `NS8012` (manual memory
scopes), `NS8017`/`NS8018` (unsupported call/generic), `NS8019` (default
imports, multi-error signatures), `NS8020` (unknown nominal type), `NS8021`
(unsupported pattern/method), `NS8024` (print type), `NS8099` (unresolved type).
All rejects abort generation (`generate` returns `Err`).

---

## 6. Remaining work

Order below is a recommendation, not a mandate. Each item lists current
behavior, a design sketch, acceptance, and risks.

### 6.A Generics monomorphization — **DONE (`90cdba1`)**

Local generic functions are monomorphized per call site. The design below is
kept as the reference for how it works (and for extending it later); the corpus
case `conformance/cases/generics.ns` now emits and clang-compiles:

```ns
fn identity<T>(value: T) -> T { return value }
pub fn main() -> i32 {
    println(identity(7))
    println(identity("ok"))
    println(identity(true))
    return 0
}
```
Expected: `7`, `ok`, `true`, `RETURN=0`.

**Design (first slice).**

1. **Substitution in `TyCtxt`** (`ty.rs`):
   - field `subst: BTreeMap<String, Type>` (empty by default);
   - `pub fn set_subst(&mut self, map: BTreeMap<String, Type>)` and
     `clear_subst()`;
   - at the top of `c_type` and `ident`: if `Type::Named(name, args)` with empty
     `args` and `subst` has `name`, recurse on the concrete type.
   - Keep the substitution out of `type_of` (returns `&Type`); substitution is a
     mapping concern only.
2. **Prescan**: skip generic functions (`!f.type_parameters.is_empty()`) —
   their signatures are emitted per instantiation, and the instantiation's
   prototype path prescans through `emit_fn_proto_named`.
3. **Instantiation discovery at call sites** (`call_c`, identifier callee):
   - unify each declared parameter type with `type_of(arg)` (apply the current
     `subst` to `type_of(arg)` first so generic-to-generic calls work);
   - only synthesize `Named(p)` where `p` is a type parameter; recurse through
     `Option`/`Result`/`Array`/`Slice`/`Map`/`Tuple`/`Named` arguments; ignore
     concrete-only patterns;
   - all type parameters must be bound, else reject `NS8018` ("cannot infer type
     arguments for generic function `f`");
   - mangle the symbol: `format!("{}_{}", fn_symbol(name), params.iter()
     .map(|p| tcx.ident(&subst[p])).join("_"))` — e.g. `ns_identity_i32`,
     `ns_identity_Str`, `ns_identity_Bool`;
   - dedupe by symbol in an `instantiations: BTreeMap<String, Instantiation>`
     (declaration clone + subst) and enqueue the symbol in a `VecDeque`.
4. **Program assembly** (this is the delicate part — see section 7):
   - emit constants + regular bodies + `main` into `self.out` as today (this
     discovers instantiations);
   - drain the queue: for each symbol, `set_subst`, emit the specialized body via
     a symbol-override variant of `emit_function`, `clear_subst`; keep looping
     because a specialization may discover more (generic-to-generic);
   - then `let code = take(out)` and write: runtime header, module comment, dep
     includes (linked), **complete typedefs**, **prototypes** (regular fns, then
     every instantiation), script prototypes, then `code`.
   - `emit_fn_proto` / `emit_function` need `_named` variants taking an explicit
     symbol; `fn_symbol` stays the default.
   - `collect_decls` must exclude generic declarations so they never emit
     prototypes/bodies under their bare name.
5. **Headers**: exported generic functions cannot have a single symbol — skip
   them in `emit_header` (boundary; document in `todo.md`).
6. **Cross-module generics**: not supported (a dependency's instantiations are
   not visible to the importer's TU). Document as a boundary; do not attempt in
   this slice.

**Shipped.**
- `generic_functions_monomorphize_per_instantiation` pins the `i32`/`Str`/`bool`
  instantiations, the specialized bodies, and a generic-to-generic forwarded
  call, and compiles the output with clang.
- `generics` left `KNOWN_GAPS`; the static gate now emits 18/19 cases (only
  `channels` remains).
- Remaining boundaries: exported generic functions are not exposed in headers;
  cross-module generic calls are not instantiated in the importer; generic
  bodies that dispatch on `T` beyond type mapping stay out of scope.

**Risks / traps.**
- Emission order: prototypes must precede bodies, and instantiation bodies are
  discovered *during* emission — hence the two-buffer assembly. Do not emit
  prototypes inline as bodies are emitted.
- `fresh_id` counters: emitting specialization bodies after regular bodies keeps
  temp names unique automatically; deterministic order = discovery order.
- `cur_ret` / `cur_err` must be set per specialization exactly as
  `emit_function` does today.
- A generic body that branches on `T` (calls methods, indexes, compares) will
  dispatch on the *unsubstituted* type in emission logic (only `c_type`/`ident`
  are substituted). Accept the boundary; document it.
- Aborting on any unbound parameter keeps failures honest rather than emitting
  broken C.

### 6.B Package linking — **DONE (path packages)**

**Done (this slice).**
- Compiler API: `ProjectCompileRequest.import_map` (JSON `importMap`), a map
  from source specifier to canonical module key. `resolve_project_target`
  consults it before identity/relative resolution, so a bare
  `import { add } from "mathlib"` binds to a package module whose key lives
  under a package namespace.
- CLI: `linkedPathPackages` reads manifest `dependencies` entries with
  `source: "path"` (honouring optional `targets` filtering), loads each
  package's manifest (`nova.entry`), discovers its module graph with the same
  `discoverProjectModules` used for the project, and re-keys every module as
  `deps/<sanitised name>/<path>`. Package modules are prepended so the
  importer's entry module stays last (`emit_project` treats the last entry as
  main). Internal relative imports keep resolving within the package.
- Bug uncovered and fixed: `RUNTIME_HEADER` was prepended to every `.c` and
  `.h`, so a dependency header re-including it produced conflicting
  anonymous-struct typedefs. It is now guarded (`#ifndef NS_RUNTIME_HEADER`).
  No test compiled a multi-module project as separate TUs before;
  `assert_project_compiles` now does.
- Verified end to end: fixture with a path dependency builds via
  `astra build --target native`, emits `deps_mathlib_src_lib.c`, qualifies
  symbols (`ns_deps_mathlib_src_lib_add`), and the binary prints 42. Pinned by
  a compiler-api JSON-RPC test and a CLI boundary test.

**Honest boundaries.**
- Both `source: "path"` and NoPM-installed (git/registry) packages are wired:
  the CLI reads `core.lock.json` (name → id) and `.astra/store/index.json`
  (id → root) and feeds installed directories through the same discovery.
- Only non-aliased dependencies resolve by name (an aliased or git-URL dep key
  is not the lockfile `name`); a package's own bare imports are not re-mapped
  (no package-of-package resolution).
- Diagnostics for package modules attribute spans to the entry file, as
  before for multi-module projects.

**Acceptance met:** fixture project with a path dependency (and an installed
registry package) builds via `astra build`; symbols qualified; package TU
cannot collide with importer stems (namespace prefix).

### 6.C GC-001 — managed memory and `@memory:manual` — **FIRST CUTS DONE**

**Landed this session.**
- Managed arena: `astra_alloc` records every block; `astra_free`/`astra_realloc`
  keep the tracker in sync (array/map growth); the emitted `main` calls
  `astra_runtime_shutdown` to release the run's arena at exit. Freestanding
  `extern astra_alloc` boundary unchanged. This is the design's "arena per
  program run, released at exit" managed-mode first step (§4.1).
- Manual-memory allocation subset: `Memory.alloc<T>(n) -> Ptr<T>` and
  `Memory.free(p)` are checker builtins (system-mode gated — `NS5004` in core),
  `Ptr<T>` maps to `T*`, and they lower to the arena. `NS8012` is removed for
  this subset; `@memory:manual` scopes emit normally.
- Manual-memory **pointer access** (spec §8.4): `ptr.read(index) -> T` and
  `ptr.write(index, value)` type-check as methods on `Ptr<T>` and lower to
  `(ptr)[i]` / `ptr[i] = v`. Unchecked by design — a manual pointer carries no
  length, so there is nothing to bounds-check against. Raw pointer *usage*
  carries the same `NS5004` system-mode gate as the allocation, so a core-mode
  function that merely receives a `Ptr` cannot read raw memory; that gate is
  pinned backend-neutrally by
  `conformance/rejects/pointer_access_requires_system`.
- Manual-memory **`Region` API** (spec 8.4): `Region.new(bytes) -> Region`,
  `region.alloc<T>(count) -> Ptr<T>`, and `region.free()` are landed with the
  user-decided fixed-block bump-arena strategy - one reserved block, carvings
  bump-style with 8-byte alignment, whole-block free, panic on exhaustion (the
  same V1 stance as `Memory.alloc`'s OOM panic until the fallible entry-point
  policy covers both). `Region` is a non-generic builtin type mapping to
  `NS_Region*` (header-plus-block, like `Channel`), every member carries the
  `NS5004` system-mode gate, and the mode gate is pinned backend-neutrally by
  `conformance/rejects/region_api_requires_system`. The spec's scope rule
  names only `Memory.alloc`, so Region use needs system mode but no
  `@memory:manual` block; whether that is an oversight is recorded as an open
  design question in `Docs/Agent/todo.md`.

**Remaining (honest handoff).**
- `ptr.as_slice(count) -> MutSlice<T>` — spec §8.4 lists the row, but `MutSlice`
  is named exactly twice in the entire spec (the type list and that row) with no
  method surface, indexing contract, or lifetime rule. It needs a design
  decision, not codegen; deliberately not invented here.
- **Precise mark-sweep GC** with compiler-generated stack maps (§4.1/§4.3). The
  arena is not a collector; the real one needs frontend precise roots (multi-week
  `GC-001`). Conservative scanning would contradict the design's precise-roots
  requirement, so it is not the chosen path.
- Fallible `Memory.alloc` returning `Result<Ptr<T>, AllocError>` (the design's
  `!` form) vs. the current infallible arena alloc (panics on OOM). This is
  blocked on a policy decision, not codegen: `!` at the call site propagates
  into the enclosing function's error set, and `emit_main` writes
  `(int)main()` with no notion of an error effect, so a fallible `main` — the
  natural home for a manual-memory block — has no specified native behavior.
  Decide that first: an unwrap-and-panic entry wrapper, a non-fallible
  `alloc_or_panic` alternative, or a native `NV0004`-style report.
- The wasm backend implements no manual memory at all, so `ptr.*` (like
  `Memory.alloc`) is C-only today; the "native or Wasm" half is a `WASM-001`
  item.

**Risk/ownership.** The remaining pointer question (`MutSlice`'s surface) and
precise-GC roots are language-semantics decisions (GC-001), not just codegen —
read `03-runtime-and-memory.md` and the D-decisions before continuing. Pointer
read/write itself is settled by spec §8.4 and needs no further design.

### 6.D CONC-001 — structured concurrency — **FIRST SLICE DONE**

**Today.** The single-threaded cooperative slice mirrors JS/Nova and is done:

- `spawn || { ... }` runs eagerly and yields `NS_Task_<T>`; `join()` returns the
  stored value, `cancel()` marks the task.
- `Channel<T>` maps to a bounded `NS_Channel*` (capacity 16, value-copy
  messages). `send`/`receive` return `Result<_, ChannelError>`; `close` is
  infallible. Sending to a full open channel or receiving from an empty open
  channel panics with the structured deadlock message; buffered messages remain
  readable after `close`.
- `NS_ChannelError` is a shared, stem-free runtime payload; `!` and `match`
  materialize it through the normal fallible-call path.
- `channels` left `KNOWN_GAPS`; the C conformance ledger is empty and all 19
  cases run.

**Remaining (full CONC-001) — a design decision, not just codegen.**
- OS-thread or worker scheduling; the current runtime is single-threaded and
  eager, so blocking operations panic instead of yielding. Moving to
  pthreads/C11 **changes these deadlock-panic semantics to blocking waits** —
  a `CONC-001` design choice that should be made deliberately, not unilaterally.
- Task-local heaps and message transfer rules (design §4.2).
- Cancellation safepoints and parent-scope join/failure propagation.
- Thread-pool sizing/policy and Web Worker mapping (Wasm path).
- Adoption note: use the platform thread API (`pthreads`/C11 threads/Web
  Workers) rather than hand-rolled primitives; the channel buffer and scheduler
  policy stay language-owned to preserve scoped-task semantics.

### 6.E Native library output + stable ABI — **DONE (provisional ABI)**

**Today.** `astra build --emit staticlib|cdylib|library` produces
`lib<name>.a`, `lib<name>.so`, or `lib<name>.dll` (+ `.dll.a` on Windows) beside
the generated sources and headers. `EmitKind::Library` (via
`generate_project_with_kind`, JSON-RPC `library: true`, and
`CompilerClient.compileProject`) suppresses `int main(void)` while keeping `pub`
functions external. A hand-written C consumer links and runs against both
artifact kinds (boundary tests).

**ABI.** `Docs/Design/V1/11-c-abi.md` records:

- symbol naming (`ns_<stem>_<name>`, `NS_<stem>_<Type>`), stem rules, and the
  exported-generic boundary;
- layouts for integers, floats, `str`, `bytes`, arrays, maps, `Option`,
  `Result`, structs, and enums, including the shared builtin error payloads;
- ownership rules: managed-until-exit runtime allocations, borrowed `str`/
  `bytes` views, and the static-vs-shared runtime linking contract;
- error/panic behavior and a consumer example.

**Risks / boundaries.** The ABI is explicitly provisional (design §3.2 says V1
must not promise unstable layout). Static libraries deliberately exclude
`astra_host.c` so two Astra archives cannot collide on runtime symbols;
consumers link the shim. Export lists are implicit (`pub` + external linkage),
not a generated visibility file. `cdylib` is tested on Windows here; Linux/macOS
shared linking follows the same `-shared` path but needs the Tier-1 run.

### 6.F Cross-platform native verification + CI gate — **DONE (this host + Tier-1)**

**What was blocking it (all fixed).**

1. `astra_host.c` was written raw and was not a valid translation unit: it used
   `NS_Bytes`, `NS_Map`, and the builtin error payloads without declaring them,
   and `PRId*`/`INFINITY` without `<inttypes.h>`/`<math.h>` (51 gcc errors).
   `runtime::host_shim()` now returns `RUNTIME_HEADER + HOST_SHIM`; `HOST_SHIM`
   is the body, adds the libc includes, and no longer duplicates the `NS_*`
   typedefs (duplicating anonymous structs is an incompatible redeclaration).
2. Compiler discovery preferred clang. This host's clang has no MSVC/Windows
   SDK, so it compiles freestanding objects but cannot find `<stdio.h>`, while
   gcc from MSYS2 UCRT64 works. Both the Rust test helpers and
   `astra build --target native` now select the first candidate that passes a
   *hosted* probe (a real `#include <stdio.h>` compile/syntax check), falling
   back to the first responder so its error still reaches the user.

**Now.**

- `crates/astra-codegen-c/tests/conformance.rs` links each emitted case against
  the shim, runs it, and compares stdout + `RETURN=` with `.expected` whenever a
  hosted compiler exists; the freestanding clang compile check still runs
  unconditionally.
- `astra build --target native` links `main.c` + `astra_host.c` (+ script TUs)
  and runs; `@script:c` and `@script:cpp` are verified end to end.
- Tests self-skip when no hosted compiler is available, so CI on a minimal host
  stays green while a normal Linux runner exercises execution.

**Evidence recorded (2026-10-28).**
`.github/workflows/ci.yml` runs the full `npm run ci` gate on `ubuntu-latest`,
`macos-latest`, and `windows-latest` (runners ship a hosted C compiler, so the
conformance corpus and native build execute on Tier-1 platforms) from the
full-fidelity remote `voidgod79-debug/Neo-Astra-CI`. Run `40677187427`
(commit `bc5d1c9`) is `success` on all three OSes. Reaching it took four
fixes, each surfaced by a run: a `pretest` hook builds `astra-compiler`
before `npm test` (and `npm test` now includes the two orphaned stress
suites, 49 → 111 tests), the workflow installs the `wasm32-unknown-unknown`
target for `test:boundary`, staticlib archiving attempts
`ar`/`llvm-ar`/`gcc-ar` instead of `--version`-probing (Apple's `ar` rejects
GNU long options), and cdylib emission detects an MSVC-flavored linker via
`-dumpmachine` and links with a generated `.def` plus `/IMPLIB` because
link.exe has no `--export-all-symbols`. Defect trail: `40655262547`
(`2206f4e`) → `40670278321` (`1811ce1`) → `40671917569` (`fb51841`) →
`40676108614` (`32b0ef5`) → `40677187427` (`bc5d1c9`, green).

### 6.G WASM-001 — Wasm target (deliberately deferred)

**Why not now.**
- The C backend targets a *hosted* shim (`stdio.h`, `malloc`, `calloc`); wasm32
  has no libc unless you bring `wasi-libc` or write a freestanding runtime, so
  WASM-001 is really "define the wasm import/export ABI + freestanding runtime" —
  `GC-001` + `FFI-001`-sized work.
- The spec/design split it into `wasm32-web` and `wasm32-wasi` with different
  capability policy; GC across the boundary and capability-allowlisted imports
  must be settled first (`Docs/Design/V1/04-targets-and-backends.md` §7).
- The planned path is "emit C, compile with `clang --target=wasm32`", which needs
  a wasm-capable clang + wasi-sysroot — a different toolchain from the host C
  compiler that now builds native binaries here.
- Target table marks `wasm32-*` as Tier 2 ("supported, best effort"); Tier 1
  native is not verified yet.

**Sketch.** Emit the same C17, compile with `clang --target=wasm32-wasi`
(wasi-sdk), provide a WASI-backed allocator/console shim, define imports/exports,
then add `wasm32-web` glue. Acceptance: runnable module for a corpus subset with
documented imports/exports and capability checks.

---

## 7. Architecture notes and hard-won invariants

**Pipeline.**
`astra build` → `compiler-client` (JSON-RPC) → `astra-compiler-api` (parse,
resolve, check) → typed module (AST + HIR + `annotations` + `manual_memory_scopes`
+ `integer_policy`) → `astra_codegen_c::generate` / `generate_project` →
`GeneratedC { source, header, host_shim, script_sources, target }` → CLI writes
files and invokes clang.

**Typedef/ordering rules (learned the hard way).**
1. `RUNTIME_HEADER` is prepended to every generated `.c` and `.h`. `HOST_SHIM`
   is only the hosted implementation *body*; `runtime::host_shim()` prepends the
   header so `astra_host.c` is self-contained (`NS_Bytes`/`NS_Map`/builtin error
   payloads). Do not duplicate the `NS_*` typedefs in the shim — anonymous-struct
   typedef redeclarations are incompatible types. See 6.F.
2. Typedefs are *ensured lazily* while emitting code (`TyCtxt::ensure`), so the
   final layout must be assembled **after** bodies are emitted: runtime header →
   module comment → dep includes → complete typedefs → prototypes → code.
   `emit_program` / `emit_program_linked` do this; keep the pattern when adding
   the generics buffer assembly.
3. Rejects are checked after **prescan** and again after **emission**. Never drop
   the post-emission check — it is what turns "silently missing behavior" into a
   hard error.

**Expression emission model.**
- `expr()` is pure-expression except for the `self.pre` side buffer: arms that
  need statement temporaries (`str.slice`, `to_int`, `checked_*`, match
  scrutinees, propagation, map inserts) append lines to `self.pre` and return a
  temp name.
- Callers must `flush_pre()` before writing the consuming statement — `emit_let`,
  `emit_return`, expression statements, and the match dispatchers do. Any new
  statement form must flush too.
- `fresh_id(prefix)` (preferred) / `freshid` produce `__<prefix><n>`; order must
  stay deterministic for tests.
- `self.expected: Option<Type>` carries contextual value types (used for bare
  `None`); set it via `expr_with_expected` at `let`, `return`, and match arm
  value positions.

**Fallible-call model.**
- The checker annotates a fallible call with its **base type** and tracks the
  error effect separately; `TypeAnnotation` has no effect field.
- `effect_error_type(expr)` knows the builtin fallible members plus local
  functions with error sets; `materialized_result_type(expr)` turns that into
  `Result<base, E>`. Used by `match` scrutinees and `expr!`.
- New fallible builtins must extend both and the corresponding `BUILTIN_NOMINALS`
  entry.

**Builtin nominal payloads.**
- `BUILTIN_NOMINALS` in `ty.rs` (`SliceError`, `ParseIntError`, `OverflowError`)
  declare shared, **stem-free** C typedefs (`NS_SliceError`, …) in
  `RUNTIME_HEADER`, each `{ NS_Str _0; }` (tuple-struct payload parity with
  Nova/JS). Add new ones in both places.

**Cross-module types.**
- `TyCtxt::foreign` + `nominal()` resolve local → foreign names; `ident()` uses
  the declaring module's prefix for containers; `install_foreign_types` runs
  before `prescan`; headers include dependency headers. Enum tag constants come
  from the owning module (`enum_tag`).

**Runtime types.**

| Type | Layout |
|---|---|
| `NS_Str` | `{ const char* data; size_t len; }` |
| `NS_Bytes` | `{ const uint8_t* data; size_t len; }` |
| `NS_Array` | `{ void* data; size_t len; size_t cap; }` |
| `NS_Map` | `{ void* keys; void* values; size_t len; size_t cap; size_t key_size; size_t val_size; bool key_is_str; }` |
| `NS_Unit` | `{ char _unit; }` |
| `Option<T>` | `typedef struct { bool some; T value; } NS_Option_<T>;` |
| `Result<T,E>` | `typedef struct { bool ok; T value; E error; } NS_Result_<T>_<E>;` |
| named/tuple structs | `NS_<stem>_<Name>`; enum = tag constant + payload union |

**String semantics.** UTF-8; `len` = bytes; `slice(start, end)` = byte offsets,
fallible (`astra_str_slice_try`, fails reversed/OOB/splits a scalar);
`char_at` is a scalar index; `byte_at` is a byte index; `to_int` is strict
decimal `[+-]?[0-9]+` within i32.

**Integer semantics.** Default policy wrapping in tests; explicit `+%`/`-%`/
`*%`/`/%`; `checked_*` via `__builtin_*_overflow` and `INTn_MIN`; division by
zero panics; `MIN / -1` fails checked and wraps for `wrapping_div`.

**Maps.** Key equality/comparison via `astra_map_*` with `key_is_str`; `.get` and
`.insert` return `Option<V>`.

**Defer.** `defers: Vec<Vec<Stmt>>`; `emit_scope_defers` / `emit_all_defers`;
deferred calls run in reverse order per scope.

**Scripts.** `collect_scripts` validates C/C++ only (`NS7002`); one TU per block
with prototypes in the main TU; CLI links script TUs and adds `-lstdc++` for C++.

**Module/entry rules.** Linked mode: `prefix = "<stem>_"`, `emit_main_entry`
true only for the last entry; standalone keeps bare `ns_<name>` and `main.c`.

---

## 8. Testing and verification

**Fast loop.**

```powershell
cargo test -p astra-codegen-c                 # 77 emit tests + conformance (links/runs when a hosted compiler exists)
cargo test -p astra-nova --test conformance   # corpus on Nova
npm run build                                 # tsc -b (needed before JS/TS tests)
node --test --test-name-pattern "shared conformance" orchestrator/tests/codegen-js.test.mjs
node --test --test-name-pattern "shared conformance" orchestrator/tests/codegen-ts.test.mjs
node --test --test-name-pattern "native" orchestrator/tests/codegen-js.test.mjs
```

**Full gate.** `npm run ci` runs: TS build, TS tests, `test:boundary`, `cargo fmt
--check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test
--workspace`.

**Conformance policy (`conformance/README.md`).**
- Cases: `cases/<name>.ns` + `cases/<name>.expected` (stdout lines + final
  `RETURN=<n>`).
- Each backend runs the corpus itself; gaps are listed in that backend's ledger,
  and a stale ledger entry fails the test.
- JS/TS/Nova ledgers are empty; C's static gate ledger currently has
  `channels` (CONC-001) and `generics` (this task).
- Panic/deadlock behavior is out of corpus scope; pin it in backend unit tests.

**Adding a C feature safely.**
1. Emit test that fails first (structure + `assert_clang_ok`).
2. Implement the smallest arm that compiles.
3. Add a corpus case only when JS/TS/Nova already implement the semantics (run
   all three before adding the `.expected`).
4. Never add a case that the C backend silently miscompiles; ledger it instead.

---

## 9. Slice workflow (must follow)

1. **Read** the relevant spec section and design doc; check `todo.md` acceptance.
2. **Implement** in `crates/astra-codegen-c` (runtime in `runtime.rs` when needed).
3. **Test** `cargo test -p astra-codegen-c`; add targeted tests, keep
   `assert_clang_ok` on every new emission path.
4. **Full CI** `cargo fmt --all` then `npm run ci` (green before committing).
5. **Docs** — update `Docs/Agent/todo.md` (acceptance, evidence, test counts) and
   append a `Docs/Agent/state-log.md` entry; refresh the "Current snapshot" row
   (Current task / Next action) and this handoff's snapshot line + commit table.
   When a conformance ledger changes, update `conformance/README.md` counts.
6. **Commit** with a temp message file:
   ```powershell
   # write C:\Users\Somya\AppData\Local\Temp\opencode\commit-msg.txt
   git add -A
   git commit -F "C:\Users\Somya\AppData\Local\Temp\opencode\commit-msg.txt"
   ```
   Message style: `<TASK>: <imperative summary>` + body with what/why/tests.
7. **TODO discipline** (explicitly required): mark the item in `todo.md` as it
   completes, with evidence, in the same commit. Do not batch doc updates.

**PowerShell quirks.**
- `cargo test` pipelines can report exit code 1 from stderr; trust
  `test result: ok`.
- Quote paths with spaces (`C:\Projects\Neo Astra`).
- `core.autocrlf=true`, no `.gitattributes`; LF warnings on commit are normal.
- Use `Select-String -Pattern ... -Context 0,N` (non-negative context).
- `rg` is not installed; use the grep tool or `Select-String`.

**Environment.**
- clang: `C:\Program Files\LLVM\bin\clang.exe` (freestanding `-c` works).
- gcc: `C:\msys64\ucrt64\bin\gcc.exe` (hosted compile/link/run works — this is
  the compiler the native path selects here). Clang cannot build hosted C on
  this host without the MSVC/Windows SDK (`stdio.h` missing); both the test
  harness and `astra build --target native` probe for hosted capability.
- Temp dir for messages: `C:\Users\Somya\AppData\Local\Temp\opencode`.

---

## 10. Known small gaps (not in the corpus)

These are unpinned; fix opportunistically or when a case needs them:

- Fixed arrays (`[T; N]`) — rejected by `ty.rs`.
- Map `.keys()` — not in the checker member table today (JS helper unused).
- Exported generic functions — cannot be exposed in headers (see 6.A).
- Cross-module generic calls — no instantiation visibility (see 6.A).
- Default imports — `NS8019` by design in this slice.
- `print` of nested/composite values — `NS8024` for unsupported shapes.
- Panic messages for `Result.unwrap`/`error` omit the rendered payload (JS
  includes it); behavior class matches, text does not.
- Multiple error types in one signature — `NS8019` reject.
- Library export lists are implicit (`pub` means external). A generated
  visibility file / version script is not implemented (see 6.E).

---

## 11. Open questions

1. Should exported generic functions be rejected outright, or emitted as
   per-instantiation header prototypes discovered from exported call sites?
2. Target naming for Wasm: `wasm32-wasi` vs `wasm32-wasip1` (design doc §7.3
   leaves this open).
3. Library ABI: the raw `NS_*` structs are recorded as **provisional** in
   `11-c-abi.md`; opaque handles remain a future option.
4. Manual memory: scope-based release vs explicit `free` API (see D-decisions).
5. Concurrency: cooperative single-thread first slice vs pthreads — recommend
   mirroring JS/Nova (cooperative) for parity.

---

## 12. Quick index

**Frequently touched functions (`emit.rs`)** — names are stable; line numbers
drift: `emit_program`, `emit_program_linked`, `emit_header`, `collect_decls`,
`find_main`, `emit_fn_proto`, `fn_params_c`, `fn_return_c`, `emit_constant`,
`emit_function`, `emit_main`, `emit_block_body`, `emit_stmt`, `emit_let`,
`emit_return`, `emit_while`, `emit_for`, `emit_expr_stmt`, `expr`,
`call_c`, `method_c`, `integer_method_c`, `assign_c`, `lvalue`, `index_c`,
`member_c`, `propagate_c`, `match_expr_c`, `emit_match_dispatch`,
`match_option/result/enum/struct/tuple/literal`, `emit_arm_body`,
`effect_error_type`, `materialized_result_type`, `expr_expecting`,
`expr_with_expected`, `mentions_unresolved`, `zero`, `fresh_id`, `flush_pre`,
`prescan`, `prescan_fn`, `prescan_items`, `prescan_script`,
`install_foreign_types`, `emit_project`.

**`ty.rs`:** `TypeEnv`, `DeclShape`, `TyCtxt::{new, add_foreign, nominal,
struct_shape, enum_shape, enum_tag, ensure, ident, c_type, user_typedef,
rejected}`, `to_type`, `path_to_type`, `sanitize`, `BUILTIN_NOMINALS`.

**Library emission:** `lib.rs::{EmitKind, generate_project_with_kind}`,
`emit::emit_project(entries, emit_main)`, CLI `--emit` + `findArchiver`; ABI in
`Docs/Design/V1/11-c-abi.md`.

**`runtime.rs`:** `RUNTIME_HEADER`, `HOST_SHIM` (body), `host_shim()` (complete
`astra_host.c`); helpers: `astra_str_*`,
`astra_bytes_*`, `astra_array_*`, `astra_map_*`, `astra_add/sub/mul_i8..i64`,
`astra_panic`, print/println families, `astra_alloc`.

**Useful greps.**
- `reject(` — every diagnostic the backend can emit.
- `self.pre` — expression arms that need statement temporaries.
- `BUILTIN_NOMINALS` — shared error payload types.
- `KNOWN_GAPS` — backend ledger(s).
- `expected` — contextual-type machinery.

---

## 13. Document history

| Date | Change |
|---|---|
| 2026-10-27 | Created at `47d2598` for the C-001 handoff (generics next). |
| 2026-10-27 | Generics monomorphization landed (`90cdba1`); snapshot, ledger, and 6.A updated. |
| 2026-10-27 | Hosted shim fixed (`host_shim()` = runtime header + body), hosted-compiler probing added, C conformance gate now links/runs every non-gap case, match-arm `pre` leak fixed; 6.F marked done on this host. |
| 2026-10-27 | `Array.get` landed; native library artifacts (`--emit staticlib\|cdylib`) and provisional C ABI (`11-c-abi.md`) added; 6.E marked done. |
| 2026-10-27 | Single-threaded `CONC-001` slice (eager spawn/Task, bounded Channel, ChannelError) landed; `channels` left the ledger and 6.D marked first-slice-done. |
