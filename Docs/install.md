# Install and upgrade

Neo Astra ships as a host-native package for v1. The source checkout remains the
development and bootstrap path; the packaged binaries need no Node.js, npm,
Cargo, or source checkout at runtime. There is no global installer and no
service to run. Project caches stay inside the project.

## Packaged distribution

From a checkout, build the package for the current host:

```sh
npm run package
```

The archive and checksum are written under `dist/`; unpack it anywhere. The
package contains:

- `bin/astra` (`.exe` on Windows) - the `astra` CLI and `astra nopm`.
- `bin/nopm` on POSIX, `bin/nopm.cmd` on Windows - a launcher for
  `astra nopm`.
- `bin/astra-compiler` - the native compiler.
- `bin/nova` - the Nova development runtime.
- `typescript/tsc` - the pinned native TypeScript compiler used by
  `@script:ts`; Node.js is not needed to transpile a script block.

The binaries are compiled and stripped; the package contains no TypeScript
sources, source maps, comments, or debug symbols. NoPM downloads npm, Cargo, or
system package managers only when a project requests a source that needs them.
Native builds still need a C compiler on `PATH`, or a pinned toolchain declared
in `build.json` (which `astra` downloads and verifies).

```sh
bin/astra --version
bin/astra check path/to/project
bin/astra build path/to/project --target native
bin/nopm --version
```

## Requirements

- **Node.js 22+ with npm** for the source checkout and for `npm run package`.
  A packaged installation does not need either at runtime.
- **Rust stable** - the bootstrap installs it through `rustup` when missing.
- **A C compiler** for `native` and `c` builds: `clang`, `gcc`, or `cc`.
  `astra doctor` probes the hosted compiler; on Windows a hosted linker (the
  MSVC build tools or a complete MinGW toolchain) is needed for executables.
- On Linux and macOS the bootstrap uses the system package manager for Git,
  CMake, and Clang when it can. On Windows it uses `winget` when available.
  Elevation is handled by that package manager, never by Neo Astra.

## Fresh install

From a checkout of the repository:

```sh
npm run bootstrap
npm run bootstrap:verify
```

`bootstrap` installs the JavaScript workspace dependencies, `rustup` and the
Rust toolchain, `rustfmt`, `clippy`, the `wasm32-unknown-unknown` and
`wasm32-wasip1` targets, and best-effort system tools. `bootstrap:verify` then
builds, tests, checks formatting, and runs clippy for both workspaces.

If only the JavaScript and Rust dependencies are wanted:

```sh
npm run bootstrap -- --skip-system
```

To inspect what is present without installing anything:

```sh
npm run bootstrap:check
```

## Run

Build the TypeScript workspace once, then use the repository scripts:

```sh
npm run build
npm run astra -- --help
npm run astra -- check .
npm run astra -- fmt . --check
npm run astra -- ast .
npm run astra -- explain .
npm run astra -- build . --target native
npm run astra -- doctor
npm run nova -- run
npm run nopm -- --help
```

`npm run astra --` forwards everything after `--` to the `astra` CLI. The same
works for `nova` and `nopm`.

- `astra check` discovers and validates both manifests, runs the frontend over
  the project entry, and rejects missing or inconsistent `ffi` mappings with
  `NS7007`.
- `astra fmt` formats `.ns` source and `core.json` / `core.toml`; `--check`
  reports differences without writing.
- `astra ast` prints the parsed AST for a file or the project entry;
  `astra explain` prints the inferred mode and its reasons. Both accept
  `--json`, `--mode`, `--target`, and `--compiler`.
- `astra build` uses the target from the build manifest, or `--target <name>`:
  `native`, `c`, `js`, or `ts`. `--emit executable|staticlib|cdylib` selects
  the artifact kind for the C/native targets.
- `astra doctor` prints the C toolchain resolution ladder, the chosen compiler,
  and the reason for a failed resolution.
- `nova run|eval|watch|repl|test` is the development runtime.

The native path needs the compiler binary. A debug build is discovered
automatically from `target/debug/astra-compiler`; build it explicitly with:

```sh
cargo build -p astra-compiler-api --bin astra-compiler
```

For a release binary:

```sh
cargo build --release -p astra-compiler-api --bin astra-compiler
```

Select a different compiler binary with `ASTRA_COMPILER_BIN` or
`astra ... --compiler <path>`. `--compiler` selects the **astra-compiler**
binary, not the C compiler.

## C toolchain

`astra doctor` resolves the C compiler in this order:

1. `ASTRA_CC` when set;
2. a pinned local cache under `<project>/.tools/toolchain/<id>/`;
3. `clang`, `gcc`, or `cc` on `PATH`, preferring one that passes the hosted
   probe.

Set the policy in the build manifest (`build.json` / `build.toml`):

```json
{
  "toolchain": { "c": "auto" }
}
```

`auto` selects a hosted compiler from `PATH`. A bare family pin (`clang`,
`gcc`) uses the cache when present and may fall back to `PATH`. A versioned pin
(`zig@0.13.0`, `clang@18`) never trusts `PATH`.

A versioned pin with no cache marker downloads the official archive for the
current OS and architecture, verifies its byte size and SHA-256, unpacks it with
the system `tar`, and installs it under the project:

```text
.tools/toolchain/<id>/toolchain.json   # {"id": "...", "family": "...", "version": "...", ...}
.tools/toolchain/<id>/<family>[.exe]   # also accepted at bin/<family>[.exe]
```

Zig `0.13.0` is pinned for Linux, macOS, and Windows on x64 and arm64. The
downloaded `zig` drives `zig cc` for compilation and `zig ar` for archives.

`astra build ... --no-download` and `astra doctor ... --no-download` disable the
fetch. An invalid existing marker, a failed download, a size or hash mismatch, a
bad archive, or a missing binary all fail with `NS9102` and print the exact URL,
SHA-256, target directory, and manual-install steps. Nothing elevates and
nothing writes outside the project. To install manually, download the archive,
verify it, extract the binary into the cache directory, and write
`toolchain.json` with the matching `id`, `family`, and `version`.

## Upgrade

From the repository root:

```sh
git pull --ff-only
npm install
npm run bootstrap
npm run bootstrap:verify
npm run astra -- --version
```

All package versions move in lockstep. If `astra --version` and the workspace
versions disagree, run `node scripts/check-versions.mjs` and follow its
diagnostic; do not edit one version in isolation.

## Clean and uninstall

```sh
npm run clean        # TypeScript build metadata
cargo clean          # Rust build products
```

A full reset also removes `node_modules/`, `orchestrator/packages/*/dist/`, and
`target/`. Project-local state lives in `.astra/` and `.tools/`; remove those
only when a project's store or toolchain cache is no longer needed.

Uninstalling is removing the checkout. Nothing is installed globally, and no
background process is registered.

## Troubleshooting

| Symptom | Meaning |
|---|---|
| `NS9001` | The project has no manifest or both `core.json` and `core.toml`; exactly one is allowed. |
| `NS9003` | The `astra-compiler` binary was not found; build it or set `ASTRA_COMPILER_BIN` / `--compiler`. |
| `NS9101` | The `toolchain.c` policy is malformed. |
| `NS9102` | A pinned toolchain cache entry is missing, mismatched, or incomplete. |
| `NS9103` | `ASTRA_CC` points at something that is not a usable C compiler. |
| `NS4019` | A task handle would cross a task boundary as a copied value. |

For the language contract, see `Docs/spec-v0.1.md`; for current status and
decisions, `Docs/state.md`.
