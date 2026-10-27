# C-001 handoff — C backend state, architecture, and remaining work

> **Purpose.** This is the transfer document for the `C-001` workstream: where
> the C backend stands, how it is built, what is left, and how to pick up
> without re-deriving context. It complements `Docs/Agent/todo.md` (task queue)
> and `Docs/Agent/state-log.md` (chronological history); when they disagree with
> this document, the task queue and spec win.
>
> **Snapshot:** `master = d8e4efa` (2026-10-27), working tree clean, full
> `npm run ci` green. The current slice (hosted shim + native conformance
> execution) is described in section 4.
>
> **Next task:** pick the next remainder item — native library output + ABI
> (section 6.E) or package linking (section 6.B) are the best next slices;
> `Array.get` is an opportunistic small fix. Cross-platform native verification
> (6.F) is now complete on this Windows host via gcc; only a Tier-1 CI matrix
> remains.

---

## 1. Resume in five minutes

```powershell
cd "C:\Projects\Neo Astra"
git log --oneline -3          # expect 47d2598 Docs: reconcile INTEROP-001 ...
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

- `master = d8e4efa`; tree clean; `npm run ci` green.
- `astra-codegen-c`: **73** emission tests + 1 conformance gate.
- Corpus: **19 positive** cases, **11 negative** rejects.
- C emits, clang-compiles, links, **and runs** **18 of 19** cases when a
  hosted-capable compiler is present. One ledgered gap:
  - `channels` — `CONC-001` (generic `Channel` builtin; spawn/channels rejected).
- Native execution is verified on this Windows host through **gcc**
  (`C:\msys64\ucrt64\bin`): `astra build --target native` links `main.c` +
  `astra_host.c` (+ `@script:c`/`.cpp` TUs) and the binary runs. Clang alone
  still cannot build hosted C here (no MSVC/Windows SDK), so both the test
  harness and the CLI probe for hosted capability instead of preferring clang.
- Linux/macOS native builds remain unverified (Tier-1 CI matrix is the remaining
  infrastructure item).

### Commits that built the current state (newest first)

| Commit | Content |
|---|---|
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
`match`, `fail`, `panic`, `abort`. `spawn` rejects with `NS8011`.

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
| arrays | `len`, `push` (**`get` missing** — checker has it) |
| maps | `len`, `contains`, `get`, `insert` |
| integers | `checked_add/sub/mul/div`, `wrapping_add/sub/mul/div` |

**Failible model.** `str.slice`, `str.to_int`, `checked_*`, user error-set
functions. Division by zero panics in every arithmetic mode.

**Multi-module.** One TU + one header per module; module-qualified symbols
(`ns_<stem>_<name>`, `NS_<stem>_<Type>`); cross-module calls and cross-module
struct/enum types; entry module emits `int main(void)`; headers include
dependency headers. Default imports are rejected (`NS8019`).

**Generics.** Local generic functions are monomorphized per call site
(`ns_<fn>_<type args>`, e.g. `ns_identity_i32`), including generic-to-generic
calls; see section 6.A for the boundaries.

**Inline scripts.** `@script:c` / `@script:cpp` → one TU per block, C ABI
prototypes, `extern "C"` for C++, CLI compiles and links script TUs
(`-lstdc++` for C++). Native link verified end to end on this host.

**Diagnostics currently emitted by the backend.** `NS8011` (spawn/concurrency),
`NS8012` (manual memory scopes), `NS8017`/`NS8018` (unsupported call/generic),
`NS8019` (default imports, multi-error signatures), `NS8020` (unknown nominal
type), `NS8021` (unsupported pattern/method), `NS8024` (print type), `NS8099`
(unresolved type). All rejects abort generation (`generate` returns `Err`).

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

### 6.B Package linking (Cargo/npm/native)

**Today.** `astra build` resolves the local module graph and the C path emits
one TU + header per module. Manifest-level dependencies (packages installed by
NoPM) do not feed the C module graph; only explicit local `import`s link.

**Design sketch.**
- `orchestrator/packages/cli` build command: after manifest resolution, append
  package modules to the project graph with canonical keys; `compileProject`
  already accepts `{key, path, text}` entries.
- Artifact layout: per-package subdirectory or flattened `<stem>.c/.h` with
  collision-free stems (module keys already provide stems).
- Determinism: stable ordering of entries (dependencies before importers) —
  `generate_project` requires that ordering (entry = last).
- Acceptance: fixture project with a path dependency builds via
  `astra build --target c`; symbols qualified; two packages declaring the same
  helper link cleanly.

**Risks.** Name/stem collisions across packages; include-path handling in the
CLI; package-provided C script TUs.

### 6.C GC-001 — managed memory and `@memory:manual`

**Today.** `@memory:manual` scopes are rejected with `NS8012`. The hosted runtime
allocates with `malloc` through `astra_alloc` (arrays, maps, `bytes.new`) and
never frees; there is no ownership model.

**Design sketch (read `Docs/Design/V1/03-runtime-and-memory.md` first).**
- Managed mode: arena/bump allocator per program run, released at exit; keep the
  freestanding `extern` boundary so a future Wasm target can supply its own
  allocator.
- Manual mode: emit explicit `astra_alloc`/`astra_free` with a lexical ownership
  rule (scope-based `defer`-style release) — must match the spec's manual rules
  before enabling.
- Acceptance: remove `NS8012` for the supported subset; add a leak/ownership
  test that runs the shim (requires the native gate); document the model.

**Risk.** This is a language-semantics change, not just codegen — do not start
without reading the memory design doc and the D-decisions.

### 6.D CONC-001 — structured concurrency

**Today.** `spawn` and channels are rejected (`NS8011`; `Channel` is generic →
`NS8018`). Nova/JS implement eager tasks and a bounded (capacity 16) channel with
`send`/`receive` returning `Result<_, ChannelError>` and deadlock panics.

**Design sketch.**
- Single-threaded cooperative first slice mirroring JS/Nova: eager `spawn`
  (run the block immediately, store the result), `Task.join`, bounded channel
  buffer with deadlock panic.
- Requires generics (Channel<T>) — do this after 6.A.
- Acceptance: `channels` flips out of `KNOWN_GAPS`; the C conformance runner (when
  the toolchain lands) executes it.

### 6.E Native library output + stable ABI

**Today.** Project mode already emits a header per module with `extern`
prototypes for `pub` functions (skipping generics), and `astra build --target c`
writes `<stem>.c`/`<stem>.h` + `astra_host.c`. There is **no library mode**
(no-`main`), no `.a`/`.so`/`.dll` output, and no documented ABI contract.

**Design sketch.**
- CLI: `--emit staticlib|cdylib|lib` (naming TBD) that compiles TUs and archives
  them; suppress the `main` wrapper.
- ABI doc (new `Docs/Design/V1/…` or a section in the C docs): symbol naming
  (`ns_<stem>_<name>`), struct layout = declaration order, enum representation
  (tag + payload), `NS_Str`/`NS_Bytes`/`NS_Option_*`/`NS_Result_*` layouts, error
  payload convention (`_0`), ownership rules for returned memory.
- Acceptance: build a library from a `.ns` module, consume it from a hand-written
  C file, document and test the ABI (compile/link may be gated on the toolchain).

### 6.F Cross-platform native verification + CI gate — **DONE on this host**

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

**Remaining.** Provision/record a Tier-1 Linux (and ideally macOS) run of the
same commands. That is infrastructure, not codegen, and it is the last gate for
calling `C-001` fully done.

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
cargo test -p astra-codegen-c                 # 73 emit tests + conformance (links/runs when a hosted compiler exists)
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

- `Array<T>.get(i) -> Option<T>` — checker has it; C does not.
- Fixed arrays (`[T; N]`) — rejected by `ty.rs`.
- Map `.keys()` — not in the checker member table today (JS helper unused).
- Exported generic functions — cannot be exposed in headers (see 6.A).
- Cross-module generic calls — no instantiation visibility (see 6.A).
- Default imports — `NS8019` by design in this slice.
- `print` of nested/composite values — `NS8024` for unsupported shapes.
- Panic messages for `Result.unwrap`/`error` omit the rendered payload (JS
  includes it); behavior class matches, text does not.
- Multiple error types in one signature — `NS8019` reject.

---

## 11. Open questions

1. Should exported generic functions be rejected outright, or emitted as
   per-instantiation header prototypes discovered from exported call sites?
2. Target naming for Wasm: `wasm32-wasi` vs `wasm32-wasip1` (design doc §7.3
   leaves this open).
3. Library ABI: do we guarantee the raw `NS_*` structs, or add opaque handles?
   Decide before shipping 6.E.
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
