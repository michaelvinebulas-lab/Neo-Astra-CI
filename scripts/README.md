# Bootstrap

The universal bootstrap command is:

```sh
npm run bootstrap
```

The only manual prerequisite is **Node.js 22+ with npm**. Everything else is
detected and installed where the platform package manager allows it.

## What it installs

- JavaScript workspace dependencies (`npm install`)
- Rust via `rustup`
- `rustfmt` and `clippy`
- Wasm targets `wasm32-unknown-unknown` and `wasm32-wasip1`
- Git and CMake when missing
- Clang/LLVM when the platform package manager can install it

On Windows, the script selects the GNU Rust toolchain when the MSVC linker is
not available. Clang/LLVM is best-effort because its installer may require
elevation; the Rust compiler core does not need it.

## Other modes

```sh
npm run bootstrap:check    # report only, install nothing
npm run bootstrap:verify   # install, then build, test, fmt-check, and clippy both workspaces
npm run bootstrap -- --skip-system
```

`--skip-system` skips Git, CMake, and Clang installation and only prepares
JavaScript and Rust dependencies.

## Packaging

`npm run package` builds the standalone host package from the current tree:

```sh
npm run package
npm run package -- --skip-build   # reuse existing dist/ and target/release
```

It builds the Rust compiler and Nova in release mode, compiles the `astra` CLI
with Bun (`--minify --bytecode`, no source maps), writes the `nopm` launcher,
copies the pinned native `typescript/tsc`, smoke-tests both commands and a
`@script:ts` build, and writes the archive plus `.sha256` under `dist/`. The
result needs no Node.js, npm, Cargo, or source checkout at runtime. The
`Release packages` workflow runs the same command on Linux, macOS, and Windows
for a tag and uploads the archives to the GitHub release.
