# PV-09 — Explainability

**Status:** Strong-ish
**Pillar:** Regular grammar, one canonical AST, structured diagnostics, `--json`, `astra explain`, canonical formatter, LSP.

## Original promise

> Explicit where it matters, inferred where it is obvious, explainable everywhere.
> — `Docs/archive/architecture.md` §2.5.2

> Structured diagnostics, machine-readable compiler output, canonical formatter, deterministic builds.
> — `Docs/archive/architecture.md` §2.5.2 rules 5-9

## What we can currently show

- Structured `NSxxxx` diagnostics with codes, spans, hints, and related spans; the error model was recently reworked.
- `astra check --json`, `astra doctor --json`, `astra build --json`, `ast --json`, `explain`.
- Canonical formatter with `--check` and edition-aware behavior; `canonical` command is planned for `1.2`.
- LSP phases 1-3 are implemented but not packaged or in the release artifact.

## Proof program

`conformance/vision/pv09-tooling.ns` combines a deprecated spelling, a type error, and a capability tag.

Run:

```sh
astra check --json .            > out/check.json
astra ast --json .              > out/ast.json
astra explain .                 > out/explain.txt
astra fmt --check .
astra canonical . --edition 0.2 > out/canonical.json
astra migrate . --change-edition --dry-run
```

A test LSP client opens the same file and requests diagnostics and formatting.

## Gate

1. Every diagnostic has a stable code, span, severity, message, and hint; snapshots are checked in.
2. `ast --json` is deterministic across runs and across native/Wasm cores.
3. `explain` names the exact source lines that forced the inferred mode.
4. `fmt --check` is stable and idempotent; `canonical` and `migrate` agree on what is deprecated.
5. The LSP returns the same diagnostics and formatted text as the CLI, and the server is packaged.

## Work implied

1. Finish packaging the LSP into the release artifact.
2. Add JSON snapshot tests for the compiler API.
3. Wire the `1.2` playground panels to this proof so browser output is checked too.

## Task list

- [ ] Add JSON snapshot tests for `check --json`, `ast --json`, and `explain`.
- [ ] Verify deterministic AST output across native and Wasm cores.
- [ ] Verify `explain` names the exact source lines that forced the inferred mode.
- [ ] Verify `fmt --check` is stable and idempotent.
- [ ] Verify `canonical` and `migrate` agree on deprecated spellings.
- [ ] Package the LSP into the release artifact.
- [ ] Add an LSP client parity test against CLI diagnostics and formatting.
- [ ] Wire the playground panels into this proof.
- [ ] Record evidence and change status to `Proven`.

## Release code

`PV-09`. Release target: TBD. `1.2` covers the tooling subset only.
