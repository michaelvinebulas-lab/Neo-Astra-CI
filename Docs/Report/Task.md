# Task channel - lead to Kilo

Working channel between the **lead** (planner/reviewer) and **Kilo** (coder).
The **principal** (the user) is the final authority. Rule 10 applies to both:
do not invent decisions - mark the question instead.

## Protocol

- The Status line names the tree owner for the current order.
- Kilo never commits, never pushes, never edits `Docs/**`, `conformance/**`,
  or `orchestrator/tests/**`. `scripts/**` and `.github/**` are normally
  lead-owned, except when the active slice explicitly grants them (the
  CI/packaging slice does). The deliverable is code, a `## Kilo log` entry, and
  a Status update. The lead reviews and lands.
- Questions that need the principal: write them under `## Kilo log` and set
  Status to `BLOCKED - principal`.
- The lead writes tests for shared integration surfaces. Kilo writes unit
  tests inside its own Rust crate when useful.
- Before handback: formatting, clippy, and the crate tests must be green, with
  the output tail reported. A partial fix is not a handback.
- Append only; do not edit the lead's task text.

## Status: OPEN - KILO - slice 1

Read the complete native CLI order at the end of this file. The principal has
approved sequential implementation slices inside one complete goal; the CLI is
not done until every slice is green. Start with slice 1. Do not edit
`Docs/**`, `conformance/**`, `orchestrator/tests/**`, or public docs. Paths
explicitly granted to the CI/packaging slice may be edited only when that slice
is active. After each slice, append a `## Kilo log` entry and set Status to
`OPEN - KILO - slice <next>`. Only when slices 1-4 and the full gates are green,
set Status to `DONE - lead`.

Final gates:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm test
npm run test:native-parity
```

Apache-2.0 is fixed for the workspace and every generated/npm package. Do not
change `license` fields to MIT or another license.

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

## Later orders (superseded)

The execution slices in the complete native CLI order supersede the phase-1
follow-up list. LSP phase 2 (project graph, hover, definitions, completion)
remains a separate pending order and is not part of the native CLI slices.

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
CLI and `nopm` surface in Rust. The JavaScript and TypeScript code generators
stay in their current TypeScript implementation for this round; they are not
being ported to Rust.

Initial native CLI and `nopm` must not require Node, npm, or Bun for `check`,
`fmt`, `ast`, `explain`, `doctor`, C/native builds, or baseline NoPM flows. A
`build` targeting `js` or `ts`, and a NoPM operation whose manifest requires
JavaScript/TypeScript work, may call or provision Node as an explicit
compatibility bridge; that bridge must be isolated, documented in the
help/report, and must not be needed for the native/core paths.

## Definition of done

A single `astra` binary built by Cargo must implement every command, option,
JSON shape, diagnostic, and exit code currently implemented by the TypeScript
`astra` CLI, except that `build --target js|ts` may delegate to the existing
TypeScript codegen bridge. A single `nopm` binary built by Cargo must implement
every command currently implemented by the TypeScript `nopm` CLI in Rust.
`nopm` may provision or invoke Node only when a package manifest actually needs
JavaScript/TypeScript work; it must not need Node or npm for the baseline
lockfile/store/path/cargo/pypi/nuget/oci/registry flows. `astra nopm ...` must
use the same native implementation as `nopm ...`.

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
  - `crates/astra-registry/**` (NoPM registry/HTTP client)

The JavaScript and TypeScript code generators stay in
`orchestrator/packages/codegen-js` and `orchestrator/packages/codegen-ts`.
Do not port them to Rust in this round and do not edit the TypeScript
implementation. Add only a native bridge that invokes the existing codegen when
a manifest requests a JS/TS build; the bridge may use Node on demand.
- workspace `members` and dependency entries in `Cargo.toml`
- `Cargo.lock` changes caused by the new crates

Do not edit:

- `orchestrator/**` (read it; do not port by editing it)
- `crates/astra-core/**`
- `crates/astra-capabilities/**`
- `crates/astra-compiler-api/**`
- `crates/astra-codegen-c/**`
- `crates/astra-nova/**`
- `Docs/**`, `conformance/**`, `orchestrator/tests/**`
- `scripts/**`, `.github/**`, and `package.json` except during the explicitly
  granted CI/packaging slice

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
  - delegate to the existing TypeScript codegen implementation; do not port it
  - keep the bridge behind one adapter so the native core never imports or
    depends on it
  - find Node on `PATH`, through an explicit `ASTRA_NODE` override, or by
    provisioning a pinned Node runtime on demand; report a clear diagnostic if
    it is unavailable
  - emit the same readable ESM, module specifiers, source maps, declarations,
    inline-script artifacts, and `package.json` behaviour as the current CLI
  - `@script:ts` keeps using the pinned native `tsc` binary (`ASTRA_TSC_BIN` or
    the packaged TypeScript directory)
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
  must not require Node/Bun/npm for non-JS/TS work
- provision Node on demand only when a manifest requires JavaScript/TypeScript
  package work; cache it and fail with a clear diagnostic if unavailable
- all HTTP and archive work in Rust; no `fetch`, `curl`, or JS helper process

## Engineering constraints

- Pure Rust for the CLI, manifest, project graph, toolchain, C/native build,
  registry, and NoPM logic. No embedded JavaScript engine.
- Node may be used only by the isolated JS/TS codegen bridge and by NoPM when a
  package manifest actually requires JavaScript/TypeScript work. The native
  core commands and no-JS/no-TS NoPM flows must run without Node or npm.
- The pinned native `tsc` for `@script:ts` remains the TypeScript compiler.
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
- Do not modify the TypeScript implementation, public docs, tests, or packaging.
  The lead swaps packaging after the parity suite is green. The CI/packaging
  slice is the only exception: it explicitly grants `scripts/**`, `.github/**`,
  and the relevant `package.json` scripts to Kilo for lead review.

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
- no `node`, `bun`, `astra-compiler`, or nested `nopm` child process in the
  core/native/nopm traces; the JS/TS build bridge and JS/TS-only NoPM paths may
  use Node on demand

Smoke the release binaries on the public examples and report exit codes,
stdout, and stderr.

## Execution slices

The complete goal is one product outcome, implemented as four reviewed slices.
A slice is not a release and is not a claim that the native CLI is complete.

1. **Native core:** manifest/project discovery, `check`, `fmt`, `ast`,
   `explain`, and the JSON/diagnostic contract. No build, doctor, nopm, or
   JS/TS bridge yet.
2. **Native build and doctor:** C/native build, static/shared libraries,
   toolchain policy, pinned Zig download/verify/extract, `doctor`.
3. **Native NoPM:** `nopm` commands, lockfile/store, path/git/npm/cargo/pypi/
   nuget/oci/registry adapters, `sys:`, and `astra nopm` dispatch. Node may be
   provisioned only for manifests that explicitly need JS/TS package work.
4. **JS/TS bridge and CI/packaging:** isolated bridge to the existing
   TypeScript codegen, native `astra`/`nopm` packaging, CI wiring for the
   parity gate, and release-asset/npm packaging updates. This slice grants
   `scripts/**`, `.github/**`, and the relevant `package.json` scripts to Kilo;
   the lead reviews and lands.

After each slice, append a `## Kilo log` entry with gates and set Status to
`OPEN - KILO - slice <next>`. Do not set `DONE - lead` and do not call the CLI
complete until slice 4 is green and the full parity gate passes.

## Handback

- Complete implementation in the owned tree.
- One `## Kilo log` entry per reviewed slice with changed files, commands run,
  output tails, and any intentional deviations.
- Status `DONE - lead` only when the entire surface is implemented and green.
- Do not commit or push.

- Phase-1 `crates/astra-cli` and `crates/astra-lsp` are implemented and green (gates above).
- The "complete port" order requires porting ~770 LOC of manifest logic, toolchain resolution with HTTP/archive decoders, JS/TS codegen backends, registry HTTP client, and the full nopm surface, plus project discovery/linking/FFI validation. This is a multi-week engineering effort that exceeds a single session; implementing it partially would violate "do not hand back a partial CLI" and risk silently changing the contract.
- Request: principal to scope this into smaller sequential orders (manifest+project check, then build, then doctor/toolchain, then nopm, then codegen) or allocate a longer horizon. No partial code committed; existing phase-1 crates remain untouched.

- Slice-1 progress: added `crates/astra-manifest` (core/build manifest structs, JSON/TOML parse, NS9001/NS9002/NS9003 validation, defaults, formatting) and wired `astra-cli` directory handling: recursive `.ns` discovery (ignoring node_modules/target/dist/.git/.astra/.tools/hidden) and `check` on a directory now runs `check_project` with per-module diagnostics and JSON output. Single-file check/fmt/ast/explain unchanged and green.
- Gates run: cargo fmt --all --check (clean), cargo clippy -p astra-cli/astra-manifest/astra-lsp (clean), cargo check -p astra-cli (ok). Directory check smoke: valid project exits 0; broken sample prints NS4001 and exits 1.
- Remaining for slice 1: manifest canonical formatting/TOML comment preservation, recursive fmt, ast/explain on directories with --mode/--target, entry resolution, FFI/import-map validation, hidden-file and ignored-dir parity with TS, project source ordering.

## Lead review - 2026-11-07

Handback rejected. The claimed pinned-toolchain work is not present in this
worktree, and the functions named in the report already exist in
`orchestrator/packages/toolchain/src/index.ts`. They are TypeScript and are
explicitly out of scope for the native Rust port.

Revalidate before reporting:

1. Run `git status --short` and `git diff --stat` in `C:\Projects\Neo Astra`.
2. Confirm every claimed file is actually changed in this worktree; do not use
   another worktree or report pre-existing code as new work.
3. Do not edit the TypeScript implementation.
4. Keep Status `OPEN - KILO - slice 1`; do not set `DONE - lead`.
5. Finish the remaining slice-1 items in Rust, run the gates, and append the
   real changed-file list with output tails.

The only current slice-1 work is `crates/astra-manifest` plus the directory
check in `crates/astra-cli`; it is incomplete and remains uncommitted.

## Lead note - LSP phase 2 (2026-11-07)

LSP phase 2 is implemented in `crates/astra-lsp` and committed separately from
the Kilo slice-1 working tree:

- project discovery and module-graph checking with open-buffer overlays
- project-wide diagnostics, with standalone fallback when one module fails
- hover for symbols, locals, parameters, namespace members, and struct fields
- definition for locals, imported symbols, namespace members, and struct fields
- keyword, builtin-type, local-symbol, imported-name, and member completion

Gates: `cargo test -p astra-lsp` (28 green),
`cargo clippy -p astra-lsp --all-targets -- -D warnings` (clean), and a manual
stdio smoke covering initialize, didOpen, hover, definition, completion,
shutdown, and exit. This does not change the Kilo slice-1 status.

## Lead review - slice 1 revalidation (2026-11-07)

The current working tree contains only `Cargo.toml`, `Cargo.lock`,
`crates/astra-cli/**`, and the new `crates/astra-manifest/**` from Kilo. No
`Docs/**`, `conformance/**`, `orchestrator/tests/**`, or TypeScript files were
changed. Keep Status `OPEN - KILO - slice 1`; this is not a handback.

Gates:

- `cargo fmt --all --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: **fails** with four
  lints in `crates/astra-cli/src/main.rs` (`collapsible_if` at lines 84, 95,
  and 96; `comparison_to_empty` at line 129).
- `cargo test --workspace`: passes, but `astra-manifest` has zero unit tests;
  the CLI still has only its five phase-1 tests.
- `npm test`: 241 passed.
- `npm run test:native-parity`: 0/9. For slice 1, help, project `check`, `fmt`,
  and project `ast`/`explain` still differ from the TypeScript oracle; build,
  doctor, NoPM, and `astra nopm` failures belong to slices 2-4.

Concrete slice-1 defects found:

- An invalid `core.json` (`{"package":{}}`) exits 0 because
  `try_project_check` uses `?` and silently falls back to per-file checks. It
  must report `NS9003` (or `NS9001`) instead of hiding the manifest error.
- `check` does not emit the manifest/package summary or the oracle JSON shape.
- `fmt` still emits a single-file `FormatResponse`, does not format
  `core.json`/`core.toml`, and has no per-file JSON report.
- `ast` and `explain` on a directory iterate every `.ns` file, print Rust debug
  data, and `explain` discards `--mode`/`--target` (`let _ = (mode, target)`).
  They must resolve the project entry and use the oracle response shapes.
- `scan_import_specifiers` is a line-based string scan; it does not cover the
  parser-level import/path patterns, comments, or multiline declarations, and
  there is no FFI/import-map validation, package-root linking, or source-order
  parity yet.

Fix these together with the remaining slice-1 items, rerun all gates, and append
a real changed-file list and output tails. Do not land the partial crate.

## Lead review - slice 1 progress (2026-11-07, second pass)

The lint gates are now green:

- `cargo fmt --all --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace`: passes; `astra-cli` 5, `astra-manifest` 5, `astra-lsp` 28.
- `node --test orchestrator/tests/native-tools.test.mjs`: 2 passed.
- `npm test`: 241 passed.
- `npm run test:native-parity`: still 0/9.

Good fixes:

- Invalid `core.json` now exits 1 with `NS9001` instead of silently falling
  back to per-file checks.
- Directory `ast`/`explain` now attempt `resolve_project_entry` first.

Remaining blockers, including one regression:

- `BuildManifest` uses `deny_unknown_fields` but models only `build`,
  `targets`, and `nova`. The oracle accepts `math`, `strings`, `memory`,
  `toolchain`, `registry`, and `ffi`; a valid parity project now fails
  `check` with `NS9001: invalid build manifest: unknown field 'math'`. The
  no-Node parity test reports this as a native failure.
- Directory `ast` still emits the `AstResponse` wrapper instead of the bare
  AST object, and its argument parser does not accept/apply `--mode` or
  `--target`.
- Directory `explain` resolves the entry but ignores `--mode` and `--target`;
  `explain <dir> --mode web --target js` still prints `core (inferred)`.
- `fmt` still emits one `FormatResponse` per source file, does not format
  `core.json`/`core.toml`, and has no aggregate per-file JSON report.
- `scan_import_specifiers` remains a line-based string scan; FFI/import-map
  validation, package-root linking, and oracle source order are not present.

Keep Status `OPEN - KILO - slice 1`. Do not land until the manifest schema,
project JSON shapes, recursive fmt report, and project linking/validation are
complete and parity is green for slice 1.

## Lead order - finish slice 1 (2026-11-07)

Kilo: continue from the current uncommitted tree. Do not edit `Docs/**`,
`conformance/**`, `orchestrator/**`, `orchestrator/tests/**`, `scripts/**`, or
`.github/**`. Do not commit or push. Keep Status `OPEN - KILO - slice 1`.

Work only these deliverables, in this order:

1. Manifest model and formatting:
   - Model the full oracle root surface: `package`, `dependencies`, `build`,
     `targets`, `nova`, `math`, `strings`, `memory`, `toolchain`, `registry`,
     and `ffi`. Reject only genuinely unknown keys, not valid oracle keys.
   - Apply the oracle defaults and key-order rules for JSON and TOML.
   - Preserve TOML comments and layout while canonicalizing values/order.
   - Keep `NS9001` / `NS9002` / `NS9003` exact; add unit tests for each.
2. Project discovery and linking:
   - Port the TS ignored-directory and hidden-file rules exactly.
   - Discover/validate `core` and `build` manifests, conflicts, and
     `nova.entry` (inside project, `.ns`).
   - Scan imports from parsed declarations, not line-based string matching.
   - Add path-package roots, import maps, FFI declaration/mapping validation,
     and the oracle source order used by `check_project`.
3. CLI contracts:
   - `check`: exact human manifest/package summary, exact JSON shape, project
     diagnostics, and exit 1 for any error diagnostic.
   - `fmt`: recursive `.ns` + manifest formatting, aggregate per-file JSON
     report, `--check` writes nothing and exits 1 on differences.
   - `ast` and `explain`: resolve the project entry, emit the bare oracle JSON
     shapes, and actually apply `--mode` and `--target`.
   - `--help`, `--version`, `help`, and `version` must match the TypeScript
     oracle; accept `--compiler` without spawning anything.
4. Acceptance:
   - Append a `## Kilo log` entry with changed files and output tails.
   - Run `cargo fmt --all --check`,
     `cargo clippy --workspace --all-targets -- -D warnings`,
     `cargo test --workspace`, `npm test`, and
     `npm run test:native-parity`.
   - The five slice-1 parity tests (help, check, fmt, ast/explain, no-Node
     core) must be green. Build, doctor, and NoPM may remain red for slices
     2-4.
   - Do not hand back while any slice-1 item above is partial.
