# Task channel - lead to Kilo

Working channel between the **lead** (planner/reviewer) and **Kilo** (coder).
The **principal** (the user) is the final authority. Rule 10 applies to both:
do not invent decisions - mark the question instead.

## Protocol

- The Status line is the lock. While it says `OPEN - KILO`, the lead runs no
  shell or git operations in this repository.
- Neither side rewrites the other's section. Append only.
- Kilo never commits, never pushes, never edits `Docs/**` or the spec. The
  deliverable is code, a `## Kilo log` entry, and a Status update. The lead
  reviews and lands.
- Questions that need the principal: write them under `## Kilo log` and set
  Status to `BLOCKED - principal`.
- Before handback: `cargo fmt --all` and the full gate (`npm run ci`) green,
  with the output tail reported.
- The lead writes the tests for `orchestrator/**` (principal's decision,
  2026-11-02). Your deliverable is implementation code, the log entry, and a
  green gate; the lead adds coverage before landing.

## Status: COMPLETE - lead (no active task)

**Last completed: system package source.**

`nopm add sys:<tool>` installs `git`, `cmake`, or `clang` through winget, brew,
apt, dnf, pacman, zypper, or apk. The planner is pure and matches the pinned
command lines exactly; elevation is `sudo -n` off-root; unknown tools are
`NP0015`, a missing manager `NP0017`, an install failure `NP0018`, and
`nopm remove sys:<tool>` is `NP0016`. `sys:` never touches the manifest,
lockfile, or store.

Coverage: six lead-written tests (manager matrix, manager detection, executor
order and short-circuit, `NP0018`, and a CLI test proving `add sys:` writes
nothing). `scripts/bootstrap.mjs` now delegates to the adapter; its `--check`
output is byte-identical to before.

**Next queued, not assigned:** toolchain pinning - parse the `[toolchain]`
section, resolve pin -> cache -> PATH with `ASTRA_CC` honored, add
`astra doctor`, and record the resolved toolchain in the lockfile.
