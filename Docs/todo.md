# Todo

Every open task, grouped by the version it lands in (same names as the
roadmap). Check items off as they are done and prune them at each release.
Completed work is summarized at the bottom; the full history is in
`Docs/archive/` and `CHANGELOG.md`.

## 1.0.0 - complete the stack

### Project layout and packages

- [x] Finish the two-manifest project layout: strict rejection of keys in the
      wrong file, one read path with no fallback to the other manifest,
      canonical key ordering per manifest kind, the restored core-order test,
      a CLI-level rejection test, and the migrated fixtures - then commit it.
- [x] Manifest edit API: add and remove a dependency without destroying
      formatting or comments, canonical output, duplicate rejection, JSON and
      TOML forms.
- [x] `nopm add`: resolve a package, edit the package manifest, rewrite the
      lockfile, install; `--json` and `--dry-run` (dry-run writes nothing).
- [x] `nopm remove`, with the same edit guarantees.
- [x] End-to-end package tests: path source through the CLI (add, remove,
      dry-run, rollback, lockfile and store); git and npm stay covered by their
      adapter tests so the gate needs no network.
- [x] System package-manager source: winget (Windows), brew (macOS), apt, dnf,
      zypper (Linux) as an explicit `sys:` source; elevation handled by the
      adapter; exact command lines unit-tested with no real installs in CI;
      `sys:` entries never land in the dependency list.
- [x] Bootstrap script delegates to the system source with unchanged dry-run
      output and no duplicated install logic.
- [x] Validate the `sys:` tool name before the installed check, so an unknown
      tool that happens to exist on `PATH` is rejected with `NP0015` instead of
      reported as already installed.
- [x] Toolchain section in the build manifest: parse it, resolve in order
      (manifest pin, local cache, PATH), honor `ASTRA_CC`, and record the
      resolved toolchain in the lockfile with a single writer.
- [x] `astra doctor`: print the resolution ladder result (rung, path, version),
      refuse unknown pins, and support `--no-download` for frozen resolution.
- [x] Build uses the resolved compiler instead of ad-hoc detection; existing
      native tests still pass on the PATH rung.
- [x] Pinned toolchain download: per-OS/arch artifact table with SHA-256,
      download through the package transport, verify, unpack into the local
      cache, and lockfile record. Zig 0.13.0 is pinned; `zig cc` compiles and
      `zig ar` archives.
- [x] Download failure behavior: checksum mismatch, truncated archive, bad
      archive, missing binary, offline/HTTP failure, invalid cache, and
      `--no-download` all fail clearly with manual instructions and never
      elevate.

### Runtime

- [x] Multithreaded concurrency: one OS thread per spawn bound to its
      structured scope, with the eager inline model kept for browser, JS, and
      interpreter targets.
- [x] Park blocked channel operations instead of spinning; add the deadlock
      watchdog (5 second runtime constant, test override via an environment
      variable).
- [x] Per-task heaps: every execution context owns an `NS_Heap`, TLS selects
      it, and captured values plus task results are deep-copied across heaps.
- [x] (1.0 blocker, D28) Thread-safe shared heap for channel storage, reused
      for interned strings and static descriptors when they exist. A sender
      never allocates into another task's single-threaded heap.
- [x] Cooperative cancellation at collection safepoints and during blocked
      channel operations; cancellation of a finished or dead task is a no-op.
- [x] (1.0 blocker, D28) `TaskError`: `join() -> T`; a child panic or
      cancellation before a value raises a structured `TaskError`; a child
      returning `Result<T, E>` keeps that value; `canceled()` reports accepted
      cancellation without joining; a late cancel is not recorded; value wins
      over cancellation and panic wins over cancellation at join.
- [x] JavaScript backend completeness: map index read/write (a missing key
      panics), `break`/`continue`/`return` through value-position expression
      blocks, and `abort` through the host termination primitive, with success
      and failure tests.
- [x] Remove the non-shipped LSP and direct Wasm/LLVM scaffolds so no stub
      package ships; V1 machine code stays on the C toolchain.
- [x] Re-pin the concurrency conformance case for the new blocking semantics
      and run the threaded matrix on Linux, macOS, and Windows.
- [x] (1.0 blocker, D28) Channel messages: deep-copy managed values for the
      receiving task's heap (spec 9.3); primitives and scalars move by value.
      The conformance case sends a heap-built `bytes` value and receives it
      after the sender heap is released.
- [x] (1.0 blocker, D28) Spawn captures resolve free names through the full
      scope chain, not a flat locals map, so an out-of-scope shadowed local is
      never captured.
- [x] (1.0 blocker, D28) A task handle nested inside a composite (struct,
      array, option, result) is rejected at every copy boundary: channel
      elements, spawn captures, and spawn results all report `NS4019`; the
      emitter no longer treats `Task` as a shared capture. A future
      single-owner shared-handle design is a separate principal decision.

### Release

- [x] Version check in the gate: all package versions and the workspace version
      must agree.
- [x] Release script: lockstep bump, changelog section move, dry-run mode.
- [x] Packaging: build every package and the native compiler; verify the
      bootstrap install from a clean machine. Every package `main`/`types`/`bin`
      entry exists, the release compiler answers an `initialize` request with
      `coreVersion: 1.0.0`, and Tier-1 starts from a clean checkout with
      `npm install` + `npm run ci`.
- [x] Install and upgrade documentation.
- [x] Error-code audit: every emitted diagnostic has a normative row, the
      overloaded `NS8001`/`NS9008` uses are split, `NS7007` FFI validation is
      wired into `astra check`, and `scripts/check-error-codes.mjs` guards the
      table in CI.
- [x] (D28) Pre-1.0 gap audit: every stub, TODO, approximation, and deferred
      item in the shipped surface is closed or recorded as a principal decision.
      The audit table is in `Docs/state.md`; the LSP and Wasm/LLVM scaffolds were
      removed, the JavaScript backend gaps are closed, and JS/TS FFI is recorded
      as a stable unsupported target path.
- [x] Standalone distribution: `npm run package` builds one host-native
      `astra` executable (minified bytecode, no source maps/comments), a tiny
      `nopm` launcher for `astra nopm`, the stripped Rust compiler/Nova
      binaries, and the pinned native `tsc` for `@script:ts`. The package runs
      without Node.js, npm, Cargo, or a checkout; NoPM downloads other package
      managers only when a project requests them.
- [x] Cut 1.0.0 across all packages, with release notes from the changelog,
      only when the 1.0 queue is empty and the D28 audit is clean. The tag and
      GitHub release are published from the green Tier-1 release commit; the
      release workflow attaches the Linux, macOS, and Windows packages.

## 1.0.1 - native CLI and NoPM

Target: Cargo-built native Rust `astra` and `nopm`; baseline `check`, `fmt`,
`ast`, `explain`, `doctor`, C/native builds, and NoPM path flows run without
Node or npm. JS/TS codegen stays behind an isolated on-demand Node bridge.
Language stays v0.1; do not inject v0.2 syntax into this patch.

### Landed

- [x] Slice 1 - project manifest discovery, defaulted build/entry handling,
      project check JSON/diagnostics, FFI mapping validation, manifest
      formatting, and project `ast`/`explain` (`7ca69be`).
- [x] Slice 2a - toolchain resolution and `doctor`.
- [x] Slice 2b - native/C executable build.
- [x] Slice 2c - pinned Zig downloads and staticlib/cdylib artifacts
      (`501e91f`; slices 2a/2b in `778bbc5`).
- [x] Slice 3a - native NoPM baseline: project read for JSON/TOML manifests,
      dependency/target mapping, manifest hash, path containment, lockfile
      generation/parsing, store index, `add`/`remove`/`lock`/`update`/`tree`/
      `install`/`why`, dispatch identity, no-Node lifecycle, and lifecycle unit
      tests.
- [x] Slice 3b - git/npm/cargo/pypi/nuget/oci/nova adapters, `search`, `audit`,
      `sys:` tools, and remote store download/extract/verify. All HTTP/archive
      work in Rust; no Node/Python/curl helper.
- [x] Slice 4 - JS/TS build bridge, native packaging, Tier-1 CI, native parity
      12/12, no-Node core/NoPM checks, and Apache-2.0 metadata. The npm
      postinstall copies `lib` and TypeScript; the isolated package smoke covers
      JS/TS, C/native, and `astra nopm` paths.

### Open

- [x] Release validation - `npm run verify`, `npm run ci`, `npm run package`,
      clean-install smoke, release dry run, error-code check, and workflow
      audit are green. Cross-platform package validation run `44964210902` is
      green on Ubuntu, macOS, and Windows.
- [x] Bump to `1.0.1`, move the `[Unreleased]` changelog entries, and make the
      native version metadata follow the workspace version.
- [x] Tag `v1.0.1` and publish the GitHub release with archives, sidecars, and attestations.
- [x] Recreate the four trusted publishers with `--allow-publish` using npm
      12.2.3+, then publish all four `1.0.1` packages. Release run
      `45125506996` is green; `npm view` reports `1.0.1` and `latest` for all
      four; `npm audit signatures` verifies signatures and attestations; a clean
      Windows install reports `astra 1.0.1`/`nopm 1.0.1`.
- [x] Delete the superseded `luminousvalentine-cell/Neo-Astra-CI` repo and the
      `NPM_TOKEN` secret.
- [ ] Revoke the npm token exposed during the 1.0.0 release.
- [ ] Update install/package docs when the native package actually ships.

## 1.1.0 - Language v0.2 syntax cleanup (edition 0.2)

**Status:** Phases 0-3 are complete on private master; Phase 4 formatter and
canonical-lint core work is landed. `spec-v0.1.md` remains normative, and
`conformance/v0.2` is not loaded by the v0.1 runner.

Plan, decisions, and reviewer RFCs: [`Report/Syntax-v0.2.md`](Report/Syntax-v0.2.md),
[`RFC-A-error-model.md`](Report/RFC-A-error-model.md), and
[`RFC-B-generics.md`](Report/RFC-B-generics.md). The concrete phased plan,
estimates, and acceptance gates are in
[`Syntax-v0.2.md` §5](Report/Syntax-v0.2.md#5-implementation-plan).
Do not implement these in the native CLI slices; they target v0.1 until the
language is frozen.

### Phase 0 evidence

- [x] Record the accepted cleanup plan and defaults (`aba181a`, `5d65ab8`,
      `d1797f0`).
- [x] Close RFC A error-model decisions and RFC B generic-call decisions
      (`dbb057f`, `fb5a207`, `44a416d`).
- [x] Add the non-normative `spec-v0.2.md` skeleton and reserve
      `NS2008`-`NS2019` (`44a416d`).
- [x] Add `tools/grammar-v0.2` canonical LR(1)/Pager conflict check (4/4 tests)
      (`44a416d`).
- [x] Add the non-loaded `conformance/v0.2` corpus, including migration
      input/golden pairs (`ec1c3f4`).
- [x] Tighten formatter/match-arm and error-pattern head rules
      (`21234aa`, `54ef1d0`).

### Grammar and disambiguation

- [x] Freeze the error-model RFC: `-> T ! E`, `?`, `fail`, `Ok`/`Err`, and the
      demotion of `Result<T, E>` to a library/FFI type. Decisions accepted;
      implementation is gated after native `1.0.1`.
- [x] Freeze the generic-call RFC: `::<...>` in expression position, method and
      associated calls, constructors, precedence, and negative comparison cases.
      Decisions accepted; implementation is gated after native `1.0.1`.
- [x] Replace `{@mode:name; ...}` / `{@memory:name; ...}` with
      `@mode(name) { ... }` / `@memory(name) { ... }` under edition `0.2`.
- [x] Use commas everywhere (trailing allowed); block-bodied match arms may
      omit the comma.
- [x] State line continuation by delimiter kind: whitespace in `(...)`/`[...]`,
      statement boundaries in blocks, list rules in struct/map literals.
- [x] Add edition-`0.2` checker rules for `Err` member heads, member
      exhaustiveness, and D7 declared-set subset checking; add an AST
      serialization contract test.
- [x] Fix tuple-index lexing: after `.` in postfix position read an integer-only
      token, never a float; keep `..` ranges and leading-dot chains distinct.
      Phase 2 added an edition-`0.2` lexer-context rule so `t.0.1` lexes as two
      integer indexes while `1.0` remains a float; v0.1 suffixed forms such as
      `t.0u8` are unchanged.
- [ ] Make the LSP edition-aware before a server is packaged: `crates/astra-lsp`
      still hardcodes `0.1` in project requests and standalone parse/format.
- [ ] Contextualize `default` and `from`; decide `module` and `type`.
- [ ] Deprecate `/%`, `spawn ||`, and the special `///` meaning; keep parsing
      the old forms and omit them from canonical docs.
- [ ] Prune `syntax-legend.md` lookalikes after duplicate syntax is gone.

### Aliases (deprecate, keep parsing)

- [ ] Keep all seven aliases accepted for compatibility; do not remove parser
      support.
- [ ] Omit aliases from canonical docs and examples.
- [x] Keep formatter canonicalization to `fn`, `let`, `const`, `type`,
      `struct`, `mut`, and `pub`; edition `0.2` also emits `fail`, `?`,
      `::<...>`, and `@mode(...)` / `@memory(...)`.
- [ ] Add the deny-by-default `canonical-syntax` lint and check-only
      `fmt --canonical`; `astra migrate` performs rewrites. Core lint is in
      `crates/astra-core/src/canonical.rs`; CLI flag wiring remains.
- [ ] Run the canonical lint in stdlib CI.
- [ ] Optional later: make the aliases contextual so the words can be
      identifiers.

### Feature gaps

- [ ] `if let` / `while let`.
- [ ] Ranges with `for`, iterator, and slice semantics.
- [ ] Bitwise `& | ^ << >>`, documenting expression `|` vs error-set `|`.
- [ ] Add `char` plus scalar literals.
- [ ] Add lambda syntax (`fn(x) => expr`) with parameter inference and capture
      rules for `spawn` and lambdas.
- [ ] Complete integer/float suffix table.

### Migration and freeze

- [x] Implement `edition` plumbing first and keep the default at `"0.1"`; no
      syntax behavior changes in that step. `Edition` lives in
      `crates/astra-core/src/edition.rs`; options-aware lex/parse/format entry
      points and `CheckOptions.edition` are threaded through the compiler API,
      manifest validation (`NS2008`), and the native CLI. Gate tests in
      `crates/astra-core/tests/edition_v0_1_compat.rs` prove v0.1 output is
      byte-identical and explicit `"0.2"` still parses the v0.1 surface.
- [ ] Parser diagnostics for every deprecated or edition-rejected spelling with
      the canonical replacement.
- [x] Formatter/codemod rewrites for aliases, `!` forms, scope blocks, and
      turbofish (`crates/astra-core/src/migrate.rs`); CLI command wiring remains.
- [x] One conformance case per changed construct, including negative parses
      (`conformance/v0.2`, `edition_v0_2_checker.rs`, `edition_v0_2_migrate.rs`).
- [ ] Error-code ledger entries for new syntax diagnostics.
- [ ] Encode the v0.2 EBNF in a parser generator and require zero grammar
      conflicts, especially for turbofish, struct literals vs blocks, and
      lambda `=>` vs match `=>`.
- [ ] 10-20 realistic programs written in the proposed syntax and parsed before
      freeze.
- [ ] Freeze `spec-v0.2.md`, then update parser, checker, formatter, codegen,
      conformance, examples, and CLI together.
- [x] Implement `edition = "0.2"` gating so the compiler accepts v0.1 and v0.2
      syntax, and migrate source. The `astra migrate` CLI command is still
      unwired.
- [ ] Treat the feature-gap list above as a separate train; do not make it a
      blocker for the v0.2 cleanup freeze.

## 1.2.0 - browser playground

The native CLI replacement moved to the `1.0.1` section above.

### Core bridge

- [ ] Compiler Wasm artifact builds in CI and is versioned with the host
      bundle.
- [ ] Host checks the protocol version and refuses mismatched artifacts with a
      clear error.
- [ ] Requests and responses are UTF-8 JSON matching the existing schema;
      traps and panics surface as internal errors, never as user syntax errors.
- [ ] Every allocation is freed; a soak test of 1,000 compiles shows stable
      memory.

### Editor

- [ ] Edit source and a minimal manifest; switch between JSON and TOML.
- [ ] Target and mode selectors sent with each request.
- [ ] Diagnostics render severity, stable code, file, line/column, message, and
      hint; selecting one moves the cursor.
- [ ] Sample gallery loads without network access after the first visit.
- [ ] Shareable editor state in the URL.

### Compiler actions

- [ ] Check, with the full diagnostic set.
- [ ] AST view (the compiler API now returns the parsed tree).
- [ ] Format through the same formatter as the CLI.
- [ ] Mode and capability explanation (the compiler API now returns it).
- [ ] JavaScript emission with copy and download.

### Execution and safety

- [ ] Generated modules run in a sandboxed frame with no access to host globals,
      storage, or credentials; the sandbox cannot navigate the host page or open
      popups.
- [ ] Output capture with attribution; infinite loops terminate at a
      configurable timeout; output is capped; runtime failures show their codes
      and a call stack when available.
- [ ] Local compilation is the default and the UI says so; no source leaves the
      browser unless a remote compiler is explicitly enabled, with upload and
      retention disclosed first.
- [ ] Restrictive content security policy with network denied by default; no
      analytics or third-party script receives source text.

### Delivery

- [ ] Tests cover the loader, codegen bridge, diagnostic mapping, URL state, and
      sandbox timeout; one headless browser test compiles and runs a sample.
- [ ] The gate includes the playground build and tests.
- [ ] Initial gzipped payload within the size budget, with the measured number
      recorded.
- [ ] Works in current Chrome, Firefox, Safari, and Edge.
- [ ] Static hosting, offline assets, README, and privacy note.

## 1.3.0 - high-level mode

- [ ] Write the language section: the new mode, the low-level surface it
      removes, and the conveniences it adds.
- [ ] Parser and lowering: top-level statements, implicit entry point and exit
      code, implicit tail return, and an error when top-level statements are
      mixed with an explicit entry function.
- [ ] Checker: reject manual memory, pointers, regions, FFI, script blocks, and
      C ABI exports in this mode.
- [ ] The `print` builtin and per-project profile selection.
- [ ] Optional return-type inference for these modules.
- [ ] Conformance: a hello-world that runs on every backend, plus one reject
      case per removed feature.

## 1.x - more language surfaces

One release per surface, in the order taken up:

- [ ] Rust target and interop.
- [ ] Go target and interop.
- [ ] Lua target and interop.
- [ ] Ruby target and interop.
- [ ] Java target and interop.
- [ ] WebAssembly as a managed target, including garbage collection in linear
      memory.
- [ ] First-party machine-code backend, replacing the generate-C path.

## Deferred - no version yet

- [ ] Fallible allocation entry points for managed and region allocation; until
      then out-of-memory panics.
- [ ] `MutSlice<T>` behind pointer slice access (the type is named, the surface
      is not).
- [ ] Adaptive collection threshold.
- [ ] Pin release API for library returns (the collector follow-up).
- [ ] Script embeddings beyond C, JavaScript, and TypeScript: Python, Wasm, and
      Java.
- [ ] Cross-target triples with the pinned toolchain.
- [ ] Thread pool as a later performance layer, after the thread-per-spawn
      model.
- [ ] Web Workers target for the concurrency model.
- [ ] Export attributes for generic struct bindings (currently rejected).
- [ ] Cross-module uniqueness for author-named export symbols (currently per
      module only).
- [ ] A structured diagnostic for mangled-name collisions (currently surfaces
      as a C duplicate-definition error).
- [ ] Remote/hosted playground compilation (out of scope unless explicitly
      enabled).

## Housekeeping

- [x] Wire the already-implemented `astra ast` and `astra explain` commands
      into the CLI.
- [ ] Finish the historical CI-failure repro notes.
- [ ] Keep the fuzz and conformance corpora growing with each feature.
- [x] Remove the temporary stack-size and single-thread test workaround once
      the parser fixes are confirmed in the gate.

## Done

- [x] Language frontend: lexer, parser, formatter with comment preservation,
      resolver, type checker, effects, diagnostics.
- [x] Capability modes with least-privileged inference.
- [x] C backend: managed GC, manual memory, regions, FFI, inline scripts,
      generics, packages, native libraries, export attributes.
- [x] JavaScript and TypeScript backends.
- [x] Nova interpreter and CLI, with project loading and linking.
- [x] Package manager: model, resolver, path/git/npm/Cargo/PyPI/NuGet/OCI
      adapters, lock/install/search/audit/why/update.
- [x] Compiler API: check, project check, compile, project compile, format,
      AST, explanation.
- [x] Conformance suite across backends; continuous integration on three
      operating systems.
- [x] Fuzzing (frontend and C emitter), nesting limit, parser hardening fixes.
- [x] Two-manifest language spec and build-manifest design.
- [x] Export attributes, side-effect ordering rule, generic-forwarding checks.
- [x] Documentation consolidation: one status file, one roadmap, one todo.
