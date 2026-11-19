# Orchestrator v2 - implementation language decision

**Status:** Accepted direction; V2 only; not normative for v1.
**Date:** 2026-11-19
**Decision:** The V2 orchestrator will be written in **C++20**.
**C:** C remains only for the ABI boundary and leaf libraries. It is not the
orchestrator implementation language.
**Core:** The language core, compiler API, C codegen, and Nova runtime remain
Rust.
**Current v1:** Rust core plus the TypeScript orchestrator/oracle and the
isolated JS/TS build bridge.

## 1. Decision

The V2 orchestrator is a C++20 codebase. The existing TypeScript orchestrator
remains the v1 implementation and the behavioral oracle until each C++ component
passes parity.

The Rust core is not rewritten in C++. The split is:

- **Rust:** parser, checker, compiler API, C codegen, Nova, native NoPM core,
  security-sensitive package/archive primitives where already implemented.
- **C++20:** V2 orchestrator command layer, manifest/registry/toolchain logic,
  NoPM orchestration, packaging helpers, and the JS/TS bridge host if it is
  ported.
- **TypeScript:** v1 oracle and JS/TS codegen bridge until deliberately
  replaced. It must not grow new runtime features once the V2 port starts.

## 2. Why C++ rather than C

C++ gives the orchestrator the tools it actually needs:

- RAII and deterministic destruction for files, archives, sockets, and handles.
- `std::string`, `std::filesystem`, containers, `std::optional`,
  `std::variant`, and `std::span`.
- Exceptions internally for construction/parsing failures, while keeping
  exceptions out of the C ABI boundary.
- Mature libraries: libcurl, libarchive, OpenSSL, nlohmann/json, CLI11, fmt,
  spdlog, SQLite, and Boost where justified.
- Templates and generics for serialization, resolution, and command dispatch.
- A stable C ABI can still be exposed with `extern "C"` and opaque handles.

C++ also keeps the door open to static linking and a single shipped binary,
which is the main reason to remove the Node runtime.

## 3. Why not C

C would make the same orchestrator materially larger and riskier:

- Manual memory management and manual ownership rules.
- No standard strings, containers, variants, or filesystem types.
- Manual JSON, HTTP, archive, and error handling.
- More boilerplate for every command, manifest, and registry path.
- More chances for use-after-free, double-free, and buffer errors in code that
  parses untrusted input.

C is appropriate for the ABI surface and for small leaf libraries. It is not
appropriate for the orchestrator's business logic.

## 4. Scope

### In scope for V2

- CLI command layer and output/exit-code compatibility.
- Manifest loading and validation.
- Registry client and package resolution.
- Toolchain resolution and installation.
- NoPM command orchestration and lockfile handling.
- Packaging and release helper logic.
- The JS/TS bridge host, if the bridge is moved out of TypeScript.

### Out of scope

- Rewriting the Rust parser, checker, compiler API, C codegen, or Nova runtime.
- Removing the TypeScript oracle before C++ parity is proven.
- New language syntax. Syntax work remains on its own v0.2 track.

## 5. Boundary with Rust

Use the narrowest boundary that fits the component:

1. **Process boundary with a JSON/CLI contract** for command-level integration.
   This is the safest and easiest to test.
2. **C ABI with opaque handles** for libraries. No C++ standard-library types,
   exceptions, or Rust types cross the boundary.
3. **`cxx` bridge** only when in-process C++ is required.

Rules:

- C++ owns C++ allocations; Rust owns Rust allocations.
- Every handle has one documented owner.
- Errors cross the boundary as codes plus owned message strings.
- No exception may escape an `extern "C"` function.
- No Rust panic may unwind into C++.
- JSON is the default data contract between processes.

## 6. Toolchain

- **Language standard:** C++20 baseline. Use C++23 only after every Tier-1
  compiler supports it.
- **Build:** CMake with a checked-in preset file.
- **Dependencies:** vcpkg manifest mode is the default; Conan 2 is the fallback
  if a required package is materially better there.
- **Compilers:** MSVC, GCC, and Clang on the Tier-1 CI matrix.
- **Static analysis:** clang-tidy and compiler warnings as errors.
- **Runtime checks:** ASan, UBSan, and TSan in CI where the platform supports
  them.
- **Fuzzing:** libFuzzer or AFL++ for manifest, registry, archive, and path
  parsing.
- **Tests:** Catch2 or GoogleTest for unit tests; the existing TypeScript suite
  remains the behavioral oracle until parity is proven.

## 7. Migration plan

This is a strangler migration, not a big-bang rewrite.

1. Freeze new runtime features in TypeScript.
2. Record the CLI/JSON contracts and add parity fixtures.
3. Port the smallest pure-logic components first:
   `manifest`, `registry`, `toolchain`.
4. Port `nopm` orchestration.
5. Port the remaining CLI command layer.
6. Port `codegen-js` and `codegen-ts` last, or keep them behind the isolated
   bridge until a native emitter is ready.
7. Delete a TypeScript component only after:
   - parity tests pass,
   - clean-install tests pass,
   - the oracle has been updated or retired deliberately.

New features added before V2 starts should go to Rust unless they specifically
need a C/C++ library or ABI. New features added during V2 should go to C++ only
when they belong to the orchestrator boundary; core features stay in Rust.

## 8. Open decisions

These are intentionally not settled by this document:

- C++20 vs C++23 for a later increment.
- vcpkg vs Conan for a specific dependency that is painful in the default.
- Process boundary vs C ABI for each orchestrator component.
- Whether `codegen-js`/`codegen-ts` become C++ or stay an isolated TypeScript
  bridge.
- The V2 start date and release boundary.

## 9. Risks

- Three toolchains: Cargo, CMake/vcpkg, and npm/tsc.
- Rust/C++ ABI and CRT mismatches, especially on Windows.
- Memory-safety bugs in code that parses untrusted manifests and archives.
- Build-time and binary-size growth.
- Long-lived hybrid state if TypeScript components are not actually retired.
- Divergence between the C++ implementation and the TypeScript oracle.

Mitigations: narrow C ABI, no exceptions across the boundary, sanitizers,
fuzzing, parity fixtures, and a hard rule that TypeScript does not grow new
runtime features after the port starts.

## 10. Non-goals

- Rewriting the Rust core in C++.
- Replacing the TypeScript oracle before parity.
- Using C for orchestrator business logic.
- Adding new language syntax as part of the orchestrator port.
