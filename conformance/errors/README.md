# Error-code case corpus

One `.ns` source per case, with a sibling `.codes` file listing the stable
diagnostic codes that the case must emit. Lines starting with `#` in a `.codes`
file are comments. Codes are checked as a subset: emitting extra diagnostics is
allowed, but every listed code must appear.

Directives (runner-only, ordinary comments to the language):

- `// edition: 0.2` — check the source with edition `0.2` (default `0.1`).
- `// conformance: mode=<core|app|web|system>` — select the active capability
  mode (default `core`).

## Running the cases

The standalone runner is `scripts/run.rs1` (std-only Rust):

PowerShell (Windows) — note the `&` call operator and the `.exe` suffix:

```powershell
cargo build -p astra-cli --bin astra-native
rustc scripts/run.rs1 -o run-errors.exe   # keep the .exe: rustc does not add it for -o
& .\run-errors.exe                 # this directory only (23 cases)
& .\run-errors.exe --all           # also conformance/rejects and v0.2 invalid (60 cases)
& .\run-errors.exe --coverage      # report ledger codes not exercised by cases
& .\run-errors.exe --root <repo>   # run from another working directory
```

bash (macOS/Linux):

```bash
cargo build -p astra-cli --bin astra-native
rustc scripts/run.rs1 -o run-errors
./run-errors                       # this directory only (23 cases)
./run-errors --all                 # also conformance/rejects and v0.2 invalid (60 cases)
./run-errors --coverage            # report ledger codes not exercised by cases
```

Set `ASTRA_BIN` to override the compiler binary path.

The runner writes a temporary project for each case, invokes
`astra-native check <file> --json`, and compares emitted codes with the
`.codes` file. It exits non-zero when any case fails.

## Coverage note

`--coverage` compares exercised codes with `Docs/error-code-ledger.md`.
Package-manager (`NP`), registry (`NR`), Nova runtime (`NV`), playground
(`PG`), CLI/build/toolchain (`NS9xxx`), and backend (`NS8xxx`) codes have their
own test suites; the cases here exercise the source front end (lexer, parser,
resolver, checker, and the edition-`0.2` diagnostics).
