# V1 Tooling and Workflow

**Status:** V1 design proposal  
**Depends on:** D13, D15, D16, D19, D20, D25  
**Related:** [`02-compiler-design.md`](./02-compiler-design.md), [`05-manifest-and-packages.md`](./05-manifest-and-packages.md), [`08-nova-and-nopm.md`](./08-nova-and-nopm.md)

---

## 1. Tooling goals

V1 tooling must make the language:

- Easy to try
- Hard to misuse silently
- Explainable when the compiler infers something
- Fast enough for interactive use
- Usable without a heavy IDE
- Friendly to AI agents and external tools

The CLI and LSP are first-class products, not wrappers around an internal test harness.

---

## 2. CLI overview

```text
astra <command> [options]
```

| Command | Purpose |
|---|---|
| `astra new <name>` | Create a new project |
| `astra init` | Add a manifest to an existing directory |
| `astra check` | Parse, resolve, type-check, and validate |
| `astra build` | Build one or more targets |
| `astra run` | Build and run the default production target |
| `astra test` | Run project tests |
| `astra fmt` | Format source and manifest |
| `astra explain` | Explain mode, capability, and dependency inference |
| `astra ast` | Print the canonical AST |
| `astra clean` | Remove build artifacts and cache |
| `astra lsp` | Start the language server |
| `astra doc` | Generate API documentation |
| `astra version` | Print compiler and API versions |

### 2.1 Nova and NoPM commands

Nova is the development runtime. It runs typed `.ns` source directly and does
not replace `astra build`.

```text
nova run <file.ns> [--watch]
nova repl
nova test [path] [--watch]
nova eval "<source>"
nova version
```

NoPM is the package manager for both Nova and Astra builds.

```text
nopm add <spec>
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

The old `astra add` / `astra remove` names are not part of the V1 surface. Use
`nopm` so package operations have one owner.

---

## 3. Common options

| Option | Meaning |
|---|---|
| `--target <name>` | Select a manifest target |
| `--mode <name>` | Override the mode |
| `--profile <name>` | Select debug or release |
| `--json` | Machine-readable output |
| `--quiet` | Suppress non-error output |
| `--verbose` | Include build detail |
| `--out-dir <path>` | Override output directory |
| `--no-cache` | Disable build cache |
| `--watch` | Rebuild on change |
| `--locked` | Require the lockfile to be current |
| `--frozen` | Refuse manifest or lockfile changes |

`--locked` and `--frozen` are important for CI.

---

## 4. Exit codes

| Code | Meaning |
|---|---|
| `0` | Success |
| `1` | Compile or type error |
| `2` | Usage error |
| `3` | Manifest or lockfile error |
| `4` | Package resolution error |
| `5` | Target or toolchain error |
| `6` | Test failure |
| `7` | Internal compiler error |

Scripts should be able to distinguish “your code is wrong” from “the toolchain is broken.”

---

## 5. JSON output

All machine-readable commands use a stable envelope:

```json
{
  "command": "check",
  "status": "error",
  "diagnostics": [],
  "metadata": {}
}
```

### `astra check --json`

```json
{
  "command": "check",
  "status": "error",
  "diagnostics": [
    {
      "code": "NS5001",
      "severity": "error",
      "message": "System-only function called from a core context.",
      "file": "src/main.ns",
      "span": { "start": 120, "end": 156 },
      "mode": "core",
      "target": "js",
      "hint": "Mark the caller @requires(system)."
    }
  ],
  "metadata": {
    "mode": "core",
    "target": "js",
    "compilerVersion": "1.0.0"
  }
}
```

### Rules

- Diagnostics are sorted by file and span.
- Codes are stable.
- Paths are repository-relative.
- `--json` writes JSON to stdout and human logs to stderr.
- A JSON response is valid even when the command fails.

---

## 6. `astra build`

```bash
astra build
astra build --target web
astra build --target native --profile release
astra build --target web --target native
astra build --locked
```

Steps:

1. Validate the manifest.
2. Resolve the lockfile.
3. Resolve target and mode.
4. Run the compiler core.
5. Emit artifacts.
6. Invoke target toolchains if needed.
7. Write build metadata.
8. Report artifacts and warnings.

`astra build` must not silently fall back to another target.

**Implementation status (2026-10-25):** the first `JS-001` slice supports one
`--target` per invocation, a single `nova.entry` module, and the `js` backend
with `module: "esm"`. It selects `--target`, then `ASTRA_TARGET`, then
`build.defaultTarget`; it applies target/build mode and `math.integers`, emits
the module under `outDir`, and adds `"type": "module"` when absent. Multi-target
builds, lockfile-aware builds, project module graphs, and native/Wasm toolchains
remain.

---

## 7. `astra run`

`astra run` builds and executes the default production target.

- For `web`, it starts or opens the playground/dev server.
- For `native`, it runs the binary.
- For `wasm`, it uses the configured host.
- For `typescript`, it does not execute TypeScript directly; it builds or uses a configured runtime.

`nova run` is the fast development path: it interprets typed `.ns` source
without a build. It is not a substitute for `astra run` when validating
production output.

Arguments after `--` are passed to the program.

---

## 8. `astra explain`

`astra explain` is a product feature, not a debug flag.

```text
Resolved mode: system (via manifest target: native)
  ├── src/main.ns -> uses app capabilities
  └── src/io.ns   -> forces system mode
       └── line 42: @requires(system) due to raw pointer usage
```

It should explain:

- Resolved mode and why
- Target-bound mode
- Source tags
- Scoped blocks
- Viral requirements
- Inferred dependencies
- Semantic policies
- Memory policy
- FFI mappings, including inline `@script` mode requirements

`astra explain --json` returns a structured explanation record.

The compiler API already emits the underlying explanation fields (`mode`,
`modeSource`, `requiredMode`, `inferredModes`, `modeReasons`, and
`modeExplanation`) for single-source and project checks. The dedicated
`astra explain` command is still a product-layer task, so this milestone exposes
the stable data instead of adding a second inference implementation.

---

## 9. Formatter

`astra fmt` formats:

- `.ns` source
- `core.json`
- `core.toml`

Rules:

- Deterministic output.
- Idempotent.
- Preserves every `//` comment exactly once; comments inside inline expressions may move to the nearest syntax boundary.
- Rejects invalid source instead of partially formatting it.
- Does not reorder semantic declarations unless the language specifies it.
- Normalizes keyword aliases to their canonical spelling.
- Normalizes `public` to `pub` and redundant `pub export` combinations to `export`.
- Preserves explicit same-line `;` groups; removes unnecessary trailing semicolons.
- Preserves declaration and export order.
- Preserves every byte of an inline `@script` body between `@impl` and `@end`;
  only the header, terminator, and surrounding indentation are formatted.
- Canonicalizes JSON key order and TOML layout.
- `--check` reports formatting differences without writing.

The formatter is part of the language contract. A formatter bug is a language tooling bug.

`astra fmt` formats `.ns` source through the Rust core. Manifest formatting is
handled by `@neo-astra/manifest`. A project directory formats its root
`core.json` or `core.toml` alongside its `.ns` files.

JSON manifests use this top-level order:

```text
$schema, package, build, nova, registry, math, strings, memory,
targets, dependencies, packageSources, ffi, capabilities
```

Unknown keys follow in sorted order. Dynamic maps such as targets, dependencies,
and FFI modules are sorted; `packageSources` uses path, Git, npm, Cargo, PyPI,
NuGet, and OCI order when present.

TOML manifests use Taplo layout and preserve declaration order and `#` comments.
Both formats reject duplicate keys, normalize a leading BOM, use two-space JSON
indentation, and end with exactly one trailing newline.

---

## 10. Language Server Protocol

The LSP is implemented in TypeScript and calls the Rust core.

V1 capabilities:

- Diagnostics
- Formatting
- Hover with type information
- Go to definition
- Find references
- Completion for keywords, symbols, and imports
- Code actions for common diagnostics
- Document symbols
- Workspace symbols
- Rename
- `astra explain` inlay hints where useful

V1 does not require:

- Full semantic refactoring
- Cross-crate rename across external packages
- AI-generated code actions
- Remote workspace support

### LSP rules

- The server must not re-parse `.ns` in TypeScript.
- Diagnostics use the same codes as the CLI.
- Formatting uses the same formatter as `astra fmt`.
- The server degrades gracefully if the Rust core fails.
- The server must not block the editor on a full rebuild.

---

## 11. Playground

The browser playground should:

- Load the Rust core as Wasm.
- Accept `.ns` source and a minimal manifest.
- Run `check`, `ast`, `fmt`, and `explain`.
- Compile to JavaScript ESM.
- Show generated code.
- Run simple programs in a sandbox.
- Display structured diagnostics.
- Work without a backend server for core features.

If a hosted compile API is used, it must be documented as a separate trust boundary.

---

## 12. Testing workflow

### 12.1 Unit tests

- Lexer
- Parser
- Resolver
- Type checker
- Formatter
- Manifest parser
- Package adapters
- Backend helpers

### 12.2 Golden tests

- AST snapshots
- IR snapshots
- Generated JS/TS snapshots
- Diagnostic snapshots

### 12.3 Conformance tests

A single source program, multiple targets, expected behavior.

```text
tests/conformance/
  integers/wrapping/
    main.ns
    expected.json
  strings/utf8-len/
    main.ns
    expected.json
  errors/propagation/
    main.ns
    expected.json
```

### 12.4 Differential tests

Compare:

- Interpreter vs JS
- JS vs native
- Native vs Wasm
- Debug vs release where behavior should match

### 12.5 Fuzz tests

- Lexer/parser input fuzzing
- Formatter round-trip fuzzing
- Manifest fuzzing
- FFI boundary fuzzing

### 12.6 Performance tests

- Compile time
- Runtime throughput
- Binary size
- Bundle size
- Memory use

---

## 13. `astra test`

Project tests can be:

- Unit tests
- Integration tests
- Conformance tests
- Golden tests
- Benchmark tests

```bash
astra test
astra test --target native
astra test --json
```

Test output uses the same diagnostic and JSON conventions as `astra check`.

---

## 14. `nopm add` and `nopm remove`

NoPM owns package mutation. `nopm add` should:

1. Parse a source-qualified package spec such as `npm:react@19`.
2. Validate that the source adapter is enabled.
3. Resolve the version and target variants.
4. Update `core.json` or `core.toml`.
5. Update `core.lock.json`.
6. Print the resolved package ID and integrity hash.

`nopm remove` reverses the manifest and lockfile changes.

Neither command should run package lifecycle scripts by default. `astra add` and
`astra remove` are not part of the V1 surface.

---

## 15. CI workflow

Recommended CI stages:

```text
1. astra check --locked
2. astra fmt --check
3. astra test --target js
4. astra test --target typescript
5. astra build --target native
6. astra build --target wasm-web
7. conformance matrix
8. security audit
9. performance budget check
```

CI must fail on:

- Stale lockfile
- Formatting drift
- Conformance failure
- Missing FFI mapping
- Manual memory in a JS build
- Unsupported target use
- Security policy violation

---

## 16. Documentation generation

`astra doc` should generate:

- Public API docs
- Type signatures
- Mode and capability requirements
- FFI mapping notes
- Example snippets

Docs must be generated from the canonical AST, not from a second parser.

---

## 17. Privacy and telemetry

- No telemetry by default.
- Any telemetry is opt-in.
- Source code is never sent to a remote service without explicit configuration.
- The playground must state whether compilation is local or remote.

---

## 18. Open tooling questions

1. Should `astra run --target web` start a dev server or open a static file?
2. Should `astra test` support snapshots or rely on external test libraries?
3. What is the minimum editor support for V1?
4. Should `nopm add` support Git and path sources in V1, or only registry sources?
5. Should `astra doc` ship in V1 or V1.1?
6. Should the LSP use the native binary or Wasm core?
7. Should TOML manifests adopt the same canonical key order as JSON, or only canonical layout?
8. Should `astra fmt --check` be part of the default CI template?
9. How should generated bindings be committed?
10. Should the playground support native/Wasm execution in-browser?
11. Should `nova` share the `astra test` runner or have a separate test discovery model?
