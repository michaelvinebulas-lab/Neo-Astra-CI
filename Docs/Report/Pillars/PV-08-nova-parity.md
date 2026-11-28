# PV-08 — Nova parity

**Status:** Unproven
**Pillar:** Nova runs typed `.ns` with the same diagnostics and capability rules as the compiler; it is never a second parser.

## Original promise

> Nova is a development interpreter. It is not the production runtime and never replaces the shared parser or semantic contract.
> — `Docs/archive/Design/V1/README.md` hard invariant 15

> Nova runs typed `.ns` programs directly with the same diagnostics and capability rules as the compiler.
> — `Docs/archive/Design/V1/README.md` release criteria

## What we can currently show

- Typed-HIR interpreter, project module loading/linking, `run`, `eval`, `repl`, `watch`, `test`.
- Conformance runners execute non-gap cases through Nova.
- No proof diffs Nova diagnostics against `astra check` for the same broken files, or diffs stdout against native/JS for valid ones.

## Proof program

Reuse the `PV-01` contract program and add three deliberately broken files:

- `pv08-parse.ns` — lexer/parser error;
- `pv08-type.ns` — type error;
- `pv08-mode.ns` — capability violation.

## Gate

1. `nova run pv01-contract.ns` produces the same stdout as native and JS.
2. For each broken file, `nova run` and `astra check` emit identical diagnostic codes, spans, severity, and hints.
3. `astra explain` and Nova's mode explanation agree on the inferred mode and reason tree.
4. Nova never produces a diagnostic that the compiler does not know, and vice versa.

## Work implied

1. Add a conformance driver that captures and diffs diagnostics across Nova and the native compiler.
2. Record existing divergences as gaps under this code; do not paper over them in the runner.
3. Keep Nova out of release artifacts — this proof is about parity, not distribution.

## Task list

- [ ] Reuse the `PV-01` program and diff Nova stdout against native/JS.
- [ ] Add parse-error, type-error, and mode-violation fixtures.
- [ ] Diff Nova and `astra check` diagnostic codes, spans, severity, and hints.
- [ ] Diff Nova and `astra explain` mode reasons.
- [ ] Record every divergence as a gap under `PV-08`.
- [ ] Add a conformance driver that captures both outputs automatically.
- [ ] Record evidence and change status to `Proven`.

## Release code

`PV-08`. Release target: TBD.
