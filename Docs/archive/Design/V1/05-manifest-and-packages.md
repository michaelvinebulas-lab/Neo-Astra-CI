# V1 Manifest, Packages, and FFI

**Status:** V1 design proposal  
**Depends on:** D10, D11, D15, D18, D21, D25  
**Related:** [`04-targets-and-backends.md`](./04-targets-and-backends.md), [`06-tooling-and-workflow.md`](./06-tooling-and-workflow.md), [`08-nova-and-nopm.md`](./08-nova-and-nopm.md)

---

## 1. Manifest rules

A Neo Astra project has exactly one manifest:

- `core.json`
- `core.toml`

If both exist, the build fails.

The manifest is:

- Strictly schema-validated
- Unknown-key rejecting
- Duplicate-key rejecting
- Type-checked
- Canonically formattable
- The source of target, mode, semantic policy, memory, dependency, package, and FFI configuration

The lockfile is always `core.lock.json`.

---

## 2. V1 manifest schema

### 2.1 JSON example

```json
{
  "$schema": "https://neovastra.dev/schema/core-1.0.json",
  "package": {
    "name": "my-universal-app",
    "version": "1.0.0",
    "edition": "2026",
    "license": "MIT"
  },
  "build": {
    "defaultTarget": "web",
    "defaultMode": "app",
    "profile": "release"
  },
  "nova": {
    "entry": "src/main.ns",
    "watch": ["src/**/*.ns"]
  },
  "registry": {
    "default": "https://registry.neovastra.dev",
    "defaultSource": "nova"
  },
  "math": {
    "integers": "wrapping"
  },
  "strings": {
    "model": "utf8"
  },
  "memory": {
    "default": "auto",
    "files": {
      "src/io.ns": "manual",
      "src/drivers/*.ns": "manual"
    }
  },
  "targets": {
    "web": {
      "backend": "js",
      "module": "esm",
      "mode": "web",
      "outDir": "dist/web"
    },
    "typescript": {
      "backend": "ts",
      "module": "esm",
      "outDir": "dist/ts"
    },
    "native": {
      "backend": "c",
      "triple": "x86_64-pc-windows-msvc",
      "mode": "system",
      "outDir": "dist/native"
    },
    "wasm-web": {
      "backend": "wasm32",
      "wasi": false,
      "outDir": "dist/wasm-web"
    },
    "wasm-wasi": {
      "backend": "wasm32",
      "wasi": true,
      "outDir": "dist/wasm-wasi"
    }
  },
  "dependencies": {
    "fast-crypto": {
      "source": "cargo",
      "version": "1.2",
      "targets": ["native", "wasm-web", "wasm-wasi"]
    },
    "ui-kit": {
      "source": "npm",
      "version": "0.4",
      "targets": ["web", "typescript"]
    },
    "local-utils": {
      "source": "path",
      "path": "../local-utils"
    }
  },
  "packageSources": {
    "path": {
      "enabled": true
    },
    "git": {
      "enabled": true
    },
    "npm": {
      "enabled": true,
      "lifecycleScripts": false
    },
    "cargo": {
      "enabled": true
    }
  },
  "ffi": {
    "math_engine": {
      "native": {
        "source": "./native/libmath.so",
        "type": "c-abi",
        "ownership": "borrowed"
      },
      "typescript": {
        "source": "./npm-package/index.js",
        "type": "esm"
      },
      "wasm-web": {
        "source": "./wasm/math_engine.wasm",
        "type": "wasm-import"
      }
    }
  },
  "capabilities": {
    "web": ["dom", "fetch"],
    "native": ["fs", "threads"]
  }
}
```

### 2.2 TOML equivalent

```toml
[package]
name = "my-universal-app"
version = "1.0.0"
edition = "2026"
license = "MIT"

[build]
defaultTarget = "web"
defaultMode = "app"
profile = "release"

[nova]
entry = "src/main.ns"
watch = ["src/**/*.ns"]

[registry]
default = "https://registry.neovastra.dev"
defaultSource = "nova"

[math]
integers = "wrapping"

[strings]
model = "utf8"

[memory]
default = "auto"

[memory.files]
"src/io.ns" = "manual"
"src/drivers/*.ns" = "manual"

[targets.web]
backend = "js"
module = "esm"
mode = "web"
outDir = "dist/web"

[targets.typescript]
backend = "ts"
module = "esm"
outDir = "dist/ts"

[targets.native]
backend = "c"
triple = "x86_64-pc-windows-msvc"
mode = "system"
outDir = "dist/native"

[targets.wasm-web]
backend = "wasm32"
wasi = false
outDir = "dist/wasm-web"

[targets.wasm-wasi]
backend = "wasm32"
wasi = true
outDir = "dist/wasm-wasi"

[dependencies.fast-crypto]
source = "cargo"
version = "1.2"
targets = ["native", "wasm-web", "wasm-wasi"]

[dependencies.ui-kit]
source = "npm"
version = "0.4"
targets = ["web", "typescript"]

[dependencies.local-utils]
source = "path"
path = "../local-utils"

[packageSources.path]
enabled = true

[packageSources.git]
enabled = true

[packageSources.npm]
enabled = true
lifecycleScripts = false

[packageSources.cargo]
enabled = true

[ffi.math_engine.native]
source = "./native/libmath.so"
type = "c-abi"
ownership = "borrowed"

[ffi.math_engine.typescript]
source = "./npm-package/index.js"
type = "esm"

[ffi.math_engine.wasm-web]
source = "./wasm/math_engine.wasm"
type = "wasm-import"
```

The schema URL is inherited from the architecture draft and must be confirmed before publishing. It is not a V1 commitment to a domain.

---

## 3. Schema rules

Required top-level sections:

- `package`
- `build`
- `targets`

Optional sections:

- `nova`
- `registry`
- `math`
- `strings`
- `memory`
- `dependencies`
- `packageSources`
- `ffi`
- `capabilities`

Rules:

- `package.name` must be a valid package identifier.
- `package.version` must be semver.
- `build.defaultTarget` must name a declared target.
- `build.defaultMode` must be a built-in or declared mode.
- Every target must have a `backend` and `mode` unless the backend supplies a default.
- Every dependency must declare a source.
- Every target-scoped dependency must name valid targets.
- Every required foreign module must have a mapping for the active target.
- `nova.entry` must reference a `.ns` file inside the project.
- `registry.default` must use HTTPS except for loopback development registries.
- `packageSources` enables adapters; a dependency using a disabled adapter is an error.
- NoPM lifecycle scripts are disabled unless a package source explicitly opts in under a documented security policy.
- Unknown keys are errors.
- Duplicate keys are errors.
- Paths must not escape the project root unless explicitly marked external.
- `math.integers`, `strings.model`, and `memory.default` must use valid values.

### 3.1 Nova and NoPM configuration

`[nova]` is development-runtime configuration, not a second manifest. It names
the default entry file and watch roots for `nova run` and `nova watch`.

`[registry]` configures the default universal registry gateway and the default
source policy for unqualified package names.

`[packageSources]` enables NoPM adapters. V1 starts with path, Git, npm, and
Cargo. A later adapter is not enabled until both the adapter and its security
policy are implemented.

The same manifest and `core.lock.json` serve Nova and `astra build`. Nova must
not resolve a different dependency graph from the compiler.

---

## 4. Lockfile

`core.lock.json` records the resolved build graph.

```json
{
  "lockfileVersion": 1,
  "generatedBy": "astra/1.0.0",
  "manifestHash": "sha256:...",
  "compiler": {
    "version": "1.0.0",
    "apiVersion": "1.0"
  },
  "semantics": {
    "integers": "wrapping",
    "strings": "utf8"
  },
  "targets": {
    "web": {
      "backend": "js",
      "mode": "web"
    },
    "native": {
      "backend": "c",
      "triple": "x86_64-pc-windows-msvc",
      "mode": "system"
    }
  },
  "packages": [
    {
      "id": "npm:ui-kit@0.4.0",
      "name": "ui-kit",
      "source": "npm",
      "version": "0.4.0",
      "resolved": "https://registry.npmjs.org/ui-kit/-/ui-kit-0.4.0.tgz",
      "registry": "https://registry.neovastra.dev",
      "integrity": "sha512-...",
      "targets": ["web", "typescript"],
      "capabilities": ["dom"],
      "lifecycleScripts": false,
      "dependencies": []
    }
  ],
  "ffi": {
    "math_engine": {
      "native": {
        "source": "./native/libmath.so",
        "type": "c-abi"
      }
    }
  }
}
```

Rules:

- Lockfile entries are deterministic.
- Package IDs are source-qualified and canonical.
- Integrity hashes are mandatory for remote packages.
- The lockfile records target-scoped resolution, capabilities, registry origin, and lifecycle-script policy.
- A manifest change that affects resolution requires a lockfile update.
- CI must fail if the lockfile is stale.
- The lockfile must not contain secrets.
- Nova and `astra build` consume the same lockfile.

---

## 5. Dependency model

### 5.1 Sources

V1 supports:

- `path`
- `git`
- `npm`
- `cargo`

Later:

- NuGet
- PyPI
- OCI
- other ecosystem adapters

Module specifiers map deterministically to these sources:

- Dotted paths such as `std.io` resolve inside the project module namespace.
- Relative paths such as `"./thing.ns"` resolve from the importing file.
- `std:` specifiers resolve to built-in modules.
- Bare specifiers resolve through the manifest dependency map.
- A specifier must not escape the project root unless the manifest explicitly allows it.

### 5.2 Target scoping

Dependencies may be limited to targets:

```json
{
  "source": "npm",
  "version": "0.4",
  "targets": ["web", "typescript"]
}
```

A dependency that is not reachable on a target is not downloaded or linked for that target.

### 5.3 Inference

If `targets` is omitted, the compiler may infer the needed variants from static imports and the call graph.

Rules:

- Explicit `targets` wins.
- Inference is static only.
- Dynamic imports and reflection require explicit declarations.
- Resolved variants are recorded in the lockfile.

### 5.4 Versioning

- npm and Cargo dependencies use their ecosystem’s semver rules.
- Path dependencies are resolved directly.
- Git dependencies must pin a commit in the lockfile.
- Floating branches are allowed only in development mode, never in a locked release build.

---

## 6. Package adapter contract

Each adapter must provide:

1. Resolve a package request to a concrete source.
2. Fetch the source.
3. Verify integrity.
4. Read package metadata.
5. Map target compatibility.
6. Expose type information where available.
7. Report side-effect metadata.
8. Produce a lockfile entry.

Adapters must not:

- Run lifecycle scripts by default.
- Modify the project without explicit action.
- Escape the project root.
- Hide transitive dependencies.
- Claim tree-shaking guarantees they cannot support.

### 6.1 npm

- Read `package.json`.
- Use `sideEffects` when present.
- Generate TypeScript declarations where possible.
- Do not run `postinstall` unless explicitly enabled.

### 6.2 Cargo

- Read `Cargo.toml` and `Cargo.lock`.
- Resolve native libraries through the C ABI.
- Do not assume Rust types cross the boundary.
- Require explicit FFI declarations.

### 6.3 Path and git

- Path packages are local and must not escape the project root without explicit permission.
- Git packages must pin a commit.
- Git submodules are not fetched automatically.

---

## 7. FFI contract

### 7.1 Source declaration

```ns
foreign module math_engine {
    fn compute_matrix(data: [f64]) -> [f64]
}
```

### 7.2 Manifest mapping

Each target must map the module:

```json
{
  "ffi": {
    "math_engine": {
      "native": {
        "source": "./native/libmath.so",
        "type": "c-abi",
        "ownership": "borrowed"
      },
      "typescript": {
        "source": "./npm-package/index.js",
        "type": "esm"
      }
    }
  }
}
```

### 7.3 Required metadata

- `type`: `c-abi`, `esm`, `wasm-import`, `wasm-export`, `inline-js`, `inline-ts`, `inline-c`, `inline-cpp`, `inline-python`
- `source`: path or package specifier
- `ownership`: `borrowed`, `owned`, `transferred`
- `threading`: `safe`, `single-threaded`, `unknown`
- `errors`: `status-code`, `exception`, `result-tag`
- `abi`: target-specific ABI identifier where applicable

If the compiler cannot prove safety, the mapping must be marked unsafe and used only in `system` mode.

### 7.4 Type mapping

| Neo Astra | JS/TS | C ABI |
|---|---|---|
| `i32`, `i64`, `f64` | `number` / `bigint` | fixed-width / `double` |
| `str` | `string` | `const char*` + length |
| `bytes` | `Uint8Array` | pointer + length |
| `[T]` | `Array<T>` | pointer + length |
| `Option<T>` | `T \| null` | tagged or nullable |
| `T ! E` | mapped exception/result | error tag + payload |

Conversion must be explicit and tested.

### 7.5 Inline script mappings

An inline `@script` block is a named FFI module whose implementation lives in a
`.ns` file. Its manifest entry uses `source: "inline"` and an `inline-*` type:

```json
{
  "ffi": {
    "math": {
      "native": {
        "type": "inline-c",
        "source": "inline",
        "ownership": "borrowed",
        "threading": "single-threaded",
        "errors": "status-code"
      }
    }
  }
}
```

Rules:

- `"inline"` is a sentinel, not a path.
- Native and Python mappings require ownership, threading, and error metadata.
- A JS/TS mapping is optional when the implementation and target are both JS/TS;
  when present, it must agree with the source.
- The mapping target must support the block's language.
- A missing required mapping or an inconsistent mapping is `NS7007`.
- Inline scripts are project source. They are not downloaded, installed, or run
  as package lifecycle hooks.

The full design is in
[`10-inline-script-interop.md`](./10-inline-script-interop.md).

---

## 8. Tree-shaking

### 8.1 Compiler-owned code

Precisely shaken on the IR call graph.

### 8.2 External packages

Shaken only as well as metadata allows.

Rules:

- Respect `sideEffects` for npm.
- Allow package authors to declare purity.
- Treat unknown packages as opaque.
- Report what was included and why.
- Do not claim exact shaking for arbitrary packages.

### 8.3 Honest promise

> Compiler-owned code is precisely shaken. External packages are shaken as well as their metadata allows.

---

## 9. Security

- Lifecycle scripts are disabled by default.
- Remote packages require integrity hashes.
- Path dependencies cannot escape the project root without explicit permission.
- Plugins are opt-in.
- FFI mappings declare trust and ownership.
- Inline `@script` bodies are project source and remain unsafe, unchecked boundaries.
- Secrets never enter the manifest or lockfile.
- CI should run `astra check` before any build.

---

## 10. Versioning and migration

- Manifest schema is versioned.
- Lockfile format is versioned.
- The compiler reports migration steps for older schemas.
- A V1 compiler must reject a newer unsupported schema.
- The language edition is separate from the manifest schema version.

---

## 11. Open manifest questions

1. What is the canonical schema domain?
2. Is `edition` numeric or year-based?
3. Should `packageSources` be top-level or nested under `dependencies`?
4. Should `outDir` be per-target or per-build?
5. What is the exact FFI ownership metadata syntax?
6. Should package adapters be plugins or built-in modules in V1?
7. How are native libraries located on Windows, Linux, and macOS?
8. Should `core.lock.json` include toolchain hashes?
9. What is the lockfile conflict strategy?
10. Should `astra fmt` canonicalize key order in `core.json`?
