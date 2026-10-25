# V1 Compiler Design

**Status:** V1 design proposal  
**Depends on:** D3, D13–D16, D19, D20  
**Related:** [`01-language-design.md`](./01-language-design.md), [`03-runtime-and-memory.md`](./03-runtime-and-memory.md)

---

## 1. Goals

The V1 compiler must:

- Parse `.ns` exactly once, in Rust.
- Preserve one semantic contract across all targets.
- Allow JavaScript/TypeScript code generation from the same typed IR used by native backends.
- Produce stable, structured diagnostics.
- Support build caching without hidden state.
- Keep the TypeScript and Rust sides independently testable.
- Make target support explicit rather than silently degraded.

Non-goals:

- Full incremental compilation
- Distributed compilation
- A stable public compiler ABI
- Third-party backend plugins in V1
- Re-parsing source in TypeScript

---

## 2. Split architecture

```text
astra CLI (TypeScript)
    │
    ├── manifest loading / validation
    ├── target and mode selection
    ├── NoPM package resolution (path, Git, npm, Cargo)
    ├── build graph
    ├── LSP
    ├── JS/TS code generation
    ├── compiler client (JSON-RPC)
    │
    └── compiler API
          │
          ▼
    Rust core
      ├── lexer
      ├── parser
      ├── resolver
      ├── type checker
      ├── capability inference
      ├── typed HIR
      ├── Nova interpreter (development)
      ├── Astra IR
      ├── optimizer
      ├── C backend
      ├── Wasm backend
      └── future LLVM backend
```

### Responsibility boundary

| Concern | Owner |
|---|---|
| CLI and user experience | TypeScript |
| Manifest parsing and strict validation | TypeScript |
| NoPM package-source adapters and universal lockfile | TypeScript |
| Target and mode precedence | TypeScript, validated by Rust |
| Lexing, parsing, semantics | Rust |
| Capability inference | Rust |
| Typed HIR and IR | Rust |
| Nova development interpreter | Rust, consuming typed HIR |
| Diagnostics | Rust for frontend, TypeScript for orchestration |
| Compiler transport | TypeScript JSON-RPC client; Rust core owns execution |
| JS/TS codegen | TypeScript, consuming typed IR |
| C/Wasm codegen | Rust |
| Build cache | TypeScript owns cache; Rust provides deterministic inputs/outputs |

TypeScript may read the manifest and source text, but it must not tokenize or parse `.ns` source. The Rust core is the single source of truth.

---

## 3. Compiler API

### 3.1 Transport

V1 supports two transports:

1. **Native binary over stdio** for CLI builds.
2. **Wasm exports** for the browser playground.

The native protocol is JSON-RPC 2.0 over stdin/stdout. The Wasm build exposes the same command names through a thin JavaScript wrapper.

Every request includes:

```json
{
  "jsonrpc": "2.0",
  "id": "req-001",
  "method": "compile",
  "params": {
    "apiVersion": "1.0",
    "target": "js",
    "mode": "core",
    "source": {
      "path": "src/main.ns",
      "text": "..."
    },
    "manifest": {},
    "options": {}
  }
}
```

Every response includes:

```json
{
  "jsonrpc": "2.0",
  "id": "req-001",
  "result": {
    "apiVersion": "1.0",
    "diagnostics": [],
    "metadata": {},
    "artifacts": []
  }
}
```

### 3.2 Commands

| Command | Purpose |
|---|---|
| `initialize` | Report core version, API version, features, and target support |
| `check` | Parse, resolve, type-check, and validate without codegen |
| `compile` | Produce typed IR and backend inputs |
| `ast` | Return the canonical AST as JSON |
| `format` | Return canonical source text |
| `explain` | Return mode, dependency, and capability explanations |
| `shutdown` | Release resources |

### 3.3 Versioning

- `apiVersion` is separate from the language version.
- Unknown API versions are rejected.
- New optional fields may be added in a minor API version.
- Removing or changing a field requires a major API version.
- The CLI and core must negotiate versions at `initialize`.

### 3.4 Determinism

The Rust core must:

- Emit deterministic diagnostics for the same input.
- Avoid unordered iteration in output.
- Use stable IDs for AST and IR nodes.
- Avoid timestamps in compiled artifacts unless explicitly requested.
- Record the compiler version and target in metadata.

---

## 4. Compilation pipeline

```text
1. Load manifest
2. Resolve target and mode
3. Resolve packages
4. Read source files
5. Lex
6. Parse to AST
7. Resolve modules and symbols
8. Type-check
9. Infer capabilities
10. Lower to typed HIR
11. Lower to Astra IR (SSA)
12. Optimize
13. Select backend
14. Emit code or runtime artifacts
15. Run target toolchain if needed
16. Write build metadata
```

Nova stops after step 10 and interprets typed HIR. It does not lower to Astra
IR, optimize, or emit target code. The production pipeline is unchanged.

### 4.1 Manifest and target resolution

The TypeScript orchestrator:

1. Finds exactly one of `core.json` or `core.toml`.
2. Validates it against the V1 schema.
3. Resolves the target from CLI, environment, or manifest.
4. Resolves the mode using the strict pipeline.
5. Loads package sources and the lockfile.
6. Sends the resolved manifest and source list to the Rust core.

The Rust core independently validates the resolved mode against source requirements. A mismatch is an error, not a warning.

### 4.2 Parsing

- One lexer.
- One parser.
- No target-specific grammar.
- Comments are discarded after span association.
- Invalid comment forms are hard errors.
- Parser recovery must not invent valid syntax.
- `@script` headers are parsed as Nova syntax; the body after `@impl` is captured
  verbatim until the `@end` sentinel and never passed to the Nova expression
  parser. See [`10-inline-script-interop.md`](./10-inline-script-interop.md).

### 4.3 Resolution

The resolver handles:

- Modules and imports
- Module export surfaces, visibility, default exports, and re-exports
- Symbol namespaces
- Type declarations
- Generic parameter binding
- Foreign module declarations
- Inline `@script` namespaces and their declared export surfaces
- Attribute validation

### 4.4 Type checking

The checker runs over resolved HIR and enforces:

- Explicit parameter types and declared return/error signatures
- Local inference for bindings and unambiguous generic calls
- No implicit numeric coercion or truthiness
- Structural checks for primitives, tuples, arrays, slices, fixed arrays, maps, functions, structs, enums, aliases, and generics
- Declared-field and constructor checking, including generic substitutions
- Recursive exhaustiveness over `Option`, `Result`, booleans, tuples, and enum variants
- Effect-tracked `!` propagation, handling, and declared error sets
- Mutability and lvalue checks
- Mode requirements, scoped mode narrowing, capability tags, foreign-module requirements, inline-script mode declarations, boundary-safe `@script` exports, and manual-memory constraints

The checker records an expression type annotation for every checked expression.
Those annotations are retained in `CheckResult.annotations` so Nova can execute
the same typed HIR without re-inferring types. Imported symbols whose defining
module is not part of the current module graph remain opaque rather than being
assumed invalid; strict multi-module checking uses the resolver's full graph.

Type errors use `NS4001`–`NS4018`; mode and capability errors use `NS5001`–`NS5006`; inline-script interop uses the provisional `NS7001`–`NS7008` range.

### 4.5 Capability inference

The capability pass:

1. Builds the static call graph.
2. Collects direct and transitive requirements.
3. Detects mode widening.
4. Selects the least-privileged mode if no explicit mode is set.
5. Produces an explanation tree.

The result is stored in build metadata and included in the cache key.

### 4.6 HIR

HIR is typed and still close to source. It preserves:

- Names and spans
- Explicit error effects
- Mode and capability requirements
- Memory-policy annotations
- Generic instantiations
- Foreign module references
- Inline script blocks, including language, namespace, declared mode, export
  signatures, and verbatim source needed by backends

### 4.7 Astra IR

Astra IR is a target-independent SSA form.

Required properties:

- Explicit types
- Explicit control flow
- Explicit error results
- Explicit task and channel operations
- Explicit memory operations for manual code
- No target-specific constructs
- Stable node IDs for diagnostics and tests

### 4.8 Optimization

V1 optimization is conservative:

- Constant folding
- Dead code elimination
- Copy propagation
- Simple inlining
- Monomorphization
- Unused declaration removal

Aggressive optimization is not required for V1 correctness.

---

## 5. Backend interface

Each backend receives:

- Typed IR
- Target configuration
- Semantic policy
- Memory policy
- Runtime ABI version
- Feature flags

Each backend returns:

- Generated artifacts
- Diagnostics
- Required runtime modules
- Toolchain commands
- Metadata for the build cache

### 5.1 Backend feature negotiation

A backend declares support for:

- Types
- Error representation
- Concurrency
- Manual memory
- FFI
- Target-specific features

If a feature is unsupported, the compiler must emit a clear diagnostic before code generation. It must not emit broken output.

---

## 6. Build cache

### 6.1 Cache location

```text
.astra/cache/
```

The cache is local and disposable. Deleting it must not change program behavior.

### 6.2 Cache key

The key includes hashes of:

- Compiler version
- API version
- Manifest
- Lockfile
- Source files, including verbatim inline `@script` bodies
- Target
- Mode
- `math.integers`
- `strings.model`
- Memory policy
- Relevant environment variables
- Backend feature flags
- Inline-script language runtime and transpiler versions

### 6.3 Cache rules

- Cache entries are content-addressed.
- Cache hits must be verified against the full key.
- A stale entry must never be used.
- Cache metadata must be inspectable.
- `astra clean` removes the cache.
- CI should not rely on a warm cache for correctness.

V1 does not promise incremental compilation at function granularity.

---

## 7. Diagnostics

### 7.1 Stable codes

Every diagnostic has a stable code:

```text
NS0001  Invalid comment syntax
NS0002  Unknown identifier
NS0003  Type mismatch
NS0004  Missing error propagation
NS2001  Expected token or delimiter
NS2002  Invalid syntax
NS2003  Invalid type
NS2004  Invalid pattern
NS2005  Invalid or unexpected item
NS2006  Missing newline or ';' boundary
NS2007  Invalid modifier
NS3001  Private declaration cannot be exported
NS3002  Conflicting visibility modifiers
NS3003  Duplicate export name
NS3004  Duplicate default export
NS3005  Unknown export target
NS3006  Duplicate import binding
NS3007  Imported name is not exported
NS3008  Conflicting re-exports
NS3009  Default export target is not a value
NS3010  Private declaration leaks through a public API
NS3011  Duplicate top-level declaration
NS3012  Imported module cannot be resolved
NS3013  Cyclic named import or re-export
...
```

Categories:

- `NS1xxx` — lexical
- `NS2xxx` — parser
- `NS3xxx` — resolver
- `NS4xxx` — type checking
- `NS5xxx` — modes and capabilities
- `NS6xxx` — memory
- `NS7xxx` — FFI and packages
- `NS8xxx` — backends and targets
- `NS9xxx` — tooling and configuration

### 7.2 Diagnostic shape

```json
{
  "code": "NS5001",
  "severity": "error",
  "message": "System-only function called from a core context.",
  "file": "src/main.ns",
  "span": { "start": 120, "end": 156 },
  "mode": "core",
  "target": "js",
  "hint": "Mark the caller @requires(system) or move the call behind an explicit boundary.",
  "related": []
}
```

### 7.3 Error recovery

- The parser recovers at statement and item boundaries.
- The checker reports multiple independent errors where safe.
- The compiler stops before codegen if any error remains.
- Warnings never hide errors.

---

## 8. Plugins

V1 has two plugin categories:

1. **Package-source adapters** — npm, Cargo, local path, git.
2. **Tooling extensions** — optional CLI and LSP extensions.

Backend plugins are post-V1. The V1 backends are shipped with the compiler.

### 8.1 Plugin trust

- Built-in adapters are trusted.
- Third-party adapters require explicit manifest opt-in.
- Plugins cannot execute install scripts by default.
- Plugins cannot escape the project root.
- Plugins receive the minimum data needed for their task.
- Plugin failures are reported as build errors, not ignored.

---

## 9. Compiler performance targets

These are initial V1 budgets, not guarantees:

| Metric | Target |
|---|---|
| Cold check of 10k lines | under 2 seconds |
| Warm check of 10k lines | under 500 ms |
| CLI startup | under 150 ms |
| JS codegen overhead | under 20% of check time |
| Peak compiler memory | under 512 MB for 50k lines |
| Diagnostic latency in LSP | under 200 ms after edit |

If these targets are missed, record the measured baseline rather than hiding it.

---

## 10. Testing the compiler

Required test layers:

1. Lexer unit tests
2. Parser and formatter round-trip tests
3. Type-checker positive and negative tests
4. Mode and capability inference tests
5. IR snapshots
6. Backend golden tests
7. Cross-target conformance tests
8. FFI boundary tests
9. Inline script raw-capture, export-surface, and opaque-body tests
10. Performance regression tests
11. Fuzz tests for parser and formatter

The compiler must be testable without launching the full CLI.

---

## 11. Failure modes

| Failure | Required behavior |
|---|---|
| Rust core crashes | CLI reports core failure and preserves the build log |
| API version mismatch | Clear negotiation error |
| Unsupported target | Error before codegen |
| Missing FFI mapping | Error before codegen |
| Unsupported inline script language/target | Error before codegen |
| Malformed inline script block | Parse error at the script header; raw body is never partially parsed |
| Manual memory on JS | Error before codegen |
| Stale cache | Cache miss, rebuild |
| Unknown manifest key | Manifest validation error |
| Package adapter failure | Build error with source and package name |

The compiler must never emit a partial binary and call it successful.

---

## 12. Open compiler questions

1. Should the native protocol use JSON only, or add a binary framing mode for large projects?
2. Should HIR/IR serialization be JSON in V1 or a compact format?
3. Which optimizer passes are required for V1 versus deferred?
4. Should the LSP use the native binary or the Wasm core?
5. How much of `astra explain` belongs in the core versus the orchestrator?
6. Should build metadata be stored in `.astra/` or alongside artifacts?
7. What is the minimum supported `clang` version?
8. How are third-party package adapters sandboxed?
