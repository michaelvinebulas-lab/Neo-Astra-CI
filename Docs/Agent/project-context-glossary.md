# Project Context and Glossary

This document gives the agent the minimum project context needed to avoid hallucinating decisions, mixing up concepts, or rebuilding something that was already settled.

---

## 1. Project snapshot

| Field | Value |
|---|---|
| Project | **Neo Astra** |
| Language | Neo Astra |
| Source extension | `.ns` |
| CLI | `astra` |
| Manifest | `core.json` or `core.toml` — exactly one |
| Lockfile | `core.lock.json` |
| Current status | `1.0.0` released. Native Rust `astra`/`nopm` port for `1.0.1` in progress: slices 1-2c landed; slice 3a open after two rejected handbacks; 3b/4 blocked; native parity 11/12. Language v0.2 is Phase 0 documentation/corpus only and non-normative. |
| Current phase | Post-1.0 native port; native NoPM slice 3a |
| Compiler architecture | Rust core and native CLI; TypeScript codegen/package work isolated behind an on-demand Node bridge |
| Primary targets | Native, Wasm, JavaScript/TypeScript |
| Later targets | PHP and other backends |
| Memory default | GC, configurable |
| Concurrency | Structured tasks and channels |
| Error model | Effect-tracked `!` propagation |

---

## 2. What Neo Astra is

Neo Astra is a universal, multi-target language for:

- Web applications
- High-level application code
- Low-level systems code

It is designed to compile to native binaries, WebAssembly, JavaScript/TypeScript, and later additional targets from one shared language and one shared semantic contract.

The language should be:

- Readable at low level
- Strict where correctness matters
- Ergonomic enough for application code
- Explainable to humans and tools
- Friendly to AI-assisted development

---

## 3. What Neo Astra is not

Neo Astra is not:

- A JavaScript framework
- A production runtime VM (Nova is a development interpreter, not the deployment runtime)
- A thin TypeScript wrapper
- A Rust clone with different keywords
- A language with separate grammars per target
- A language with runtime-switchable modes
- A language that hides fallible operations behind invisible exceptions
- A language that promises true zero-runtime while shipping a large runtime

The honest runtime promise is:

> **Zero VM, pay-as-you-go runtime.**

---

## 4. Canonical naming

Use these names consistently:

- **Neo Astra** — official project and language name
- **`.ns`** — source file extension
- **`astra`** — compiler and build CLI
- **`nova`** — development runtime and REPL
- **`nopm`** — NoPM, the universal package manager
- **`registry.neovastra.dev`** — universal registry gateway
- **`core.json` / `core.toml`** — manifest
- **`core.lock.json`** — lockfile

Do not reintroduce Omni, OmniScript, or any earlier name.

---

## 5. Core concepts

### Target

**Target** means what the compiler emits.

Examples:

- `native`
- `wasm`
- `js`
- `typescript`

A target is not a mode.

### Mode

**Mode** means which capabilities, standard-library surface, memory features, and target constraints are allowed.

Built-in modes:

- `core`
- `app`
- `web`
- `system`

Modes are compile-time only. They do not change core syntax or core semantics.

### Semantic policy

A **semantic policy** is an explicit manifest setting that changes observable language behavior.

Current policies:

- `math.integers`: `wrapping`, `safe-checked`, `arbitrary`
- `strings.model`: `utf8`, `native`

Semantic policies are not modes. They are recorded in build metadata.

### Capability

A **capability** is a permission or feature surface, such as:

- `dom`
- `fetch`
- `fs`
- `net`
- `threads`
- `ffi`
- `raw-memory`

Capabilities are granted by modes and declared by source tags.

### Source tag

A **source tag** declares what a module or function requires.

Example:

```ns
@requires(system)
fn read_file(path: str) -> bytes ! IoError {
    // system-only code
}
```

A source tag does not secretly change the project mode. The build fails if the selected mode does not provide the requirement.

### Scoped mode block

A **scoped mode block** narrows or explicitly widens capabilities inside a lexical region.

Example:

```ns
{@mode:core;
    // portable code
}
```

Nested blocks may narrow capabilities, but may not silently widen them beyond the parent context.

### Least privilege

**Least privilege** means the compiler selects the least powerful mode that satisfies the static requirements of the program.

Inference must be:

- Static
- Explainable
- Recorded in build metadata
- Overridden by explicit annotations

Requirements join by containment, not numeric rank. `app` contains `core`, `web`
contains `core` and `app`, and `system` contains `core` and `app`; `web` and
`system` are incomparable. A project requiring both fails with `NS5006`.
Requirements propagate through the local call graph to a fixed point, including
recursion. Compiler metadata records the resolved mode, its source, per-module
inferences, and a reason tree; a project build selects one mode for all modules.

### Viral requirement

A **viral requirement** means a requirement propagates to callers.

If a `core` function calls a `system` function, the caller must become system-required or use an explicit boundary. Requirements cannot be hidden.

### Effect-tracked error

An **effect-tracked error** is a typed, explicit failure that appears in the function signature.

Example:

```ns
fn parse_age(input: str) -> i32 ! InvalidAge | ParseIntError {
    if input.is_empty() {
        return !InvalidAge("Age cannot be empty")
    }
    return input.to_int()!
}
```

Call sites must use `!` to propagate a possible error. Recoverable errors are not invisible exceptions. Programmer bugs use `panic` or `abort`.

Each effect retains its construction or propagation span. Effects merge by
nominal type with deterministic ordering and duplicate elimination. Unhandled
effects are reported at their sites; an exhaustive `match` handles the scrutinee
effect, while a non-exhaustive match reports only `NS4010`. Imported error types
stay nominal through exported type surfaces, so an importing function can
declare and propagate the exact error type produced by another module.

### Structured concurrency

**Structured concurrency** means tasks are owned by their enclosing scope.

- `spawn` creates a scoped task.
- The parent waits for or cancels child tasks.
- Channels use message passing.
- Detached tasks and shared mutable state are not allowed in the portable core.
- Native maps tasks to an OS thread pool.
- Web maps tasks to Web Workers.

### FFI

**FFI** means calling foreign code.

Neo Astra uses polymorphic foreign modules:

```ns
foreign module math_engine {
    fn compute_matrix(data: [f64]) -> [f64]
}
```

The manifest maps the module per target:

- JavaScript/TypeScript: direct ESM imports
- Native: C ABI
- Wasm: imports/exports
- C++: requires a C shim

### Inline script block

An **inline script block** is the proposed `{@script:<language> as <name>; ... }`
form. It switches the parser into raw foreign-source mode and declares a typed
export surface:

```ns
{@script:js as math;
    @mode(core)
    @exports {
        fn add(a: i32, b: i32) -> i32
    }

    @impl
    export function add(a, b) {
        return a + b
    }
@end
}
```

The body is opaque to Nova parsing and checking. Calls are checked against
`@exports`; `@mode` is a trusted declaration, not a proof. The explicit `@end`
sentinel ends raw capture. JS/TS targets come first, C/C++ native second, Python
later, and Java only after a JVM backend exists.

It extends the foreign-module model; it does not replace `foreign module`,
manifest FFI mappings, or NoPM packages. See
[`../archive/Design/V1/10-inline-script-interop.md`](../archive/Design/V1/10-inline-script-interop.md).

### Tree-shaking

**Tree-shaking** means removing unreachable code and unused dependencies.

It should happen on the IR call graph, not only in the manifest. External packages may be opaque, so shaking claims must be honest.

### HIR and IR

- **HIR** — typed high-level intermediate representation
- **IR** — lower-level intermediate representation used for optimization and code generation

### Nova

**Nova** is the development runtime. It parses, resolves, type-checks, and
interprets typed `.ns` source without a build step. It is not the production
runtime and does not create a second parser or semantic model.

### NoPM

**NoPM** (Nova Package Manager) is the universal package manager. It resolves
path, Git, npm, Cargo, and later ecosystem packages into a single
`core.lock.json` graph. It is a client and resolver above existing ecosystems,
not a replacement for their registries.

### Universal package ID

A **universal package ID** is a source-qualified package identifier such as
`npm:react@19.0.0`, `cargo:serde@1.0`, or `nova:@scope/name@1.2.3`.

### Adapter

A **package adapter** maps one ecosystem’s metadata and artifacts into Neo
Astra’s universal package descriptor. An ecosystem is supported only when its
adapter exists.

### Canonical formatter

A **canonical formatter** means there is one expected formatting for a given syntax tree. `astra fmt` should be deterministic.

### Conformance test

A **conformance test** verifies that a language behavior is identical across targets, or that a documented target-specific boundary behaves as specified.

---

## 6. Confirmed decisions D1–D25

These are locked. Do not reopen them without an explicit user request and a recorded decision change.

| ID | Decision |
|---|---|
| D1 | GC by default; memory policy configurable by manifest, file, and block; manual memory only on native/Wasm; JS rejects manual files |
| D2 | Hybrid JavaScript output: exact semantics in core, JS-native behavior at explicit web boundaries |
| D3 | Hybrid compiler: Rust core engine plus TypeScript orchestrator |
| D4 | Whole-system architecture from day one; all components designed around shared interfaces |
| D5 | Custom syntax: Rust/JavaScript bareness with C/C++ sophistication |
| D6 | Fixed-width wrapping by default; `math.integers` can select `safe-checked` or `arbitrary`; explicit `+%`, `wrapping_add`, `checked_add` overrides |
| D7 | UTF-8 bytes are the canonical core string model; `strings.model` can map to native target strings |
| D8 | Effect-tracked explicit error bubbling with `!`; panic/abort for programmer bugs |
| D9 | Structured concurrency in the portable core: scoped tasks and channels, mapped to OS threads or Web Workers |
| D10 | Polymorphic manifest FFI: abstract `foreign module` mapped to npm/ESM or C ABI per target |
| D11 | Local packages plus native integration with npm, Cargo, and other package managers; no custom registry required initially |
| D12 | Official name **Neo Astra**; `.ns` source extension; `astra` CLI |
| D13 | Strict mode pipeline: `--mode` > `ASTRA_MODE` > target-bound mode > `defaultMode` > least-privileged inference |
| D14 | Modes change capabilities, standard library, memory features, and target constraints only; core syntax and semantics stay fixed |
| D15 | Infer least-privileged mode and target-specific dependencies from static imports and call graph; explicit annotations win; explain results |
| D16 | Human-readable and AI-friendly: explicit regular grammar, canonical formatter, structured diagnostics, machine-readable AST, explainable inference |
| D17 | Only `//` comments in `.ns` source; `core.toml` keeps standard TOML `#` comments |
| D18 | Support `core.json` or `core.toml`, exactly one; same strict semantic schema; lockfile `core.lock.json` |
| D19 | TypeScript orchestrator + Rust core engine + modular backend modules/crates |
| D20 | Strict at API boundaries, local inference, no implicit coercions, exhaustive matching, structured diagnostics |
| D21 | Memory precedence: block > file wrapper > manifest `memory.files` > manifest `memory.default` |
| D22 | Keyword aliases: `fn`/`function`, `let`/`var`, `const`/`constant`, `type`/`typedef`, `struct`/`record`, `mut`/`mutable`, `pub`/`public`; formatter emits the short form |
| D23 | Optional declaration visibility: normal by default, `pub`/`public`/`export` to export, `private` to forbid export; project-path `import path` / `import path as alias` and JS/TS import/export forms |
| D24 | Semicolon as an optional same-line separator and a required scope-header separator; no ASI |
| D25 | Nova interprets typed `.ns` HIR for development; NoPM manages universal packages; production stays compiled; manifest extended, not duplicated |

---

## 7. Architecture summary

### Compiler split

| Layer | Language | Responsibility |
|---|---|---|
| Orchestrator | TypeScript | CLI, manifest parsing, NoPM, plugins, LSP, JS/TS code generation |
| Core engine | Rust | Lexer, parser, resolver, type checker, capability inference, IR, Nova |
| Low-level backends | Rust | C, Wasm, LLVM, future direct machine code |
| Web backends | TypeScript | JavaScript ESM, TypeScript output |

Rust is the single source of truth for parsing and semantics. TypeScript must never re-parse `.ns` with a second grammar.

### Mode selection

```text
--mode
  > ASTRA_MODE
  > target-bound mode
  > build.defaultMode
  > least-privileged inference
```

The first four sources are explicit; they are authoritative and disable inference.
When none applies, the compiler infers the least-privileged mode. The selected
mode and whether it was explicit or inferred are recorded in build metadata.
After mode resolution, source tags, scoped blocks, and imported requirements are
validated. Nova JSON reports use the same resolved mode.

### Memory precedence

```text
block {@memory:...;}
  > file wrapper
  > manifest memory.files
  > manifest memory.default
```

### Manifest rules

- Exactly one of `core.json` or `core.toml`
- Strict schema
- Unknown keys rejected
- Duplicate keys rejected
- Wrong types rejected
- Invalid target, mode, memory, `math.integers`, or `strings.model` values rejected
- Lockfile is `core.lock.json`

---

## 8. Known tensions and caveats

These are known design tensions, not unresolved decisions. Do not “fix” them by silently changing semantics.

1. **UTF-8 core strings vs native JS strings** — representation swapping is efficient, but byte-level operations may require helpers or explicit semantics.
2. **Web Workers are not free** — structured concurrency maps cleanly to workers, but messages require serialization.
3. **Semantic policies vs modes** — `math.integers` and `strings.model` change observable behavior; modes do not. Keep the layers separate.
4. **D4 does not mean every backend ships on day one** — it means every target and interface is designed and scaffolded together, while runnable milestones can land in dependency order.
5. **Package interoperability is a core feature** — Neo Astra depends on existing ecosystems. Do not treat npm/Cargo integration as an optional afterthought.
6. **“Zero-runtime” is not literally true** — use “zero VM, pay-as-you-go runtime.”
7. **Nova is not the production runtime** — it is a development interpreter over typed HIR. Do not let it become a second semantic implementation.
8. **“Any ecosystem” means any adapter** — a package must still have target-compatible bindings; fail early instead of pretending otherwise.
9. **The universal registry is optional for local development** — NoPM must work against upstream registries without it.
10. **Cross-module checking is end-to-end in the API, not yet in Nova execution** — `astra-core` checks module graphs, and `astra-compiler-api` exposes the additive `check_project` method with per-module diagnostics and `moduleOrder`. `@neo-astra/compiler-client` exposes `checkProject()`. Nova's interpreter is still single-module, so use `check_project` for project checking and wait for `NOVA-003` for multi-module execution.
11. **Unwired modules are invisible** — a file in `src/` that is not declared in `lib.rs` (or `mod.rs`) is never compiled, so CI cannot catch it and it can rot indefinitely. Every source module must be reachable at all times; interrupted work must be marked `[~]` and logged in `../state.md`.

---

## 9. Current repository state

As of 2026-11-11:

- The repository is a git repository on branch `master`; it has remotes `ci`
  and `origin`. Do not push `origin` from this private tree; only
  `C:\Projects\Neo Astra-source` may push `origin`.
- `Docs/state.md` is the decision index and current status; `context.md` is the
  session handoff; `Docs/Report/Task.md` is the active lead/Kilo work order.
- `Docs/Agent/` is the agent operating system.
- `Docs/archive/Design/V1/` is the frozen V1 design baseline.
- `Docs/spec-v0.1.md` is the normative v0.1 language specification.
  `Docs/spec-v0.2.md` is a non-normative skeleton; v0.2 implementation has not
  started.
- `orchestrator/` contains the TypeScript CLI, codegen, and npm workspace.
- `crates/` contains the Rust compiler, capabilities, CLI, LSP, Nova, and the
  in-progress `astra-nopm` crate.
- `1.0.0` is published; tag `v1.0.0` targets `9d832e0`; public master is
  `17ceaf6`.
- Native Rust port commits: slice 1 `7ca69be`, slices 2a/2b `778bbc5`, slice 2c
  `501e91f`. Slice 3a is uncommitted Kilo work and has been rejected twice.
- Current tree is dirty: Kilo's `Cargo.toml`, `Cargo.lock`, `astra-cli`, and
  `astra-nopm` changes plus untracked `astra-nopm` sources/tests. Do not stage
  them. Pre-existing untracked `432+5`, `700`, `Docs/error-code-ledger.md`,
  `tmp.py`, and `tmp_err.txt` are also not yours.
- Last verified gates during review: `cargo fmt --all --check`, targeted clippy,
  `cargo test -p astra-nopm` (0 library + 1 SHA test), and native parity 11/12.
  The workspace test suite was green on the first handback; the current NoPM
  code is not accepted.
- `conformance/v0.2/` is deliberately outside the v0.1 runner. Do not move its
  fixtures into `conformance/cases` or `conformance/rejects` before the parser
  phase gate.
- npm trusted publishing is configured but unvalidated; `npm trust list` needs
  passkey re-auth and returns `EOTP`. The old token still needs revocation.
  `1.0.0` has no provenance and its GitHub release reports `immutable: false`.
- The LSP `serverInfo` still reports `2.0.0`; fix it before the next package
  that ships the server.

---

## 10. Quick glossary index

| Term | Meaning |
|---|---|
| Target | Output format or platform |
| Mode | Compile-time capability profile |
| Semantic policy | Manifest setting that changes observable behavior |
| Capability | Permission or feature surface |
| Source tag | Declaration of required capabilities |
| Scoped block | Lexical region with narrowed or explicitly widened mode |
| Least privilege | Minimal mode that satisfies static requirements |
| Mode source | Whether the selected mode was explicit or inferred |
| Viral requirement | Requirement that propagates to callers |
| Keyword alias | Accepted spelling variant normalized by the canonical formatter |
| Soft separator | `;` used to separate related statements or items on one line; a newline is the normal boundary |
| Normal declaration | Module-local declaration that can be exported explicitly |
| Public/exported declaration | Declaration included in the module export surface |
| Private declaration | Module-local declaration that cannot be exported or imported |
| Export surface | The names a module exposes to importers |
| Type-only export | A compile-time export erased from runtime output |
| Default export | The single `default` export of a module |
| Re-export | Exporting a binding that came from another module |
| Effect-tracked error | Typed explicit failure with `!` |
| Effect provenance | Source span of a construction or propagation site |
| Structured concurrency | Scoped tasks and channels |
| FFI | Foreign function interface |
| Tree-shaking | Removing unreachable code and dependencies |
| HIR / IR | Typed high-level / lower-level intermediate representation |
| Nova | Development interpreter for typed `.ns` source |
| NoPM | Universal package manager for Nova and compiled builds |
| Universal package ID | Source-qualified package identifier such as `npm:react@19` |
| Adapter | Ecosystem-specific bridge into the universal package descriptor |
| Registry gateway | Optional service that hosts `nova:` packages and proxies upstream registries |
| Conformance test | Cross-target behavior test |
