# Neo Astra conformance suite

Backend-neutral expected-behavior cases. Every language target (JavaScript,
TypeScript once TS-001 lands, and future backends) compiles and runs each
case and must reproduce its recorded output exactly. The cases pin down
shared semantics from `Docs/spec-v0.1.md` — currently §6 UTF-8 string/byte
operations and §9 channels — so backends cannot drift from the spec or from
each other.

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

The runner module is `orchestrator/tests/conformance.mjs`:

```js
import { loadConformanceCases } from "./conformance.mjs";

for (const conformanceCase of loadConformanceCases()) {
  // compile conformanceCase.source with your backend,
  // run it, and compare stdout to conformanceCase.expected
}
```

`orchestrator/tests/codegen-js.test.mjs` runs the suite for the JavaScript
backend in the test `javascript backend passes the shared conformance
suite`. New backends must add the same loop over `loadConformanceCases()`.

## Adding a case

1. Write `cases/<name>.ns` with a `pub fn main() -> i32` that exercises one
   spec behavior deterministically (no clock, randomness, or platform I/O
   beyond `println`).
2. Record `cases/<name>.expected` by running the program on a known-good
   backend and pasting its output, ending with `RETURN=<code>`.
3. Run the JavaScript backend test to confirm the case passes.
