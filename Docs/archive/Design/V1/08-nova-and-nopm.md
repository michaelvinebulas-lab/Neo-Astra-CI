# V1 Nova Runtime and NoPM

**Status:** V1 design proposal  
**Depends on:** D3, D4, D10, D11, D15, D19, D25  
**Related:** [`02-compiler-design.md`](./02-compiler-design.md), [`03-runtime-and-memory.md`](./03-runtime-and-memory.md), [`05-manifest-and-packages.md`](./05-manifest-and-packages.md), [`06-tooling-and-workflow.md`](./06-tooling-and-workflow.md)

---

## 1. Purpose

Neo Astra needs two developer-facing layers that are not part of the compiled production runtime:

- **Nova** — a development runtime that executes `.ns` source directly.
- **NoPM** — a universal package manager that resolves dependencies from multiple ecosystems for both Nova and Astra builds.

Nova exists so a developer can create a file, run it, and see behavior without running a full build. NoPM exists so the same project can use packages from path, Git, npm, and Cargo without forcing those ecosystems into one package format.

Neither layer creates a second language, a second parser, or a second semantic authority.

---

## 2. Nova

### 2.1 What Nova is

Nova is a **development interpreter** for typed Neo Astra HIR.

It is not:

- A replacement for `astra build`
- A production deployment target in V1
- A second parser
- A separate type system
- A promise that interpreted performance matches compiled output

The production promise remains:

> **Zero VM, pay-as-you-go runtime.**

Nova is a development tool. The compiled JS, native, and Wasm targets remain the deployment story.

### 2.2 Execution pipeline

```text
.ns source
  → Rust lexer
  → Rust parser
  → Rust resolver
  → Rust type checker
  → typed HIR
  → Nova interpreter
```

Nova must not read `.ns` source with its own grammar. It consumes the same frontend and HIR used by later backends.

### 2.3 Interpreter model

The first Nova engine is a **tree-walking interpreter over typed HIR**.

Why this order:

- It ships before a bytecode VM.
- It validates the semantic contract early.
- It gives the compiler team a runnable oracle for examples and conformance cases.
- It keeps the interpreter behind a runtime interface so a bytecode engine can replace it later.

The interpreter has:

- A value stack and lexical environment chain.
- Explicit call frames.
- Structured error propagation using the language `!` model.
- Panic/abort handling.
- Host-managed memory for development values.
- Deterministic execution for tests where no host capability is involved.

Nova V1 does not need to match native performance, manual-memory edge cases, or full FFI behavior. It does need to match language semantics for the features it claims to support.

#### `NOVA-001` implementation status

The first engine in `crates/astra-nova` implements:

- Unit, Boolean, fixed-width integer, floating-point, `str`, `bytes`, array, map,
  tuple, struct, enum, `Option`, `Result`, function, constructor, and task values.
- Lexical environments with shadowing, mutable bindings, lazy top-level
  constants, explicit call frames, `defer`, and reverse-order scope cleanup.
- Functions, recursion, struct construction and field mutation, tuple struct
  constructors, enum variants, indexing, members, and insertion-ordered maps.
- `if`, `while`, `for`, `break`, `continue`, `match`, tuple patterns, Option and
  Result patterns, and enum-variant patterns.
- Error propagation through `!`, fallible calls exposed to `match` as `Ok`/`Err`,
  panic, and abort. Panics carry an interpreted call stack.
- The default wrapping integer policy and an opt-in safe-checked policy, plus
  explicit wrapping and checked integer methods.
- A deterministic single-threaded task stub. `spawn || { ... }` runs eagerly,
  `join` observes the result, and an unjoined task panic fails its lexical scope.
- Preflight rejection of every `@memory:manual` scope, even in system mode.

Boundaries for this milestone:

- One checked module executes at a time. Imported values and foreign modules fail
  with a clear `NV0003` diagnostic when execution reaches them; project-wide
  module linking belongs to `NOVA-003`. The implementation design for linking is
  recorded in `09-nova-project-linking.md`; its prerequisites, `TYPE-003`
  project type surfaces and the `TYPE-004` multi-source compiler API, are
  complete.
- Channels, OS threads, Web Workers, and real FFI host adapters are not implemented.
- `usize` and `isize` use the 64-bit development model.

Runtime failures use stable `NV0001`-`NV0007` codes defined in the specification.
Frontend failures still use the same `NSxxxx` diagnostics as `astra check`.

### 2.4 Strictness

Nova parses and resolves every program. Type checking is on by default.

A future `--no-typecheck` escape hatch is allowed only as an explicit development option. It must never become the default and must not be used by `astra build` or release tests.

### 2.5 Modes and capabilities

Nova enforces the same mode and capability pipeline as the compiler:

1. CLI flag
2. `NOVA_MODE` / `ASTRA_MODE`
3. target-bound mode
4. manifest default
5. least-privileged inference

Nova must reject a capability that the selected mode does not grant. A development runtime is not a security bypass.

`NovaOptions::infer_mode` defaults to `true`. `nova run`, `eval`, `repl`,
`watch`, and `test` resolve the source mode before checking and interpreting;
`--mode` disables inference and remains authoritative. JSON reports carry the
resolved mode, while inference failures use the same `NS5001`/`NS5006`
diagnostics as the compiler. `resolve_source_mode` exposes the same decision to
library callers without executing the program.

### 2.6 Memory and concurrency

Nova V1 prioritizes the portable managed subset:

- GC-managed values use the interpreter's host memory.
- Manual-memory blocks are rejected with a clear diagnostic until Nova has a checked dev allocator.
- Structured tasks start with a single-threaded cooperative scheduler.
- OS-thread and Web Worker mapping is a later Nova milestone.
- FFI calls are staged behind explicit host adapters.

This means Nova is useful for language development, scripts, tests, and most app logic before full system/runtime support lands.

### 2.7 Commands

```text
nova run <file.ns> [--watch] [--json]
nova repl [--json]
nova test [path] [--watch]
nova eval "<source>"
nova version
```

`nova run` uses the manifest `[nova].entry` when no file is given.

`nova watch` re-parses and re-runs on source changes. It does not claim hot module replacement.

`nova test` is the development test runner. The compiler and conformance suite may also use it, but they must not depend on interpreted performance for release timing.

The `NOVA-002` CLI now implements `nova run`, `nova eval`, `nova repl`, `nova watch`, `nova test`, `nova version`, and manifest `[nova].entry` discovery, with mode selection, text diagnostics, and JSON reports. Project-wide linking and richer test filtering remain.

The first NoPM stack is implemented in `orchestrator/packages/nopm`: source-qualified package IDs, descriptors, deterministic in-memory graph resolution, target filtering by name/backend/triple, capability-derived mode checks, cycle detection, structured diagnostics, canonical `core.lock.json` creation/formatting/parsing/validation, lockfile I/O and staleness checks, a content-addressed store, and `nopm lock`, `nopm tree`, `nopm install`, `nopm search`, `nopm audit`, `nopm why`, and `nopm update`.

Source adapters cover local path packages, Git packages pinned to commits, npm metadata/tarballs with lifecycle scripts disabled, Cargo ranges and exact versions through crates.io, PyPI distributions, NuGet packages, and OCI distributable layers. The optional registry client and an in-process HTTP gateway are implemented in `@neo-astra/registry` with publish, versions, search, yank, audit, Ed25519 signing/verification, and upstream mirroring. Production key management, hosted federation hardening, `add`/`remove`/`publish` CLI flows, and artifact hosting remain future work.

### 2.8 Relationship to `astra`

- `astra` compiles, checks, formats, and builds.
- `nova` executes source in development.
- `astra run` remains the command that builds and runs a configured production target.
- `nopm` manages packages for both.

The commands are separate so the user does not confuse interpreted development with a production build.

### 2.9 Inline script blocks

Nova type-checks the declared `@exports` surface of an inline `@script` block, but
it cannot execute the foreign body. In V1, any program that reaches a script call
fails with `NV0008` before execution:

> foreign script `math` requires a production target

This is deliberate. Running JavaScript, Python, or native code from the
development interpreter would require a host runtime, a sandbox, and a separate
capability model. Those are not part of `NOVA-001`–`NOVA-003`.

A later Nova release may support a specific embedded runtime. That requires a new
decision covering packaging, sandboxing, and capability reporting. The inline
script design is in [`10-inline-script-interop.md`](./10-inline-script-interop.md).

---

## 3. NoPM

### 3.1 What NoPM is

NoPM is the **Nova Package Manager**, a universal resolver and package client.

It is not a replacement for npm or Cargo. It sits above them:

- It understands their metadata.
- It records exact resolutions and integrity hashes.
- It gives Neo Astra a common dependency graph.
- It exposes typed bindings or FFI mappings for foreign packages.

### 3.2 First sources

V1 supports:

- `path`
- `git`
- `npm`
- `cargo`
- `pypi`
- `nuget`
- `oci`

Each ecosystem is supported only when its adapter exists and the package has a compatible target binding.

### 3.3 Package specifications

```text
nova:@scope/name@1.2.3
npm:react@19
cargo:serde@1
git+https://example.com/repo.git#v1.2.0
path:../local-package
```

Rules:

- Source-qualified IDs are canonical in the lockfile.
- Unqualified names use the `[registry].defaultSource` policy.
- A package ID is not a URL by itself; the adapter resolves it through the configured registry.
- Exact versions are pinned in `core.lock.json`.

### 3.4 Universal package descriptor

Every adapter produces the same internal descriptor:

```json
{
  "id": "npm:react@19.0.0",
  "name": "react",
  "source": "npm",
  "version": "19.0.0",
  "integrity": "sha256-...",
  "targets": ["web", "typescript"],
  "capabilities": ["dom"],
  "dependencies": [],
  "bindings": {
    "web": { "kind": "esm", "entry": "index.js" }
  },
  "build": null,
  "scripts": "disabled",
  "license": "MIT",
  "metadata": {}
}
```

The descriptor is the boundary between ecosystem-specific metadata and Neo Astra’s target/mode-aware compiler.

### 3.5 Resolution pipeline

```text
specs
  → fetch metadata
  → resolve versions
  → apply target/mode filters
  → resolve transitive graph
  → verify integrity
  → collect capabilities and FFI mappings
  → build or fetch target variants
  → write core.lock.json
```

Rules:

- Resolution is deterministic.
- Target and mode constraints are applied before download when possible.
- Lifecycle scripts are disabled by default.
- Native build steps are declared, not inferred from arbitrary scripts.
- A package that cannot satisfy the requested target fails early.
- The lockfile records why a variant was selected.

### 3.6 Lockfile

`core.lock.json` remains the only lockfile. It records:

- Universal package IDs.
- Source registry and adapter.
- Exact versions.
- Integrity hashes.
- Transitive graph.
- Target variants.
- Capabilities and FFI metadata.
- Compiler and manifest versions.
- Registry origin and mirror information.
- Whether lifecycle scripts were disabled.

The lockfile must be reproducible and reviewable.

### 3.7 Store

```text
.astra/
  store/              # project-local content-addressed packages
  toolchains/         # pinned compiler and Nova versions
  cache/              # build and evaluation caches
  env/                # resolved environment metadata
```

A global store may exist for sharing downloads. Content-addressed entries are immutable.

### 3.8 Commands

```text
nopm add <spec> [--target <name>]
nopm remove <name>
nopm install [--locked|--frozen]
nopm update [name]
nopm lock
nopm tree
nopm why <name>
nopm audit
nopm vendor
nopm publish
nopm search <query>
```

`--locked` requires the lockfile to match the manifest. `--frozen` also forbids writing dependency metadata during CI.

### 3.9 Security

- Lifecycle scripts are disabled by default.
- Package adapters run with least privilege.
- Downloaded artifacts are integrity-checked.
- Native build steps are explicit and isolated.
- Capability requirements are recorded and checked by modes.
- Publishing to the Astra registry requires signatures.
- `nopm audit` reports known-vulnerable or yanked packages.

---

## 4. Universal registry

### 4.1 Model

The registry is a **federated package gateway**, not a walled garden.

It provides:

- Hosting for Neo Astra packages.
- Metadata and proxy resolution for npm, Cargo, and later ecosystems.
- Mirrors and offline caches.
- Signing, yanking, and audit metadata.

Local development does not require the registry. NoPM can talk directly to upstream registries when configured.

### 4.2 Namespaces

```text
nova:@scope/name@version
npm:name@version
cargo:name@version
pypi:name@version
nuget:Name@version
git+https://...#ref
```

Neo Astra packages use the `nova:` namespace. They are not allowed to masquerade as another ecosystem’s package.

### 4.3 Gateway API

Proposed endpoints:

```text
GET  /v1/packages/:id
GET  /v1/packages/:id/versions
GET  /v1/packages/:id/:version
GET  /v1/search?q=
POST /v1/publish
POST /v1/yank
GET  /v1/audit/:id
```

The exact API is an implementation detail until `NOPM-003`.

### 4.4 Publishing

A published Nova package contains:

- `core.toml` or `core.json`
- Source files.
- Build metadata.
- Target support declarations.
- Capability declarations.
- FFI exports, if any.
- Integrity hashes and signature.

Package names are scoped. A package can be yanked but not silently replaced.

---

## 5. Manifest extensions

The manifest remains exactly one of `core.json` or `core.toml`.

```toml
[nova]
entry = "src/main.ns"
watch = ["src/**/*.ns"]

[registry]
default = "https://registry.neovastra.dev"
defaultSource = "nova"

[packageSources]
path = true
git = true
npm = true
cargo = true

[dependencies.ui-kit]
source = "npm"
version = "0.4"
targets = ["web", "typescript"]

[dependencies.math]
source = "cargo"
version = "1.2"
targets = ["native", "wasm-wasi"]

[dependencies.local]
source = "path"
path = "../local"
```

Rules:

- `[nova]` is development-runtime configuration, not a second manifest.
- `[registry]` configures the default registry and source policy.
- `[packageSources]` enables adapters.
- Dependency entries may use source-qualified IDs.
- Unknown keys are still rejected.
- The lockfile records the resolved source and integrity data.

---

## 6. Relationship to the compiler roadmap

Nova and NoPM land after the resolver and type checker, before the JS backend:

```text
FMT-004 (done) → TYPE-001 (done) → TYPE-002 (done) → NOVA-001 (done) → NOVA-002 → NOPM-001 → NOPM-002 → JS-001
```

Rationale:

- The resolver and type checker must exist before Nova can consume typed HIR.
- Nova gives the team a runnable semantic oracle before code generation.
- NoPM can use the same resolved package graph for Nova and compiled targets.
- JS, native, and Wasm backends remain the production path.

---

## 7. Acceptance criteria

### Nova

- `nova run` executes a typed `.ns` program without a build step.
- `nova repl` evaluates expressions and preserves session state.
- `nova watch` re-runs on source changes.
- Parse, resolve, and type errors are reported with the same codes as `astra check`.
- Mode and capability violations fail with structured diagnostics.
- No second parser or type checker exists.

### NoPM

- `nopm add` resolves path, Git, npm, and Cargo packages.
- `core.lock.json` is deterministic and reproducible.
- `--locked` and `--frozen` work in CI.
- Lifecycle scripts are off by default.
- A package incompatible with the selected target fails with a clear diagnostic.
- The same lockfile serves Nova and `astra build`.

### Registry

- The gateway can resolve at least one upstream ecosystem without acting as a mandatory proxy.
- Neo Astra packages can be published under the `nova:` namespace.
- Mirrored packages retain integrity metadata.
- Local development works without the registry.

---

## 8. Open questions

1. When should Nova add a bytecode engine, and is it worth the extra semantic surface?
2. How much concurrency should Nova support before the compiled runtime lands?
3. Should Nova support FFI in V1, or only standard-library host functions?
4. What is the first registry deployment target, and who operates it?
5. How are native package build recipes sandboxed and reproduced?
6. Should NoPM support lockfile-less development installs, or require a lockfile always?
7. How should package signatures and transparency logs work for the Astra registry?
8. What is the exact package format for a published Neo Astra package?
9. Should Nova ever execute inline `@script` blocks through a sandboxed host runtime?
