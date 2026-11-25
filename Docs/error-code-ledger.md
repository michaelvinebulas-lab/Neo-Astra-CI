# Neo Astra Error Code Ledger

**Status:** Reference
**Date:** 2026-11-06
**Source of truth:** `master` @ `cadb04f` (Release 1.0.0 line), plus the v0.2 edition rows below
**Total:** 152 codes — 134 documented in [`spec-v0.1.md`](spec-v0.1.md) §14, 12 reserved for edition `0.2` in [`spec-v0.2.md`](spec-v0.2.md) §6, 6 historical/reserved

This is a flat `code — meaning` index. The normative tables live in
`spec-v0.1.md` §14; this file collects them in one place and records the
historical/reserved codes that §14.17 mentions but does not tabulate.

Keep in sync with `scripts/check-error-codes.mjs`.

---

## Prefix legend

| Prefix | Owner | Meaning |
|---|---|---|
| `NP` | NoPM | Package, dependency, lockfile, and system-tool errors |
| `NR` | Registry | Registry gateway HTTP/service errors |
| `NS` | Compiler | Language, type, mode, backend, and CLI errors |
| `NV` | Nova | Interpreter/runtime errors |
| `PG` | Playground | Wasm compiler-bridge errors |

Within `NS`, the second digit is the phase:

| Digit | Phase |
|---|---|
| `0` | Historical / test fixture |
| `1` | Lexing |
| `2` | Parsing |
| `3` | Modules, imports, exports |
| `4` | Types, effects, task boundaries |
| `5` | Modes and capabilities |
| `6` | Memory (manual vs GC) |
| `7` | Inline `@script` interop and `@export_as` |
| `8` | Backends |
| `9` | CLI, manifest, build, toolchain |

---

## NS0 — Historical (not emitted)

| Code | Meaning |
|---|---|
| `NS0001` | Invalid token — unit-test fixture only, not user-facing (§14.17) |
| `NS0002` | Unknown identifier — historical design-doc code, superseded by `NS4002` |
| `NS0003` | Type mismatch — historical design-doc code, superseded by `NS4001` |
| `NS0004` | Missing error propagation — historical design-doc code, superseded by `NS4013` |

---

## NS1 — Lexing

| Code | Meaning |
|---|---|
| `NS1001` | Invalid comment form |
| `NS1002` | Invalid escape sequence or byte-literal value |
| `NS1003` | Unterminated string or byte literal |
| `NS1004` | Reserved — invalid UTF-8 source is reported by the CLI as `NS9007` |
| `NS1005` | Invalid identifier, malformed literal, or unexpected character |

---

## NS2 — Parsing

| Code | Meaning |
|---|---|
| `NS2001` | Expected token or delimiter |
| `NS2002` | Invalid syntax |
| `NS2003` | Invalid type |
| `NS2004` | Invalid pattern |
| `NS2005` | Invalid or unexpected item |
| `NS2006` | Missing newline or `;` boundary |
| `NS2007` | Invalid modifier - reserved |
| `NS2008` | Unsupported or invalid edition in a manifest |
| `NS2009` | Deprecated canonical alias (canonical-syntax lint) |
| `NS2010` | Expression-position type arguments require `::<...>` |
| `NS2011` | Malformed turbofish (`:: <`, standalone/second/misplaced `::`) |
| `NS2012` | Deprecated `spawn \|\|` spelling |
| `NS2013` | Deprecated wrapping division `/%` (never rewritten) |
| `NS2014` | Invalid tuple index — reserved; lexer prevents a float after `.` |
| `NS2015` | Deprecated or removed v0.1 spelling under edition `0.2` |
| `NS2016` | Canonical-syntax lint family — reserved aggregate |
| `NS2017` | Reserved for v0.2 front-end follow-ups |
| `NS2018` | Reserved for v0.2 front-end follow-ups |
| `NS2019` | Reserved for v0.2 front-end follow-ups |

---

## NS3 - Modules, imports, exports

| Code | Meaning |
|---|---|
| `NS3001` | Private declaration cannot be exported |
| `NS3002` | Conflicting visibility modifiers |
| `NS3003` | Duplicate export name |
| `NS3004` | Duplicate default export |
| `NS3005` | Unknown export target |
| `NS3006` | Duplicate import binding |
| `NS3007` | Imported name is not exported |
| `NS3008` | Conflicting re-exports |
| `NS3009` | Default export target is not a value |
| `NS3010` | Private declaration leaks through a public API |
| `NS3011` | Duplicate top-level declaration |
| `NS3012` | Imported module cannot be resolved |
| `NS3013` | Cyclic named import or re-export |

---

## NS4 — Types, effects, task boundaries

| Code | Meaning |
|---|---|
| `NS4001` | Type mismatch |
| `NS4002` | Unknown identifier |
| `NS4003` | Unknown type |
| `NS4004` | Wrong argument count |
| `NS4005` | Value is not callable |
| `NS4006` | Invalid operand types |
| `NS4007` | Missing return on a reachable path |
| `NS4008` | Reserved |
| `NS4009` | Assignment to an immutable binding |
| `NS4010` | Non-exhaustive match |
| `NS4011` | Unhandled error effect |
| `NS4012` | Error is not declared by the function |
| `NS4013` | Invalid error propagation |
| `NS4014` | Invalid control flow |
| `NS4015` | Invalid member or index access |
| `NS4016` | Invalid, missing, or duplicate struct field |
| `NS4017` | Invalid pattern or pattern arity |
| `NS4018` | Invalid type argument or arity |
| `NS4019` | A value carrying a `Task` handle would cross a task boundary by copy: channel element, spawn capture, or spawn result |

---

## NS5 — Modes and capabilities

| Code | Meaning |
|---|---|
| `NS5001` | Mode requirement is not satisfied |
| `NS5002` | Mode widening is not allowed |
| `NS5003` | Capability requirement is not satisfied |
| `NS5004` | Manual memory requires system mode |
| `NS5005` | Invalid capability or mode annotation |
| `NS5006` | No single built-in mode satisfies a project requirement |
| `NS5007` | System mode: two or more state-changing calls in a target-native-order position (binary operands, array elements, struct fields, map entries) — hoist the calls into bindings first |

---

## NS6 — Memory

| Code | Meaning |
|---|---|
| `NS6001` | Manual allocation outside a manual-memory scope |
| `NS6002` | Manual allocation of a GC-managed element type (`Ptr<T>` is never a GC root, §8.4) |

---

## NS7 — Inline `@script` interop and `@export_as`

| Code | Meaning |
|---|---|
| `NS7001` | Unknown or unsupported script language |
| `NS7002` | Script language is not supported by the active target |
| `NS7003` | Missing, empty, or duplicate `@exports` declaration |
| `NS7004` | Malformed script block: missing `@end` or a required header field |
| `NS7005` | Invalid or non-boundary-safe script export signature |
| `NS7006` | Script namespace collision |
| `NS7007` | Missing or inconsistent manifest `ffi` mapping for a foreign module or a required inline-script link/runtime entry |
| `NS7008` | Script backend or link failure, missing implementation export, or no pinned TypeScript transpiler |
| `NS7010` | `@export_as` on a non-generic or non-exported function |
| `NS7011` | `@export_as` binding list does not bind every type parameter exactly once |
| `NS7012` | `@export_as` bound type is not C-boundary-safe |
| `NS7013` | `@export_as` symbol is not a valid C identifier, or duplicates another symbol or instantiation |

`NS7009` is not allocated.

---

## NS8 — Backends

| Code | Meaning |
|---|---|
| `NS8001` | A backend or target does not support the requested construct or configuration |
| `NS8002` | A backend request or response is malformed or incomplete: invalid typed module, missing module, output path, entry module, or expected artifact |
| `NS8003` | Manual memory is not supported by the JavaScript or TypeScript backend |
| `NS8004` | A generated artifact path escapes the build output directory |
| `NS8011` | Historical: `spawn` was unsupported by the C backend first slice (`CONC-001`); superseded by real concurrency support |
| `NS8012` | Historical: `@memory:manual` was unsupported by the C managed-subset backend; removed when the manual-memory first cut landed |
| `NS8017` | Calling this callee is not supported by the C backend first slice |
| `NS8018` | A generic function requires monomorphization |
| `NS8019` | Imports are not supported by the C backend first slice |
| `NS8020` | C member error: undeclared enum, argument-less enum variant, or missing struct field |
| `NS8021` | C `for`-loop iteration is limited to arrays, maps, and bytes in the first slice |
| `NS8022` | Fixed-size arrays are not supported by the C backend first slice |
| `NS8024` | Printing this value type is not supported by the C backend first slice |
| `NS8025` | The tuple type is unavailable for this tuple expression in the C backend |
| `NS8099` | `Never` appears as a stored type, which the C backend cannot emit |

Unallocated: `NS8005`–`NS8010`, `NS8013`–`NS8016`, `NS8023`, `NS8026`–`NS8098`.

---

## NS9 — CLI, manifest, build, toolchain

| Code | Meaning |
|---|---|
| `NS9000` | Unexpected CLI or tooling failure; not a source diagnostic |
| `NS9001` | No manifest found; exactly one `core.json` or `core.toml` is required |
| `NS9002` | Unable to read or parse a manifest |
| `NS9003` | Compiler binary not found |
| `NS9004` | Compiler process failed |
| `NS9005` | Compiler returned an invalid, incomplete, or incompatible response |
| `NS9006` | JSON-RPC request failed |
| `NS9007` | Source or module is not valid UTF-8 |
| `NS9008` | A project or source path cannot be resolved, read, or accessed |
| `NS9009` | Unsupported compiler API version |
| `NS9010` | A compiler request contains no source units |
| `NS9011` | The build target is missing or not declared in the manifest |
| `NS9012` | A build configuration value is invalid: target mode, integer policy, or emit kind |
| `NS9013` | The entry point is missing, outside the project, or not a `.ns` file |
| `NS9014` | A build output directory or file cannot be created or written |
| `NS9015` | A native compile, link, or archive step failed |
| `NS9016` | A path dependency is missing or escapes its package or project root |
| `NS9101` | The build manifest or `toolchain` section is malformed |
| `NS9102` | No cached or `PATH` toolchain was found for the requested family |
| `NS9103` | `ASTRA_CC` does not exist or is not a usable compiler |
| `NS9999` | Reserved sentinel; tests assert it never appears in output |

Unallocated: `NS9017`–`NS9100`, `NS9104`–`NS9998`.

---

## NV — Nova runtime

| Code | Meaning |
|---|---|
| `NV0000` | Frontend diagnostics were returned unchanged |
| `NV0001` | Manual memory is unsupported by the first Nova engine |
| `NV0002` | Missing entry point |
| `NV0003` | Unsupported runtime feature or host adapter |
| `NV0004` | Unhandled error escaped the entry point |
| `NV0005` | Panic, including the interpreted call stack |
| `NV0006` | Abort without unwinding |
| `NV0007` | Nova internal invariant failure |
| `NV0008` | Foreign script requires a production target |

---

## NP — NoPM packages

| Code | Meaning |
|---|---|
| `NP0001` | Invalid source-qualified package ID |
| `NP0002` | Invalid dependency declaration or duplicate descriptor |
| `NP0003` | Missing package descriptor |
| `NP0004` | Dependency cycle |
| `NP0005` | Conflicting root dependency alias |
| `NP0006` | Package is incompatible with the selected target |
| `NP0007` | Package capability is incompatible with the selected mode |
| `NP0008` | Invalid lockfile, missing integrity, or unpinned Git dependency |
| `NP0009` | Missing or stale lockfile |
| `NP0010` | Lockfile write failure |
| `NP0011` | Package adapter, path, fetch, or installation failure |
| `NP0012` | NoPM command-line usage error |
| `NP0013` | Dependency is already declared |
| `NP0014` | Dependency is not declared |
| `NP0015` | Unknown system tool |
| `NP0016` | NoPM does not uninstall system tools |
| `NP0017` | No supported package manager was found |
| `NP0018` | System tool installation failed |

---

## NR — Registry gateway

| Code | Meaning |
|---|---|
| `NR0001` | Package already exists (HTTP 409) |
| `NR0002` | Unknown registry route, or signature/public-key verification failed |
| `NR0003` | Package or version not found |
| `NR0004` | Package is yanked |

---

## PG — Playground Wasm bridge

| Code | Meaning |
|---|---|
| `PG0001` | Protocol version mismatch between the bridge and the compiler |
| `PG0002` | Wasm trap or panic inside the compiler |
| `PG0003` | Malformed or truncated compiler response |
| `PG0004` | Allocation failure inside the Wasm instance |

These are bridge-level failures, never user syntax errors.

---

## Coverage notes

1. **All 134 live codes have a spec §14 row.** The six exceptions are
   `NS0001`–`NS0004` (historical/test) and `NS8011`–`NS8012` (removed), which
   §14.17 describes but does not tabulate.
2. **`NS8001` is no longer overloaded.** The 1.0.0 line split the former
   API-version and empty-source cases into `NS9009` and `NS9010`.
3. **`NS7007` is implemented** in the CLI manifest/FFI path.
4. **`NS8011`/`NS8012` are historical.** Both were removed when real
   manual memory and concurrency support landed; they remain only in
   `Docs/archive/`.
5. **`NS1004` is reserved** — invalid UTF-8 is reported as `NS9007`.
