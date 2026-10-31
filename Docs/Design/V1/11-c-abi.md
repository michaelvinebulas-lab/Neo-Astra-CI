# C ABI and library artifacts

**Status:** Provisional V1 implementation contract
**Depends on:** D1, D6–D10, D21
**Related:** [`03-runtime-and-memory.md`](./03-runtime-and-memory.md),
[`04-targets-and-backends.md`](./04-targets-and-backends.md),
[`05-manifest-and-packages.md`](./05-manifest-and-packages.md)

---

## 1. Scope and stability

This document records the exact contract the C backend emits so a hand-written
C consumer can link generated code. It is **provisional**: `03-runtime-and-memory.md`
§3.2 says V1 must not promise unstable layout as a public guarantee. The names,
layouts, and ownership rules here may change before 1.0; consumers that depend on
them should pin the compiler version.

Out of scope in this slice: GC, manual memory, structured concurrency, FFI
ownership metadata, and cross-package module linking.

---

## 2. Artifacts

`astra build` produces one of three artifact kinds, selected by `--emit` or the
manifest target's `emit` key:

| `--emit` | Artifact | Contents |
|---|---|---|
| `executable` (default) | `main.exe` / `main` | All module objects, script objects, and the runtime shim |
| `staticlib` / `library` | `lib<name>.a` | Module and script objects only |
| `cdylib` | `lib<name>.so` / `lib<name>.dll` (+ `.dll.a` import library on Windows) | Module/script objects plus the runtime shim |

`<name>` is the manifest package name with characters outside
`[A-Za-z0-9_.-]` replaced by `_`.

The build also writes the generated sources and per-module headers beside the
artifact, plus `astra_host.c` and an `obj/` directory.

### 2.1 Linking contract

- **Static library:** the archive does **not** contain the runtime. The consumer
  links `astra_host.c` (or a prebuilt runtime object) as well. This keeps two
  Astra static libraries from colliding on `astra_*` symbols.
- **Shared library:** the runtime shim is linked in, so the consumer needs only
  the shared library (plus the import library on Windows).
- `@script:c` / `@script:cpp` objects are included in the same artifact. C++
  scripts require the C++ runtime (`-lstdc++` is added by `astra build`).

Example consumer for a static library:

```c
#include <stdio.h>
#include "src_lib.h"          /* generated per-module header */

int main(void) {
    printf("%d\n", ns_src_lib_add(20, 22));
    return 0;
}
```

```sh
cc consumer.c -I dist/lib dist/lib/libmathlib.a dist/lib/astra_host.c -o consumer
```

---

## 3. Symbol naming

| Declaration | C symbol |
|---|---|
| function `f` in module key `src/lib.ns` | `ns_src_lib_f` |
| struct/enum `T` in module key `src/lib.ns` | `NS_src_lib_T` |
| exported function list | `<module>.h` |

The stem is the module key with `.ns` stripped and `/`, `.`, and `-` replaced by
`_`. Standalone single-module builds keep the bare `ns_<name>` symbols and
`main.c`/no header (unchanged from the first slice).

`pub` functions have external linkage. Non-`pub` functions stay `static`.
Exported **generic** functions are not exposed in headers and cannot be called
across the C boundary by default; each project-internal call site gets its own
`ns_<fn>_<type args>` specialization. A module may opt into exporting concrete
instantiations with the `@export_as("symbol", T = type, ...)` attribute
(spec §11.5); each such instantiation is emitted as a concrete, externally
visible function whose symbol is **exactly the string given in the attribute** —
the compiler does not mangle or decorate it. The author chooses the symbol and is
responsible for it being a valid, unique C identifier; a collision is `NS7013`.
The recommended convention is `ns_<module stem>_<fn>_<type args>`, using the
monomorphization suffix already used for internal specializations (one segment
per type parameter in declaration order, C-side encoding: `i32`, `u8`, `str`,
`NS_<stem>_<Name>` for structs and enums), but the attribute string wins. The
exported instantiation's signature is the generic's signature with every type
parameter substituted by its bound type. Because the symbol is author-authored,
consumers treat it as the author-declared entry point rather than reconstructing
it from a mangling scheme. The refusal above stays in force for any generic
function that carries no `@export_as` attribute.

---

## 4. Value layouts

| Type | C representation |
|---|---|
| `i8`–`i64`, `u8`–`u64` | `<stdint.h>` fixed-width integers |
| `f32` / `f64` | `float` / `double` |
| `bool` | `bool` (`<stdbool.h>`) |
| `usize` / `isize` | `size_t` / `intptr_t` |
| `Unit` | `NS_Unit { char _unit; }` |
| `str` | `NS_Str { const char* data; size_t len; }` (UTF-8 bytes, not NUL-terminated by contract) |
| `bytes` | `NS_Bytes { const uint8_t* data; size_t len; }` |
| `[T]` / arrays | `NS_Array { void* data; size_t len; size_t cap; }` |
| maps | `NS_Map { void* keys; void* values; size_t len; size_t cap; size_t key_size; size_t val_size; bool key_is_str; }` |
| `Option<T>` | `NS_Option_<T> { bool some; T value; }` |
| `Result<T,E>` | `NS_Result_<T>_<E> { bool ok; T value; E error; }` |
| struct | `NS_<stem>_<Name>` with fields in declaration order |
| enum | `NS_<stem>_<Name>` with a `tag` plus a payload union; tag values are stable per declaration order |

Built-in error payloads are shared and stem-free: `NS_SliceError`,
`NS_ParseIntError`, and `NS_OverflowError` each wrap the offending text as
`{ NS_Str _0; }`.

Alignment follows the platform C ABI. No packing attributes are emitted in this
slice.

---

## 5. Ownership and memory

This slice runs a two-family allocator with a non-moving mark-sweep collector
(GC-001 Phase B; collection is off by default until Phase C). Managed allocation
goes through `astra_gc_alloc`, which places an `NS_Gc_Head` (payload extent plus
a compiler-emitted `NS_Gc_Shape`) immediately before every payload, zeroes the
payload, and records the block; `astra_runtime_shutdown` releases whatever
remains at exit. Manual memory (`Memory.alloc`, `Region`) goes through
`astra_manual_alloc`, a plain `malloc` that is never recorded, never traced, and
released only by `astra_manual_free` (`Memory.free`, `region.free`). `NS6002`
keeps GC-managed element types out of manual allocation because a manual block is
never a root. There is no public free for managed values; the collector reclaims
unreachable blocks.

- `str` and `bytes` values are borrowed views. A consumer must not free the
  `data` pointer, and a view returned from a function stays valid until the
  process exits because returned managed values are pinned for the run
  (GC-001 §5.8) — not merely until the next collection.
- Arrays and maps returned across the boundary are owned by the runtime heap and
  must not be freed by the consumer; they are pinned on return for the same
  reason.
- The pinning rule applies to values *returned* to the foreign module. Values
  *borrowed* by it are not pinned: a borrowed pointer is valid only for the call
  (spec §8.5), exactly like any other non-root, and a foreign module that keeps
  one for later use is violating that rule.
- A future release API (post-V1) will let a consumer hand ownership back;
  consumers should not encode pinning as a permanent guarantee beyond this slice.

Boundary conversions are explicit: an empty `str`/`bytes` view has a non-null or
null `data` pointer with `len == 0`; consumers must branch on `len`, not on
`data`.

### 5.1 FFI root registration (GC-001 §5.9; signature decided 2026-10-30)

A hand-written `@script:c` block runs in a foreign frame the collector cannot
see, so managed values it borrows for the call must be registered as roots.
The approved API (`[P9]`, proposal §5.9) is a scoped registration triple:

```c
size_t astra_gc_scope_enter(void);
void   astra_gc_root(size_t scope, const void* payload);
void   astra_gc_scope_exit(size_t scope);
```

- `astra_gc_scope_enter` opens a root region and returns its handle;
  `astra_gc_root` adds one payload pointer to that region (NULL ignored) — a
  `str`/`bytes` value registers its `data` view, a map registers `keys` and
  `values`, a channel registers the handle;
  `astra_gc_scope_exit` releases every root the region holds.
- Roots are released at exit, so a borrowed pointer is valid only for the call
  (spec §8.5) — the same borrowed-value rule as the pinning pair above,
  stated here so the registration and ownership rules sit together.
- Regions nest LIFO, like function frames; a debug build MAY trap on an
  out-of-order exit.
- The **emitter**, not the script author, owns the pairing: it brackets the
  whole foreign call, so an early `return` inside a script block cannot leave a
  registration unpopped.

The emitter now emits the triple at every inline `@script:` call that passes
managed arguments, in positions whose evaluation is unconditional: it opens a
scope, registers each managed argument's payload slots (§5.2 table), calls the
foreign body, closes the scope, and then publishes a managed result as a caller
frame root. Two limits are recorded rather than hidden:

- **Conditional positions are not bracketed.** In a loop condition or under the
  right operand of `&&`/`||` the emitter has no unconditional point to hoist the
  bracket to, so the call is emitted without registration — the same class of
  limitation as §5.3's materialisation rules. Arguments there are covered only
  by whatever caller-frame roots already hold them.
- **Released at scope close, not pinned.** A value borrowed by the foreign frame
  is valid only for the call; a script that keeps the pointer afterwards
  violates spec §8.5, which is the intended consequence.

§7 lists the boundary-rooting test that pins this behavior.

---

## 6. Errors and panics

Recoverable errors use the `Result<T,E>` layout above; the caller checks `ok`
before reading `value` or `error`. Panics call `astra_panic`, which writes a
message and terminates the process; there is no C++-style unwinding, and a panic
must not be caught across the boundary.

Division by zero and out-of-bounds indexing panic in every integer policy.

---

## 7. Verification

The contract is pinned by:

- `crates/astra-codegen-c/tests/emit_c.rs`:
  `library_emission_omits_main_and_exports_public_functions`,
  `library_pins_values_returned_across_the_c_boundary`,
  `only_library_exports_pin_managed_returns`,
  `array_get_returns_bounded_option`,
  `test_script_c_scope_registration_for_managed_arg` (the §5.1 triple is
  emitted in order: enter < root < call < exit), and the hosted
  boundary-rooting test (a managed `@script:c` argument survives a collection
  forced inside the foreign frame, and the root count returns to its baseline
  after the call).
- `orchestrator/tests/codegen-js.test.mjs`:
  `astra build --emit staticlib produces a linkable C library` and
  `astra build --emit cdylib produces a runnable shared library`, which build a
  library from `.ns`, compile a hand-written consumer, and run it; and
  `astra build --target native roots managed args across a @script:c call`,
  which links and runs a program whose script collects inside the foreign frame.
- The conformance gate exercises the same runtime layouts for every case whose
  backend lowering exists.
