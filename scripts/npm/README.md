# Neo Astra

Install the Neo Astra toolchain with npm:

```sh
npm install --global neo-astra
astra --version
nopm --version
nova version
```

The npm package is an installer. The installed `astra`, `nopm`, and `nova`
commands are the same native binaries as the GitHub archive; they do not need
Node.js, npm, Cargo, or a source checkout at runtime. NoPM downloads npm, Cargo,
or system package managers only when a project requests a source that needs
them.

Supported npm hosts:

- Windows x64
- Linux x64
- macOS arm64

For other hosts, download the archive from the GitHub release or build the
package from a source checkout with `npm run package`.

Do not install with `--ignore-scripts`, and use npm rather than a package
manager that skips install scripts. If the postinstall step is skipped, the
commands print an error; run it manually with:

```sh
node node_modules/neo-astra/postinstall.mjs
```

The `astra` command provides `check`, `build`, `run`, `fmt`, `ast`, `explain`,
and `doctor`. `astra nopm` is the package manager; the `nopm` command is an
alias for it. `@script:ts` blocks use the bundled native TypeScript compiler.
