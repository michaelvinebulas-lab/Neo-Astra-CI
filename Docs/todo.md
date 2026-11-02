# Todo

Every open task, grouped by the version it lands in (same names as the
roadmap). Check items off as they are done and prune them at each release.
Completed work is summarized at the bottom; the full history is in
`Docs/archive/` and `CHANGELOG.md`.

## 1.0.0 - complete the stack

### Project layout and packages

- [x] Finish the two-manifest project layout: strict rejection of keys in the
      wrong file, one read path with no fallback to the other manifest,
      canonical key ordering per manifest kind, the restored core-order test,
      a CLI-level rejection test, and the migrated fixtures - then commit it.
- [x] Manifest edit API: add and remove a dependency without destroying
      formatting or comments, canonical output, duplicate rejection, JSON and
      TOML forms.
- [x] `nopm add`: resolve a package, edit the package manifest, rewrite the
      lockfile, install; `--json` and `--dry-run` (dry-run writes nothing).
- [x] `nopm remove`, with the same edit guarantees.
- [x] End-to-end package tests: path source through the CLI (add, remove,
      dry-run, rollback, lockfile and store); git and npm stay covered by their
      adapter tests so the gate needs no network.
- [x] System package-manager source: winget (Windows), brew (macOS), apt, dnf,
      zypper (Linux) as an explicit `sys:` source; elevation handled by the
      adapter; exact command lines unit-tested with no real installs in CI;
      `sys:` entries never land in the dependency list.
- [x] Bootstrap script delegates to the system source with unchanged dry-run
      output and no duplicated install logic.
- [ ] Validate the `sys:` tool name before the installed check, so an unknown
      tool that happens to exist on `PATH` is rejected with `NP0015` instead of
      reported as already installed.
- [ ] Toolchain section in the build manifest: parse it, resolve in order
      (manifest pin, local cache, PATH), honor `ASTRA_CC`, and record the
      resolved toolchain in the lockfile with a single writer.
- [ ] `astra doctor`: print the resolution ladder result (rung, path, version),
      refuse unknown pins, and refuse a resolution that would need a download
      when running frozen.
- [ ] Build uses the resolved compiler instead of ad-hoc detection; existing
      native tests still pass on the PATH rung.
- [ ] Pinned toolchain download: per-OS/arch artifact table with SHA-256,
      download through the package transport, verify, unpack into the local
      cache, and lockfile record.
- [ ] Download failure behavior: checksum mismatch, truncated archive, and
      offline all fail clearly, name the rung, give manual instructions, and
      never elevate.

### Runtime

- [ ] Multithreaded concurrency: one OS thread per spawn bound to its
      structured scope, with the eager inline model kept for browser, JS, and
      interpreter targets.
- [ ] Park blocked channel operations instead of spinning; add the deadlock
      watchdog (5 second runtime constant, test override via an environment
      variable).
- [ ] Per-task heaps plus a thread-safe immortal heap for interned strings and
      static descriptors; update the collector's single-heap assumption.
- [ ] Cooperative cancellation at collection safepoints and during blocked
      channel operations; cancellation of a finished or dead task is a no-op.
- [ ] Re-pin the concurrency conformance case for the new blocking semantics
      and run the threaded matrix on Linux, macOS, and Windows.
- [ ] Send copies managed values only; primitives and scalars move by value.

### Release

- [ ] Version check in the gate: all package versions and the workspace version
      must agree.
- [ ] Release script: lockstep bump, changelog section move, dry-run mode.
- [ ] Packaging: build every package and the native compiler; verify the
      bootstrap install from a clean machine.
- [ ] Install and upgrade documentation.
- [ ] Cut 1.0.0 across all packages, with release notes from the changelog.

## 1.1.0 - browser playground

### Core bridge

- [ ] Compiler Wasm artifact builds in CI and is versioned with the host
      bundle.
- [ ] Host checks the protocol version and refuses mismatched artifacts with a
      clear error.
- [ ] Requests and responses are UTF-8 JSON matching the existing schema;
      traps and panics surface as internal errors, never as user syntax errors.
- [ ] Every allocation is freed; a soak test of 1,000 compiles shows stable
      memory.

### Editor

- [ ] Edit source and a minimal manifest; switch between JSON and TOML.
- [ ] Target and mode selectors sent with each request.
- [ ] Diagnostics render severity, stable code, file, line/column, message, and
      hint; selecting one moves the cursor.
- [ ] Sample gallery loads without network access after the first visit.
- [ ] Shareable editor state in the URL.

### Compiler actions

- [ ] Check, with the full diagnostic set.
- [ ] AST view (the compiler API now returns the parsed tree).
- [ ] Format through the same formatter as the CLI.
- [ ] Mode and capability explanation (the compiler API now returns it).
- [ ] JavaScript emission with copy and download.

### Execution and safety

- [ ] Generated modules run in a sandboxed frame with no access to host globals,
      storage, or credentials; the sandbox cannot navigate the host page or open
      popups.
- [ ] Output capture with attribution; infinite loops terminate at a
      configurable timeout; output is capped; runtime failures show their codes
      and a call stack when available.
- [ ] Local compilation is the default and the UI says so; no source leaves the
      browser unless a remote compiler is explicitly enabled, with upload and
      retention disclosed first.
- [ ] Restrictive content security policy with network denied by default; no
      analytics or third-party script receives source text.

### Delivery

- [ ] Tests cover the loader, codegen bridge, diagnostic mapping, URL state, and
      sandbox timeout; one headless browser test compiles and runs a sample.
- [ ] The gate includes the playground build and tests.
- [ ] Initial gzipped payload within the size budget, with the measured number
      recorded.
- [ ] Works in current Chrome, Firefox, Safari, and Edge.
- [ ] Static hosting, offline assets, README, and privacy note.

## 1.2.0 - high-level mode

- [ ] Write the language section: the new mode, the low-level surface it
      removes, and the conveniences it adds.
- [ ] Parser and lowering: top-level statements, implicit entry point and exit
      code, implicit tail return, and an error when top-level statements are
      mixed with an explicit entry function.
- [ ] Checker: reject manual memory, pointers, regions, FFI, script blocks, and
      C ABI exports in this mode.
- [ ] The `print` builtin and per-project profile selection.
- [ ] Optional return-type inference for these modules.
- [ ] Conformance: a hello-world that runs on every backend, plus one reject
      case per removed feature.

## 1.x - more language surfaces

One release per surface, in the order taken up:

- [ ] Rust target and interop.
- [ ] Go target and interop.
- [ ] Lua target and interop.
- [ ] Ruby target and interop.
- [ ] Java target and interop.
- [ ] WebAssembly as a managed target, including garbage collection in linear
      memory.
- [ ] First-party machine-code backend, replacing the generate-C path.

## Deferred - no version yet

- [ ] Fallible allocation entry points for managed and region allocation; until
      then out-of-memory panics.
- [ ] `MutSlice<T>` behind pointer slice access (the type is named, the surface
      is not).
- [ ] Adaptive collection threshold.
- [ ] Pin release API for library returns (the collector follow-up).
- [ ] Script embeddings beyond C, JavaScript, and TypeScript: Python, Wasm, and
      Java.
- [ ] Cross-target triples with the pinned toolchain.
- [ ] Thread pool as a later performance layer, after the thread-per-spawn
      model.
- [ ] Web Workers target for the concurrency model.
- [ ] Export attributes for generic struct bindings (currently rejected).
- [ ] Cross-module uniqueness for author-named export symbols (currently per
      module only).
- [ ] A structured diagnostic for mangled-name collisions (currently surfaces
      as a C duplicate-definition error).
- [ ] Remote/hosted playground compilation (out of scope unless explicitly
      enabled).

## Housekeeping

- [ ] Wire the already-implemented `astra ast` and `astra explain` commands
      into the CLI.
- [ ] Finish the historical CI-failure repro notes.
- [ ] Keep the fuzz and conformance corpora growing with each feature.
- [x] Remove the temporary stack-size and single-thread test workaround once
      the parser fixes are confirmed in the gate.

## Done

- [x] Language frontend: lexer, parser, formatter with comment preservation,
      resolver, type checker, effects, diagnostics.
- [x] Capability modes with least-privileged inference.
- [x] C backend: managed GC, manual memory, regions, FFI, inline scripts,
      generics, packages, native libraries, export attributes.
- [x] JavaScript and TypeScript backends.
- [x] Nova interpreter and CLI, with project loading and linking.
- [x] Package manager: model, resolver, path/git/npm/Cargo/PyPI/NuGet/OCI
      adapters, lock/install/search/audit/why/update.
- [x] Compiler API: check, project check, compile, project compile, format,
      AST, explanation.
- [x] Conformance suite across backends; continuous integration on three
      operating systems.
- [x] Fuzzing (frontend and C emitter), nesting limit, parser hardening fixes.
- [x] Two-manifest language spec and build-manifest design.
- [x] Export attributes, side-effect ordering rule, generic-forwarding checks.
- [x] Documentation consolidation: one status file, one roadmap, one todo.
