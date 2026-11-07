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

## Status: DONE - lead

Read both tasks below, write your `## Kilo log` entry, then code. Finish the
CLI task first, then the LSP task. Do not leave either one partial. When all
four gates are green - `cargo fmt --all --check`, `cargo clippy -p astra-cli
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
