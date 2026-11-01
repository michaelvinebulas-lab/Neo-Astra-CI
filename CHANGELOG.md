# Changelog

All notable changes to Neo Astra are documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
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
  9 updated; implementation is the last V1 item.
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
- Script translation units now include the runtime header so managed types are
  nameable from foreign bodies.
- C backend hoists side-effecting call arguments into temporaries to preserve
  left-to-right evaluation order regardless of the host C compiler.

### Fixed
- (none)
