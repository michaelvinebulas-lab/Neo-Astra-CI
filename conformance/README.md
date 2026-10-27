# Neo Astra conformance suite

Backend-neutral expected-behavior cases. Every language target (Nova, the
JavaScript backend, TypeScript once TS-001 lands, and future backends) compiles
and runs each case and must reproduce its recorded output exactly. The cases pin
down shared semantics from `Docs/spec-v0.1.md` — strings and bytes (§6), modes
(§4), errors (§8), and channels (§9) — so backends cannot drift from the spec or
from each other.

## Layout

```
conformance/
  cases/
    <name>.ns        a self-contained Neo Astra module
    <name>.expected  the exact stdout the module must produce
  README.md
```

Each `<name>.ns` declares a `pub fn main() -> i32` entry point. The module
must compile with no diagnostics under the default core profile and print
with `println`. Panic and deadlock behavior is intentionally out of scope
here (no exit status other than 0 is expected); those semantics are pinned
by backend unit tests such as `orchestrator/tests/codegen-js.test.mjs`.

## Expected output format

- `.expected` contains the program's stdout line by line.
- The final line is always `RETURN=<code>` where `<code>` is the value
  `main` returns (the test harness prints it after the program runs).
- Trailing whitespace at the end of the file is ignored, and CRLF/LF
  differences are normalized, so editors on any platform can touch these
  files safely.

## Running

The JavaScript loader is `orchestrator/tests/conformance.mjs`:

```js
import { loadConformanceCases } from "./conformance.mjs";

for (const conformanceCase of loadConformanceCases()) {
  // compile conformanceCase.source with your backend,
  // run it, and compare stdout to conformanceCase.expected
}
```

`orchestrator/tests/codegen-js.test.mjs` runs the suite for the JavaScript
backend in the test `javascript backend passes the shared conformance
suite`. That file is part of `npm run test:boundary`, so CI enforces it.

Nova runs the same corpus from Rust in
`crates/astra-nova/tests/conformance.rs`, which is picked up by
`cargo test --workspace`.

New backends must add the same loop over the corpus and run it in CI.

## Known backend gaps

A case may be temporarily unreachable for one backend. Gaps are recorded in
the backend's runner, not by deleting the case:

- Nova keeps an explicit ledger in `crates/astra-nova/tests/conformance.rs`
  (`KNOWN_GAPS`). A ledger entry fails the test if the case starts passing, so
  gaps cannot silently rot into permanent skips.

Current Nova gaps: the `bytes` builtin, the full `str` method set beyond
`len` (byte/char indexing, slicing, `char_count`, `chars`, `bytes`), and the
`Channel` runtime. These are tracked by `NOVA-004` and `CONC-001`.

Current JavaScript gaps: `break`/`continue` and early `return` inside an
`if` block are not emitted yet (`NS8001`). Cases therefore use loops with a
single exit and `match` expressions until the backend closes that gap.

## Adding a case

1. Write `cases/<name>.ns` with a `pub fn main() -> i32` that exercises one
   spec behavior deterministically (no clock, randomness, or platform I/O
   beyond `println`).
2. Record `cases/<name>.expected` by running the program on a known-good
   backend and pasting its output, ending with `RETURN=<code>`.
3. Run both `npm run test:boundary` and
   `cargo test -p astra-nova --test conformance` to confirm every backend
   passes. If a backend cannot run the case yet, add it to that backend's
   documented gap ledger with an owning task.
