# Install and upgrade

Neo Astra `1.0.0` ships as a host-native package. The release binaries need no
Node.js, npm, Cargo, or source checkout at runtime. There is no background
service and no global state beyond the installed commands.

## Supported hosts

| Host | Release archive | npm platform package |
| --- | --- | --- |
| Windows x64 | yes | `neo-astra-windows-x64` |
| Linux x64 | yes | `neo-astra-linux-x64` |
| macOS arm64 | yes | `neo-astra-darwin-arm64` |

Other hosts can build from source, but they are not part of the 1.0 release
contract.

## Install from npm

Node.js 22+ is needed for the install step only:

```sh
npm install --global neo-astra
astra --version
nopm --version
nova version
```

Do not use `--ignore-scripts`; the package installs the host-native binaries.
The npm package is an installer, not a runtime. After installation, the
`astra`, `nopm`, and `nova` commands do not need Node.js or npm.

## Install from a release archive

Download the archive for your host and the matching `.sha256` sidecar from the
GitHub releases page.

```sh
sha256sum -c neo-astra-*.sha256
tar -xf neo-astra-*.tar.gz
./neo-astra-*/bin/astra --version
```

On Windows, use `Get-FileHash -Algorithm SHA256` and extract the `.zip` with
`Expand-Archive`.

The archive contains:

- `bin/astra` (`.exe` on Windows) - the CLI and `astra nopm`.
- `bin/nopm` (`.exe` on Windows) - a launcher for `astra nopm`.
- `bin/astra-compiler` - the native compiler.
- `bin/nova` - the Nova development runtime.
- `typescript/tsc` - the pinned native TypeScript compiler used by
  `@script:ts`.

The binaries are compiled and stripped. The package contains no TypeScript
sources, source maps, comments, or debug symbols.

A `.sha256` sidecar detects download corruption. It does not prove that an
archive was not replaced. Release provenance attestations are enabled for
releases after `1.0.0`; verify them with the GitHub CLI when present:

```sh
gh attestation verify neo-astra-*.tar.gz --repo luminousvalentine-cell/Neo-Astra
```

## Requirements

- **Node.js 22+ with npm** for the npm install step and for source builds. Not
  needed by an installed package at runtime.
- **Rust stable** for source builds. The bootstrap installs it through `rustup`
  when missing.
- **A C compiler** for `native` and `c` builds: `clang`, `gcc`, or `cc`. On
  Windows, a hosted linker (MSVC build tools or a complete MinGW toolchain) is
  needed for executables.
- **Git** for Git package sources; NoPM also uses npm, Cargo, and system
  package managers only when a project requests a source that needs them.

## First run

```sh
astra --help
astra check path/to/project
astra fmt path/to/project --check
astra build path/to/project --target native
astra doctor
```

`astra check` validates the project manifests and runs the frontend.
`astra build` selects the target from `build.json` / `build.toml`, or from
`--target`: `native`, `c`, `js`, or `ts`. `astra doctor` prints the C toolchain
resolution ladder.

A minimal project needs exactly one core manifest (`core.json` or `core.toml`)
and an optional build manifest (`build.json` or `build.toml`). See
[`examples/hello`](examples/hello).

## C toolchain

`astra doctor` resolves the C compiler in this order:

1. `ASTRA_CC` when set.
2. A pinned local cache under `<project>/.tools/toolchain/<id>/`.
3. `clang`, `gcc`, or `cc` on `PATH`, preferring one that passes the hosted
   probe.

Set the policy in the build manifest:

```json
{
  "toolchain": { "c": "auto" }
}
```

`auto` selects a hosted compiler from `PATH`. A bare family pin (`clang`,
`gcc`) uses the cache when present and may fall back to `PATH`. A versioned pin
(`zig@0.13.0`, `clang@18`) never trusts `PATH`.

A versioned pin downloads the official archive for the current OS and
architecture, verifies its byte size and SHA-256, unpacks it with the system
`tar`, and installs it under the project. `--no-download` disables the fetch.
Every failure names the archive and prints manual-install instructions.

## Upgrade

For npm:

```sh
npm update --global neo-astra
astra --version
```

For an archive, download the new archive, verify its sidecar, and replace the
old unpacked directory. Published tags are immutable; never edit a released
archive in place. A source change after `1.0.0` is released as a new patch or
minor version.

All package versions move in lockstep. If `astra --version` and the package
metadata disagree, reinstall instead of editing one version in isolation.

## Uninstall

For npm:

```sh
npm uninstall --global neo-astra
```

For an archive, remove the unpacked directory. Project-local state lives in
`.astra/` and `.tools/`; remove those only when the project's store or
toolchain cache is no longer needed.

## Troubleshooting

| Symptom | Meaning |
| --- | --- |
| `NS9001` | The project has no core manifest, or has both `core.json` and `core.toml`. Exactly one is allowed. |
| `NS9003` | The `astra-compiler` binary was not found. Reinstall the package. |
| `NS9101` | The `toolchain.c` policy is malformed. |
| `NS9102` | A pinned toolchain cache entry is missing, mismatched, or incomplete. |
| `NS9103` | `ASTRA_CC` points at something that is not a usable C compiler. |
| `NS4019` | A task handle would cross a task boundary as a copied value. |

`app` mode is partial, and `web` / `system` are preview in 1.0. If a mode or
standard-library surface is missing, the compiler should reject it explicitly;
report a silent fallback or a wrong success as a bug.
