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
