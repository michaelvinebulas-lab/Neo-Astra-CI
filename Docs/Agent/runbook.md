# Step-by-Step Runbook

This runbook breaks Neo Astra into sequential phases. Work on one phase and one task at a time. Do not skip a phase because a later phase looks more interesting.

---

## 1. Phase map

| Phase | Name | Output | Status |
|---|---|---|---|
| 0 | Orientation | Verified project state | Ongoing at session start |
| 1 | Architecture decisions | Confirmed D1–D25 | Done |
| 2 | Specification v0.1 | `Docs/spec-v0.1.md` | Done |
| 3 | Workspace scaffolding | TS orchestrator + Rust core skeleton | Done |
| 4 | Lexer and parser | AST + diagnostics | Done |
| 5 | Types, modes, and errors | Typed HIR + capability checks | Current |
| 6 | Nova and NoPM | Executable `.ns` source and reproducible packages | In progress |
| 7 | JS/TS backend and playground | Browser-runnable output | In progress |
| 8 | C backend, native, Wasm | Native and Wasm binaries | Not started |
| 9 | FFI and concurrency | Foreign modules, inline `@script`, and structured tasks | Not started |
| 10 | Hardening | Stable v0.1 | Not started |

---

## 2. Phase 0 — Orientation

**Goal:** Establish the real state of the repository before changing anything.

### Steps

1. Read `state-log.md`.
2. Read `todo.md`.
3. Read the relevant section of `../architecture.md`.
4. List the files that exist.
5. Identify the current phase.
6. Identify blockers.
7. Select one task.

### Gate

- The current phase is known.
- The selected task is in `todo.md`.
- No unresolved blocker prevents the task.

---

## 3. Phase 1 — Architecture decisions

**Status:** Done.

### Output

- D1–D25 confirmed in `../architecture.md`.
- Naming normalized to Neo Astra, `.ns`, `astra`.
- Manifest, memory, mode, error, concurrency, FFI, package, and compiler decisions recorded.

### Rule

Do not reopen a decision unless the user explicitly asks. If a new requirement conflicts with a decision, follow `edge-case-protocol.md`.

---

## 4. Phase 2 — Specification v0.1

**Status:** Done. `Docs/spec-v0.1.md` is drafted and reviewed against D1–D25. Open questions remain listed as proposals in §17.

**Goal:** Turn the architecture into an implementable language specification.

### Steps

1. Create `docs/spec-v0.1.md` with a table of contents.
2. Define lexical rules:
   - Identifiers
   - Literals
   - `//` comments only
   - Canonical keywords and aliases
   - Whitespace and newlines
3. Define the grammar:
   - Modules
   - Functions
   - Types
   - Statements
   - Expressions
   - Patterns
   - Declaration visibility
   - Imports and exports
   - Mode blocks
   - Foreign modules
4. Define the type system:
   - Primitive types
   - Arrays, maps, tuples, structs, enums
   - `Option`, `Result`
   - Generics
   - Inference boundaries
5. Define the semantics matrix:
   - Integers
   - Strings
   - Arrays
   - Maps
   - Errors
   - Memory
   - Concurrency
   - FFI
6. Define mode rules:
   - `core`, `app`, `web`, `system`
   - Selection pipeline
   - Source tags
   - Scoped blocks
   - Monotonic capability rule
   - Viral requirements
   - `astra explain`
7. Define memory rules:
   - GC default
   - Manual memory on native/Wasm
   - Block/file/manifest precedence
   - JS rejection behavior
8. Define the error model:
   - `!` effects
   - Error unions
   - Propagation
   - Panic/abort
   - Target representation
9. Define structured concurrency:
   - `spawn`
   - `Channel`
   - Scope and cancellation
   - Native and Web Worker mapping
10. Define FFI and package rules:
    - `foreign module`
    - Manifest mappings
    - Type mapping
    - Package sources
11. Define diagnostics:
    - Error codes
    - Source spans
    - JSON format
    - Explain output
12. Define the conformance plan:
    - Portable cases
    - Target-specific cases
    - Expected behavior

### Gate

- Every confirmed decision has a corresponding spec rule.
- No section contradicts another.
- Every syntax example is plausible.
- Every semantic rule has a test plan.
- `state-log.md` records completion.

---

## 5. Phase 3 — Workspace scaffolding

**Status:** Done. `SCAF-001`–`SCAF-004` are complete; the boundary smoke test and CI workflow pass.

**Goal:** Create the shared interfaces before implementing features.

### Steps

1. Create the TypeScript orchestrator workspace.
2. Create the Rust core workspace.
3. Define the compiler API boundary:
   - Versioned JSON protocol
   - WASM entrypoint
   - Native binary entrypoint
4. Add build commands and CI scripts.
5. Add a minimal smoke test for the boundary.
6. Add the manifest parser entrypoint.
7. Add the diagnostic JSON schema.

### Gate

- TypeScript can call the Rust core.
- The Rust core can parse a trivial input.
- The boundary is versioned.
- No second parser exists.
- The smoke test passes.

---

## 6. Phase 4 — Lexer, parser, and formatter

**Status:** `LEX-001`, `PAR-001`, and `FMT-001`–`FMT-004` are complete; `astra fmt` formats `.ns` source through the Rust core and `core.json` / `core.toml` through the manifest package. `astra ast --json` remains a tooling follow-up.

**Goal:** Parse `.ns` source into a canonical AST.

### Steps

1. Implement tokens and spans.
2. Implement `//` comments only.
3. Implement expressions, statements, and newline/semicolon separators.
4. Implement types and patterns.
5. Implement imports, exports, and declaration visibility.
6. Implement mode blocks and foreign modules.
7. Implement error recovery with structured diagnostics.
8. Implement `astra ast --json`.
9. Add parser tests and invalid-syntax tests.

### Gate

- Valid examples parse.
- Invalid comments are rejected.
- Visibility, import, and export forms parse with structured diagnostics.
- Diagnostics include spans and codes.
- The AST is machine-readable.
- The formatter can consume the AST.

---

## 7. Phase 5 — Types, modes, and errors (current)

**Goal:** Enforce the semantic contract.

**Status:** `TYPE-001`-`TYPE-004`, `MODE-001`, `ERR-001`, `NOVA-001`,
`NOVA-002`, and `NOVA-003` are complete, including manifest `[nova].entry`
discovery and project module linking. The dedicated `astra explain` command
remains. Compiler metadata already exposes the stable mode explanation tree.

### Steps

1. Implement the resolver and module export surfaces. *(complete)*
2. Implement primitive and composite types. *(complete)*
3. Implement local inference. *(complete)*
4. Implement strict API boundaries. *(complete)*
5. Implement exhaustive matching. *(complete)*
6. Implement `!` error effects with effect provenance. *(complete)*
7. Implement mode/capability checking. *(complete)*
8. Implement source tags and scoped blocks. *(complete)*
9. Implement least-privileged inference. *(complete)*
10. Implement `astra explain`. *(metadata exposed; CLI pending)*
11. Add negative tests for every rule. *(core checker, inference, and Nova covered)*

### Gate

- Type errors are structured.
- Fallible calls without `!` are rejected.
- Mode widening without explicit annotation is rejected.
- Private exports and import/export conflicts are rejected.
- Inferred modes are explainable.
- No implicit coercion is accepted.

---

## 8. Phase 6 — Nova and NoPM

**Goal:** Run typed `.ns` source directly and resolve universal packages before
code generation.

Nova is a development runtime, not the production backend. It reuses the Rust
frontend and starts as a typed-HIR interpreter. NoPM is the single package
manager for Nova and compiled builds.

`NOVA-001` is complete; the Nova CLI (`NOVA-002`) provides `run`, `eval`, `repl`,
`watch`, `test`, `version`, inferred or explicit modes, diagnostics, JSON
output, and manifest entry discovery. `NOVA-003` adds canonical project module
loading, dependency-ordered checking, re-export/namespace linking, and
module-aware runtime errors and panic frames. `MODE-001` resolves the source
mode before checking; pass `--mode` to disable inference. NoPM is complete
through NOPM-004: package/lockfile models, lockfile I/O, store, all first
adapters, registry gateway with signing/federation, and
`lock`/`tree`/`install`/`search`/`audit`/`why`/`update`. `TYPE-004` exposes the
multi-source `check_project` API through the compiler boundary. The remaining
work is production backends, starting with `JS-001`, and conformance coverage.

### 8.1 Nova

1. Implement the tree-walking interpreter over typed HIR. *(complete)*
2. Implement values, environments, and call frames. *(complete)*
3. Implement errors, propagation, and panics. *(complete)*
4. Implement arrays, maps, structs, enums, and pattern matching. *(complete)*
5. Implement the portable managed subset; reject unsupported manual memory clearly. *(complete)*
6. Implement a deterministic single-threaded task stub. *(complete)*
7. Add `nova run`, `nova repl`, `nova watch`, `nova test`, and `nova eval`. *(complete)*
8. Load and link local project modules through one module-aware interpreter. *(complete)*
9. Add interpreter conformance tests.

### 8.2 NoPM

1. Implement the universal package descriptor and source-qualified IDs. *(package IDs, descriptors, and deterministic graph complete)*
2. Implement path, Git, npm, and Cargo adapters.
3. Implement target/mode-aware resolution and `core.lock.json`.
4. Implement the content-addressed store and `--locked` / `--frozen`.
5. Add `nopm install`, `nopm lock`, `nopm add`, and `nopm remove`.
6. Keep lifecycle scripts disabled by default.
7. Add the optional `registry.neovastra.dev` gateway after local resolution works.

### Gate

- A meaningful subset of typed core programs runs.
- Diagnostics match `astra check`.
- Mode and capability violations fail.
- Nova is not treated as the production backend.
- No second parser or type checker exists.
- NoPM resolution is deterministic, target-aware, and integrity-checked.
- The same lockfile serves Nova and compiled builds.
- Local development works without the universal registry.

---

## 9. Phase 7 - JavaScript/TypeScript backend and playground

**Status:** In progress. The first `JS-001` single-module slice is implemented,
including recoverable errors, checked integer methods, eager tasks, and source
maps. Project imports, channels/scheduler, the `arbitrary` policy, `.d.ts`, and
`TS-001` remain.

**Goal:** Produce readable, runnable web output.

### Steps

1. Consume typed HIR/IR from the Rust core.
2. Emit readable ESM.
3. Emit named, default, namespace, and re-export forms; erase type-only imports and exports.
4. Implement source maps.
5. Implement error mapping to `try`/`catch`.
6. Implement string and integer policies.
7. Implement `strings.model`.
8. Build the browser playground with the Rust core compiled to WASM.
9. Add generated-output tests.

### Current JS slice

- `compile` returns a versioned `ModuleIr` with AST, HIR, annotations,
  manual-memory scopes, target, and integer policy.
- `@neo-astra/codegen-js` emits functions, constants, control flow, defer,
  structs, enums, arrays, structural maps, tuples, options, results, match,
  built-ins, and tree-shaken local helpers.
- `astra build` selects a target, checks the entry module, emits ESM and a
  source map under the target `outDir`, and adds `"type": "module"` when needed.
- Recoverable failures, `?`, checked integer methods, and eager spawn/tasks are
  implemented; unsupported imports, channels, the `arbitrary` policy, and manual
  memory fail with source-located diagnostics instead of changing semantics.
- Next: project-wide typed modules and import emission, then channels/scheduler,
  the `arbitrary` policy, UTF-8 byte operations, and the TypeScript backend.

### Gate

- Output is readable and framework-free.
- Core semantics are preserved.
- Web boundary behavior is explicit.
- The playground runs the same source as the CLI.
- Source maps work.

---

## 10. Phase 8 — C backend, native, and Wasm

**Goal:** Produce native and Wasm output through C and `clang`.

### Steps

1. Implement the C backend.
2. Implement a minimal runtime.
3. Implement GC.
4. Integrate `clang`.
5. Add the native target.
6. Add `wasm32` targets.
7. Add binary-size checks.
8. Add native conformance tests.

### Gate

- Native binaries run the conformance suite.
- Wasm output runs in the chosen host.
- Memory policy is respected.
- Manual memory is rejected on JS.

---

## 11. Phase 9 — FFI and concurrency

**Goal:** Connect Neo Astra to native and web hosts and run structured tasks.

### Steps

1. Implement abstract foreign modules.
2. Implement manifest FFI mappings.
3. Implement C ABI bindings.
4. Implement JS/TS import bindings.
5. Implement inline `@script` parsing and typed exports after the Phase 7 JS/TS
   backends; add JS emission, then the C/C++ translation-unit path.
6. Implement structured concurrency on native.
7. Implement Web Worker generation.
8. Add boundary, inline-script, and concurrency conformance tests.

### Gate

- A foreign module works on at least two targets.
- An inline `@script` block works on JS and native, and unsupported language/target
  pairs fail before code generation.
- Structured tasks are scoped.
- Worker messages are serialized correctly.

---

## 12. Phase 10 — Hardening

**Goal:** Stabilize v0.1.

### Steps

1. Freeze the grammar.
2. Run the full conformance suite.
3. Audit diagnostics.
4. Audit generated-code readability.
5. Audit memory behavior.
6. Audit mode inference.
7. Audit package security.
8. Write migration and contribution docs.
9. Tag the release.

### Gate

- Conformance suite passes on all supported targets.
- No known semantic-contract violations.
- The spec matches the implementation.
- The release is reproducible.

---

## 13. Per-session workflow

```text
Read state
  → Choose one task
  → Restate the contract
  → Check decisions
  → Plan
  → Change
  → Verify
  → Record
  → Report
  → Stop
```

### Start of session

- [ ] Read `state-log.md`
- [ ] Read `todo.md`
- [ ] Read the relevant architecture section
- [ ] Confirm the current phase
- [ ] Select one task

### End of session

- [ ] Task status is accurate
- [ ] Evidence is recorded
- [ ] State log updated
- [ ] Todo updated
- [ ] Next action explicit

---

## 14. Handoff format

At the end of a session, leave:

```markdown
### Handoff
- Phase:
- Task:
- Status:
- Files changed:
- Verification:
- Blockers:
- Next action:
```

A future agent should be able to resume without reading the full conversation.
