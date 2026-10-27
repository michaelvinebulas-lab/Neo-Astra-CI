# Todo

This is the active work queue. Keep it short, ordered, and honest.

---

## Status legend

- `[ ]` todo
- `[~]` in progress
- `[x]` done
- `[!]` blocked
- `[-]` deferred

Only one item may be `[~]` at a time.

---

## Current milestone

**Phase 5 — Types, Nova, and NoPM**

`FMT-004`, `TYPE-001`-`TYPE-004`, `MODE-001`, `ERR-001`, `NOVA-001`-`NOVA-004`,
and `NOPM-001`-`NOPM-004` are complete. Nova now loads, checks, and links local
project modules through one interpreter, and cross-module diagnostics and panic
stacks carry canonical module paths. `JS-001` is complete on the `js-interop`
branch (commit `39437d0`): linked multi-module ESM, inline `@script:js` blocks,
recoverable errors, integer policies, eager tasks, bounded channels, UTF-8
string/byte operations, source maps, `.d.ts`, and the shared `conformance/`
suite — now merged into `master`. `TS-001` is complete (shared-emitter TS mode,
`astra build --target ts`, and the pinned `@script:ts` transpiler), and the
conformance suite (19 positive + 11 negative cases) runs green on JavaScript,
TypeScript, and Nova with empty backend gap ledgers. The
`INTEROP-001` inline-script design is complete in
`Docs/Design/V1/10-inline-script-interop.md`; the C/C++ emission slice is
done and waits on the `C-001` toolchain gate for an end-to-end native link.
Open questions remain documented as proposals in `Docs/spec-v0.1.md` §17.

---

## Done

- [x] `AGENT-001` Create the agent operating system under `Docs/Agent/`
- [x] `ARCH-001` Confirm D1–D21 in `docs/architecture.md`
- [x] `ARCH-002` Normalize naming to Neo Astra, `.ns`, `astra`
- [x] `ARCH-003` Define hybrid TypeScript + Rust compiler architecture
- [x] `ARCH-004` Define modes, semantic policies, memory, errors, concurrency, FFI, and package strategy
- [x] `ARCH-005` Define declaration visibility and module import/export system (D23)
- [x] `ARCH-006` Define Nova development runtime and NoPM universal package manager (D25)
- [x] `DESIGN-001` Write the V1 design baseline under `Docs/Design/V1/`
- [x] `DESIGN-002` Check the V1 design against D1–D21 and record open questions
- [x] `SPEC-001`–`SPEC-014` Write and review `Docs/spec-v0.1.md`
- [x] `SCAF-001` Scaffold the TypeScript orchestrator
- [x] `SCAF-002` Scaffold the Rust core
- [x] `SCAF-003` Define the compiler API boundary
- [x] `SCAF-004` Add build and CI scripts
- [x] `TOOL-001` Add cross-platform bootstrap and toolchain verification
- [x] `DOC-001` Write the syntax and operator legend
- [x] `SYNTAX-001` Define semicolon soft-separator policy (D24)
- [x] `FMT-004` Format `core.json` / `core.toml` and include manifests in `astra fmt`
- [x] `TYPE-001` Resolve modules, imports, exports, visibility, and symbol tables
- [x] `TYPE-002` Check types, inference, exhaustiveness, effects, modes, and capabilities
- [x] `TYPE-003` Build project type surfaces and cross-module checking
- [x] `TYPE-004` Add the multi-source compiler API for project checking
- [x] `NOVA-003` Load, check, and link local project modules

---

## Phase 2 — Specification (complete)

### [x] SPEC-001 — Create the spec skeleton

**Acceptance**
- [x] `Docs/spec-v0.1.md` exists
- [x] Table of contents is present
- [x] Sections exist for grammar, types, modes, memory, errors, concurrency, FFI, diagnostics, and conformance
- [x] Every confirmed decision has a home

### [x] SPEC-002 — Define lexical rules

**Acceptance**
- [x] Identifiers, keywords, literals, and whitespace are defined
- [x] Only `//` comments are valid in `.ns`
- [x] Invalid comment forms have diagnostics
- [x] Valid and invalid examples exist

### [x] SPEC-003 — Define the grammar

**Acceptance**
- [x] Modules, functions, types, statements, expressions, patterns, mode blocks, and foreign modules are covered
- [x] Ambiguities are resolved
- [x] Grammar is regular and formatter-friendly
- [x] At least one parse example per construct exists

### [x] SPEC-004 — Define the type system

**Acceptance**
- [x] Primitive and composite types are defined
- [x] `Option`, `Result`, generics, and local inference are defined
- [x] Strict API boundaries are specified
- [x] Exhaustive matching rules are specified
- [x] No implicit coercions are allowed

### [x] SPEC-005 — Define the semantics matrix

**Acceptance**
- [x] Integers, strings, arrays, maps, errors, memory, concurrency, and FFI are covered
- [x] Target differences are explicit
- [x] `math.integers` and `strings.model` behavior is defined
- [x] Conformance cases are listed

### [x] SPEC-006 — Define mode and capability rules

**Acceptance**
- [x] `core`, `app`, `web`, and `system` are defined
- [x] Selection pipeline is specified
- [x] Source tags and scoped blocks are specified
- [x] Monotonic capability rule is specified
- [x] Viral requirements are specified
- [x] `astra explain` output is specified

### [x] SPEC-007 — Define memory policy

**Acceptance**
- [x] GC default is specified
- [x] Manual memory is limited to native/Wasm
- [x] Block/file/manifest precedence is specified
- [x] JS rejection behavior is specified
- [x] Allocation and failure behavior is defined

### [x] SPEC-008 — Define the error model

**Acceptance**
- [x] `!` error effects are specified
- [x] Error unions and propagation are specified
- [x] Panic/abort behavior is specified
- [x] Native and JS target representations are specified
- [x] FFI error mapping is specified

### [x] SPEC-009 — Define structured concurrency

**Acceptance**
- [x] `spawn` scoping is specified
- [x] Channel semantics are specified
- [x] Cancellation and backpressure are specified
- [x] Native thread-pool mapping is specified
- [x] Web Worker mapping is specified
- [x] No shared mutable state is allowed in the portable core

### [x] SPEC-010 — Define FFI and packages

**Acceptance**
- [x] `foreign module` syntax is specified
- [x] Manifest target mappings are specified
- [x] C ABI and ESM type mappings are specified
- [x] npm/Cargo package-source adapters are specified
- [x] Lockfile behavior is specified
- [x] Package security rules are specified

### [x] SPEC-011 — Define diagnostics and conformance

**Acceptance**
- [x] Diagnostic codes and JSON shape are specified
- [x] Source spans and related diagnostics are specified
- [x] `astra explain` shape is specified
- [x] Portable conformance cases are listed
- [x] Target-specific conformance cases are listed

### [x] SPEC-012 — Review the spec against D1–D21

**Acceptance**
- [x] Every decision maps to a spec section
- [x] No contradictions remain
- [x] Open questions are listed as proposals, not hidden assumptions
- [x] Next phase is unblocked

### [x] SPEC-013 — Add keyword aliases (D22)

**Acceptance**
- [x] `fn`/`function`, `let`/`var`, `const`/`constant`, `type`/`typedef`, `struct`/`record`, `mut`/`mutable`, and `pub`/`public` are accepted
- [x] Aliases are exact lexical synonyms
- [x] Canonical formatter normalizes aliases to the short form
- [x] Grammar, examples, and decision traceability are updated

### [x] SPEC-014 — Add declaration visibility and import/export system (D23)

**Acceptance**
- [x] Normal, public/exported, and private declarations are defined
- [x] `export { ... }` can appear at item position and promote normal declarations
- [x] `export { ... }` rejects private declarations
- [x] Project-path `import path` / `import path as alias` and JS/TS-style imports are accepted
- [x] Named, default, namespace, side-effect, re-export, and `export default` forms are accepted
- [x] Type-only exports/imports are erased and default exports must be values
- [x] Private declarations cannot be exported, imported, or re-exported
- [x] Private declarations cannot leak through public signatures, aliases, or fields
- [x] Grammar, diagnostics, examples, and decision traceability are updated

---

## Phase 3 — Scaffolding (complete)

All scaffolding tasks are complete. The lexer, parser, and formatter are implemented, including comment preservation; `astra fmt` is wired to the Rust core.

### [x] SCAF-001 — Scaffold the TypeScript orchestrator

- [x] CLI package
- [x] Manifest parser package
- [x] LSP package stub
- [x] Plugin API stub
- [x] JS/TS codegen package stubs
- [x] Workspace build and manifest/CLI smoke tests pass

### [x] SCAF-002 — Scaffold the Rust core

- [x] `astra-core`
- [x] `astra-capabilities`
- [x] `astra-compiler-api`
- [x] C, direct Wasm, and LLVM backend crate stubs
- [x] Workspace build, tests, formatting, and clippy pass

### [x] SCAF-003 — Define the compiler API boundary

- [x] Versioned JSON-RPC 2.0 protocol
- [x] Native `astra-compiler` binary entrypoint
- [x] Raw Wasm entrypoint with linear-memory request/response
- [x] Native and Wasm smoke tests

### [x] SCAF-004 — Add build and CI scripts

- [x] Formatting
- [x] Linting
- [x] TypeScript and Rust tests
- [x] Native/Wasm boundary smoke test
- [x] Cross-platform GitHub Actions workflow

---

## Phase 4 — Lexer, parser, and formatter (complete)

### [x] FMT-004 — Format manifests

- [x] Canonical JSON key order, two-space indentation, and trailing newline
- [x] Canonical TOML layout with `#` comments preserved exactly once
- [x] `astra fmt` includes `core.json` / `core.toml` for directories and direct file targets
- [x] `astra fmt --check` and `--json` include manifest results
- [x] Invalid manifests prevent all writes
- [x] Unit and CLI tests

### [x] LEX-001 — Lexer and spans

- [x] UTF-8 byte spans and leading BOM handling
- [x] `//` comments; invalid comment diagnostics
- [x] Unicode identifiers and canonical keyword aliases
- [x] Integer, float, string, and byte-string literals
- [x] Escape validation and unterminated-literal diagnostics
- [x] Longest-match operators and punctuation
- [x] Newline tokens and lexical recovery
- [x] Unit tests for tokens, spans, and `NS1001`–`NS1005`

### [x] PAR-001 — Parser and AST

- [x] Recursive-descent parser over the lexer
- [x] Module/import/export/item grammar
- [x] Expressions, statements, patterns, and types
- [x] Newline boundaries and optional same-line `;` separators
- [x] Structured parser diagnostics and recovery
- [x] AST integration tests

### [x] FMT-001 — Canonical AST formatter

- [x] Deterministic, idempotent output
- [x] Canonical aliases, visibility, spacing, and indentation
- [x] Same-line `;` groups and required scope-header semicolons
- [x] Composite types, imports, exports, and nested blocks
- [x] JSON-RPC `format` method
- [x] Formatter tests

### [x] FMT-002 — Preserve comments in formatted output

- [x] Collect `//` comments as lexer trivia
- [x] Reinsert comments deterministically
- [x] Preserve leading, trailing, standalone, empty-body, and EOF comments
- [x] Add round-trip and idempotence tests with comments

### [x] FMT-003 — Wire `astra fmt` to the Rust core

- [x] TypeScript compiler client for JSON-RPC
- [x] `astra fmt` file/directory handling
- [x] `--check` support
- [x] `--json` report and explicit `--compiler` override
- [x] Recursive `.ns` discovery with build-directory exclusions
- [x] No partial writes when any source has an error

---

## Phase 5 — Types, Nova, and NoPM

`TYPE-001`, `TYPE-002`, and `NOVA-001` are complete. `NOVA-002` follows. Nova and
NoPM are designed in `Docs/Design/V1/08-nova-and-nopm.md` and confirmed by D25.

### [x] TYPE-001 — Resolver

- [x] Resolve modules, imports, exports, and visibility
- [x] Build module export surfaces and symbol tables
- [x] Enforce private-type non-leakage in public APIs
- [x] Report structured resolution diagnostics

### [x] TYPE-002 - Type checker

- [x] Primitive and composite types
- [x] Local inference and strict API boundaries
- [x] Exhaustive matching
- [x] Effect-tracked `!` checking
- [x] Mode and capability checks

**Evidence**

- `crates/astra-core/src/checker.rs` checks resolved HIR and records expression
  annotations for Nova.
- Compiler API `check` and `compile` now run parse → resolve → type check and
  return `NS4xxx`/`NS5xxx` diagnostics before producing a program.
- 109 `astra-core` tests cover composites, generics, aliases, structs, enums,
  tuples, arrays, maps, indexing, inference, argument/return checks, mutability,
  return paths, recursive exhaustive matching, effects, failures, mode
  requirements, capability tags, scoped modes, foreign-module requirements, and
  manual memory.
- `npm run ci` passes with 9/9 TypeScript tests, 13/13 boundary/CLI tests, 109
  `astra-core` tests, and 120 Rust tests total; formatting and clippy pass.

### [x] TYPE-003 — Project type surfaces and cross-module checking

- [x] `ModuleTypeSurface` extraction from a checked module (values and type defs)
- [x] `check_module_graph` checks modules in dependency order and feeds surfaces forward
- [x] Checker accepts imported surfaces for cross-module calls and type definitions
- [x] Wire `CheckOptions::module_surfaces` into `check_resolved` via `register_imports`
- [x] 13 cross-module type-surface tests pass

**Evidence**

- `crates/astra-core/src/type_surface.rs` implements `ModuleTypeSurface`,
  `module_type_surface`, `check_module_graph`, `ProjectModule`, `GraphCheckResult`,
  `lookup_import_type`, `lookup_import_type_def`, and `surfaces_for_imports`.
- `crates/astra-core/src/checker.rs`: `CheckOptions` now carries `module_surfaces:
  BTreeMap<String, ModuleTypeSurface>`; `register_imports` resolves real
  `Type::Function` types for imported values, inserts `StructDef`/`EnumDef`
  entries for imported type definitions, and populates namespace members from
  surfaces rather than leaving them as open/opaque namespaces.
- 13 `type_surface` tests: surface extraction (value types, struct defs, enum
  defs, unexported omission), cross-module correct call, type mismatch caught,
  wrong argument count caught, opaque imports permissive, graph two-module pass,
  graph type error caught, graph resolution failure graceful, graph mode option.
- `npm run ci` passes: 40 TS, 13 boundary, 123 `astra-core` (up from 110),
  11 compiler API, 26 nova lib, 50 nova CLI, 210 Rust tests total; formatting
  and clippy clean.

**Remaining scope**

- The compiler API (`astra-compiler-api`) still calls `check_resolved` for a
  single module. Extending the boundary to carry a module graph is `TYPE-004`.
- Nova's `build_interpreter` is still single-module; multi-module execution
  belongs to `NOVA-003`.

### [x] TYPE-004 — Multi-source compiler API for project checking

**Why**

`CompileRequest` carries exactly one `SourceUnit` and the API calls
`check_resolved` directly. `check_module_graph` from `TYPE-003` is therefore
unreachable from the orchestrator, the `astra` CLI, and Nova. Imported symbols
are checked as `Type::Unknown`, so cross-module type errors pass silently and
multi-module programs cannot be checked or executed through the API.

**Acceptance**

- [x] Add an additive, versioned way to send multiple source units without
      breaking the existing single-source `check`/`compile` methods
- [x] Preserve existing request/response shapes and keep `apiVersion` compatible
- [x] Return diagnostics attributed to their module
- [x] Record resolved module order in response metadata for reproducible builds
- [x] Add API tests for a valid cross-module program and a cross-module type error
- [x] Expose the project check to `@neo-astra/compiler-client`
- [x] Keep `npm run ci` green; no module may exist in `src/` without being
      declared and compiled

**Evidence**

- `astra-compiler-api` adds `ProjectCheckRequest`, `ProjectSourceUnit`,
  `ModuleCheckResult`, `ProjectCheckResponse`, and `check_project`, exposed as
  the additive JSON-RPC method `check_project` with a per-module `modules`
  array, `program`, and `metadata.moduleOrder`.
- `check_project` parses every source, calls `check_module_graph`, returns
  diagnostics against their module key/path, and preserves request order in the
  response.
- `@neo-astra/compiler-client` exposes `checkProject()` and validates the
  multi-source response shape and API version.
- 7 new compiler API unit tests plus 3 boundary integration tests cover a valid
  two-module project, cross-module `NS4001`/`NS4004` failures, module ordering,
  empty sources, and per-module parse errors.
- `npm run ci` passes: 40 TypeScript tests, 16 boundary tests, 123
  `astra-core` tests, 18 compiler API tests, 26 Nova library tests, 50 Nova CLI
  tests, and 217 Rust tests total.

**Depends on**

- `TYPE-003` (done), and it unblocks `NOVA-003`.

### [x] NOVA-001 — Typed-HIR interpreter

- [x] Values, environments, and call frames
- [x] Functions, structs, enums, arrays, maps, and pattern matching
- [x] Error propagation and panic/abort
- [x] Portable managed subset; reject unsupported manual memory clearly
- [x] Deterministic single-threaded task stub

**Evidence**

- `crates/astra-nova` consumes checked HIR from `astra-core`; it never re-parses
  `.ns` source.
- The interpreter covers primitives, composites, structs, enums, `Option`,
  `Result`, tuples, arrays, insertion-ordered maps, functions, recursion,
  constants, `defer`, loops, pattern matching, `!` propagation, panics with call
  stacks, aborts, and a deterministic eager task stub.
- `@memory:manual` scopes are recorded by the checker and rejected in preflight,
  even when the source otherwise passes system-mode checking.
- Runtime failures use stable `NV0001`-`NV0007` codes; frontend failures preserve
  the original `NSxxxx` diagnostics.
- 26 `astra-nova` tests cover the managed subset, runtime failures, task scoping,
  imported-value rejection, integer policies, host output, and diagnostic forwarding.
- `cargo test --workspace`, `cargo fmt --all --check`, and
  `cargo clippy --workspace --all-targets -- -D warnings` pass with 147 Rust tests
  (11 compiler API, 110 `astra-core`, and 26 `astra-nova`).
- Scope boundary: `NOVA-001` executes one checked module. Project-wide module
  linking, imported runtime values, FFI, channels, and OS threads remain later work.

### [x] NOVA-002 — Nova CLI

- [x] `nova run` and `nova eval` foundation
- [x] `nova version`, help, `--mode`, and JSON reports
- [x] Shared diagnostics with `astra check` (`NSxxxx` preservation and `NVxxxx` runtime codes)
- [x] `nova repl`
- [x] `nova watch` and `nova run --watch`
- [x] `nova test`
- [x] Manifest `[nova].entry` discovery when no file is given

**Evidence**

- `crates/astra-nova/src/bin/nova/` is a thin adapter over the interpreter.
- 50 integration tests cover help/version, run/eval, bare expression evaluation,
  modes, frontend diagnostics, panics, manual memory, JSON capture, usage errors,
  REPL state and recovery, watch-once behavior, test discovery/results, and JSON
  and TOML manifest entry discovery.
- `npm run ci` passes with 33/33 TypeScript tests, 13/13 boundary tests, 110
  `astra-core` tests, 11 compiler API tests, 26 Nova library tests, 50 Nova CLI
  tests, and 197 Rust tests total.
- Remaining work: project-wide linking, test filters, and parallel test execution.

### [x] NOVA-003 - Project module loading and runtime linking

**Why**

`NOVA-001` executed one checked module and rejected imported runtime values, so
`nova run` could not execute a real multi-file project even though the compiler
API could check one. Project execution needs canonical module identity,
dependency-ordered checking, and one linked interpreter with shared host, task,
constant, and panic state.

**Acceptance**

- [x] Canonical `ModuleId` values derived from project-relative paths, never raw
      import specifiers (`Docs/Design/V1/09-nova-project-linking.md`)
- [x] Filesystem and in-memory loaders discover local modules from imports and
      re-exports, parse each file once, and check in dependency order
- [x] Relative `.ns` paths and dotted project paths resolve through an explicit
      `(importer, raw specifier) -> target` map; external/`std:` modules fail
      with `NV0003`
- [x] Root containment, missing modules (`NS3012`), cycles (`NS3013`), invalid
      UTF-8 (`NS9007`), unreadable paths (`NS9008`), and case-insensitive id
      collisions are rejected deterministically
- [x] One module-aware `Interpreter` owns per-module tables and switches context
      for imported functions, constants, structs, enums, namespaces, tasks, and
      defers
- [x] Imported named/tuple structs and enums, namespace imports, named/star
      re-exports, default function exports, and transitive imports link
- [x] Project diagnostics sort and deduplicate by module/path/span; runtime
      faults identify the defining module; panic stacks carry module-aware frames
- [x] Manual-memory scopes are rejected for every module before execution
- [x] `nova run` and watch use the project loader; `nova eval` remains a single
      in-memory module
- [x] Type surfaces propagate re-exported values and nested namespace members and
      type imported tuple-struct constructors and enum members

**Evidence**

- `crates/astra-core/src/resolver.rs` adds `ModuleGraph`, canonical import
  binding, and specifier collection; graph tests cover importer-local specifiers,
  re-exports, and required mappings.
- `crates/astra-nova/src/project.rs` implements canonical ids, deterministic
  discovery/checking, project errors, runtime linking, and module-path fault
  attribution.
- `crates/astra-nova/src/interpreter.rs` owns one `RuntimeModule` per module,
  carries `FunctionValue`/`Frame` module identity, and tracks the faulting module.
- `crates/astra-nova/tests/project_loader.rs` covers filesystem loading, nested
  projects, missing entries, invalid UTF-8, case mismatches, and root escapes;
  `nova_cli.rs` covers multi-file `nova run`, cycles, and imported type errors.
- Project tests cover functions/constants, transitive/star/named/namespace
  re-exports, defaults, tuple structs/enums, dotted paths, side-effect imports,
  cross-module constants/panics, manual-memory preflight, absolute paths, and
  diagnostic ordering.
- `cargo test --workspace`, `cargo fmt --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, and `npm run ci`
  pass (456 Rust tests, 40 TypeScript tests, and 18 native/Wasm boundary tests).
- Scope boundary: anonymous default expressions still fail with `NV0003`, module
  cycles are rejected, and external package/`std:` resolution waits for backend
  adapters.

**Depends on**

- `TYPE-003`/`TYPE-004` and `NOVA-001`/`NOVA-002` (done). It unblocks `JS-001`.

### [x] NOPM-001 - Universal package model and resolver

- [x] Universal package IDs, descriptors, and deterministic in-memory graph
- [x] Target/mode-aware resolution
- [x] `core.lock.json` model, validation, canonical formatting, I/O, and staleness checks
- [x] Content-addressed store and verified extraction
- [x] `nopm install --locked` and `--frozen`
- [x] `nopm lock` and `nopm tree`

**Evidence**

- `orchestrator/packages/nopm` implements IDs, descriptors, target/mode filtering,
  deterministic graphs, lockfile I/O, store installation, and the path adapter.
- 27 NoPM tests and 4 registry tests cover package IDs, graph resolution,
  targets/modes, lockfiles, staleness, adapters, CLI lock/install, store
  extraction, signing, gateway publish/search/yank/audit, and federation.
- `npm run ci` passes with 40/40 TypeScript tests and 197 Rust tests total.
- Provisional diagnostics are `NP0001`–`NP0012`.
- Lockfile checks cover canonical IDs, sorted packages/dependencies, remote
  integrity, pinned Git commits, and stale manifests.

### [x] NOPM-002 — Path, Git, npm, and Cargo adapters

- [x] Path packages
- [x] Git packages with pinned refs
- [x] npm metadata and tarball resolution with lifecycle scripts disabled
- [x] Cargo metadata, ranges, exact versions, normal dependencies, and build metadata
- [x] Integrity checks and clear target-incompatibility diagnostics
- [x] `nopm install` for path, Git, npm, Cargo, and Nova tarballs

### [x] NOPM-003 — Universal registry gateway

- [x] Registry client for Nova package descriptors
- [x] In-process development gateway server
- [x] Publish, versions, search, yank, and audit endpoints
- [x] Ed25519 package signing and verification
- [x] Upstream mirroring and federation fallback
- [x] Local development works without the registry

### [x] NOPM-004 — Later ecosystem adapters

- [x] PyPI
- [x] NuGet
- [x] OCI

---

## MODE-001 — Capability and mode inference

- [x] Define a deterministic mode-inference result with the inferred requirement,
  explicit requirement, required capabilities, reasons, source spans, and
  diagnostics.
- [x] Infer direct requirements from module/item `@requires` and `@tags`,
  foreign modules, manual-memory scopes, and capability-bearing built-ins.
- [x] Build the local static call graph and propagate requirements to a fixed
  point, including recursion and mutually recursive functions.
- [x] Validate scoped `@mode:` blocks: narrowing is allowed, widening beyond an
  explicit enclosing requirement is rejected, and an inferred enclosing scope is
  upgraded when no explicit requirement exists.
- [x] Join requirements from multiple modules and report `NS5006` when no single
  built-in mode satisfies the project (notably `web` plus `system`).
- [x] Integrate inference into the single-source and project compiler API. An
  omitted `mode` is inferred; an explicit `mode` remains authoritative and is
  still validated by the checker.
- [x] Integrate inference into Nova so `nova run/eval/repl/watch/test` infer a
  least-privileged mode unless `--mode` is supplied. Report the resolved mode in
  JSON output.
- [x] Expose a stable explanation tree (`ModeInference::explain`) in compiler
  response metadata for `astra explain` and tooling.
- [x] Add tests for direct tags, built-ins, transitive calls, recursion,
  narrowing, explicit widening rejection, explicit override, project joins,
  `web`+`system` conflicts, and Nova JSON reporting.
- [x] Update the specification, architecture, runbook, glossary, README, todo,
  and state log.

## ERR-001 — Effect-tracked error checking

- [x] Attach a source span to every recoverable error effect instead of tracking
  only the error type.
- [x] Merge effects deterministically by nominal type, preserving the first
  construction/propagation site and eliminating duplicates.
- [x] Validate propagation against the enclosing function's declared error set
  at the propagation site, not at the whole-function span.
- [x] Reject failure construction for non-nominal, unknown, or non-constructible
  error types, and report malformed error fields at the failing field.
- [x] Require every unhandled effect to be reported at each effect site with
  `NS4011`, including calls inside arguments, collections, conditions, defers,
  loops, match scrutinees, and spawned blocks.
- [x] Treat a `match` over a fallible expression as handling the effect only when
  its patterns exhaust the resulting `Ok`/`Err` shape; otherwise report
  `NS4010` without also emitting `NS4011` for the same scrutinee.
- [x] Preserve imported function error sets through project type surfaces and
  verify cross-module propagation and declared-error compatibility.
- [x] Add an explicit effect-compatibility helper so declared-error membership is
  one rule shared by propagation and failure construction; match handling is
  structural through the exhaustive `Ok`/`Err` shape.
- [x] Add tests for precise spans, duplicate and multiple effects, nested
  expressions, match handling, malformed failures, cross-module effects, and
  generic function-value calls.
- [x] Update the specification, architecture, runbook, glossary, README, todo,
  and state log.

---

## JS-001 - JavaScript ESM backend

**Status:** Complete. The JavaScript backend emits linked multi-module
projects, inline `@script:js` ESM modules, recoverable error effects, checked and
arbitrary integer policies, eager spawn/tasks with bounded channels, UTF-8
string/byte operations, line-level source maps, and TypeScript declaration
artifacts. Shared conformance cases live in `conformance/` for `TS-001` to reuse.

**Goal**

Generate readable, framework-free JavaScript ESM from the typed module produced
by the Rust frontend, without re-parsing `.ns` source in TypeScript.

**Acceptance**

- [x] Add typed-module output to the `compile` JSON-RPC method: AST, HIR,
      expression annotations, manual-memory scopes, target, and integer policy.
- [x] Add `@neo-astra/codegen-js` as a real backend consuming the typed module.
- [x] Emit functions, constants, local bindings, `if`, `while`, `for`, `defer`,
      blocks, calls, assignments, and the supported operators.
- [x] Emit primitive and composite values: integers, floats, booleans, strings,
      bytes, arrays, structural maps, tuples, structs, enums, `Option`, and
      `Result`.
- [x] Emit `match`, including struct/tuple, enum, `Option`, and `Result` patterns.
- [x] Emit built-ins (`print`, `println`, `dbg`, `assert`) and checked collection,
      string, option, and result methods.
- [x] Lower integer arithmetic to wrapping or checked helpers according to the
      manifest policy, including `i64`/`u64` through `BigInt`.
- [x] Tree-shake backend helpers: only helpers used by the module are emitted.
- [x] Emit named, default, and `pub` exports; erase type-only exports.
- [x] Emit `@script:js` blocks as separate ESM artifacts, import their namespace,
      validate declared exports, convert `Option` boundaries, map declared error
      effects, and write script source maps.
- [x] Preserve source-name recognizability, escape JavaScript keywords, and use
      deterministic helper/temporary names.
- [x] Implement `astra build` target selection (`--target`, `ASTRA_TARGET`, then
      `build.defaultTarget`), mode/policy selection, `outDir`, and output writing.
- [x] Emit an out-directory `package.json` with `"type": "module"` when absent so
      generated `.js` files run directly under Node ESM.
- [x] Report compiler and codegen diagnostics with source locations.
- [x] Add execution tests for structs, enums, options, maps, tuples, defer,
      wrapping/checked integers, and `astra build`.
- [x] Link and emit project imports, namespace imports, default imports, named
      imports, named re-exports, namespace re-exports, and star re-exports.
- [x] Implement recoverable error effects, `!Error(...)` failure construction, and
      `?` propagation.
- [x] Implement checked integer methods (`checked_add`, `checked_sub`,
      `checked_mul`, `checked_div`).
- [x] Implement `spawn` and eager `Task` join/cancel, including unjoined task
      panics at function-scope exit.
- [x] Implement channels and a structured-concurrency scheduler.
- [x] Emit line-level source maps with `sourcesContent` and a
      `sourceMappingURL` comment.
- [x] Emit `.d.ts` artifacts for modules and inline script modules.
- [x] Implement the `arbitrary` integer policy with `BigInt` arithmetic,
      arbitrary `usize`/`isize`, length values, and explicit wrapping methods.
- [x] Implement full UTF-8 byte-slice and byte-index string operations.
- [x] Add conformance cases shared with `TS-001`.
- [x] Emit `@script:js` blocks as separate ESM artifacts, import their namespace,
      validate declared exports, convert `Option` boundaries, map declared error
      effects, and write script source maps.
- [x] Emit statement-position `if` and `match` as real JavaScript statements so
      `break`, `continue`, and early `return` work inside them (pinned by the
      `early_exit` conformance case).

**Current scope boundary**

- `astra build` discovers local modules from relative imports, checks the
  project graph, and emits one ESM module per source with rewritten relative
  specifiers.
- External/non-project imports remain compile-time errors; V1 does not yet link
  package-provided modules.
- Inline `@script:js` blocks are project source. `@script:ts` is rejected by the
  JavaScript backend until the TypeScript target or a pinned transpiler exists;
  C/C++ scripts require the native backend.
- `Option` is a tagged object, not JavaScript `null`; `Result` is a tagged object.
- Maps use a small structural-key helper rather than JavaScript `Map`, preserving
  Neo Astra structural key equality and insertion order.
- `usize`/`isize` use the safe JavaScript integer range; fixed-width integer
  semantics are enforced for `i8`-`i64` and `u8`-`u64`.
- Floats are tagged numeric values so `f64`/`f32` rendering keeps their fractional
  identity; `f32` operations use `Math.fround`.
- Render helpers preserve integer, float, string, tuple, struct, enum, map,
  `Option`, and `Result` shapes; `f64`/`f32` values use tagged numbers so integral
  floats render with a trailing `.0`.
- Recoverable failures use a tagged `{ $failure: true, error }` value. A
  fallible function returns `Ok` for normal completion and a failure sentinel
  for `!Error(...)` or a propagated `?`; a `match` subject normalizes a failure
  sentinel to `Err` so the existing pattern lowering applies.
- Checked integer methods return `Result` and produce `OverflowError` on
  overflow; division by zero remains a panic, matching Nova.
- `spawn || { ... }` runs eagerly and returns a task value. `join` retrieves the
  outcome or re-raises a task panic; `cancel` marks the task cancelled. A
  function containing spawn owns a task scope, and an unjoined task panic is
  raised when that function exits.
- `Channel.create<T>()` returns a bounded channel (capacity 16). `send` and
  `receive` are recoverable (`ChannelError` when the channel is closed);
  `close` is infallible. Because spawn stays eager and the V1 runtime is
  single-threaded, a send to a full channel or a receive from an empty open
  channel raises a structured deadlock panic instead of blocking. Channels are
  plain runtime records, and `.d.ts` artifacts describe them as
  `AstraChannel<T>`.
- String operations follow spec §6: `len` stays the UTF-8 byte length, and
  `char_count`, `bytes`, `chars`, `byte_at`, `char_at`, and `slice` operate on
  UTF-8 bytes and scalar values. `slice` returns `Result` and fails with
  `SliceError` when the range is out of bounds or would split a scalar value.
  `bytes.new(n)` allocates a bounds-checked `u8` buffer; indexed writes
  require `let mut` and go through a checked `$indexSet` helper.
- `astra build` writes a line-level `.js.map` with `sourcesContent`, rewrites the
  source path for the output directory, and appends a `sourceMappingURL`
  comment.
- Statement-position `if` and `match` lower to plain JavaScript statements, so
  `break`, `continue`, and early `return` bind to the enclosing loop or
  function. In a value-position expression (`let x = match ... { _ => { return
  0 } }`), the IIFE lowering still rejects that control flow with `NS8001`;
  closing it requires expression-statement hoisting.

**Evidence**

- `crates/astra-compiler-api/src/lib.rs` exposes `ModuleIr` through `compile` and
  adds serde derives to the typed AST/HIR/type surface.
- `orchestrator/packages/codegen-js/src/ir.ts` describes the serialized typed
  module; `generate.ts` implements ESM emission and diagnostics.
- `orchestrator/packages/cli/src/index.ts` implements `astra build`, including
  local module discovery, project checking, per-module source maps, and ESM
  output rewriting.
- `orchestrator/tests/codegen-js.test.mjs` compiles real `.ns` sources, executes
  generated ESM, covers integer policies, recoverable failures, `?` propagation,
  checked integer methods, eager spawn/join, task panic scope failure, UTF-8
  string/byte operations, bounded channels (closed-channel errors and the
  single-threaded deadlock panic), bounds-checked bytes buffers, `AstraChannel`
  declarations, source maps, linked multi-module projects with re-exports, inline
  `@script:js` modules with boundary conversions and error mapping, missing
  script exports, `.d.ts` artifacts, rejects unsupported constructs, runs the
  shared conformance suite, and builds a project end to end.
- `conformance/cases/*.ns` with `.expected` output plus
  `orchestrator/tests/conformance.mjs` provide the backend-neutral suite that
  `TS-001` reuses; `conformance/README.md` documents the format.
- `crates/astra-core/src/checker.rs` adds the `Channel` and `bytes` builtin
  namespaces, the `ChannelError`/`SliceError` error sets, spec §6 string/bytes
  member signatures, and lvalue type annotations that back indexed writes.
- Verification (merged tree): 548 Rust tests, 41 TypeScript unit tests, 46 native/Wasm/boundary
  tests (including 28 codegen/build tests), formatting, clippy with warnings
  denied, and `npm run ci` pass.

**Depends on**

- `TYPE-004` and `NOVA-003` (done).

---

## TS-001 - TypeScript ESM backend

**Status:** Complete. Slices: shared-emitter TS mode + thin `codegen-ts` wrapper
(`83ec555`), CLI `--target ts` wiring + TS test suite (`6f0e80b`), and
`@script:ts` pinned-transpiler emission.

**Why**

The JavaScript backend proves the typed-AST/HIR pipeline, but TypeScript is a
release-blocking target. It must emit strict TypeScript that preserves the same
runtime semantics as the JavaScript backend while giving editors, `tsc`, and
downstream TypeScript users real types. The backend must consume the shared
typed module from the Rust compiler; it must not re-parse `.ns` source in
TypeScript.

**Goal**

Add `@neo-astra/codegen-ts` and wire `astra build --target <typescript target>`
so the same `.ns` project can produce strict TypeScript ESM, declarations, and
source maps that compile with ordinary `tsc` and run through Node or a bundler.

**Acceptance**

- [x] Add a `codegen-ts` package that consumes the same serialized typed module
      as `codegen-js` (AST, HIR, annotations, manual-memory scopes, integer
      policy, and project module keys).
- [x] Share JS runtime semantics and tree-shaken helpers with the JavaScript
      backend without forking behavior or duplicating source parsing.
- [x] Emit strict TypeScript ESM for the supported language subset: functions,
      constants, locals, control flow, `defer`, calls, assignments, arrays,
      maps, tuples, structs, enums, `Option`, `Result`, `match`, errors, tasks,
      and the supported built-ins.
- [x] Emit explicit types for parameters, returns, locals, constants, struct
      fields, enum payloads, maps, tuples, and `Option`/`Result` values; do not
      rely on implicit `any` except at an explicitly marked FFI boundary.
- [x] Support linked multi-module projects, imports, default/named/namespace
      imports, re-exports, star re-exports, and rewritten relative specifiers
      with parity to `JS-001`.
- [x] Emit `.ts`, `.ts.map`, and `.d.ts` artifacts; keep `sourcesContent` and
      line-level mappings. Write `package.json` with `"type": "module"` when the
      out directory needs it.
- [x] Support `wrapping`, `safe-checked`, and `arbitrary` integer policies with
      the same observable semantics as the JavaScript backend.
- [x] Support eager `spawn`/`join`/`cancel` and recoverable error effects with
      the same boundary behavior as the JavaScript backend.
- [x] Implement `@script:ts` emission on the TypeScript target with one pinned
      TypeScript compiler version. Missing or mismatched transpilers are hard
      errors; no best-effort fallback. `@script:js` remains valid where the
      target permits it.
- [x] Extend `astra build` target selection to `backend: "ts"`, including
      `--target`, `ASTRA_TARGET`, `build.defaultTarget`, target `outDir`, and
      mode/integer-policy selection.
- [x] Report frontend and backend diagnostics with `.ns` source locations, and
      reject unsupported constructs instead of changing semantics.
- [x] Add end-to-end tests that build a real project, run `tsc --strict
      --noEmit`, execute the emitted JavaScript, and verify linked imports,
      source maps, declarations, error propagation, tasks, and integer policies.
- [x] Add conformance cases shared with `JS-001` so both backends execute the
      same portable-core programs.

**Current boundary**

- `@script:ts` bodies are transpiled with the pinned `typescript@7.0.2` via the
  `tsc` CLI (`--noCheck`: the foreign body is opaque to Nova and may use host
  globals); syntax errors are hard `NS7008` errors. The artifact's source map
  is composed from the transpiler's map by padding the body so its lines are
  `.ns`-absolute, then shifting past the artifact header — no VLQ re-encode.
- The TypeScript target must not emit `any` merely to make checking pass; use
  the typed IR or reject the construct.
- TypeScript-specific packaging (bundler presets, `.d.ts` library layout, and
  declaration rollups) follows the same V1 module and target rules as JS.
- Runtime helpers stay framework-free and target the same portable semantics as
  `JS-001`; Node-only APIs are not allowed in `web` mode.

**Evidence**

- `Docs/Design/V1/04-targets-and-backends.md` defines the TypeScript output
  shape, strictness rules, `.d.ts` requirements, and pinned-transpiler rule for
  `@script:ts`.
- `orchestrator/packages/codegen-js` and the `JS-001` test suite are the semantic
  reference and the starting point for shared generator infrastructure.
- `orchestrator/packages/compiler-client` and `astra build` already expose the
  typed module and target selection needed to add a second codegen package.

**Depends on**

- `JS-001` (typed AST/HIR consumption, module linking, errors, tasks, integer
  policies, source maps, and `.d.ts` output).
- `INTEROP-001` frontend for `@script` AST/HIR and target/language validation.
- `TS-001` unblocks TypeScript-target `@script` emission and completes the first
  JS/TS interop slice.

---

## INTEROP-001 - Inline `@script` foreign-language blocks

**Status:** Frontend (Slices A+B), the JavaScript backend slice (C), the
TypeScript-target slice (pinned-transpiler `@script:ts`), and the C/C++ emission
slice are complete. End-to-end native script linking is unverified until the
`C-001` toolchain gate; deferred runtime work remains.

**Why**

A `.ns` file cannot currently contain a bounded foreign-language implementation
without a separate `foreign module` declaration and a manifest-mapped external
source. The `@script` proposal makes the foreign boundary visible in the file,
keeps the body opaque to Nova parsing, and checks calls against an explicit
export surface.

**Design**

- [x] Define `{@script:<language> as <name>; ... }` as a declaration-level block.
- [x] Define the typed `@exports` surface and required `@mode(...)` declaration.
- [x] Define verbatim capture from `@impl` to an explicit `@end` sentinel.
- [x] Define the language/target matrix: JS/TS first, C/C++ second, Python later,
      Java only with a JVM backend.
- [x] Define `NS7001`–`NS7008`, `NV0008`, manifest `inline-*` mappings, and the
      unsafe-boundary security rules.
- [x] Record the proposal in `Docs/Design/V1/10-inline-script-interop.md` and
      update the specification, architecture, design baseline, runbook, glossary,
      syntax legend, README, and state log.

**Implementation**

- [x] Add lexer/parser raw-source capture, AST/HIR nodes, and `NS7001`–`NS7006`
      diagnostics.
- [x] Preserve `@script` bodies byte-for-byte in `astra fmt` and LSP formatting.
- [x] Register script namespaces and validate `@mode`, `@exports`, boundary types,
      calls, target compatibility, and manifest mappings (`NS7007`).
- [x] Implement the JavaScript backend slice: real ESM modules, export
      validation, source maps, boundary conversion, error mapping, and
      end-to-end tests.
- [x] Extend script emission to the TypeScript target after `TS-001`: `@script:ts`
      bodies are transpiled by the one pinned `typescript@7.0.2` (missing or
      mismatched transpilers are hard `NS7008` errors); the JS backend keeps
      rejecting `@script:ts`.
- [x] Implement the C/C++ emission slice: one translation unit per block,
      freestanding headers, C ABI prototypes in `main.c`, `extern "C"` blocks in
      C++ scripts, and CLI script compilation/linking (`-lstdc++` when C++ is
      used).
- [ ] Verify a native script link end to end. Blocked by the `C-001` native
      toolchain gate on this host (no libc/SDK headers for `astra_host.c`).
- [x] Make Nova reject foreign script calls with `NV0008` before execution.
- [x] Add conformance and security tests for unsupported target/language pairs,
      malformed terminators, opaque bodies, and mode propagation.
- [ ] Defer Python, Wasm script embedding, Java, and Nova host adapters to
      post-V1 design work.

**Depends on**

- `JS-001` for the JavaScript slice (done). `TS-001` for TypeScript-target
  script emission (done: `@script:ts` pinned transpiler).
- C backend emission for the second slice (done: per-block TUs plus CLI
  compilation/linking); native link verification waits on the C-001 toolchain
  gate.
- NoPM is not required; inline scripts are project source, not packages.

---

## C-001 - C17 native backend

**Status:** `[~]` The managed C17 first slice is implemented and tested. The
production native backend is **not complete**; the current C-001 checkbox in the
backlog means "first slice", not the full backend.

> **Handoff:** `Docs/Agent/c001-handoff.md` is the transfer document for this
> task — architecture, invariants, the remaining work with design sketches, and
> the slice workflow. Read it before starting any C-001 sub-task.

**Why**

The C backend is the bootstrap path for native and Wasm targets:

```text
Astra IR -> C17 -> clang -> native binary or Wasm module
```

It must produce correct, explicit C for the portable managed subset, reject
unsupported constructs before emission, and eventually provide the runtime
pieces (GC, bounds checks, concurrency, FFI, and platform abstractions) required
by a production native target.

**Done**

- [x] Consume the shared typed module (AST, HIR, annotations, manual-memory
      scopes, and integer policy) instead of re-parsing `.ns` source.
- [x] Emit a self-contained C17 translation unit plus freestanding runtime
      header and `astra_host.c` host shim.
- [x] Fixed-width integers, wrapping operations, checked helpers, floats, and
      the `wrapping`/`safe-checked`/`arbitrary` policy plumbing.
- [x] Functions, constants, locals, assignment, `if`/`else`, `while`, `for`
      over arrays, calls, returns, blocks, and `defer` with LIFO execution.
- [x] `print`/`println`/`assert`/`panic`, string literals, arrays, named and
      tuple structs, enums with payloads, `Option`, `Result`, single-error `!`
      propagation, and matches over bool/enum/`Option`/`Result`/struct.
- [x] `main` delegation, generated prototypes, deterministic symbol sanitizing,
      and `NS8011`-`NS8025` diagnostics for unsupported constructs.
- [x] Compiler API `c` target returns `c-source` and `astra_host.c` artifacts.
- [x] `astra build --target c` writes `main.c` and `astra_host.c`;
      `--target native` discovers `clang`/`gcc`/`cc` and attempts to link a
      host executable.
- [x] 71 `astra-codegen-c` tests pass, including generated-C checks with the
      available clang toolchain and freestanding object compilation.
- [x] Map literals `{ k: v, ... }` emit `astra_map_new` + `astra_map_set`.
- [x] Map methods `.len()`, `.contains(k)`, `.get(k)`, `.insert(k, v)` emit typed C via
      the `NS_Map` runtime helpers; `.get` and `.insert` return `NS_Option_V`.
- [x] `for` loops iterate over arrays, maps (yielding keys via `astra_map_key_at`),
      and bytes (yielding `u8` via `astra_bytes_get`).
- [x] Byte operations: byte indexing (`b[i]`), slicing (`b.slice`, `s.slice`),
      comparisons (`==`, `!=`, `<`, `<=`, `>`, `>=` via `astra_bytes_eq`,
      `astra_bytes_cmp`, `astra_str_eq`, `astra_str_cmp`), and methods (`.len()`,
      `.is_empty()`, `.slice()`).
- [x] Slice semantics (spec §6.2/§6.4): `str.slice(start, end)` takes byte
      offsets and is fallible — the call site builds `Result<str, SliceError>`
      from `astra_str_slice_try`, which returns the failure payload when the
      range is reversed, out of bounds, or splits a UTF-8 scalar value.
      `bytes.slice(start, end)` is infallible and bounds-checked (reversed or
      out-of-range panics, mirroring Nova). Both runtime helpers take end
      offsets; the earlier lowering passed the `end` argument as a length, so
      non-zero starts returned the wrong range and a bare `NS_Str` where the
      checker expects a `Result`. `SliceError` is a shared, stem-free runtime
      typedef, and `match`/`!` consumers of a fallible call now materialize the
      checker's `Result<base, E>` so Ok/Err arms dispatch and bind correctly
      (and propagation types its temp with the call's own Result, not the
      enclosing function's) instead of falling through a literal match. Body-time
      rejects (previously ignored by the standalone emitter) now fail generation
      instead of emitting code that silently omits behavior.
- [x] Numeric fallible builtins (spec §6.1/§6.2): `str.to_int()` parses a
      strict decimal `i32` via `astra_str_to_int_try` and builds
      `Result<i32, ParseIntError>` with the original text as the payload;
      `checked_add`/`checked_sub`/`checked_mul`/`checked_div` build
      `Result<T, OverflowError>` from `__builtin_*_overflow` (division by zero
      panics, signed `MIN / -1` fails checked and wraps for `wrapping_div`);
      `wrapping_add`/`wrapping_sub`/`wrapping_mul`/`wrapping_div` reuse the
      signed runtime wrap helpers and C's defined unsigned wrap-around.
      `ParseIntError`/`OverflowError` are shared stem-free runtime typedefs,
      and their fallible calls materialize in `match`/`!` like `str.slice`.
- [x] String views and indexing (spec §6.2): `char_count`, `byte_at`
      (`Option<u8>` at a byte index), `char_at` (`Option<str>` at a scalar
      index), `bytes()` (`Array<u8>`), and `chars()` (`Array<str>`) lower to
      UTF-8-aware runtime helpers; `bytes.new(length)` allocates a zero-filled
      buffer via `astra_bytes_new`. With this the C backend implements the
      whole `str`/`bytes` method table JavaScript and Nova expose.
- [x] `bytes` writes (`buf[i] = v`) lower to the bounds-checked
      `astra_bytes_set`; they previously fell through the generic array lvalue
      path and produced invalid C.
- [x] `Option`/`Result` accessors: `is_ok`, `is_err`, `unwrap`, `unwrap_or`,
      and `error` lower to checked reads with panic guards; `unwrap_or` keeps
      its fallback in the branch so it is only evaluated when needed.
- [x] Contextual `None`: the checker annotates bare `None` as
      `Option<Unknown>`, so value positions (`let`, `return`, and match arms)
      pass their expected type down and materialize the concrete Option.
      Prescan skips annotation types that still contain checker placeholders,
      which previously rejected valid programs like `find(false).unwrap_or(0)`.
- [x] Generic functions (`fn f<T>(...)`): monomorphized per call site from the
      argument types (`Type::Generic` resolved through a `TyCtxt` substitution
      table). Each distinct type-argument set emits one specialized copy
      (`ns_<fn>_<types>`, e.g. `ns_identity_i32`) with the substituted signature
      and body; generic-to-generic calls instantiate the callee with the caller's
      concrete substitution. Prototypes and bodies are assembled after discovery
      so declarations precede use. Boundaries: exported generics are not exposed
      in headers, cross-module generic calls are not instantiated in the
      importer, and bodies that dispatch on `T` beyond type mapping remain out of
      scope.
- [x] `contains` added to the type checker's `Type::Map` member resolution.
- [x] Byte literals `b"..."` emit `NS_Bytes` compound literals with hex-encoded
      byte arrays; empty byte literals use the zero-length sentinel.
- [x] `println(bytes_val)` dispatches to `astra_println_bytes`.
- [x] Full expression/pattern coverage: string/byte literals in `match` (`astra_str_eq`,
      `astra_bytes_eq`), wildcard and identifier bindings in match arms.
- [x] FFI and inline scripts (`INTEROP-001` Slice D):
      - Emits per-block translation units (`script_<name>.c`, `script_<name>.cpp`)
        with freestanding headers `<stdint.h>`, `<stdbool.h>`, `<stddef.h>`.
      - Generates C ABI prototypes in `main.c` and `extern "C"` blocks in C++ scripts.
      - Dispatches direct calls `math.add(...)` -> `add(...)`.
      - Rejects non-C/C++ script languages with `NS7002`.
      - Compiler API returns `c-script` artifacts.
      - CLI compiles all script translation units and links with `-lstdc++` when C++ is used.
- [x] Native build options in `astra build`: `cflags` array, `optLevel`/`optimization`,
      and target triple (`--target=<triple>`).
- [x] Multi-module project linking (`C-001` project-linking slice):
      - One translation unit + header per module (`<stem>.c`/`<stem>.h`) with the
        module-qualified symbols (`ns_<stem>_<name>`, `NS_<stem>_<Type>`) so two
        modules declaring the same name cannot collide at link time. Standalone
        single-module output is unchanged (`ns_<name>`, `main.c`).
      - Cross-module calls resolve through HIR import bindings to the
        dependency's qualified symbol, and callers include `<dep stem>.h`.
      - Compiler API `compile_project` method returns per-module artifacts plus a
        shared `astra_host.c` shim; relative import specifiers (`./lib`) resolve
        against the importer key before type-checking (mirrors `check_project`).
      - `CompilerClient.compileProject` and `astra build --target c` wire the
        multi-module path (single-module builds keep the standalone emitter).
      - Cross-module struct/enum types (`NS8020` slice): imported, renamed, and
        namespace-dotted type names resolve to the dependency's qualified
        typedef (`NS_<dep stem>_<Type>`), which stays in the dependency —
        signatures, literals, tuple constructors, field access, enum
        construction, tag constants, and match cases all lower with the
        declaring module's prefix, and public headers include dependency
        headers so exported signatures resolve. Bare names inferred without an
        explicit type import resolve to the declaring dependency (first by
        sorted key; local declarations always win).
      - Honest boundary: default-import calls are rejected with `NS8019`.

**Actually left**

- [ ] Multi-module/project linking remainder: Cargo/npm/native package linking
      remains unimplemented; function-level and type-level cross-module imports
      link today.
- [ ] Manual memory and GC. `@memory:manual` is rejected with `NS8012`; native
      allocation, ownership, roots, and finalization are not implemented
      (`GC-001`).
- [ ] Concurrency. `spawn` and channels are rejected with `NS8011`; no task
      scheduler, channel runtime, cancellation, or thread/worker mapping
      (`CONC-001`).
- [ ] Native library output and stable ABI. Only an executable via `main` is
      produced; static/dynamic libraries, exported headers, and a declared C ABI
      are not implemented.
- [ ] Cross-platform native verification. The first slice has been exercised on
      this Windows host, but hosted native linking currently fails here because
      clang lacks MSVC/Windows SDK headers for `astra_host.c` (`stdio.h` not
      found). `astra build --target c` succeeds; `--target native` needs a
      complete host toolchain. Linux/macOS natively linked builds are unverified.
      Known latent defect to fix first: `astra_host.c` is written raw, but the
      shim uses `NS_Bytes`/`NS_Map` from the runtime header while typedefing only
      `NS_Str`/`NS_Array` (see `Docs/Agent/c001-handoff.md` 6.F).
- [ ] Native CI gate. C tests skip hosted execution when no libc/SDK is
      available; CI must compile and run native binaries on at least one Tier-1
      host before C-001 can be marked fully done.
- [ ] Wasm path through C17 (`WASM-001`) remains deferred.

**Evidence**

- `crates/astra-codegen-c/tests/emit_c.rs`: 71 passing tests covering emission,
  runtime helpers, `Option`/`Result`, errors, matches (string, bytes, enum, bool, option),
  arrays, structs, enums, `defer`, map literals/methods (`.len`/`.contains`/`.get`/`.insert`),
  `for` loops (arrays, maps, bytes), byte literals/indexing (reads and
  bounds-checked writes)/slicing/comparisons,
  `str` slice `Result` lowering (`SliceError`), `str.to_int`/`checked_*`/
  `wrapping_*` lowering, `str` scalar/sequence views (`char_count`, `byte_at`,
  `char_at`, `bytes`, `chars`, `bytes.new`), `Option`/`Result` accessors,
  generic monomorphization (per-instantiation symbols, generic-to-generic
  calls), and `!` propagation of fallible builtin calls, `@script:c` and
  `@script:cpp` emission, compiler API artifacts, rejection paths,
  and multi-module project emission (cross-module call resolution, symbol
  collision avoidance, qualified header prototypes, entry-only `main`, namespace
  imports, cross-module struct/enum/tuple-struct types, the `NS8019` rejection,
  and header dependency includes).
- `crates/astra-codegen-c/tests/conformance.rs`: the static C conformance gate
  lowers every shared case and clang-compiles the result freestanding. 18 of
  19 cases emit cleanly; `channels` (`CONC-001`) sits in an explicit
  `KNOWN_GAPS` ledger that fails when a listed case starts passing. Native
  execution still waits on the toolchain gate.
- `cargo test --workspace` passes (71 in `astra-codegen-c`, 142 in `astra-core`,
  31 in `astra-compiler-api`, 51 in `astra-nova`, etc.).
- `astra build --target c` emits `main.c` (single module) or one qualified
  `<stem>.c`/`<stem>.h` per module plus `astra_host.c` (multi module); boundary
  tests cover both paths.
- `astra build --target native` currently fails on this Windows host at host
  shim compilation because the clang installation has no Windows SDK/libc
  headers. This is a toolchain/CI gap that must be closed before claiming a
  working native binary.

**Depends on**

- `TYPE-001`-`TYPE-004`, `ERR-001`, and the shared typed-module boundary.
- `GC-001` for native manual memory and managed allocation.
- `CONC-001` for tasks and channels.
- `FFI-001` and `INTEROP-001` Slice D for C/C++ foreign blocks.
- `WASM-001` for the C-to-Wasm path.

---

## PLAY-001 — Browser playground

**Status:** Not started. Backlog entry expanded into a runnable plan. No design decision blocks it, but two API methods it needs (`ast`, `explain`) are still stubs.

**Goal**

A static, browser-hosted playground where a visitor edits Neo Astra source plus a minimal manifest, runs `check`, `ast`, `fmt`, and `explain`, generates JavaScript ESM, and executes simple programs — with no backend server for core features.

**Architecture**

This follows the existing split in `Docs/architecture.md` §7.5; it does not introduce a second compiler.

- The Rust core is compiled to `wasm32-unknown-unknown` and keeps its linear-memory ABI: `astra_protocol_version`, `astra_alloc`, `astra_compile`, `astra_free`.
- The host is a TypeScript package, `orchestrator/packages/playground`, that loads the `.wasm`, writes JSON-RPC requests, and reads packed responses.
- JavaScript emission stays in TypeScript: the browser bundles `@neo-astra/codegen-js` and feeds it the typed-module JSON returned by the core. The Rust core does not grow a JS backend.
- Execution runs generated ESM inside an origin-isolated sandboxed iframe (`sandbox="allow-scripts"`), never in the host page.
- Local compilation is the default. A hosted compile API is out of scope for V1 unless explicitly enabled, and must be disclosed as a separate trust boundary (`Docs/Design/V1/07-quality-security-and-delivery.md` §5.8).

**Prerequisite gaps found while expanding**

- `astra-compiler-api` advertises `ast` and `explain` but returns an error for both; `astra explain` is also a CLI stub. Slice C must either implement them or show the typed-module/metadata view instead, and the task cannot be marked done while the UI promises a method that fails.
- `check_project` and multi-source `compile` exist, so multi-module samples are possible, but in-browser module resolution and ESM URL mapping still need to be built.

**Scope slices**

- **A — Core bridge:** wasm loader, protocol-version check, alloc/free, request/response framing, timeout, trap/panic mapping.
- **B — Editor surface:** source editor with `.ns` highlighting, manifest editor (`core.json` / `core.toml`), target and mode selectors, sample gallery, shareable URL state.
- **C — Compiler actions:** `check`, `ast`, `fmt`, `explain`, and JS emission, each with a structured output pane.
- **D — Sandbox execution:** run generated ESM, capture `print`/`println`/`dbg`, enforce timeout and output caps, terminate on demand, deny network by default.
- **E — Hosting:** static build, deploy config, offline-friendly assets, size budget, README and privacy note.

**Acceptance**

Core bridge
- [ ] `astra-wasm` builds for `wasm32-unknown-unknown` in CI; the artifact is versioned with the host bundle
- [ ] The host checks `astra_protocol_version` and refuses mismatched artifacts with a clear error
- [ ] Requests and responses are UTF-8 JSON matching the existing JSON-RPC schema
- [ ] A wasm trap or panic surfaces as an internal error, never as a user syntax error
- [ ] Every allocation is freed; a soak test of 1,000 compiles shows stable memory

Editor
- [ ] Edit `.ns` source and a minimal manifest; switch between JSON and TOML manifests
- [ ] Target and mode are selectable and sent in the request
- [ ] Diagnostics render severity, stable code, file, line/column, message, and hint; selecting one moves the cursor
- [ ] Samples load without network access after first visit

Compiler actions
- [ ] `check` shows `NS4xxx`/`NS5xxx` diagnostics
- [ ] `ast` shows the parsed tree, or the task records the API dependency and shows the typed module instead
- [ ] `fmt` rewrites the editor through the same formatter as `astra fmt`
- [ ] `explain` shows mode and capability inference, or is explicitly deferred with the stub noted in the UI
- [ ] JS emission uses `@neo-astra/codegen-js` and shows generated ESM with copy and download

Execution
- [ ] Generated ESM runs in a sandbox with no access to host page globals, storage, or credentials
- [ ] `print`/`println`/`dbg` output is captured and attributed to the sandbox
- [ ] Infinite loops terminate at a configurable timeout; output is capped
- [ ] Runtime failures show `NVxxxx` codes and a call stack when available
- [ ] The sandbox cannot navigate the host page or open popups

Security and privacy
- [ ] Compilation is local by default and the UI states this
- [ ] No source leaves the browser unless the user explicitly opts into a remote compiler
- [ ] A remote compiler, if enabled, discloses upload, retention, and trust boundary before the first request
- [ ] The sandbox runs under a restrictive CSP with network access denied by default
- [ ] No analytics or third-party script receives source text

Quality
- [ ] Tests cover the wasm loader, codegen bridge, diagnostic mapping, URL state, and sandbox timeout
- [ ] One end-to-end test compiles and runs a sample in a headless browser
- [ ] `npm run ci` includes the playground build and tests
- [ ] Initial payload (wasm + JS, gzipped) targets ≤ 5 MB; the measured number is recorded in the README
- [ ] Works in current Chrome, Firefox, Safari, and Edge

**Non-goals**

- No second compiler, parser, or JS backend in Rust.
- No package installation or NoPM resolution in the browser for V1.
- No LSP in the playground; that belongs to editor integration.
- No native or Wasm execution target in V1; execution is generated JS only.
- No accounts, persistence, or server-side project storage.

**Risks**

- Wasm bundle size may exceed budget; mitigate with `wasm-opt`, `opt-level = "z"`, and lazy-loaded codegen.
- The TS codegen bundle duplicates logic that also runs in the CLI; keep one package and one set of golden tests.
- Sandbox escape or credential exposure; keep the iframe origin-isolated and never inject host tokens.
- Protocol drift between the wasm artifact and the host; pin both to the same `apiVersion` at build time.
- In-browser module resolution may be too large for V1; if so, ship single-module samples first and keep multi-module as a follow-up slice.

**Open decisions to settle before slice B**

1. Local-only at launch? Proposal: yes (`07-quality-security-and-delivery.md` Q9).
2. In-browser native/Wasm execution? Proposal: no, generated JS only (`06-tooling-and-workflow.md` Q10).
3. Shareable state: encode source in the URL fragment, or `localStorage` only? Fragment avoids any server retention.

**Evidence**

- None yet.

**Depends on**

- `JS-001` for JavaScript ESM output and the typed-module contract.
- `TYPE-004` for multi-source project checking if multi-module samples are in scope.
- `SCAF-003` and `astra-wasm` for the existing JSON-RPC and linear-memory boundary.
- Not blocked by `WASM-001`: that task is the C-to-Wasm target, not the compiler's own wasm build.

---

## Implementation backlog

These are intentionally high-level. Break them into smaller tasks when their phase begins.

- [x] `LEX-001` Lexer and spans
- [x] `PAR-001` Parser and AST
- [x] `FMT-001` Canonical AST formatter
- [x] `FMT-002` Comment preservation
- [x] `FMT-003` `astra fmt` CLI wiring
- [x] `FMT-004` Manifest formatting (`core.json` / `core.toml`)
- [x] `TYPE-001` Resolver
- [x] `TYPE-002` Type checker
- [x] `TYPE-003` Project type surfaces and cross-module checking
- [x] `TYPE-004` Multi-source compiler API for project checking
- [x] `MODE-001` Capability and mode inference
- [x] `ERR-001` Effect-tracked error checking
- [x] `NOVA-001` Typed-HIR interpreter
- [x] `NOVA-002` Nova CLI
- [x] `NOVA-003` Project module loading and linking
- [x] `NOVA-004` Nova language gaps — spec §6 `str` methods (`char_count`, `bytes`, `chars`, `byte_at`, `char_at`, `slice` with `SliceError`), the `bytes` builtin (`bytes.new`, bounds-checked index read/write, `len`/`is_empty`/`slice`, `for`-over-bytes), and the bounded Channel runtime (`Channel.create`, fallible `send`/`receive`, `close`, capacity-16 deadlock panics); Nova runs the whole shared conformance corpus with an empty `KNOWN_GAPS` ledger
- [x] `NOPM-001` Universal package model and resolver
- [x] `NOPM-002` Path, Git, npm, and Cargo adapters
- [x] `NOPM-003` Universal registry gateway
- [x] `NOPM-004` Later ecosystem adapters
- [x] `JS-001` JavaScript ESM backend (complete at `39437d0`; merged from `js-interop`)
- [x] `TS-001` TypeScript backend (complete at `df4ffc9`: shared-emitter TS mode, `astra build --target ts`, pinned `@script:ts` transpiler)
- [~] `INTEROP-001` Inline `@script` foreign-language blocks (JS, TS, and C/C++
  emission slices done; end-to-end native script linking waits on the `C-001`
  toolchain gate)
- [ ] `PLAY-001` Browser playground (expanded plan above)
- [~] `C-001` C backend - managed first slice + multi-module project linking (functions and cross-module struct/enum types) done; package linking, GC/manual memory, concurrency, FFI/`@script:c`, full patterns, native target policy, and cross-platform native verification remain
- [ ] `GC-001` Native GC
- [ ] `WASM-001` Wasm target
- [ ] `FFI-001` Polymorphic FFI
- [ ] `CONC-001` Structured concurrency runtime
- [~] `CONF-001` Conformance suite — 19 positive + 11 negative cases; JavaScript, TypeScript, Nova, and frontend-reject runners wired and green with empty gap ledgers; the C backend runner waits on the native toolchain gate
- [ ] `HARD-001` v0.1 hardening

---

## Blocked

Nothing is blocked. Rust 1.99.1 (GNU toolchain), `cargo`, `rustfmt`, and `clippy` are installed.

---

## Rules

1. Keep one task `[~]` at a time.
2. Do not start a phase before its gate is met.
3. Add new ideas to this file instead of expanding the current task.
4. Every completed item needs evidence in `state-log.md`.
5. If a task cannot meet its acceptance criteria, mark it `[!]` or `[-]`, not `[x]`.
6. Do not use this file as a wish list. It is a queue, not a brainstorm.
7. Never leave a source file in `src/` that is not declared in `lib.rs` (or its `mod` tree); unwired modules are invisible to CI.
8. If work stops before a task is complete, mark it `[~]`, record what compiles, and log the interruption in `state-log.md`.
