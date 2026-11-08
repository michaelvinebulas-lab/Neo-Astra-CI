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

The current line is **1.0.0**. Future releases move in lockstep; every entry
below names the version it lands in, and nothing is scheduled beyond what is
written.

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

### Language v0.2 - syntax cleanup (package version TBD)

A breaking cleanup of the v0.1 surface: one error vocabulary, turbofish for
expression type arguments, `@mode(...)` / `@memory(...)` scope blocks, settled
separators and line continuation, tuple-index lexing, and removal of `/%`,
`spawn ||`, and the special `///` meaning. Aliases stay as an adoption aid only
if they become contextual and a strict canonical mode is added. Feature gaps
(`if let`, ranges, bitwise operators, `char`, lambdas) are added only after the
grammar is frozen. Plan and open decisions:
[`Report/Syntax-v0.2.md`](Report/Syntax-v0.2.md).

### 1.1.0 - Browser playground

- A static, browser-hosted playground: edit source and manifest, check, format,
  view the AST and mode explanation, generate JavaScript, and run simple
  programs in a sandboxed frame - no server required.

### 1.2.0 - High-level mode

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

### 2.0.0

- Reserved for an incompatible change, if one ever becomes necessary. Nothing is
  planned for it.
