# Task channel - lead to Kilo

Working channel between the **lead** (planner/reviewer) and **Kilo** (coder).
The **principal** (the user) is the final authority. Rule 10 applies to both:
do not invent decisions - mark the question instead.

## Protocol

- The Status line is the lock. While it says `OPEN - KILO`, the lead runs no
  shell or git operations in this repository.
- Neither side rewrites the other's section. Append only.
- Kilo never commits, never pushes, never edits `Docs/**` or the spec. The
  deliverable is code and tests on disk, a `## Kilo log` entry, and a Status
  update. The lead reviews and lands.
- Questions that need the principal: write them under `## Kilo log` and set
  Status to `BLOCKED - principal`.
- Before handback: `cargo fmt --all` and the full gate (`npm run ci`) green,
  with the output tail reported. Report the actual test count.

## Status: OPEN - KILO (task: system package source)

Read the task below, write your `## Kilo log` entry, then code. When the gate
is green, write the report in the same block and set Status to `DONE - lead`.

---

# Task: system package source (`sys:`)

## Goal

Move the tool-install logic out of `scripts/bootstrap.mjs` into NoPM:
`nopm add sys:<tool>` installs `git`, `cmake`, or `clang` through the
platform's package manager. A system tool is not a package: it must never
enter `core.*` dependencies, `core.lock.json`, or the store.

**Scope split.** You own `orchestrator/**` only: the adapter, the CLI routing,
and tests in the existing `orchestrator/tests/nopm.test.mjs` (do not create a
new test file - the root `package.json` test list is the lead's). The lead
rewrites `scripts/bootstrap.mjs` to delegate. Do not touch `scripts/**`,
`package.json`, or `Docs/**`.

## Deliverables

1. **`orchestrator/packages/nopm/src/sys-adapter.ts`**
   - Tool table for `git`, `cmake`, `clang` with the IDs already used by
     bootstrap:
     - git: winget `Git.Git`, brew `git`, Linux `git`
     - cmake: winget `Kitware.CMake`, brew `cmake`, Linux `cmake`
     - clang: winget `LLVM.LLVM`, brew `llvm`, Linux `clang`
   - A **pure planner** - `planSysInstall(tool, { platform, uid, managers })`
     returns the ordered commands - so the matrix is unit-tested without
     spawning anything. Export it.
   - An executor that runs the plan through the existing injectable command
     runner (`command.ts`), with inherited stdio.
   - Already on PATH: report `already installed`, run no manager, exit 0.
   - Elevation: Linux non-root prefixes `sudo -n`; root runs directly; macOS as
     root is an error (Homebrew refuses to run as root); Windows runs
     directly.
   - Linux manager selection order, first found: `apt-get`, `dnf`, `pacman`,
     `zypper`, `apk` (same as bootstrap today).
   - Exact command forms (the contract, pin them in tests):
     - winget: `winget install --id <ID> -e --accept-package-agreements
       --accept-source-agreements --silent`
     - brew: `brew install <pkg>`
     - apt-get: `apt-get update`, then `apt-get install -y <pkg>`
     - dnf: `dnf install -y <pkg>`
     - pacman: `pacman -S --noconfirm <pkg>`
     - zypper: `zypper --non-interactive install <pkg>`
     - apk: `apk add <pkg>`
   - Errors: unknown tool -> `NP0015` (message lists the known tools); no
     supported manager -> `NP0017`.

2. **CLI** (`orchestrator/packages/nopm/src/cli.ts`)
   - `nopm add sys:<tool>` routes to the adapter before any package parsing,
     and takes no project directory (a second positional is a usage error).
   - `--dry-run` prints the exact command lines and executes nothing.
   - `--json` prints
     `{ ok, action: "sys-install", tool, manager, commands, installed }`,
     where `commands` is an array of argv arrays and `already installed`
     yields `manager: null, commands: [], installed: true`.
   - `nopm remove sys:<tool>` -> `NP0016`: NoPM does not uninstall system
     tools.
   - `nopm --help` lists the `sys:` form.
   - `sys:` must never read or write the project manifest, the lockfile, or
     the store.

3. **Tests** (append to `orchestrator/tests/nopm.test.mjs`, mock runner, no
   real installs and no network):
   - planner matrix: win32, darwin, linux-root, linux-sudo, each manager's
     exact argv;
   - macOS-root refusal; unknown tool `NP0015`; no manager `NP0017`;
   - already-installed short-circuit runs nothing;
   - CLI `add sys:git --dry-run` leaves a project's `core.*`,
     `core.lock.json`, and `.astra/` byte-identical;
   - `remove sys:git` -> `NP0016`; `--json` shape.

## Acceptance

- Full gate green with the new tests; the test count reported.
- `git`, `cmake`, `clang` each plan the exact argv above on every supported
  platform/manager, with and without `sudo -n`.
- `nopm add sys:<tool>` never edits the manifest, lockfile, or store.
- No real package-manager invocation in CI.

## Notes

- `rustup` stays out of the adapter: on Linux it installs through rustup.rs,
  not a system package manager, so bootstrap keeps that one branch. Say so in
  a comment rather than inventing a `sys:rustup` special case.
- Node and npm stay prerequisites in bootstrap, not `sys:` tools.
- The adapter's message text and JSON are the interface the lead's bootstrap
  delegation will consume, so keep `commands` machine-readable.
