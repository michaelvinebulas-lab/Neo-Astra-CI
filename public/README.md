# Neo Astra

Neo Astra is a statically typed, multi-target language for web, app, and
system targets. Source files use `.ns`; the command-line tool is `astra`.

This repository is the public source mirror. It contains the production source
and the public onboarding files; behavior is
documented in the files below and in the language specification shipped with
the release.

## Status

Neo Astra `1.0.0` is released. Use the table as the support contract; it is
deliberately narrower than "the language can do everything".

| Surface | Status | Notes |
| --- | --- | --- |
| `core` mode | **Stable** | The only mode described as stable in 1.0. |
| `app` mode | **Partial** | Usable parts work; the surface is not complete. |
| `web` mode | **Preview** | Capability and backend pieces exist; not a finished library surface. |
| `system` mode | **Preview** | Same caveat as web. |
| `high` mode | **Planned** | Not implemented in 1.0. |
| C17 target | **Shipped** | Executable, static library, and shared library emission. |
| JavaScript ESM target | **Shipped** | Readable ESM output. |
| TypeScript ESM target | **Shipped** | Readable TypeScript output. |
| Nova runtime | **Shipped** | Development `run`, `eval`, `watch`, `repl`, and `test`. |
| NoPM | **Shipped** | Package graph, lockfile, store, and source adapters. |
| Language server (LSP) | **In progress** | Phase 1 is being built; it is not part of the 1.0 package. |
| Plugin host | **Later** | Not shipped. |
| Wasm execution backend | **Later** | Not shipped. |
| Browser playground | **Later** | Not shipped. |

Supported hosts for the 1.0 release archives: **Windows x64**, **Linux x64**,
and **macOS arm64**. Other hosts may build from source but are not release
targets yet.

## Install

From npm, with Node.js 22+ available for the install step:

```sh
npm install --global neo-astra
astra --version
nopm --version
nova version
```

The npm package installs host-native binaries. Node.js, npm, Cargo, or a source
checkout are not needed after installation. Do not install with
`--ignore-scripts`.

You can also unpack a release archive from the GitHub releases page:

```sh
tar -xf neo-astra-*.tar.gz
./neo-astra-*/bin/astra --version
```

See [`INSTALL.md`](INSTALL.md) for prerequisites, archive layout, checksums,
C toolchain setup, upgrades, and troubleshooting.

## First project

```sh
mkdir hello
cd hello
```

Create `core.json`:

```json
{
  "package": { "name": "hello" }
}
```

Create `build.json`:

```json
{
  "build": { "defaultTarget": "native", "defaultMode": "core" },
  "nova": { "entry": "src/main.ns" }
}
```

Create `src/main.ns`:

```ns
module app.main

pub fn main() -> i32 {
    println("hello, neo astra")
    return 0
}
```

Then run:

```sh
astra check .
astra build . --target native
```

Native builds need a C compiler on `PATH`, or a pinned toolchain declared in
`build.json`. See [`INSTALL.md`](INSTALL.md).

A ready-to-run copy is in [`examples/hello`](examples/hello).

## What 1.0 ships

- A statically typed frontend: lexer, parser, resolver, capability and mode
  inference, type checker, error effects, and structured diagnostics.
- The `astra` CLI: `check`, `fmt`, `build`, `doctor`, `ast`, and `explain`.
- Native, C, JavaScript, and TypeScript backends. Inline `@script:js`,
  `@script:ts`, `@script:c`, and `@script:cpp` blocks are emitted with boundary
  validation. Other script targets are rejected explicitly.
- The `nova` development runtime.
- NoPM package management, including path, Git, npm, Cargo, PyPI, NuGet, OCI,
  and `sys:` sources.
- Standalone, stripped release archives for the three supported hosts.

Still staged for later: the language server, plugin host, Wasm execution,
Web Worker code generation, the browser playground, and `high` mode.

## Contributing

Read [`CONTRIBUTING.md`](CONTRIBUTING.md). The public repository is
owner-reviewed: pull requests are required, direct pushes to `master` are
blocked, and published tags are immutable. Commercial users are encouraged to
contribute fixes, tests, and maintenance time; Apache-2.0 is a permissive
license and does not legally require it.

## Security

Report vulnerabilities privately through the repository's security advisory
flow. Do not open a public issue for an unfixed vulnerability. See
[`SECURITY.md`](SECURITY.md).

## License

Apache License 2.0. See [`LICENSE`](LICENSE), [`NOTICE`](NOTICE), and
[`CONTRIBUTORS.md`](CONTRIBUTORS.md).

The public source mirror strips comments from source files. Generated output
from the compiler is also comment-free. If you need an explanation, put it in
the documentation rather than in a source comment.
