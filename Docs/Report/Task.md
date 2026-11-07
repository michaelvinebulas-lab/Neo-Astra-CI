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

## Status: OPEN - KILO

Read the task below, write your `## Kilo log` entry, then code. When
`cargo fmt --all --check`, `cargo clippy -p astra-cli --all-targets -- -D
warnings`, and `cargo test -p astra-cli` are green, write the report in the
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
- the workspace `members` entry in `Cargo.toml`
- `Cargo.lock` changes caused by the new crate

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
- Status set to `DONE - lead`.
- Do not commit or push.

## Later orders

After this lands, the lead will open separate orders for:

1. project manifest discovery and project `check`
2. native `build` and `run`
3. `doctor` and toolchain resolution
4. NoPM dispatch and JS/TS codegen fallback

Do not start those here.

---

## Kilo log

(append here)
