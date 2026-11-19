# Changelog

All notable changes to Neo Astra are documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Native Rust CLI, NoPM package manager, toolchain resolution, and JS/TS build bridge.

## [1.0.0] - 2026-11-06

### Added
- Standalone distribution: `npm run package` compiles the `astra` CLI and the
  NoPM dispatch into one host-native executable with Bun bytecode, minified
  identifiers, no source maps, and no comments; ships the Rust compiler and
  Nova binaries with symbols stripped; and bundles the pinned native
  TypeScript compiler for `@script:ts`. The package runs without Node.js, npm,
  Cargo, or a source checkout. `nopm` is a small launcher for `astra nopm`, so
  the runtime is embedded once.
- Structured `TaskError` results at `join`: a child panic or cancellation that
  prevents a value raises `TaskError` in the joining task, while a child that
  returns `Result<T, E>` keeps that value. `task.canceled()` reports accepted
  cancellation without joining; a late cancellation is not recorded, a produced
  value wins, and a recorded panic wins at join. The scope applies the same rule
  to unjoined children (`Docs/spec-v0.1.md` 9.5).
- Pinned C toolchain downloads: official Zig 0.13.0 archives for Linux, macOS,
  and Windows on x64/arm64, with per-host URL, size, and SHA-256; verified
  download into the project cache; `zig cc` compilation and `zig ar` archives;
  a lockfile record; `--no-download`; and manual-install instructions on every
  refusal or failure. No elevation and no writes outside the project.
- JavaScript backend map indexing (`map[key]` panics on a missing key;
  `map[key] = value` inserts or replaces), `return`/`break`/`continue` through
  value-position expression blocks, and `abort` through the host termination
  primitive, each with success and failure tests.
- `@script:c` §5.1 foreign-frame root registration (`astra_gc_scope_enter` /
  `astra_gc_root` / `astra_gc_scope_exit`) at managed-argument boundaries.
- Normative left-to-right call-argument evaluation order (D27; spec §3.10). Other expression positions - binary operands, array elements, struct fields, map entries - are target-native and carry no portability guarantee; the system-mode side-effect rejection pass is specced and queued.
- Hybrid `NS6002` micro-pass: generic forwarding of managed element types into
  manual allocation is now rejected at instantiation sites.
- `@export_as("symbol", T = type)` instantiation exports (spec 11.5): the
  parser and formatter carry typed bindings, the checker gates them with
  `NS7010`-`NS7013`, and the C backend emits each listed instantiation as an
  externally visible function under exactly the author-chosen symbol (also
  prototyped in the module header).
- `CONC-001` concurrency model decided (2026-11-01): native `spawn` runs on an
  OS thread bound to its structured scope (eager inline fallback for Web/JS/Nova);
  blocked channel operations park and a runtime watchdog (5 s,
  `ASTRA_CHANNEL_DEADLOCK_TIMEOUT` in tests) turns a deadlock into a structured
  panic; native tasks get per-task heaps plus a thread-safe immortal heap;
  cancellation is cooperative at GC safepoints and channel operations. Spec
  9 updated; the implementation landed with structured `TaskError` results.
- Compiler API `ast` and `explain` (both were advertised and stubbed): `ast`
  returns the parsed surface tree, `explain` returns the least-privileged mode
  inference as structured evidence plus its human-readable rendering. Parse
  errors come back as failed responses, never JSON-RPC errors. Exposed to the
  browser playground through `astra-wasm` (which delegates to `process_json`).
- C emitter fuzz pass (HARD-001 A): `astra-codegen-c/tests/fuzz_emit.rs` proves
  that any input which parses, resolves, and type-checks cleanly also emits C
  without panicking (`BackendError` is a valid outcome; a panic is not).
- Structural fuzzing pass for the parser (pseudo-random inputs never panic).
- Structural fuzzing pass for the whole frontend (`fuzz_frontend.rs`): token
  soup and mutants of valid programs must never panic the parser, resolver,
  checker, or formatter; clean parses must survive `astra fmt` twice.
- Parser nesting limit (HARD-001): expression, type, or block nesting deeper
  than 64 levels is rejected with `NS2002` (and the rest of the input is
  skipped) instead of overflowing the stack and aborting the process.
- Block-vs-map disambiguation no longer re-parses (HARD-001): when the
  speculative parse of a `{` expression finds no map colon and no errors, it is
  reused as the block's first statement instead of restoring and re-parsing the
  whole subtree. The old backtracking doubled work per nesting level, so deeply
  nested blocks took exponential time (28 levels parsed for minutes).

### Changed
- The V1 tree no longer contains the private LSP stub or the direct Wasm/LLVM
  backend scaffolds. V1 machine code stays on the C toolchain; unsupported
  targets and foreign modules on the JS/TS backends reject explicitly with a
  stable diagnostic.
- Script translation units now include the runtime header so managed types are
  nameable from foreign bodies.
- C backend hoists side-effecting call arguments into temporaries to preserve
  left-to-right evaluation order regardless of the host C compiler.

### Fixed
- (none)
