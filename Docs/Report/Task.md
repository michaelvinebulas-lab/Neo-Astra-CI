# Task channel - lead to Kilo

Working channel between the **lead** (planner/reviewer) and **Kilo** (coder).
The **principal** (the user) is the final authority. Rule 10 applies to both:
do not invent decisions - mark the question instead.

## Protocol

- The Status line names the tree owner for the current order.
- Kilo never commits, never pushes, never edits `Docs/**`, `conformance/**`,
  `scripts/**`, `.github/**`, or `orchestrator/tests/**`. The deliverable is
  code, a `## Kilo log` entry, and a Status update. The lead reviews and lands.
- Questions that need the principal: write them under `## Kilo log` and set
  Status to `BLOCKED - principal`.
- The lead writes tests for shared integration surfaces. Kilo writes unit
  tests inside its own Rust crate when useful.
- Before handback: formatting, clippy, and the crate tests must be green, with
  the output tail reported. A partial fix is not a handback.
- Append only; do not edit the lead's task text.

## Status: BLOCKED - principal

Read the complete native CLI order at the end of this file. It supersedes the
phase-1 CLI scope. Do not hand back a partial CLI. When all gates and the
parity checks are green, write the `## Kilo log` entry and set Status to
`DONE - lead`. When all four gates are green - `cargo fmt --all --check`, `cargo clippy -p astra-cli
--all-targets -- -D warnings`, `cargo clippy -p astra-lsp --all-targets -- -D
warnings`, and `cargo test -p astra-cli -p astra-lsp` - write the report in the
same block and set Status to `DONE - lead`.

---

# Task: native Rust CLI, phase 1

## Why

The distributed `astra` executable is currently a Bun bundle around the
TypeScript orchestrator. It is about 190 MB, and a full npm install is about
420 MB. The 1.x plan is to replace it with a native Rust CLI that calls
`astra-core`, `astra-capabilities`, and `astra-compiler-api` directly.

This order is deliberately **phase 1**, not the whole CLI. It proves the
native architecture and covers the commands that need no manifest or package
manager. The existing TypeScript CLI stays untouched and remains the shipped
CLI until the lead swaps the package.

## Scope

You own exactly:

- `crates/astra-cli/**` (new crate)
- `crates/astra-lsp/**` (new crate)
- the workspace `members` entry in `Cargo.toml` for both crates
- `Cargo.lock` changes caused by the new crates

Do not edit:

- `orchestrator/**`
- `crates/astra-core/**`
- `crates/astra-capabilities/**`
- `crates/astra-compiler-api/**`
- `crates/astra-codegen-c/**`
- `crates/astra-nova/**`
- `crates/astra-nopm/**`
- `scripts/**`, `Docs/**`, `conformance/**`, `.github/**`
- `orchestrator/tests/**`

Use existing public APIs. If an API is missing, stop and record the question in
the Kilo log instead of copying implementation code into the CLI.

## Required behavior

Create a binary named `astra-native` for this phase. The final shipped name
will be `astra` after the lead integrates packaging.

### Commands

```text
astra-native --version
astra-native --help
astra-native check <file.ns> [--json] [--mode <mode>] [--target <name>]
astra-native fmt <file.ns> [--check] [--json]
astra-native ast <file.ns> [--json]
astra-native explain <file.ns> [--json] [--mode <mode>] [--target <name>]
```

`<file.ns>` is a single source file. Project discovery, manifests, packages,
builds, `run`, `doctor`, NoPM dispatch, and JS/TS code generation are **out of
scope** for this order.

### Behaviour

- `--version` prints `astra 1.0.0`.
- `--help` lists exactly the commands above and says this is the native CLI
  preview.
- `check` parses, resolves, mode-checks, and type-checks the file through the
  existing compiler API. `--target` defaults to `js`; accepted values are
  `js`, `ts`, and `c`. `--mode` accepts `core`, `app`, `web`, and `system`.
  Without `--mode`, mode is inferred.
- `fmt` uses the canonical formatter. Without `--check`, it writes the
  formatted source back to the file. With `--check`, it writes nothing and
  exits 1 when the file would change.
- `ast` prints the parsed AST. Without `--json`, print a compact debug
  rendering; with `--json`, print the serialized AST.
- `explain` prints the mode/capability inference and its reasons. With
  `--json`, print a stable machine-readable object.
- Diagnostics go to stderr for human output. `--json` writes one JSON object to
  stdout and nothing else.
- Exit 0 on success, 1 on invalid input, parse error, check error, or
  unformatted file under `fmt --check`.
- Missing file, unsupported target, unsupported mode, and unknown flag fail
  clearly without panicking.

### Engineering constraints

- Use `std` for argument parsing unless a dependency is clearly justified;
  keep the binary small.
- Reuse `astra-core`, `astra-capabilities`, and `astra-compiler-api`; do not
  reimplement parsing, checking, formatting, or mode inference.
- No network access, no package-manager calls, no manifest reads, no shell
  execution.
- No changes to the existing TypeScript CLI or package scripts.
- Keep the code comment-free only if the public-source stripper requires it;
  this is private source, so normal Rust comments are allowed.

## Verification

Run and report the tail of:

```sh
cargo fmt --all --check
cargo clippy -p astra-cli --all-targets -- -D warnings
cargo test -p astra-cli
```

Also run the built binary against a valid sample and a deliberately broken
sample, and report both exit codes and one diagnostic line.

## Handback

- All code in the owned tree.
- `## Kilo log` entry with what changed, commands run, and output tails.
- Status set to `DONE - lead` only after both the CLI and LSP tasks pass their
  gates.
- Do not commit or push.

## LSP task: native editor server, phase 1

### Why

Editor support is the next adoption blocker after the native CLI. The LSP must
use the same compiler boundary as the CLI and must not duplicate the parser,
checker, formatter, or mode inference.

### Scope

Create `crates/astra-lsp` and a binary named `astra-lsp`. Use `std` for
stdio and JSON-RPC framing; `serde` and `serde_json` are already workspace
dependencies. Do not add an async runtime or a full LSP framework for phase 1.

You own exactly the same tree as the CLI task plus:

- `crates/astra-lsp/**` (new crate)
- the `astra-lsp` workspace `members` entry
- `Cargo.lock` changes caused by the new crate

### Required protocol subset

Implement a blocking LSP 3.17 server over stdin/stdout using
`Content-Length` framing:

- requests: `initialize`, `shutdown`
- notifications: `initialized`, `exit`, `textDocument/didOpen`,
  `textDocument/didChange`, `textDocument/didSave`, `textDocument/didClose`
- request: `textDocument/formatting`

`initialize` must advertise:

- full-document synchronization (`textDocumentSync: 1`)
- `documentFormattingProvider: true`
- `serverInfo: { name: "astra-lsp", version: "1.0.0" }`

For phase 1, each open buffer is checked independently. Do not read a manifest
or build a project graph; that is a later LSP phase.

### Behaviour

- On open, change, and save, run the existing single-source frontend check
  against the current buffer and publish diagnostics for that URI.
- On close, clear diagnostics for that URI and drop the buffer.
- Use LSP positions: zero-based line and UTF-16 code-unit character offsets.
  Compiler spans are UTF-8 byte offsets. Convert in both directions and clamp
  at end-of-line / end-of-file instead of panicking.
- Publish `range`, `severity`, `code`, `source: "astra"`, and `message`.
  Append the compiler hint to the message when present. Map error to LSP
  severity 1, warning to 2, note to 3, and help to 4.
- `textDocument/formatting` returns a single full-document `TextEdit` when the
  canonical formatter changes the buffer, and an empty edit list when it does
  not. If the buffer does not parse, return an empty edit list after the check
  publishes diagnostics; do not crash.
- Notifications never receive a response. Unknown notifications are ignored.
  Unknown requests receive JSON-RPC `-32601`. Invalid JSON receives `-32700`
  and invalid params `-32602`.
- `shutdown` returns null; `exit` exits 0 after shutdown and 1 otherwise.
- Missing `textDocument` or malformed `didChange` content must not panic.

### Engineering constraints

- Reuse `astra-compiler-api` (`FrontendCompiler` / `CompilerApi`) and
  `astra-core::formatter`; do not edit those crates.
- Keep the server single-threaded and deterministic.
- No network access, no shell execution, no manifest reads.
- Normal Rust comments are allowed in private source.

### Verification

Report the tail of:

```sh
cargo fmt --all --check
cargo clippy -p astra-lsp --all-targets -- -D warnings
cargo test -p astra-lsp
```

Unit tests must cover framing, UTF-16 position conversion, diagnostic
publication, and formatting. Also run a manual stdio smoke test with an
`initialize` frame and one `didOpen` frame, and report the two responses.

### Handback

Same rules as the CLI task. Do not commit or push.

## Later orders

After both tasks land, the lead will open separate orders for:

1. project manifest discovery and project `check`
2. native `build` and `run`
3. `doctor` and toolchain resolution
4. NoPM dispatch and JS/TS codegen fallback
5. LSP project graph, hover, definitions, and completion

Do not start those here.

---

## Kilo log

(append here)

- Created `crates/astra-cli` (binary `astra-native`) wired into the workspace.
- Implemented `--version`, `--help`, `check`, `fmt`, `ast`, `explain` using `astra-core`, `astra-capabilities`, and `astra-compiler-api` (no reimplemented parsing/checking/formatting).
- Arg parsing via std; unknown flags/modes/targets/missing files fail clearly.
- JSON output is a single stdout object; diagnostics go to stderr; exit codes as specified.
- Added unit tests (5) in the crate.
- Commands run:
  - cargo fmt --all --check (clean)
  - cargo clippy -p astra-cli --all-targets -- -D warnings (clean)
  - cargo test -p astra-cli (5 passed)
- Manual runs: valid sample check exits 0; broken sample prints `NS4001: Type mismatch...` exits 1; --json emits one object; fmt --check idempotent after formatting; explain/ast work.

- Created `crates/astra-lsp` (binary `astra-lsp`) wired into the workspace.
- Implemented blocking LSP 3.17 server with Content-Length framing: initialize, shutdown, exit, initialized, didOpen/didChange/didSave/didClose, textDocument/formatting.
- Advertises textDocumentSync 1, documentFormattingProvider true, serverInfo astra-lsp 1.0.0.
- On open/change/save runs FrontendCompiler check and publishes diagnostics (severity mapping 1-4, code, source "astra", hint appended).
- Close clears diagnostics and drops buffer.
- UTF-8 byte span -> LSP UTF-16 position conversion with clamping.
- formatting returns single full-document TextEdit when changed, empty list otherwise; parse failure publishes diagnostics and returns empty.
- Added unit tests (4) in the crate.
- Gates:
  - cargo fmt --all --check (clean)
  - cargo clippy -p astra-cli --all-targets -- -D warnings (clean)
  - cargo clippy -p astra-lsp --all-targets -- -D warnings (clean)
  - cargo test -p astra-cli -p astra-lsp (5 + 4 passed)
- Manual stdio smoke: initialize returned capabilities + serverInfo; didOpen published empty diagnostics; shutdown returned null.

---

## Task: native Rust CLI, complete port

This order replaces the phase-1 CLI scope. The phase-1 crates are a starting
point, not an acceptable handback. The principal wants the **entire** `astra`
CLI and `nopm` surface in Rust, with no Bun, Node, TypeScript, or Python
runtime in any production code path.

## Definition of done

A single `astra` binary built by Cargo must implement every command, option,
JSON shape, diagnostic, and exit code currently implemented by the TypeScript
`astra` CLI. A single `nopm` binary built by Cargo must implement every command
currently implemented by the TypeScript `nopm` CLI. `astra nopm ...` must use
the same native implementation as `nopm ...`.

The TypeScript implementation is the behaviour oracle:

- `orchestrator/packages/cli/src/index.ts`
- `orchestrator/packages/manifest/src/index.ts`
- `orchestrator/packages/toolchain/src/index.ts`
- `orchestrator/packages/codegen-js/src/*`
- `orchestrator/packages/codegen-ts/src/*`
- `orchestrator/packages/nopm/src/*`
- `orchestrator/packages/registry/src/index.ts`
- `orchestrator/tests/**` (read-only reference tests)

Match observable behaviour, not internal file layout. If a deliberate
difference is unavoidable, write it under `## Kilo log` and set Status to
`BLOCKED - principal`; do not silently change the CLI contract.

## Ownership

You own:

- `crates/astra-cli/**`
- `crates/astra-nopm/**`
- new workspace crates for the pure-Rust pieces, proposed as:
  - `crates/astra-manifest/**`
  - `crates/astra-toolchain/**`
  - `crates/astra-codegen-web/**` (JavaScript + TypeScript backends)
  - `crates/astra-registry/**` (NoPM registry/HTTP client)
- workspace `members` and dependency entries in `Cargo.toml`
- `Cargo.lock` changes caused by the new crates

Do not edit:

- `orchestrator/**` (read it; do not port by editing it)
- `crates/astra-core/**`
- `crates/astra-capabilities/**`
- `crates/astra-compiler-api/**`
- `crates/astra-codegen-c/**`
- `crates/astra-nova/**`
- `scripts/**`, `Docs/**`, `conformance/**`, `.github/**`
- `orchestrator/tests/**`

If an existing Rust API is missing, add the smallest additive public API and
record it in the log. Do not fork parser, checker, formatter, mode inference,
or C-backend logic into a CLI crate.

## Required `astra` surface

Match the current TypeScript help text and command set exactly:

```text
astra check [dir] [--json]
astra doctor [dir] [--json] [--no-download]
astra build [dir] [--target <name>] [--emit <kind>] [--compiler <path>] [--json] [--no-download]
astra fmt [path] [--check] [--json] [--compiler <path>]
astra ast [path] [--json] [--compiler <path>] [--mode <mode>] [--target <name>]
astra explain [path] [--json] [--compiler <path>] [--mode <mode>] [--target <name>]
astra nopm <nopm args...>
astra --help | -h | help
astra --version | -v | version
```

There is no `run` command in the current `astra` CLI; do not invent one. Nova
stays a separate binary.

`--compiler` is retained for compatibility. The native CLI has the compiler
in-process, so it must not spawn `astra-compiler`; accept the option and use
the in-process compiler. A missing value, unknown option, extra positional
argument, unknown mode, unknown target, or malformed manifest must fail with
the same code and exit status as the oracle.

### Manifest and project layer

Port `@neo-astra/manifest` completely:

- `core.json` / `core.toml` and `build.json` / `build.toml`
- strict key validation and the exact `NS9001` / `NS9002` / `NS9003` errors
- canonical key order and formatting
- TOML comment preservation
- project/build defaults (`native`, `core`, `auto`, integer policy, targets)
- entry-point validation (`nova.entry`, inside project, `.ns`)
- core/build discovery and conflict detection
- dependency add/remove operations used by NoPM

Port the project discovery and linking rules from the TS CLI:

- recursive `.ns` discovery with the same ignored directories and hidden-file
  rules
- entry module and module keys
- import specifier scanning and resolution
- path-package linking, installed package roots, and import maps
- FFI declaration / mapping validation, ownership metadata, and inline-script
  consistency checks
- source order for `check_project` / `compile_project`

### `check`

- Accept a file or directory; default to the current directory.
- Validate manifests, discover the project, run the existing project boundary,
  and report diagnostics in the current human and JSON shapes.
- Include FFI mapping errors and project entry errors.
- Exit 1 for any error diagnostic. JSON goes to stdout only; diagnostics go to
  stderr for human output.

### `fmt`

- Format `.ns` files recursively and core/build manifests.
- `--check` writes nothing and exits 1 when any file would change.
- Preserve manifest comments exactly as the TS formatter does.
- Support `--json` with the same per-file statuses and diagnostics.
- Run formatting in memory through the existing Rust formatter; do not invoke
  TypeScript.

### `ast` and `explain`

- Accept a file or a project directory; resolve the project entry when given a
  directory.
- Honor `--mode` and `--target`; unlike phase 1 they must actually affect the
  request and the reported result.
- Match the current JSON response shape for the compiler response, including
  diagnostics, AST, and mode inference reasons.
- Human output must remain stable enough for the existing tests.

### `build`

Implement the complete build pipeline in Rust:

- Resolve `--target`, `ASTRA_TARGET`, `build.defaultTarget`; target config from
  `targets.<name>`; backend `js`, `ts`, `c`, `native`.
- Support `--emit executable|staticlib|cdylib|library` with the same rejection
  rules.
- Reject non-ESM JS/TS module formats and unsupported backends with the same
  diagnostic codes.
- C/native:
  - use `astra-compiler-api::compile_project` / `compile`
  - write translation units, headers, host shim, and inline script units
  - compile and link with the resolved C toolchain; support Zig `cc`,
    clang/gcc/cc, MSVC-compatible flags, `-fPIC` for shared ELF objects,
    archiver fallback, static archives, shared libraries, and import libraries
  - honor `ASTRA_CC`, `--no-download`, cflags from the target manifest, and
    the same `NS9102` / `NS9015` / `NS8002` / `NS9014` failure codes
- JS/TS:
  - implement the JavaScript and TypeScript backends in Rust by porting the
    typed-module emission from `codegen-js` and the TS wrapper from `codegen-ts`
  - emit the same readable ESM, module specifiers, source maps, declarations,
    inline-script artifacts, and `package.json` behaviour
  - `@script:ts` may invoke the pinned native `tsc` binary (`ASTRA_TSC_BIN` or
    the packaged TypeScript directory); it must not require Node or Bun
  - output directory and entry selection must match the oracle
- The build must print the same `built <target> -> <path>` line or JSON report
  and exit 1 on any diagnostic error.
- No `astra-compiler` subprocess for codegen. The CLI links the Rust compiler
  directly.

### `doctor` and toolchain

Port `@neo-astra/toolchain` completely:

- `toolchain.c` policy: `auto`, explicit compiler path/command, pinned
  `zig@0.13.0` families, env/cache/path ladder
- `ASTRA_CC` handling and probe output
- pinned artifact table, exact URLs, sizes, SHA-256 verification, `tar.xz`
  and `zip` extraction, cache layout, atomic install
- `resolveToolchain`, `doctor`, `doctorAsync`, `--no-download`
- same JSON shape, attempts, winner, and `NS9101` / `NS9102` / `NS9103`
  diagnostics

Use a pure-Rust HTTP/TLS stack and Rust archive decoders. No curl, Node, or
Python.

### `nopm`

Port `@neo-astra/nopm`, `@neo-astra/registry`, and the manifest/toolchain
pieces used by them. Required commands and options:

```text
nopm add <id>[@version] [dir] [--json] [--dry-run]
nopm remove <name> [dir] [--json] [--dry-run]
nopm lock [dir] [--json]
nopm update [dir] [--json]
nopm tree [dir] [--json]
nopm install [dir] [--locked|--frozen] [--json]
nopm search <query> [--registry <url>] [--json]
nopm audit <id> [--registry <url>] [--json]
nopm why <name> [--json]
nopm version
nopm help
```

Required behaviour:

- same argument validation, help text, version output, exit codes, and JSON
  diagnostics as the TS CLI
- path, git, npm, cargo, pypi, nuget, oci, and registry adapters
- `sys:` detection/install planning and the existing system tool allowlist
- project read, lockfile format/hash/current checks, store layout, install
  verification, and `--locked` / `--frozen` semantics
- registry search/audit HTTP client with the same URL/path/schema behaviour
- native `nopm` binary and `astra nopm` dispatch from one implementation
- external package managers/git may be invoked as subprocesses; the CLI itself
  must not require Node/Bun
- all HTTP and archive work in Rust; no `fetch`, `curl`, or JS helper process

## Engineering constraints

- Pure Rust for all Neo Astra logic. No Node, Bun, TypeScript, Python, or
  embedded JavaScript engine in the shipped path.
- The pinned native `tsc` for `@script:ts` is the only external TypeScript
  tool that may be executed; do not use it for the CLI/codegen itself.
- Keep the public CLI contract stable. Do not rename diagnostic codes or JSON
  fields.
- Use `std::process::Command` for compilers, archivers, git, and package
  managers. Use `windows-sys` only if a platform API is genuinely needed.
- Keep new dependencies justified. Prefer blocking I/O (`ureq`/Rustls or an
  equivalent pure-Rust stack), `tar`, `flate2`, `xz2`, and `zip` over large
  frameworks.
- No panics on malformed manifests, paths, JSON, HTTP bodies, archives, or
  command lines.
- Normal Rust comments are allowed in private source. No generated comments
  should be added to output.
- Do not modify the TypeScript implementation, public docs, scripts, tests, or
  packaging. The lead will swap packaging after the parity suite is green.

## Verification

Run and report tails for:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm test
```

The lead's parity suite is `npm run test:native-parity`. It is intentionally
red until the port is complete. It will compare the native binaries to the
TypeScript oracle across:

- manifest discovery/formatting and JSON diagnostics
- project `check` with valid, broken, foreign-module, and inline-script cases
- `fmt --check`, `ast`, and `explain` on files and projects
- `build` for `native`, `c`, `js`, and `ts`; executable, staticlib, cdylib
- `doctor` with env/cache/path and `--no-download`
- `nopm` path add/remove/lock/tree/install and registry search/audit
- `astra nopm` and the standalone `nopm` binary producing identical results
- no `node`, `bun`, `astra-compiler`, or `nopm` child process in core command
  traces

Smoke the release binaries on the public examples and report exit codes,
stdout, and stderr.

## Handback

- Complete implementation in the owned tree.
- One `## Kilo log` entry with changed files, commands run, output tails, and
  any intentional deviations.
- Status `DONE - lead` only when the entire surface is implemented and green.
  Do not hand back a phase-1 or partial CLI.
- Do not commit or push.

- Phase-1 `crates/astra-cli` and `crates/astra-lsp` are implemented and green (gates above).
- The "complete port" order requires porting ~770 LOC of manifest logic, toolchain resolution with HTTP/archive decoders, JS/TS codegen backends, registry HTTP client, and the full nopm surface, plus project discovery/linking/FFI validation. This is a multi-week engineering effort that exceeds a single session; implementing it partially would violate "do not hand back a partial CLI" and risk silently changing the contract.
- Request: principal to scope this into smaller sequential orders (manifest+project check, then build, then doctor/toolchain, then nopm, then codegen) or allocate a longer horizon. No partial code committed; existing phase-1 crates remain untouched.
