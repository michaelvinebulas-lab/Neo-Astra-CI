# V1 Quality, Security, and Delivery

**Status:** V1 design proposal  
**Depends on:** D1–D25  
**Related:** [`README.md`](./README.md), [`06-tooling-and-workflow.md`](./06-tooling-and-workflow.md)

---

## 1. Quality strategy

V1 quality is proven by behavior, not by feature count.

The three evidence layers are:

1. **Specification** — the rule is written.
2. **Conformance** — the rule is tested across targets.
3. **Diagnostics** — violations fail clearly.

A feature without all three is not V1-ready.

---

## 2. Conformance strategy

### 2.1 Portable conformance

Portable features must pass on every Tier 1 target:

- JavaScript
- TypeScript
- Windows x64 native
- Linux x64 native
- macOS arm64 native

Tier 2 targets must pass their documented subset.

### 2.2 Conformance case format

Each case has:

- Source
- Manifest
- Target list
- Expected output or behavior
- Expected diagnostics for invalid cases
- Timeout
- Notes for target-specific behavior

```text
tests/conformance/strings/utf8-len/
  main.ns
  core.json
  expected.json
```

### 2.3 Required conformance areas

- Integer wrapping
- Checked integers
- Arbitrary integers
- String byte length
- Unicode scalar count
- String/bytes conversion
- Array bounds
- Map order
- Option and Result
- Error propagation
- Error unions
- Panic behavior
- Mode selection
- Capability inference
- Source tags
- Scoped blocks
- Memory policy precedence
- Manual memory rejection on JS
- Task scoping
- Channel send/receive
- Channel close
- Cancellation
- FFI strings
- FFI errors
- FFI ownership
- Inline script export checking
- Inline script target/language rejection
- Inline script raw-body formatter preservation
- Package resolution
- Tree-shaking metadata

---

## 3. Compatibility policy

### 3.1 Language editions

- V1 is language edition `1.0`.
- The manifest `edition` field selects language behavior.
- A compiler may support multiple editions.
- Breaking language changes require a new edition.

### 3.2 Compiler versions

- Patch releases fix bugs without changing semantics.
- Minor releases add features without breaking V1 source.
- Major releases may change semantics only with a new edition.

### 3.3 Manifest schema

- Schema changes are versioned.
- Adding optional fields is a minor change.
- Removing or changing fields is a major change.
- The compiler rejects newer unsupported schemas.

### 3.4 Diagnostics

- Codes are stable.
- Message wording may improve.
- A code is never reused for a different meaning.
- Removing a diagnostic code requires a deprecation note.

### 3.5 Runtime ABI

- The native/Wasm runtime ABI is internal in V1.
- Generated code and runtime must be version-matched.
- A stable public ABI is post-V1.

### 3.6 Package adapters

- Adapter behavior is part of the build contract.
- Changing resolution rules requires a migration note.
- Lockfile format changes require a migration path.

---

## 4. Performance budgets

Initial V1 targets:

| Area | Budget |
|---|---|
| Cold check, 10k lines | under 2 seconds |
| Warm check, 10k lines | under 500 ms |
| CLI startup | under 150 ms |
| Peak compiler memory, 50k lines | under 512 MB |
| LSP diagnostics after edit | under 200 ms |
| Native hello world | under 64 KB stripped |
| Wasm hello world | under 32 KB after `wasm-opt` |
| JS hello world | under 2 KB gzipped |
| GC runtime | under 32 KB native |
| Concurrency runtime | under 24 KB native |
| Typical JS helpers | under 4 KB gzipped |

Budgets are baselines. A missed budget must be measured, documented, and accepted consciously.

---

## 5. Security model

### 5.1 Trust boundaries

| Boundary | Risk |
|---|---|
| Source → compiler | Malformed input, denial of service |
| Manifest → build | Path escape, unexpected tool execution |
| Package source → project | Supply-chain compromise |
| Nova runtime → host | Capability escape through development commands |
| FFI → native | Memory unsafety, ownership bugs |
| Inline `@script` → host | Unchecked foreign code, lying mode declaration, build-time tool execution |
| WASM → host | Capability escape, unsafe imports |
| Web Worker → app | Message spoofing, serialization bugs |
| Compiler API → CLI | Malformed protocol messages |
| Playground → host | Remote code execution, data leakage |

### 5.2 Supply chain

- Lifecycle scripts off by default.
- Integrity hashes required for remote packages.
- Lockfile required for release builds.
- Git dependencies pinned to commits.
- Path dependencies restricted to the project root unless explicitly allowed.
- Dependency graph diff shown in CI.
- Exports and re-exports do not launder capabilities; requirements flow through the export graph.

### 5.3 Compiler input

- Parser must handle malicious input without crashing.
- Diagnostics must not execute source.
- Manifest parsing must reject path traversal.
- Build scripts are not allowed to run from the manifest.
- The compiler must not fetch arbitrary URLs during `check`.

### 5.4 FFI

- FFI is unsafe by default.
- Ownership and threading must be declared.
- Missing mappings are errors.
- Foreign exceptions must not unwind into Neo Astra code.
- FFI pointers must not be captured by GC-managed closures without an explicit boundary.
- Inline `@script` bodies are opaque, unsafe boundaries. Their `@mode` declaration
  is trusted, not verified.
- `astra check` and `astra fmt` must not execute foreign bodies.
- C/C++ inline scripts compile through the declared native toolchain; the build
  summary must record that toolchain and every generated translation unit.
- `nova` must reject inline scripts until a sandboxed host adapter is approved.

### 5.5 Wasm

- Host imports are allowlisted by capability.
- WASI capabilities are limited to what the program declares.
- Memory growth limits must be configurable.
- The browser target must not claim WASI support by default.

### 5.6 Web Workers

- Messages are validated and typed.
- Transferables are explicit.
- The main thread must not be blocked by CPU work.
- Worker termination must clean up tasks.

### 5.7 Secrets

- Never write secrets to build metadata.
- Never include secrets in generated source maps.
- Never print environment variables wholesale.
- Redact credentials in diagnostics.

### 5.8 Playground

- Local compilation is preferred.
- Remote compilation must be disclosed.
- Uploaded source must not be retained without consent.
- The sandbox must not expose host credentials.

---

## 6. Error handling and observability

- Every compiler failure has a stable code.
- Every build writes a machine-readable summary.
- Build logs distinguish user errors from internal errors.
- Panics in the compiler are internal errors and must not be presented as user syntax errors.
- `astra explain` records inference decisions.
- CI stores conformance and performance reports.

---

## 7. Delivery plan

### V1.0-alpha

Goal: prove the language and compiler loop.

- Specification draft
- Rust core lexer/parser
- Type checker subset
- JS backend
- CLI `check`, `build`, `run`
- Playground prototype
- Basic conformance suite

Exit criteria:

- Core programs run in the browser.
- Diagnostics are stable enough to test.
- No second parser exists.

### V1.0-beta

Goal: prove multi-target consistency.

- Native C backend
- Wasm backend
- GC runtime
- Manual memory
- Structured concurrency
- FFI
- Inline `@script` interop, JS first and native second
- Manifest and lockfile
- LSP diagnostics and formatting

Exit criteria:

- Tier 1 conformance passes.
- Manual memory is rejected on JS.
- FFI works on at least two targets.
- Inline `@script` works on JS and native, or is explicitly deferred with a
  stable error path.

### V1.0-rc

Goal: freeze and harden.

- Freeze language edition 1.0
- Freeze manifest schema
- Freeze diagnostic codes
- Complete documentation
- Security review
- Performance budgets measured
- Release build reproducible

Exit criteria:

- All release blockers closed.
- Known issues documented.
- Migration notes complete.

### V1.0

Goal: stable release.

- Tier 1 targets pass.
- Tier 2 targets pass their documented subset.
- Packages resolve reproducibly.
- Tooling is usable without a private setup.
- Release artifacts are signed or checksummed.

---

## 8. Risk register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Semantic drift across targets | High | High | Conformance suite, shared IR, explicit boundaries |
| GC complexity | High | High | Non-moving mark-sweep first; task-local heaps |
| Web Worker serialization cost | Medium | Medium | Document cost; use workers for CPU tasks only |
| FFI ownership bugs | High | High | Explicit metadata, boundary tests, unsafe marking |
| Opaque inline scripts hide capability or safety violations | High | High | Explicit `@mode`, unsafe marking, boundary tests, no Nova execution, target allowlist |
| Package supply-chain attack | Medium | High | No lifecycle scripts, integrity hashes, lockfile |
| Compiler boundary drift | Medium | High | Versioned API, Rust-only parser, protocol tests |
| Scope creep | High | High | V1 non-goals, phase gates, release criteria |
| JavaScript output becomes unreadable | Medium | Medium | Golden output tests, readability review |
| Manual memory misuse | High | High | Debug checks, unsafe boundaries, documentation |
| Performance misses budgets | Medium | Medium | Measure early, publish baselines |
| LSP lags behind CLI | Medium | Medium | Shared diagnostics, same core, integration tests |
| Manifest becomes a junk drawer | Medium | Medium | Strict schema, unknown-key rejection |
| Wasm threads unavailable | Medium | Medium | Worker fallback, feature detection |
| PHP backend distorts core | Low | High | Keep PHP post-V1 and boundary-limited |

---

## 9. Release checklist

### Language

- [ ] Edition 1.0 frozen
- [ ] Grammar complete
- [ ] Semantics matrix complete
- [ ] Error model complete
- [ ] Mode rules complete
- [ ] Memory rules complete
- [ ] Concurrency rules complete
- [ ] FFI rules complete
- [ ] Standard library surface documented

### Compiler

- [ ] Rust core passes frontend tests
- [ ] Compiler API versioned
- [ ] JS/TS backends pass golden tests
- [ ] C backend passes native tests
- [ ] Wasm backend passes host tests
- [ ] Diagnostics stable
- [ ] Build cache safe

### Tooling

- [ ] `astra check`
- [ ] `astra build`
- [ ] `astra run`
- [ ] `astra test`
- [ ] `astra fmt`
- [ ] `astra explain`
- [ ] `astra ast`
- [ ] `nova run` / `nova repl` / `nova watch` / `nova test`
- [ ] `nopm add` / `remove` / `install` / `update` / `lock` / `tree` / `why`
- [ ] `astra lsp`
- [ ] Playground
- [ ] Documentation generation

### Project system

- [ ] Manifest schema frozen
- [ ] Lockfile schema frozen
- [ ] Universal package IDs and descriptor
- [ ] npm adapter
- [ ] Cargo adapter
- [ ] Path and git packages
- [ ] FFI mappings
- [ ] Integrity checks
- [ ] Lifecycle scripts disabled by default
- [ ] Optional `registry.neovastra.dev` gateway

### Quality and security

- [ ] Tier 1 conformance passes
- [ ] Tier 2 subset passes
- [ ] Performance budgets measured
- [ ] Security review complete
- [ ] Supply-chain policy enforced
- [ ] Manual memory JS rejection tested
- [ ] FFI boundary tests pass
- [ ] Release artifacts checksummed

### Documentation

- [ ] Language guide
- [ ] Specification
- [ ] CLI reference
- [ ] Manifest reference
- [ ] FFI guide
- [ ] Migration guide
- [ ] Known issues

---

## 10. Open delivery questions

1. Which Tier 2 targets are release-blocking versus best effort?
2. What is the exact V1 support window for compiler releases?
3. Are release binaries signed, checksummed, or both?
4. What is the policy for breaking diagnostic message changes?
5. How long is the V1 language edition supported?
6. Who owns security response for package adapters and the Nova package registry?
7. What is the minimum browser baseline?
8. What is the minimum `clang` version?
9. Is the playground local-only at launch?
10. What is the first post-V1 feature: PHP, LLVM, or traits?
