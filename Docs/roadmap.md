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

The current line is **0.1.0** (pre-1.0). **1.0.0 is cut when the "To be built"
list under it is complete** - not before. Every entry below names the version it
lands in; nothing is scheduled beyond what is written.

---

## Built (0.1.0)

### Language

- Statically typed language for web, app, and system targets; `.ns` source,
  `astra` tool.
- Four capability modes - `core`, `app`, `web`, `system` - with least-privileged
  inference and stable diagnostics.
- Modules, imports/exports, generics and monomorphization, structs, enums,
  `match`, `if` expressions, loops, error unions and `?`/`!`, defer, channels and
  `spawn` (single-threaded), string/array/map/slice/tuple types.
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

- `astra` CLI: check, format, build, run.
- Package manager: lock, tree, install, search, audit, why, update; registry
  adapters (npm, Cargo, PyPI, NuGet, OCI, git, path); manifest formatting.
- Continuous integration on Linux, macOS, and Windows.
- Conformance suite shared across backends; frontend and C-emitter fuzzing.

### Completed since 0.1.0 (on the way to 1.0.0)

- **Two-manifest project layout:** a required package manifest and an optional
  build manifest, with strict rejection of keys in the wrong file and a single
  read path.
- **Package add and remove:** `nopm add` / `nopm remove` that edit the package
  manifest in place, preserving formatting and comments, then rewrite the
  lockfile, install, and roll back if resolution or installation fails.

---

## To be built

### 1.0.0 - complete the stack

- **System package manager integration:** install tools through winget, brew,
  apt, dnf, or zypper as an explicit package source; bootstrap delegates to it.
- **Toolchain pinning:** a toolchain section in the build manifest, a resolution
  ladder (pin, then local cache, then PATH), and a doctor command that explains
  what was found.
- **Pinned toolchain download:** fetch and verify a pinned Zig compiler
  (`zig cc`) for cross-compilation, offline-safe, without implicit elevation.
- **Multithreaded concurrency:** an OS thread per spawn bound to its structured
  scope, channel operations that park instead of spin, a deadlock watchdog,
  per-task heaps, and cooperative cancellation.
- **Release engineering:** version and changelog automation, packaging, install
  and upgrade documentation, and the 1.0 release itself.

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
  the pattern already used for C, Wasm, JavaScript, and TypeScript.
- WebAssembly as a first-class managed target (including garbage collection in
  linear memory).
- A first-party machine-code backend, replacing the current path of generating C
  and calling the platform C compiler.

### 2.0.0

- Reserved for an incompatible change, if one ever becomes necessary. Nothing is
  planned for it.
