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

**Phase 7 — Backends**

`JS-001`, `TS-001`, and `INTEROP-001` are complete, and the shared conformance
suite (19 positive + 16 reject cases) is green on JavaScript, TypeScript, Nova,
and C with empty gap ledgers. `C-001` is the workstream: the managed C17 slice,
multi-module and package linking, generics, native libraries, the
single-threaded concurrency slice, and the Tier-1 Linux/macOS/Windows matrix are
landed; what remains was design-gated until **2026-10-31, when the user
answered all eight pending decisions** (`Docs/Design/pending-decisions.md`)
and added **D26** (two manifests: `core.*` + `build.*`), **D27** (evaluation
order: call arguments normative, other positions target-native), the
NoPM/toolchain plan (`Docs/Design/toolchain-and-packages.md`), and the V2
machine-code roadmap. Queue item 1 (the `@script:c` §5.1 boundary slice) is
implemented and **in review** with required fixes; nothing is committed yet. `GC-001` Phases 0/A/B/C are landed
(Phase C - collection on by default at a 1 MiB safepoint threshold - landed
2026-10-30 after S2 pinning, S3 poison oracle, and S4 signature; every
Tier-1 run from 32 onward is green on all three OSes, including the
per-case conformance-mode directive that closed the `NS6001`/`NS6002`
corpus gap). Only GC-001's Phase D (Wasm) remains, deferred until a managed
allocator in linear memory lands. Live GC status, open bugs, and the
ordered slices are owned by `Docs/Agent/gc-001-workstream.md`.
Open questions remain documented as proposals in `Docs/spec-v0.1.md` §17.

---

## In progress and up next

Every item that is **not** done, in queue order. Detail and acceptance
criteria live in each item's section further down; this table is the map.
Decisions made 2026-10-31: the eight pending verdicts (P1/P2/P3 included),
**D26** (two manifests: ``core.*`` + ``build.*``), **D27** (evaluation order),
the NoPM/toolchain plan (`Docs/Design/toolchain-and-packages.md`), and **V2**
(first-party machine-code backend). Kilo's first pass is on disk and **in review**;
nothing is committed yet. The package/CLI track (items 7–11) has its detailed
slice-by-slice task list in `Docs/Agent/nopm-cli-track.md`.

**Landed 2026-10-31 (`[x]`)**

1. `[x]` `@script:c` §5.1 boundary slice — landed as `2b8de7a` after the lead's
   review round (F1–F11): managed-arg script calls bracket enter/root/call/exit
   and publish managed results; Unit returns handled; `const void*` matches
   §5.1; hosted root-count pairing test; conditional positions keep the bare
   call (recorded limitation).
2. `[x]` `[P11]` — D27 dual rule in spec §3.10; the C emitter hoists
   side-effecting call arguments; the `arg_order` conformance case pins order
   across C/JS/TS/Nova. The system-mode side-effect rejection remains item 6.
3. `[x]` Hybrid `NS6002` — definition-site deferral + instantiation replay +
   transitive worklist fixpoint (P3); two-hop regression test (`2b8de7a`).

**Queued (`[ ]`)**
4. `[ ]` `HARD-001` — scope = A+B hybrid. Slice 1 landed as `d65397f` (Tier-1
   run 49 green): frontend structural fuzzing (`fuzz_frontend.rs`), parser
   nesting limit (64, NS2002, drain-to-EOF), and two fuzz-found parser fixes
   (exponential block-vs-map backtracking; sticky guard drain vs speculation
   restores). Remaining: panic/OOM-path audit, historical CI failure repros,
   release tooling half (B).
5. `[x]` `@export_as` implementation - landed as `c24a358` + `2a169ff` (Tier-1 runs
   44 and 45 green). The parser carries typed `TypeParameter = type` bindings
   (`Attribute.export_as`), the formatter round-trips them, the checker owns
   `NS7010` (target rules), `NS7011` (exact binding coverage), `NS7012`
   (C-boundary safety), and `NS7013` (invalid identifier, module-wide symbol
   reuse, repeated instantiation). The C backend registers each listed
   instantiation under the author symbol, drains it through normal
   monomorphization, and prototypes it in the module header; a hosted consumer
   calls `ns_demo_identity_i32`. Three reject cases in the corpus. Known V1
   limitation: a binding to a generic struct (e.g. `Box<i32>`) is rejected
   because the boundary predicate does not substitute struct type arguments -
   non-generic structs, enums, slices, arrays, and `Option` are supported.
6. `[x]` System-mode side-effect detection pass (P1) — landed as `2adaab6`
   (`NS5007`): rejects two or more state-changing calls in target-native
   positions with a hoist hint; call arguments exempt; corpus case + spec row.
7. `[~]` **D26 manifest split** — fix round in review (F6–F9: remove the
   last dual-read in `runBuild`, restore capability key ordering, delete dead
   `ROOT_ORDER`, restore the core canonical-order test + add a CLI-level
   rejection test, revert the out-of-area `scripts/ci.mjs` stack workaround).
   F1/F3/F5 were accepted; fixtures verified 88/88 across manifest, nopm,
   codegen-js, codegen-ts. Blocks NOPM-005 and TOOLCHAIN-001a.
8. `[ ]` `NOPM-005` `add`/`remove` with comment-preserving manifest edits.
9. `[ ]` `NOPM-006` `sys:` system-package-manager source; `bootstrap.mjs`
   becomes a caller.
10. `[ ]` `TOOLCHAIN-001a` `[toolchain]` + resolution ladder (pin → cache →
    PATH) + `astra doctor` + lockfile record — no network.
11. `[ ]` `TOOLCHAIN-001b` pinned download/verify/unpack (Zig `zig cc`),
    offline-safe, checksum table in-repo.
12. `[ ]` `CONC-001` - **decided 2026-11-01**. A2 (OS thread per spawn; A1
    eager fallback for Web/JS/Nova), B3 (park + watchdog panic, 5 s runtime
    constant, `ASTRA_CHANNEL_DEADLOCK_TIMEOUT` in tests), C2 (per-task heaps +
    immortal global heap), D1 (cooperative cancellation at GC safepoints and
    channel operations); `cancel()` on a completed task is an idempotent no-op,
    `send` copies managed values only, Web Workers deferred as a recorded gap.
    Verdicts and rationale: `Docs/Design/CONC-001-proposal.md` 10;
    pending-decisions 5 addendum; spec 9 updated. Implementation is the **last**
    V1 work item: runtime threads + park/watchdog, per-task heaps, safepoint
    cancellation, then spec 9.4/9.5 final wording, conformance re-pin of
    `channels`, and Tier-1 threaded CI.
13. `[-]` `PLAY-001` — starts after the queue; slice A (wasm bridge) merged as
    `61a5163`.

**Deferred / V2 (`[-]`) — confirmed 2026-10-31 unless noted**

14. `[-]` **V2:** first-party machine-code backend (`astra-codegen-llvm`
    reserved slot). V1 uses the C toolchain (C → gcc/clang/MSVC → machine code).
15. `[-]` `GC-001` Phase D (Wasm) — needs a managed allocator in linear memory.
16. `[-]` `WASM-001` C-to-Wasm path — needs the target ABI and runtime design.
17. `[-]` Fallible `Memory.alloc`/`region.alloc` entry-point policy — until
    then OOM panics, like `astra_gc_alloc`.
18. `[-]` `MutSlice<T>` surface behind `ptr.as_slice` — the spec names the
    type but defines no surface; building it would mean inventing semantics.
19. `[-]` Post-V1: pin release API (GC-001 `[P9]` follow-up), non-C `@script`
    embeddings (Python, Wasm, Java), `TOOLCHAIN-001c` cross-target triples.
20. `[-]` **`high` mode** — **decided 2026-11-01** (addendum §8): concept and
    design approved (`Docs/Design/high-mode-proposal.md`) — app capabilities +
    ban list (Memory/`@memory:manual`, `Ptr`, `Region`, `ffi`, `@script`,
    C-ABI/`@export_as`) + full sugar (top-level statements, implicit main/exit 0,
    `print`, tail return, `fn main` mixing error, optional return-type
    inference), profile via manifest `defaultMode`, importable as app. Not in
    V1; ships as a post-1.0 minor bump (1.1.0) with spec text + conformance
    written when scheduled.
21. `[-]` **More language surfaces** — decided direction 2026-11-01 (addendum
    §7): extend the C/Wasm/JS/TS pattern (build targets, interop, toolchain
    rungs) to Rust, Go, Lua, Ruby, Java. Per-language scope decided when each is
    picked up; V2+.

> **Versioning policy (decided 2026-11-01, addendum §6):** SemVer, lockstep
> packages, cut `1.0.0` when the V1 queue completes; big features (new mode,
> backend, or language surface) bump the minor, fixes bump the patch,
> incompatible rework bumps the major.

> The former "open design questions" are all answered. The `Region`
> `@memory:manual` question was decided 2026-10-30 (no gate; spec §8.4).

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
plus native-link slice are complete: `astra build --target native` links a
`@script:c` translation unit and runs it. Deferred runtime work remains.

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
- [x] Verify a native script link end to end: `astra build --target native`
      links `script_<name>.c` (and `.cpp` with `-lstdc++`) and runs the
      executable on this host, pinned by the boundary test
      `astra build --target native links an inline @script:c block`.
- [x] Make Nova reject foreign script calls with `NV0008` before execution.
- [x] Add conformance and security tests for unsupported target/language pairs,
      malformed terminators, opaque bodies, and mode propagation.
- [-] Deferred to post-V1 design work: Python, Wasm script embedding, Java, and
      Nova host adapters.

**Depends on**

- `JS-001` for the JavaScript slice (done). `TS-001` for TypeScript-target
  script emission (done: `@script:ts` pinned transpiler).
- C backend emission for the second slice (done: per-block TUs plus CLI
  compilation/linking); native link verification waits on the C-001 toolchain
  gate.
- NoPM is not required; inline scripts are project source, not packages.

---

## C-001 - C17 native backend

**Status:** `[ ]` The managed C17 first slice is implemented and tested, and
native executables now compile, link, and run on this host (gcc). The production
native backend is **not complete**: the remaining work is design-gated
(`GC-001` S2+, `CONC-001`, `FFI-001`, `MutSlice`, and the alloc entry-point
policy), so the task stays open while `GC-001` is the active slice.

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
      `--target native` probes for a *hosted-capable* compiler (clang, gcc, cc),
      links a host executable, and runs it; verified end to end through gcc.
- [x] 77 `astra-codegen-c` tests pass, including generated-C checks with the
      available clang toolchain, freestanding object compilation, and hosted
      compile/link/run checks.
- [x] Hosted runtime shim is a complete translation unit:
      `runtime::host_shim()` prepends `RUNTIME_HEADER` to `HOST_SHIM` (which adds
      `<inttypes.h>`/`<math.h>`), so `astra_host.c` sees `NS_Bytes`, `NS_Map`,
      and the builtin error payload typedefs. It previously failed to compile
      with 51 gcc errors.
- [x] C conformance gate executes the corpus: when a hosted-capable compiler is
      available it links each case against `astra_host.c`, runs it, and compares
      stdout plus `RETURN=` with `.expected` (18/19; `channels` is the CONC-001
      gap). A real miscompile was caught and fixed this way: match-arm block
      statements leaked into the next arm's branch (`early_exit` printed `-1`
      instead of `3`).
- [x] Native inline scripts: `astra build --target native` links `@script:c`
      (and `.cpp` with `-lstdc++`) translation units and runs the binary
      (INTEROP-001).
- [x] Single-threaded concurrency (`CONC-001` first slice): `Channel<T>` maps
      to a bounded `NS_Channel*` (capacity 16, value copies, `send`/`receive`
      returning `Result<_, ChannelError>`, `close`, deadlock panics on a full or
      empty open channel). `spawn` runs eagerly and returns `Task<T>`
      (`join`/`cancel`); `ChannelError` is a shared runtime payload. The
      `channels` corpus case leaves `KNOWN_GAPS`, so the C ledger is empty and
      all 19 cases run end to end.
- [x] `Array<T>.get(i) -> Option<T>`: bounds-checked into the option shape
      (no panic on an out-of-range read), matching the checker's signature.
- [x] Native library output and stable-ish ABI:
      - `generate_project_with_kind` / `EmitKind::Library` suppress the
        `int main(void)` wrapper while keeping `pub` functions external.
      - `astra build --emit staticlib|cdylib|library` compiles objects and uses
        the platform toolchain (`ar`/`llvm-ar`/`gcc-ar`, `-shared`) rather than
        writing artifacts by hand. Static archives contain module/script objects
        only; shared libraries include the hosted runtime.
      - `Docs/Design/V1/11-c-abi.md` records symbol naming, value layouts,
        error/panic behavior, ownership, and the linking contract; a
        hand-written C consumer links and runs against both artifacts (pinned by
        boundary tests).
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

- [x] Manifest path-package linking (`C-001` package-linking slice):
  - `ProjectCompileRequest.import_map` resolves bare specifiers (e.g.
    `"mathlib"`) to canonical module keys; `resolve_project_target` consults it
    before identity/relative resolution.
  - `astra build` reads `dependencies` entries with `source: "path"`, discovers
    the package's module graph (same `discoverProjectModules`), and re-keys it
    under `deps/<name>/...`, prepended so the importer stays the entry module.
  - `RUNTIME_HEADER` is include-guarded; previously a dependency header
    re-including it produced conflicting anonymous-struct typedefs (no test
    compiled a multi-module project as separate TUs).
  - Pinned by a compiler-api JSON-RPC test, a CLI boundary test, and
    `assert_project_compiles` (multi-module clang TU compilation).
  - Non-path (git/registry) packages installed by NoPM also link: the CLI reads
    `core.lock.json` (name → id) and `.astra/store/index.json` (id → root) and
    feeds the installed directory through the same `deps/<name>/` discovery.
  - Boundary: only non-aliased dependencies resolve by name (an aliased or
    git-URL dependency key does not appear as the lockfile `name`), and a
    package's own bare imports are not re-mapped (no package-of-package
    resolution).
- [x] Managed arena memory (GC-001 managed-mode first step). `astra_alloc`
      records every block; `astra_free`/`astra_realloc` keep the tracker in sync
      across array/map growth; the emitted `main` calls `astra_runtime_shutdown`
      to release the whole run's arena at exit. The freestanding `extern
      astra_alloc` boundary is unchanged (a Wasm backend can supply its own
      allocator). Pinned by an arena-lifecycle harness and an allocation-heavy
      hosted run.
- [x] Tier-1 CI matrix workflow (`.github/workflows/ci.yml`) runs the full gate
      on Linux, macOS, and Windows so the native build + conformance corpus
      execute on Tier-1 platforms. Correction (2026-10-28): the workflow cannot
      run from the source-only mirror (`luminousvalentine-cell/Neo-Astra`) -
      that remote intentionally carries no `.github/`, `scripts/`, `tests/`, or
      `conformance/`, so no run is ever triggered there, and the pre-push guard
      enforces the exclusion. Recorded runs require a second, full-fidelity CI
      remote that receives the whole repo; this was previously mis-stated as
      "waiting on a push". Evidence now exists: `voidgod79-debug/Neo-Astra-CI`
      is that remote, and run `40677187427` (commit `bc5d1c9`) passed all
      three OSes.
- [x] Manual-memory allocation subset: `Memory.alloc<T>`/`Memory.free` builtins
      (system-mode gated, `NS5004` in core), `Ptr<T>` → `T*`, arena lowering,
      `NS8012` removed for the supported subset.
- [x] Tier-1 gate correctness fixes, each surfaced by a recorded run: `npm test`
      now runs a `pretest` hook that builds `astra-compiler` (which
      `codegen-ts` and the stress suites spawn) and includes
      `stress_compiler_api`/`stress_nopm`, which were in no npm script and
      therefore never ran (49 → 111 tests); the workflow installs the
      `wasm32-unknown-unknown` target for `test:boundary`; staticlib archiving
      *attempts* `ar`/`llvm-ar`/`gcc-ar` instead of `--version`-probing them
      (Apple's `ar` rejects GNU long options, so macOS always reported
      NS9008); cdylib emission on an MSVC-flavored linker writes a `.def` from
      the generated headers' export section and links with `/DEF` + `/IMPLIB`
      (link.exe has neither `--export-all-symbols` nor `--out-implib`).

**Actually left**

- [x] Non-path package adapters. git/registry `.ns` packages installed by NoPM
      resolve through the lockfile + store index and link via the import map
      (boundary: non-aliased names only; package-of-package deps not resolved).
- [x] Manual memory, allocation/free subset. `Memory.alloc<T>(n) -> Ptr<T>` and
      `Memory.free(p)` are checker builtins (system-mode gated, `NS5004` in
      core), `Ptr<T>` maps to `T*`, and they lower to the run arena
      (`astra_alloc`/`astra_free`); `@memory:manual` scopes are no longer
      rejected (`NS8012` removed). Pinned by emission + checker-mode tests.
- [x] Manual-memory pointer **access** (spec §8.4). `ptr.read(index)` and
      `ptr.write(index, value)` type-check as methods on `Ptr<T>` and lower to
      plain C indexing on the `T*` that `Ptr<T>` already maps to, so a write is
      visible to a later read and to the eventual `Memory.free`. Both are
      unchecked by design (a manual pointer carries no length, so there is
      nothing to bounds-check against). Raw pointer *usage* now carries the same
      `NS5004` system-mode gate as the allocation call, so a core-mode function
      that merely receives a `Ptr` and reads through it is rejected rather than
      silently touching raw memory. Boundary: `ptr.as_slice(count) ->
      MutSlice<T>` stays unimplemented - the spec names `MutSlice` but defines
      no surface for it, so building it would mean inventing semantics. Pinned
      by `emit_c` tests (including a hosted compile-and-run) and by the
      backend-neutral reject case `conformance/rejects/pointer_access_requires_system`.
- [x] Precise mark-sweep GC with compiler stack maps (`GC-001`, C backend) —
      **complete on the C backend 2026-10-30.** Phases 0/A/B/C and slices
      S1-S5 are landed and Tier-1 verified (runs 29-36 green on Linux, macOS,
      and Windows): the NS6002 manual-element gate, the `NS_Gc_Head`/shape
      object model, root frames + snapshots + materialisation, the collector
      with the stress leg, §5.8 boundary pinning, the §8.2 poison oracle, the
      `[P9]` scoped-handle signature (`11-c-abi.md` §5.1, before any call
      site), and **collection on by default** at a 1 MiB safepoint threshold
      with the §8.7 budget measured (116 B static). Verification-plan §8
      items 1-7 are all done. The design decisions (`[P1]`-`[P11]`, user-
      decided 2026-10-29-30 with amendments) and the landing records live in
      `Docs/Design/GC-001-proposal.md` and `Docs/Agent/gc-001-workstream.md`
      section 6. Only Phase D (Wasm) remains and is deferred (`[-]`, needs a
      managed allocator in linear memory); `[P11]`'s spec review stays open
      as its own item below.
- [x] Phase 0 of GC-001: forbid managed element types in manual allocation as
      `NS6002` "Manual allocation of a GC-managed element type" - **landed
      2026-10-29; the code number was confirmed by the user ("ok do it"), so it
      is no longer proposed-but-unapproved.** The predicate is `holds_managed_refs`
      in the checker: `str`, `bytes`, `[T]`, maps (even scalar-payload ones -
      the buffer itself is managed), `Channel`, `Task`, `Option`/`Result`,
      tuples, and structs/enums whose fields hold any of them; allowed are
      scalars, `Ptr<U>` (a raw address is one machine word), `Region` (a
      manual-block handle the collector never traces), and composites built
      only from pointer-free types - exactly the inverse of the GC-001 shape
      predicate, so no new analysis. Mechanics: `Memory.alloc` registers its
      member span after the mode and scope gates pass, `region.alloc` registers
      on any `Region`-typed receiver, and `check_call` applies `NS6002` once
      the call's `T` is bound in the return type (explicit or inferred), so
      ordering stays mode `NS5004` -> scope `NS6001` -> element type
      `NS6002`: one program, one violated condition. Spec: one §8.4 rule line
      plus a §14.9 row and explanation. Tests: reject matrix (7 simple forms +
      3 composites + region site), accept matrix (11 pointer-free forms incl.
      generic instantiation and fixed arrays), and the ordering test. Honest
      limitations: (1) **no negative-conformance case was possible** at
      landing - the rejects corpus ran at the default core mode, so `NS5004`
      always fired first (the same reason `NS6001` had no corpus case);
      **closed 2026-10-30**: the rejects runner supports a per-case
      `// conformance: mode=<mode>` directive (test infrastructure only, not
      language surface), and three corpus cases now pin the rules -
      `manual_alloc_in_gc_scope` (`NS6001`), `managed_element_in_memory_alloc`
      and `managed_element_in_region_alloc` (`NS6002` at both sites);
      (2) **generic forwarding** -
      `Memory.alloc<Wrap<T>>` inside a function whose own `T` is still a type
      parameter - is silent until the element type is concrete at the
      definition site, the same instantiation model the whole checker uses
      (bodies are not re-checked per call instantiation).
- [x] Phase A of GC-001 (object model + allocator split) - landed 2026-10-29
      in `a7c3b58` (Tier-1 run 20 red on Ubuntu, fixed by `753c54f`).
      Details: `Docs/Agent/gc-001-workstream.md` section 6 (landing records).
- [x] Phase B of GC-001 (root frames, snapshots, materialisation, collector
      under `ASTRA_GC_STRESS=1`) - landed 2026-10-29 in `3e9893b`,
      `0233ce2`, `18fc3fa`, with helper-root follow-ups (`1b6fa85`,
      map growth). Collection is still off by default. Details, open bugs and
      the honest limits: `Docs/Agent/gc-001-workstream.md` sections 2 and 6.
- [x] GC-001 Phase C prerequisites, in order (see
      `Docs/Agent/gc-001-workstream.md` section 5) - **all landed 2026-10-30**:
      S1 a green stress run on all three OSes (run 27); S2 §5.8 boundary
      pinning (`25763da`, run 29); S3 §8.2 poison oracle (`2fe0784` + the
      portable pivot `16f2e52`, run 32); S4 §5.9 FFI signature (scoped handle,
      `11-c-abi.md` §5.1). Phase C itself then landed the same day (`8fd881f`,
      run 33): collection is on by default at a 1 MiB safepoint threshold.
      Only Phase D (Wasm) remains, deferred.
- [x] Spec §8.4 rule "Memory.alloc in a GC-managed scope is a compile-time
      error" - **was unimplemented, now enforced as `NS6001`.** A probe test
      confirmed the gap was real: system mode accepted `Memory.alloc` with no
      `@memory:manual` anywhere, because `manual_memory_scopes` had no
      diagnostic reader. The checker now rejects it with `NS6001` "Manual
      allocation outside a manual-memory scope", the first code assigned in
      the `NS6xxx` Memory range that the spec declares in §14.3 but had never
      populated (added as spec §14.9). Two details mattered: recorded scope
      spans now cover the whole `{@memory:manual; ...}` block instead of only
      its header, so containment sees the body and correctly rejects an
      allocation made *after* a manual block closes; and the mode gate runs
      first, so a core-mode program reports `NS5004` only - one program, one
      violated condition. Boundary: only `alloc` is scoped, because §8.4 names
      only `alloc` (`Memory.free` and `ptr.*` have no scope rule), and there is
      no conformance reject case because the corpus runs the default core
      profile where the mode condition is what fails, making `NS6001`
      unreachable there. Pinned by the checker test
      `rejects_manual_alloc_outside_a_manual_scope`.
- [x] `Region.new`, `region.alloc`, and `region.free` (spec §8.4, spec lines
      1308-1310) - **were unimplemented, now landed with the user-decided
      fixed-block bump-arena strategy.** `Region.new(bytes)` reserves one
      block; `region.alloc<T>(count)` carves `count * sizeof(T)` bytes
      bump-style (8-byte aligned carvings) and panics on exhaustion, like
      `Memory.alloc` does on OOM, until the fallible entry-point policy
      covers both; `region.free()` releases the whole block and zeroes the
      header so a double-free is a no-op. `Region` maps to `NS_Region*` (a
      header-plus-block handle, same shape as `Channel`), so the bump cursor
      is shared by every allocation from that handle. Frontend: `Region` is a
      non-generic builtin type (`Region<T>` is a type-argument error), the
      `Region.new` path and the `region.alloc`/`region.free` members all carry
      the NS5004 system-mode gate, and `method_c` gained the call expression
      so `region.alloc<T>` can read its element type off the call (the
      receiver carries no type arguments). The spec's scope rule names only
      `Memory.alloc`, so Region use needs system mode but no
      `@memory:manual` block - that literal reading is deliberate; whether
      Region should also require the manual scope is recorded below as an
      open design question. Verified by
      `checker::tests::region_api_type_checks_in_system_mode`,
      `checker::tests::rejects_region_api_outside_system_mode`,
      `emit_c::region_bump_arena_lowers_and_runs` (hosted compile-and-run:
      two carvings distinct, writes stable until `region.free()`), and the
      conformance reject case `conformance/rejects/region_api_requires_system`
      (discovery proven by failing it with a wrong expectation first).
- [x] Design question (spec surface): should `Region.new` and `region.alloc`
      also require an `@memory:manual` scope, like `Memory.alloc` does via
      `NS6001`? **Decided 2026-10-30: no.** Spec §8.4 names only
      `Memory.alloc`, matching the implementation; `Region` is scoped
      allocation whose blocks are released by `region.free()`. The §8.4
      clarification line landed with the verdict and records the hazard: a
      region that is never freed leaks its blocks, and the GC never reclaims
      manual memory. Closed when `Docs/Design/pending-decisions.md` landed.
- [ ] Full structured concurrency (OS-threaded runtime). The single-threaded
      eager slice (`spawn`/`Task`, bounded `Channel`, `ChannelError`) is the
      landed V1 scope and matches the JS/Nova model (deadlock panics). Moving to
      OS threads (pthreads/C11) changes those deadlock semantics to blocking
      waits and needs task-local heaps + cancellation safepoints — a `CONC-001`
      design decision, recorded as a follow-up rather than done unilaterally.
- [x] Native library output and stable ABI. `--emit staticlib|cdylib|library`
      produces `lib<name>.a`, `lib<name>.so`, or `lib<name>.dll` (+ import
      library), suppresses `main`, and is documented in
      `Docs/Design/V1/11-c-abi.md`; a hand-written C consumer links and runs
      against both artifact kinds.
- [x] Native verification on this Windows host. `astra build --target native`
      compiles, links, and runs through gcc (MSYS2 UCRT64). The earlier "no libc
      headers" failure came from preferring clang without a Windows SDK plus the
      shim missing `NS_Bytes`/`NS_Map`; both are fixed, and the native execution
      path is pinned by `astra build --target native links and runs a hosted
      binary`.
- [x] Native verification on Linux and macOS — **recorded.** The full-fidelity
      CI remote `voidgod79-debug/Neo-Astra-CI` runs `.github/workflows/ci.yml`
      on `ubuntu-latest`/`macos-latest`/`windows-latest`, where the runners
      ship a hosted C compiler, so the conformance corpus and native build
      execute there. Run `40677187427` (commit `bc5d1c9`) is `success` on all
      three OSes; the four earlier failing runs are the defect trail (listed
      under Evidence below).
- [x] Native CI gate in the test suite. `cargo test -p astra-codegen-c` runs
      every non-gap conformance case end to end whenever a hosted compiler is
      available (and self-skips otherwise), so a normal Linux runner already
      exercises native execution.
- [x] Tier-1 CI matrix **provisioned and green** (`.github/workflows/ci.yml`,
      three OSes). Recorded Linux/macOS/Windows evidence landed 2026-10-28:
      run `40677187427` on `bc5d1c9`.
- [-] Wasm path through C17 (`WASM-001`) is deferred until the target ABI and
      runtime design land.

**Evidence**

- Tier-1 CI (full-fidelity remote `voidgod79-debug/Neo-Astra-CI`): run
  `40677187427` on `bc5d1c9` is `success` on ubuntu-latest, macos-latest, and
  windows-latest, which is the recorded Linux/macOS native verification.
  Defect trail: `40655262547` (`2206f4e`, TS tests spawned a binary that was
  never built on a fresh runner) → `40670278321` (`1811ce1`, missing
  `wasm32-unknown-unknown` target) → `40671917569` (`fb51841`, Ubuntu green;
  macOS `ar` probe and Windows link.exe flavor exposed) → `40676108614`
  (`32b0ef5`, Windows green; test asserted an import library off-Windows) →
  `40677187427` (`bc5d1c9`, all three green).
- `crates/astra-codegen-c/tests/emit_c.rs`: 80 passing tests covering emission,
  runtime helpers, `Option`/`Result`, errors, matches (string, bytes, enum, bool, option),
  arrays (`len`/`push`/`get`), structs, enums, `defer`, map literals/methods (`.len`/`.contains`/`.get`/`.insert`),
  the managed arena lifecycle (`astra_alloc`/`astra_free`/`astra_runtime_shutdown`
  released at exit) and `Memory.alloc`/`Memory.free` lowering (`Ptr<T>` → `T*`,
  system-mode gated),
  `for` loops (arrays, maps, bytes), byte literals/indexing (reads and
  bounds-checked writes)/slicing/comparisons,
  `str` slice `Result` lowering (`SliceError`), `str.to_int`/`checked_*`/
  `wrapping_*` lowering, `str` scalar/sequence views (`char_count`, `byte_at`,
  `char_at`, `bytes`, `chars`, `bytes.new`), `Option`/`Result` accessors,
  generic monomorphization (per-instantiation symbols, generic-to-generic
  calls), `!` propagation of fallible builtin calls, the complete hosted shim
  translation unit and match-arm branch placement, library emission (no `main`,
  external `pub` symbols), `@script:c` and `@script:cpp` emission, compiler API
  artifacts, rejection paths,
  and multi-module project emission (cross-module call resolution, symbol
  collision avoidance, qualified header prototypes, entry-only `main`, namespace
  imports, cross-module struct/enum/tuple-struct types, the `NS8019` rejection,
  and header dependency includes).
- `crates/astra-codegen-c/tests/conformance.rs`: the C conformance gate lowers
  every shared case and clang-compiles the result freestanding. When a
  hosted-capable compiler is available it additionally links each case against
  `astra_host.c`, runs it, and compares stdout plus `RETURN=` with `.expected`:
  all 19 cases pass end to end, including `channels`. `KNOWN_GAPS` is empty and
  fails if a listed case starts passing (or a listed case disappears).
- `cargo test --workspace` passes (77 in `astra-codegen-c`, 142 in `astra-core`,
  31 in `astra-compiler-api`, 51 in `astra-nova`, etc.).
- `astra build --target c` emits `main.c` (single module) or one qualified
  `<stem>.c`/`<stem>.h` per module plus `astra_host.c` (multi module); boundary
  tests cover both paths.
- `astra build --target native` links and runs a hosted executable on this
  Windows host through gcc (MSYS2 UCRT64), pinned by the self-skipping boundary
  test `astra build --target native links and runs a hosted binary`. Clang alone
  still cannot build hosted C here without the MSVC/Windows SDK, so the CLI
  probes for hosted capability before selecting a compiler.
- `astra build --emit staticlib|cdylib` produces a real library with the
  platform toolchain, and hand-written C consumers link and run against both
  (`astra build --emit staticlib produces a linkable C library`,
  `astra build --emit cdylib produces a runnable shared library`).
- `Docs/Design/V1/11-c-abi.md` documents the artifact, symbol, layout, and
  linking contract.

**Depends on**

- `TYPE-001`-`TYPE-004`, `ERR-001`, and the shared typed-module boundary.
- `GC-001` for native manual memory and managed allocation.
- `CONC-001` for tasks and channels.
- `FFI-001` and `INTEROP-001` Slice D for C/C++ foreign blocks.
- `WASM-001` for the C-to-Wasm path.

---

## PLAY-001 — Browser playground

**Status:** Not started, queued last (decision 7b: starts once the V1 queue clears). Backlog entry expanded into a runnable plan; slice A (the wasm bridge) already merged from `work/play-001-a` as `61a5163` (decision 7a, executed 2026-10-31). No design decision blocks it, but two API methods it needs (`ast`, `explain`) are still stubs.

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
- [x] `INTEROP-001` Inline `@script` foreign-language blocks (JS, TS, and C/C++
  emission slices done; native C/C++ script linking verified end to end)
- [ ] `PLAY-001` Browser playground — **not started, queued last** (decision
  7b: starts only after the queue clears; see the expanded `PLAY-001`
  section above for slices A-E with full acceptance). Decision 7a is
  executed: the `work/play-001-a` bridge was inspected, judged valuable,
  and merged as `61a5163` (slice A exists), and the branch was deleted.
  The `ast`/`explain` API methods it wants are still stubs.
- [x] `C-001` C backend — managed first slice, multi-module project linking,
  generics monomorphization, hosted runtime shim, native compile-and-run
  verification, `Array.get`, native library output/ABI, the single-threaded
  concurrency slice, path/installed package linking, the managed arena,
  manual-memory alloc/free plus `ptr.read`/`ptr.write`, the precise
  mark-sweep collector (`GC-001` Phases 0/A/B/C, collection on by default),
  and the Tier-1 Linux/macOS/Windows matrix (green through run 36) are
  landed. Remaining C work is design-gated and tracked by the `CONC-001`
  and `FFI-001` items plus the unspecced `MutSlice` surface and the
  fallible `Memory.alloc` entry-point policy.
- [x] `GC-001` Precise mark-sweep GC (C backend) — Phases 0/A/B/C and
  slices S1-S5 landed and Tier-1 verified (runs 27-36 across the workstream;
  Phase C run 33, mode-directive run 35). Collection is on by default at a
  1 MiB safepoint threshold; the §8.7 budget is measured (116 B static).
  Live status and landing records: `Docs/Agent/gc-001-workstream.md`.
- [-] `GC-001` Phase D (Wasm) — deferred until a managed allocator in linear
  memory exists (`WASM-001` territory); the §5.1 FFI root-registration triple
  is landed and emitted at the first foreign-frame boundary whenever that is.
- [-] `WASM-001` Wasm target (deliberately deferred until the target ABI and runtime design land)
- [ ] `FFI-001` Polymorphic FFI — **decided 2026-10-31** (`pending-decisions.md`
  items 3 and 4); implementation queued as queue items 1 and 4:
  - **Item 4 verdict — do it now (queue item 1):** the first `@script:c`
    boundary that passes managed values into a foreign frame emits the
    `11-c-abi.md` §5.1 registration triple
    (`astra_gc_scope_enter`/`astra_gc_root`/`astra_gc_scope_exit`) and lands
    the proposal §7 boundary-rooting test with it. Implemented by Kilo; in
    review with fixes F1–F6/F8 (see `Docs/Report/Task.md`).
  - **Item 3 verdict — `@export_as("symbol", T = type)` (queue item 5):**
    new language surface for V1 — spec §11.5 and `11-c-abi.md` §3 are written
    (author-named symbols used verbatim, `NS7010`–`NS7013` gates);
    implementation (checker, parser/formatter, C emission, consumer test) is
    its own slice. The current §3 refusal stays in force until it lands.
  - The generic-forwarding half was answered as decision 2 (queue item 3),
    not here: definition-site flag + instantiation-site `NS6002` micro-pass.
  - Acceptance when each piece lands: §5.1 slice pinned by the §7
    boundary-root test; the attribute form implemented on C, pinned by a
    hand-written C consumer test, documented in `11-c-abi.md`, and covered
    by accept/reject conformance cases.
- [ ] `CONC-001` OS-threaded structured concurrency (second slice) —
  **decided 2026-10-31 (decision 5): write the proposal (option B), as the
  last V1 work item.** The single-threaded eager `spawn`/`Task<T>`, bounded
  `Channel<T>` with `ChannelError`, close, and deadlock panics are landed and
  the `channels` corpus case passes on C, JS, TS, and Nova. The proposal
  decides, at minimum:
  - Threading model: pthreads/C11 threads vs a pool, and join semantics.
  - Deadlock semantics change: full/empty channel waits become blocking, so
    the `channels` conformance case and its `.expected` output must be
    re-pinned per target or the case gains a mode directive.
  - Task-local heaps: GC-001 collects one task heap with one root-frame
    stack; a second task on another thread needs its own frames/directory or
    a stop-the-world protocol - the collector's single-heap assumption is
    recorded, not accidental.
  - Cancellation safepoints: where cancellation is observed (the safepoint
    hook already exists at function entry and loop back-edges).
  - Acceptance: proposal approved; single-threaded semantics unchanged by
    default; the OS-threaded mode is pinned by tests on all three OSes; spec
    §9 and the JS/Nova model are updated or explicitly scoped out.
- [x] `CONF-001` Conformance suite - 19 positive + 16 negative cases (the three
  2026-10-30 additions cover `NS6001` and `NS6002` via the per-case mode
  directive); JavaScript, TypeScript, Nova, frontend-reject, and C runners are
  wired and green with empty gap ledgers (C executes every case when a hosted
  compiler is available)
- [ ] `HARD-001` v0.1 hardening — **scope decided 2026-10-31 (decision 6:
  A+B hybrid): "Structural Fuzzing Pass with Distributed Release Tooling."**
  Part of (A) landed as `d65397f` (run 49 green): frontend fuzz suite,
  nesting limit, exponential-backtracking and sticky-drain parser fixes.
  Remaining: panic/OOM-path audit, historical CI failure repros, and the
  release tooling half (B).

- [ ] `D26` Two manifests - split into `core.*` (package identity, dependencies, sources, registry, capabilities, ffi) and optional `build.*` (build directives, targets, nova, semantic policies, toolchain). Spec §12 rewritten; implementation (manifest package, formatter, CLI discovery, fixture migration) is queue item 7.
- [ ] `NOPM-005` `add`/`remove` - npm-install UX that edits `core.*` dependencies (comment-preserving) and rewrites the lockfile. Queue item 8.
- [ ] `NOPM-006` `sys:` source - system package manager integration (winget/brew/apt/dnf/zypper) as a NoPM adapter; `bootstrap.mjs` becomes a caller. Queue item 9.
- [ ] `TOOLCHAIN-001a` Toolchain ladder - `[toolchain]` in `build.*`, pin -> `.tools` cache -> PATH resolution, `astra doctor`, lockfile record; no network. Queue item 10.
- [ ] `TOOLCHAIN-001b` Pinned toolchain download - verify/unpack a pinned Zig `zig cc` (one artifact family, cross-compilation included), offline-safe; no implicit elevation. Queue item 11.
- [-] V2: first-party machine-code backend (`astra-codegen-llvm` reserved slot). V1 uses the C toolchain (C -> gcc/clang/MSVC -> machine code).

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
