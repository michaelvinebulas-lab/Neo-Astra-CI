# Neo Astra V1 — Design

**Status:** V1 design proposal  
**Date:** 2026-10-24  
**Product:** Neo Astra  
**Language version:** 1.0  
**Source extension:** `.ns`  
**CLI:** `astra`, with `nova` and `nopm`  
**Related architecture:** [`../../architecture.md`](../../architecture.md)  
**Agent operating system:** [`../../Agent/agent.md`](../../Agent/agent.md)

---

## 1. What V1 means

**V1 is the first stable release of the language and compiler.**

It is not the same thing as the earlier **v0.1 implementation slice**. The v0.1 slice is the first runnable path: parser, type checker, interpreter, and JavaScript backend. V1 is the release that makes Neo Astra usable as a real multi-target language.

V1 must deliver:

- A stable language specification
- A working compiler with a shared Rust core
- JavaScript and TypeScript output
- Native and Wasm output through the C backend
- GC by default and manual memory where allowed
- Effect-tracked errors
- Structured concurrency
- Polymorphic FFI
- Manifest and lockfile
- Nova development runtime for direct `.ns` execution
- NoPM universal package manager with path, Git, npm, and Cargo sources
- CLI, formatter, diagnostics, and `astra explain`
- A conformance suite that proves cross-target behavior

V1 does not need to deliver every future idea. It does need to avoid architectural dead ends.

---

## 2. Relationship to the confirmed decisions

The V1 design is bound by D1–D25 in [`../../architecture.md`](../../architecture.md). Those decisions are not reopened here.

This folder specifies **how** the confirmed decisions are realized. If a design detail conflicts with a confirmed decision, the confirmed decision wins.

New design choices in these documents are proposals until they are recorded in the architecture decision log.

### Hard invariants

1. One semantic contract across targets.
2. One parser, owned by the Rust core.
3. Modes are compile-time capability layers.
4. `math.integers` and `strings.model` are semantic policies, not modes.
5. GC is the default; manual memory is native/Wasm only.
6. JavaScript rejects manual-memory files.
7. Only `//` comments are valid in `.ns` source.
8. Exactly one of `core.json` or `core.toml`.
9. Recoverable errors use `!`; programmer bugs use `panic`/`abort`.
10. Structured concurrency uses scoped tasks and channels.
11. FFI is polymorphic and manifest-resolved.
12. Declaration visibility is optional; private declarations cannot be exported.
13. Imports and exports are compile-time boundaries and never bypass capabilities.
14. “Zero-runtime” is not a claim. The claim is “zero VM, pay-as-you-go runtime.”
15. Nova is a development interpreter. It is not the production runtime and never replaces the shared parser or semantic contract.

---

## 3. V1 goals

| Goal | V1 outcome |
|---|---|
| Language | Stable syntax and semantics for core, app, web, and system code |
| Compiler | TypeScript orchestrator + Rust core + modular backends |
| Web | Readable ESM and TypeScript output with source maps |
| Native | C backend through `clang`, with a native runtime |
| Wasm | Browser and WASI-oriented Wasm output |
| Memory | GC by default; explicit manual memory on native/Wasm |
| Errors | Effect-tracked errors with target-specific representations |
| Concurrency | Scoped tasks and channels, mapped to threads and Web Workers |
| Interop | Polymorphic FFI and package-source adapters |
| Development runtime | Nova runs typed `.ns` source without a build step |
| Packages | NoPM resolves path, Git, npm, and Cargo packages into one lockfile |
| Tooling | CLI, formatter, LSP, playground, diagnostics, explain |
| Quality | Conformance suite, compatibility policy, security rules |

---

## 4. V1 non-goals

These are deliberately outside V1:

- Direct machine-code backend
- A hosted Astra registry as a V1 release blocker; the gateway is designed, but local NoPM works without it
- User-defined macros
- Ownership and borrow checking as the default model
- Runtime-switchable modes
- Full algebraic effect system
- PHP as a first-class launch target
- Hot reload across all targets
- Full incremental compilation
- Advanced typeclasses/traits
- Shared mutable memory in the portable concurrency model
- Seamless FFI claims

Some may land later. They must not compromise V1’s semantic contract.

---

## 5. Target support tiers

V1 uses tiers so “supports native” does not mean “supports every platform.”

| Tier | Targets | V1 commitment |
|---|---|---|
| Tier 1 | `js` (ESM), `typescript`, `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`, `aarch64-apple-darwin` | Release-blocking |
| Tier 2 | `wasm32-web`, `wasm32-wasi`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin` | Supported, best effort |
| Tier 3 | Other C-ABI platforms | Community or later |

The support matrix must be explicit in the manifest and diagnostics. A target that is not supported must fail with a clear error, not silently degrade.

---

## 6. Design principles

1. **Semantics before syntax.** A feature is not added until its behavior is defined on every supported target.
2. **Explicit boundaries.** Web, FFI, memory, and concurrency boundaries are visible.
3. **Strict at APIs, local where safe.** Inference reduces ceremony without hiding correctness.
4. **Generated code should be readable.** Web output should look like intentional ESM, not compiler sludge.
5. **One frontend.** No target-specific parser.
6. **Pay for what you use.** Runtime helpers are tree-shaken and explicit.
7. **Fail loudly.** A missing FFI mapping, unsupported target, or manual-memory JS build is an error.
8. **Explain the compiler.** `astra explain` is part of the product, not a debug afterthought.
9. **No false promises.** Document the cost of Web Workers, FFI, GC, and package interop.
10. **Design for AI and humans.** Regular grammar, structured diagnostics, stable AST, canonical formatting.

---

## 7. Document map

| Document | Purpose |
|---|---|
| [`01-language-design.md`](./01-language-design.md) | Syntax, types, semantics, errors, concurrency, modes, memory surface |
| [`02-compiler-design.md`](./02-compiler-design.md) | Orchestrator/core split, compiler API, pipeline, IR, caching, plugins |
| [`03-runtime-and-memory.md`](./03-runtime-and-memory.md) | Runtime model, GC, manual memory, errors, task runtime |
| [`04-targets-and-backends.md`](./04-targets-and-backends.md) | JS/TS, C/native, Wasm, target fidelity, generated-code rules |
| [`05-manifest-and-packages.md`](./05-manifest-and-packages.md) | Manifest, lockfile, dependencies, packages, FFI, tree-shaking |
| [`06-tooling-and-workflow.md`](./06-tooling-and-workflow.md) | CLI, LSP, formatter, playground, testing, CI |
| [`07-quality-security-and-delivery.md`](./07-quality-security-and-delivery.md) | Conformance, compatibility, security, performance, release criteria |
| [`08-nova-and-nopm.md`](./08-nova-and-nopm.md) | Nova development runtime, NoPM universal package manager, registry gateway, manifest extensions |
| [`09-nova-project-linking.md`](./09-nova-project-linking.md) | Project-wide module loading, canonical module identity, runtime linking, and diagnostics |
| [`10-inline-script-interop.md`](./10-inline-script-interop.md) | Inline `@script` foreign-language blocks, typed export surfaces, raw capture, and target support |

---

## 8. V1 release criteria

V1 is not “feature complete” in the abstract. It is done when all of these are true:

- The language specification is frozen for the 1.0 edition.
- Tier 1 targets pass the portable conformance suite.
- Tier 2 targets pass their documented subset.
- JavaScript output is readable, framework-free ESM.
- Native and Wasm output use the same core semantics.
- Manual memory is rejected on JS with a clear diagnostic.
- Structured concurrency passes native and browser tests.
- FFI works on at least two targets with typed bindings.
- Inline `@script` blocks work on JS and native, or are explicitly deferred in the
  release notes.
- The manifest and lockfile are reproducible.
- Nova runs typed `.ns` programs directly with the same diagnostics and capability rules as the compiler.
- NoPM resolves path, Git, npm, and Cargo packages into a deterministic `core.lock.json`.
- npm and Cargo package sources work with integrity checks and lifecycle scripts disabled by default.
- `astra check`, `build`, `run`, `fmt`, `explain`, and `ast` work.
- `nova run`, `nova repl`, `nova watch`, and `nopm install --locked` work.
- The LSP provides diagnostics and formatting for V1 syntax.
- The release build is reproducible.
- Known security risks are documented and mitigated or accepted.

---

## 9. How to change this design

A design change follows this order:

1. Identify the affected confirmed decision, if any.
2. If a confirmed decision must change, update `../../architecture.md` first.
3. Update the relevant V1 design document.
4. Update the conformance or acceptance criteria.
5. Record the change in `../../Agent/state-log.md`.

Do not change a design by editing only one file and leaving the others stale.

---

## 10. Current status

- Architecture decisions: D1–D25 confirmed
- V1 design: drafted, including Nova, NoPM, and proposed inline `@script` interop
- Language specification: drafted in `Docs/spec-v0.1.md`
- Compiler source: lexer, parser, formatter, resolver, type checker, compiler boundary, `astra fmt`, and manifest formatting implemented
- JavaScript backend: `JS-001` is in progress; the first single-module ESM slice consumes typed AST/HIR, supports the portable value/control-flow subset, and is wired into `astra build`
- Nova: the `NOVA-001` interpreter, `NOVA-002` CLI (`run`, `eval`, `repl`, `watch`, `test`, manifest entry), and `NOVA-003` project module loading/linking are implemented
- NoPM: complete stack through NOPM-004 - package IDs, descriptors, target/mode filtering, deterministic graph, lockfile model/I/O, content-addressed store, path/Git/npm/Cargo/PyPI/NuGet/OCI adapters, CLI lock/tree/install/search/audit/why/update, and registry gateway with signing/federation
- Conformance suite: not written
- Inline script interop: design proposed in `10-inline-script-interop.md`; implementation remains after the current backend work
- Next artifact: finish `JS-001` project imports/effects/concurrency and source maps, then `TS-001`, then `INTEROP-001`

V1 is a design baseline. It is not a claim that all of it is implemented.
