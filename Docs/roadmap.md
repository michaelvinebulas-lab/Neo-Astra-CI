# Roadmap

What exists today, what comes next, and the version each piece ships in.
Updated whenever something is built. No internal task codes - plain names only.

Open tasks for the current version live in [`todo.md`](todo.md).

## Versioning

The project follows semantic versioning, and every package moves together:

- **Patch** (`1.0.1`) - fixes and small internal work.
- **Minor** (`1.1.0`) - new capability that does not break existing code: a new
  mode, a new language target, a new backend.
- **Major** (`2.0.0`) - only for a change that makes older projects or older
  generated output incompatible.

The published line is **1.0.1**. The native CLI/NoPM replacement is released.
Future releases move in lockstep; every entry below names the version it lands
in, and nothing is scheduled beyond what is written.

---

## Built (1.0.0)

### Language

- Statically typed language for web, app, and system targets; `.ns` source,
  `astra` tool.
- Four capability modes - `core`, `app`, `web`, `system` - with least-privileged
  inference and stable diagnostics.
- Modules, imports/exports, generics and monomorphization, structs, enums,
  `match`, `if` expressions, loops, error unions and `?`/`!`, defer, channels and
  `spawn`, string/array/map/slice/tuple types.
- Annotations: `@requires`, `@tags`, `@mode`, `@memory`, `@script`, `@export_as`.
- Canonical formatter with comment preservation; `astra fmt`.

### Compilers and runtimes

- C17 backend: managed and manual memory, `Region` arenas, precise root frames,
  mark-sweep GC with stress mode, boundary pinning, native FFI, inline C
  interop, multi-module and package linking, author-named generic exports.
- JavaScript and TypeScript ESM backends.
- Nova: a typed interpreter for development, with project loading and linking.
- Compiler API over JSON-RPC (native binary and Wasm): check, project check,
  compile, project compile, format, AST, and mode explanation.

### Tooling and quality

- `astra` CLI: check, fmt, build, doctor, ast, and explain; `nova` runs code.
- Package manager: lock, tree, install, search, audit, why, update; registry
  adapters (npm, Cargo, PyPI, NuGet, OCI, git, path); manifest formatting.
- Continuous integration on Linux, macOS, and Windows.
- Conformance suite shared across backends; frontend and C-emitter fuzzing.

### Completed on the way to 1.0.0

- **Two-manifest project layout:** a required package manifest and an optional
  build manifest, with strict rejection of keys in the wrong file and a single
  read path.
- **Package add and remove:** `nopm add` / `nopm remove` that edit the package
  manifest in place, preserving formatting and comments, then rewrite the
  lockfile, install, and roll back if resolution or installation fails.
- **System package source:** `nopm add sys:<tool>` installs git, cmake, or
  clang through winget, brew, apt, dnf, pacman, zypper, or apk, with the exact
  command lines unit-tested and bootstrap delegating to the adapter.
- **Multithreaded concurrency:** an OS thread per spawn bound to its structured
  scope, parked channel operations with a deadlock watchdog, per-task heaps,
  cooperative cancellation, and structured `TaskError` results at `join`.
- **Pinned toolchain download:** official upstream archives with per-OS/arch
  SHA-256 verification, a project-local cache, `zig cc`/`zig ar`, a lockfile
  record, `--no-download`, and manual instructions on every failure.
- **Error-code and D28 gates:** one meaning per diagnostic code, a consistency
  check in CI, and a pre-1.0 audit that closes shipped-surface gaps rather than
  hiding them as later work.
- **Release engineering:** lockstep `1.0.0` versions, changelog freeze,
  packaging, and the clean-machine install verification.

---

## To be built

### 1.0.1 - native CLI and NoPM

Replace the Bun-compiled `astra` package with Cargo-built native Rust `astra`
and `nopm` binaries. Baseline `check`, `fmt`, `ast`, `explain`, `doctor`,
C/native builds, and NoPM path flows must run without Node or npm; the JS/TS
code generators stay isolated behind an on-demand Node bridge.

Current checkpoints: slices 1, 2a, 2b, 2c, 3a, 3b, and 4 are accepted. The
packaged CLI is native Cargo output, the JS/TS bridge is isolated under
`lib/cli`, native parity is 12/12, and release validation is green. `1.0.1` is
released on GitHub and npm; all four packages carry SLSA provenance.

### 1.1.0 / 1.1.1 - Language v0.2 syntax cleanup (edition 0.2)

Shipped as **1.1.1**: `v1.1.0` was tagged but its release failed on a packaging
fixture with an unsupported edition, so that tag has no published artifacts.
The cleanup reuses the existing AST and adds a stable edition gate: edition
`0.1` keeps the old syntax byte-for-byte, edition `0.2` selects `fail`, postfix
`?`, `::<...>` turbofish, `@mode(...)` / `@memory(...)` scope blocks, canonical
aliases, settled separators and line continuation, and integer-only tuple
indexes. `/%` is preserved; `spawn ||` and old scope blocks are deprecated and
rewritten only by `astra migrate`. `spec-v0.2.md` is normative for edition
`0.2`; `spec-v0.1.md` remains normative for edition `0.1`. Plan and RFCs:
[`Report/Syntax-v0.2.md`](Report/Syntax-v0.2.md).

### 1.1.2 - Phase 7 feature gaps (planned)

Add the deferred syntax features with AST nodes and explicit backend support,
each with its runtime story: `if let` / `while let`, ranges (`a..b`, `a..=b`)
with `for`/iterators/slicing, bitwise operators (`& | ^ << >>`) with `|` in
expression versus error-set context documented, `char` and scalar literals,
and lambdas (`fn(x) => expr`) with parameter inference and capture rules.
Complete the integer/float suffix table. Do not ship any of these without a
new edition gate or an additive release.

### 1.2.0 - Browser playground (released)

- A static, browser-hosted playground in its own public repo
  (`Neo-Astra-Playground`), deployed with GitHub Actions Pages on merge and on
  `v1.2.0`. No server.
- Full virtual project: multi-file tree, JSON and TOML manifests, compiler
  panels (check, format, AST, explain), edition/canonical/migration controls,
  per-module JS/TS generation, and a core-only sandboxed runner.
- Browser-safe NoPM: local and virtual path packages, npm/Cargo live fetch with
  integrity, and declared-only adapters for Git/PyPI/NuGet/OCI/sys.
- This release is the first **frontend/tooling proof**: playground diagnostics
  and generated output must match the native CLI for the fixture set. It does
  not close the semantic-contract pillars, which are tracked one per file in
  [`Report/Pillars/`](Report/Pillars/README.md).
- Plan: [`Report/Playground-v1.2.md`](Report/Playground-v1.2.md).

### 1.3.0 - High-level mode

- A `high` mode that reads like a high-level language: no manual memory,
  pointers, regions, FFI, or script interop; top-level statements, an implicit
  entry point, `print`, and implicit tail returns. Strings stay quoted; the
  profile is selected per project.

### 1.x - More language surfaces

One release per surface, in the order we take them up:

- Rust, Go, Lua, Ruby, and Java as build targets and interop surfaces, following
  the pattern already used for C, JavaScript, and TypeScript.
- WebAssembly as a first-class managed target (including garbage collection in
  linear memory).
- A first-party machine-code backend, replacing the current path of generating C
  and calling the platform C compiler.

### V2 - orchestrator rewrite (post-1.x)

- Replace the TypeScript orchestrator with a C++20 implementation. The Rust
  core stays Rust; C is limited to the ABI boundary and leaf libraries.
  TypeScript remains the behavioral oracle and the isolated JS/TS bridge until
  parity is proven. The release number is decided when the port starts; this
  entry records the implementation direction, not a version commitment. See
  [`Report/Orchestrator-v2.md`](Report/Orchestrator-v2.md).

### 2.0.0

- Reserved for an incompatible change, if one ever becomes necessary. Nothing is
  planned for it.
