# Toolchain and package-manager plan (owned by the lead)

Status: **direction decided 2026-10-31; slices not started.** This is the
reference for the NoPM / CLI / toolchain work orders. It records the user's
decisions; it does not add new ones.

## Decisions on record

- **D26** — two manifests: `core.*` (package identity, dependencies, sources,
  registry, capabilities, ffi) and `build.*` (build directives, targets, `nova`,
  semantic policies, `toolchain`). `build.*` is optional; clean break, no
  dual-read. Spec §12 rewritten; `Docs/architecture.md` D26.
- **V1 machine code path = C.** We emit C17 and let a C toolchain produce
  machine code. A first-party machine-code backend (empty `astra-codegen-llvm`
  crate) is a **V2 roadmap item**, not V1 work.
- **`@export_as` (P2)** — author-named generic instantiations; spec stage done
  (spec §11.5, `11-c-abi.md` §3); implementation is its own queued slice.
- **P3** — transitive `NS6002` worklist replay; implementation in Kilo's fix
  round (F11).

## 1. `nopm add` / `remove` (npm-install UX)

**Shape:**

```
nopm add <source-qualified-id>[@version] [dir]
nopm remove <name> [dir]
```

Flow: resolve through the existing adapters → validate the ID → edit
`core.*` `[dependencies]` → rewrite `core.lock.json` → install into the store.

**Reality of the pieces:** resolution, lock writing, and install already exist
(`lock`, `install` commands + `installLockedPackages`). The new part is
**surgical manifest editing**:

- §12.1 requires `astra fmt` to preserve TOML `#` comments, so `add` must
  preserve them too: either a comment-preserving edit or a canonical rewrite
  that carries comments forward. Recommended: parse → insert key → emit
  canonically with comment carry-over; a diff test pins it.
- JSON form: same schema, canonical formatting.
- Reject duplicates with a clear message; `--json` supported.

**Constraints:** no network needed beyond resolution; `--dry-run` recommended;
never edits `build.*`.

## 2. System package manager source (`sys:`)

Make the `scripts/bootstrap.mjs` install logic (winget / brew / apt / dnf /
zypper) a NoPM **source** so bootstrap and the toolchain ladder share one path:

```
nopm add sys:gcc        # or sys:zig, sys:pkg-config
```

- Tools only (compilers, linkers, build helpers) — never linkable Neo Astra
  packages.
- Elevation is the adapter's business and only on an explicit command.
- **`astra build` must never elevate.** Builds may download into the project
  cache (user-space) only.
- Versions vary by distro: `sys` is the convenience rung, not the
  reproducibility rung.

## 3. Toolchain resolution ladder

```
1. build.* [toolchain] pinned id          -> reproducible, may download
2. .tools/toolchain/<id> cache            -> offline-ok
3. system PATH (cc/gcc/clang/cl/zig)      -> fast path
4. sys adapter (policy = auto only)       -> convenience, explicit cmd only
5. pinned download into project cache     -> reproducible fallback, no elevation
6. fail loudly with instructions
```

Policy: a manifest-pinned toolchain skips `sys`; `auto` allows it.

**Slices:**

| Slice | Content | Depends on |
|---|---|---|
| TOOLCHAIN-001a | `[toolchain]` parsing + ladder (pin → cache → PATH) + `astra doctor` + lockfile record. **No network.** | D26 |
| TOOLCHAIN-001b | pinned download / verify / unpack — recommend **Zig `zig cc`** as the single vendor artifact family (one binary per OS/arch, cross-compilation included, MIT). SHA-256 table in-repo, offline + `--no-download` behavior. | 001a, NOPM-006 |
| TOOLCHAIN-001c | `--target-triple` cross compilation. Post-V1 candidate. | 001b |
| NOPM-005 | `add`/`remove` + comment-preserving manifest edits. | D26 |
| NOPM-006 | `sys:` source; `bootstrap.mjs` becomes a caller. | — |

**Open (needs the user):** CLI home for `add` (`nopm add` recommended vs
`astra add`); `remove` in the same slice (recommended); `sys:` naming
(recommended); ladder policy above (recommended).

## 4. What must not happen

- No toolchain binaries in `[dependencies]` — they are not linkable packages.
- No implicit elevation during builds.
- No silent mixing: `astra doctor` reports which rung resolved, and the lockfile
  records the exact toolchain.
- No dual-read manifest compatibility layer (D26 is a clean break).
