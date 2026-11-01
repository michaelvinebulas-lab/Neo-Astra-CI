# Todo

Open tasks, grouped by the version they land in (same names as the roadmap).
Completed work is recorded in `roadmap.md` and `CHANGELOG.md` - this list holds
only what is still open. Check items off as they are done and prune them at
each release.

## 1.0.0 - complete the stack

### Project layout and packages

- [ ] Finish the two-manifest project layout: strict key placement, a single
      read path with no fallback, canonical ordering, and the missing tests;
      then commit it.
- [ ] `nopm add`: resolve a package, edit the package manifest without
      destroying formatting or comments, rewrite the lockfile, install; dry-run
      support.
- [ ] `nopm remove`, with the same edit guarantees.
- [ ] JSON and TOML manifest coverage plus end-to-end tests for path, git, and
      npm sources.
- [ ] System package-manager source: winget, brew, apt, dnf, zypper; the
      bootstrap script delegates to it.
- [ ] Toolchain section in the build manifest, resolution ladder (pin, local
      cache, PATH), and a doctor command that explains the result.
- [ ] Pinned toolchain download: fetch, verify, unpack a pinned Zig compiler
      for cross-compilation; offline-safe, no implicit elevation.

### Runtime

- [ ] Multithreaded concurrency: one OS thread per spawn bound to its
      structured scope, channel operations that park instead of spin, a deadlock
      watchdog, per-task heaps, and cooperative cancellation.
- [ ] Re-pin the concurrency conformance case and run the threaded test matrix
      on Linux, macOS, and Windows.

### Release

- [ ] Version and changelog automation for the lockstep bump.
- [ ] Packaging plus install and upgrade documentation.
- [ ] Cut 1.0.0 across all packages.

## 1.1.0 - browser playground

- [ ] Compiler Wasm artifact wired into the host bundle (the bridge exists;
      the build and version check need to be part of the release).
- [ ] Editor: source and manifest panes, target and mode selectors, sample
      gallery, shareable state.
- [ ] Compiler actions: check, format, AST, mode explanation, JavaScript
      generation - each with a structured output pane.
- [ ] Sandboxed execution with captured output, timeouts, and output limits.
- [ ] Static hosting, offline-friendly assets, size budget, privacy note.

## 1.2.0 - high-level mode

- [ ] Write the language section: the new mode, what it removes, what it adds.
- [ ] Parser and lowering: top-level statements, implicit entry point and exit
      code, implicit tail return, and the error for mixing top-level statements
      with an explicit entry function.
- [ ] Checker: reject manual memory, pointers, regions, FFI, script blocks, and
      C ABI exports in this mode.
- [ ] The `print` builtin and per-project profile selection.
- [ ] Conformance: a running hello-world on every backend, plus one reject
      case per removed feature.

## 1.x - more language surfaces

One release per surface, in the order we take them up:

- [ ] Rust target and interop.
- [ ] Go target and interop.
- [ ] Lua target and interop.
- [ ] Ruby target and interop.
- [ ] Java target and interop.
- [ ] WebAssembly as a managed target, including garbage collection in linear
      memory.
- [ ] First-party machine-code backend, replacing the generate-C path.

## Housekeeping

- [ ] Wire the already-implemented `astra ast` and `astra explain` commands
      into the CLI.
- [ ] Finish the historical CI-failure repro notes.
- [ ] Release tooling: version check in the gate, release script.
- [ ] Keep the fuzz and conformance corpora growing with each feature.
