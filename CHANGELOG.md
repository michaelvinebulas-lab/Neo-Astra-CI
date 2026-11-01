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
- Structural fuzzing pass for the parser (pseudo-random inputs never panic).

### Changed
- Script translation units now include the runtime header so managed types are
  nameable from foreign bodies.
- C backend hoists side-effecting call arguments into temporaries to preserve
  left-to-right evaluation order regardless of the host C compiler.

### Fixed
- (none)
