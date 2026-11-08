# Neo Astra conformance suite

Backend-neutral expected-behavior cases. Every language target (Nova, the
JavaScript and TypeScript backends, and future backends) compiles and runs each
case and must reproduce its recorded output exactly. The cases pin down shared
semantics from `Docs/spec-v0.1.md` — strings and bytes (§6), modes (§4), errors
(§8), and channels (§9) — so backends cannot drift from the spec or from each
other.

## Layout

```
conformance/
  cases/
    <name>.ns        a self-contained Neo Astra module
    <name>.expected  the exact stdout the module must produce
  rejects/
    <name>.ns        a module the frontend must reject
    <name>.codes     stable diagnostic codes that must be reported
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

The C runner is `crates/astra-codegen-c/tests/conformance.rs`. It always lowers
every case and compiles the generated C with `clang -std=c17 -ffreestanding
-Wall -c`; when a hosted-capable compiler is available (libc headers plus a
linker), it also links the case against `astra_host.c`, runs it, and compares
stdout plus `RETURN=` with the `.expected` file. A freestanding-only clang (for
example on Windows without the MSVC/Windows SDK) is skipped in favour of gcc.
The C ledger is empty: all 19 cases lower and run, including `channels` after
the single-threaded `CONC-001` slice (eager `spawn`, bounded `Channel`, deadlock
panics on a full/empty open channel).

Negative cases run from `crates/astra-core/tests/conformance_rejects.rs`, also
picked up by `cargo test --workspace`.

New backends must add the same loop over the corpus and run it in CI.

## Known backend gaps

A case may be temporarily unreachable for one backend. Gaps are recorded in
the backend's runner, not by deleting the case:

- Nova keeps an explicit ledger in `crates/astra-nova/tests/conformance.rs`
  (`KNOWN_GAPS`). A ledger entry fails the test if the case starts passing, so
  gaps cannot silently rot into permanent skips.

Current Nova gaps: none. The `bytes` builtin, the full `str` method set, and
the `Channel` runtime landed with NOVA-004; the `KNOWN_GAPS` ledger stays in
place, empty, for the next gap a new case uncovers.

Current JavaScript gaps: none — every case runs. Statement-position `if` and
`match` now lower to real JavaScript statements, so `break`, `continue`, and
early `return` work inside them (pinned by the `early_exit` case and the test
`javascript backend emits statement-position if and match control flow`).
Control flow inside a *value-position* expression - `let x = match ... { _ => {
return 0 } }` and similar IIFE-lowered `if`/`match`/block expressions - is still
rejected with `NS8001`; that boundary is pinned by the test `javascript backend
rejects control flow inside value-position expressions`.

`bytes.slice` no longer clamps like the native `Uint8Array.slice`: it is
bounds-checked and panics on reversed or out-of-range ranges (like Nova),
pinned by the test `javascript backend bounds-checks bytes slices instead of
clamping`. The `slice_bounds` case pins `str`/`bytes` end-offset semantics
(byte offsets, `Err(SliceError)` for reversed/out-of-bounds/UTF-8-splitting
`str` ranges) for every executing backend.

Current TypeScript gap: none. `TS-001` removed the `readonly` emission that
failed `tsc` with `TS2540` on `user.name = "Neo"`; the `struct_mutation` case
pins struct field writes under `tsc --strict` so the bug cannot return.

## Negative cases

Programs that must be rejected live in `conformance/rejects`:

- `<name>.ns` must fail to parse, resolve, or type-check.
- `<name>.codes` lists the stable diagnostic codes that must be reported, one
  per line. Blank lines and `#` comments are ignored.
- Codes are matched as a **subset**: extra diagnostics are allowed, but every
  listed code must appear.
- `crates/astra-core/tests/conformance_rejects.rs` runs the whole frontend
  pipeline for each case, so the contract is backend-neutral.
- A case may declare the capability mode the frontend must run it in with a
  `// conformance: mode=<core|app|web|system>` comment anywhere in the file.
  Without one the case runs at the default core mode. This is **test
  infrastructure only**, not language surface: the frontend sees an ordinary
  comment, and only the reject runner parses the directive. It exists because
  some rules are ordered - at core mode the mode requirement (`NS5004`) fires
  before the scope (`NS6001`) and element-type (`NS6002`) rules, so those
  rules' cases must run at system mode to pin them ("one program, one violated
  condition"). Malformed or duplicate directives fail the run loudly rather
  than silently falling back to core.

The current rejects cover lexical/parse failure (`NS2001`), visibility
(`NS3001`, `NS3002`, `NS3005`, `NS3010`, `NS3011`), exports (`NS3003`), types
(`NS4001`, `NS4002`), effects (`NS4011`), mode requirements (`NS5001`,
`NS5004` - the latter for raw pointer access, which is manual memory and so
needs system mode even when the pointer arrives as a parameter), the manual
scope rule (`NS6001`, via the mode directive), and the manual element-type
rule (`NS6002`, at both the `Memory.alloc` and `region.alloc` sites).

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

For a rejection case, write `rejects/<name>.ns` and list the required codes in
`rejects/<name>.codes`, then run
`cargo test -p astra-core --test conformance_rejects`.

## v0.2 corpus (not loaded yet)

The `conformance/v0.2/` tree holds the edition-0.2 cleanup corpus. It is
intentionally separate from `cases/` and `rejects/` and is not read by
`loadConformanceCases()` until the edition switch, parser, checker, formatter,
and migration command are accepted. See `conformance/v0.2/README.md` for the
layout and code-match rules.

Do not move v0.2 fixtures into `cases/` or `rejects/` before Phase 5: the v0.1
runners would try to compile them against the v0.1 surface.
