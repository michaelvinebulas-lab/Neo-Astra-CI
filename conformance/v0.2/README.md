# v0.2 corpus (not loaded yet)

This directory holds the edition-0.2 cleanup corpus. It is intentionally
**separate** from `conformance/cases` and `conformance/rejects` until the v0.2
parser, checker, formatter, and migration command land.

`orchestrator/tests/conformance.mjs` currently loads only
`conformance/cases/*.ns`; it does not read this tree. Do not move these files
into the v0.1 directories. Phase 5 wires the v0.2 runner after the edition
switch and parser are accepted.

## Layout

```text
conformance/v0.2/
  valid/
    <name>.ns             must compile and run under edition 0.2
  deprecated/
    <name>.ns             valid under edition 0.2 by default;
                          `astra check --canonical-syntax` must report
    <name>.codes          canonical-syntax codes (subset match)
  invalid/
    <name>.ns             must be rejected under edition 0.2
    <name>.codes          required diagnostic codes (subset match)
  migrate/
    <name>.input.ns       edition 0.1 input
    <name>.migrated.ns    expected `astra migrate` output
```

Every `.ns` file starts with a metadata comment:

```text
// edition: 0.2
```

For migration inputs the comment is `// edition: 0.1`; a `// migration:
--edition 0.2` line records the flag the golden assumes. Comments are test
infrastructure, not language surface. The v0.2 runner reads the directive; the
compiler sees an ordinary comment.

## Case rules

- `valid/` cases must be deterministic and self-contained. They will get
  `.expected` files when the v0.2 runner exists.
- `invalid/` `.codes` use the same subset-match rule as
  `conformance/rejects`: extra diagnostics are allowed, but every listed code
  must appear.
- `deprecated/` cases are parsed successfully under edition 0.2. Their `.codes`
  are the canonical-syntax diagnostics, not parse failures.
- `migrate/` golden files assume `astra migrate --edition 0.2`; migration must
  be idempotent and must not rewrite `/%` to `/`.

## Current focus

The fixtures pin the Phase 0 decisions:

- type arguments in type position versus `::<...>` in expression position;
- struct initializers versus block syntax, including the `if`/`while`/`for`/
  `match` head restriction;
- `fail Type(...)` and `fail Type { ... }`;
- `Err` member patterns and exhaustiveness;
- canonical `@mode(...)` / `@memory(...)` scopes;
- trailing commas in every list;
- deprecated aliases, `spawn ||`, `/%`, and old scope blocks;
- migration of aliases, old scopes, `!Error(...)`, postfix `!`, `spawn ||`, and
  separator changes.
