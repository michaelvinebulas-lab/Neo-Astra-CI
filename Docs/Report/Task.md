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
  with the output tail reported. Report the actual test count, not the count
  before your change.

## Status: COMPLETE - lead (no active task)

**Last completed: package add and remove.**

Landed with the comment-preserving manifest edit API (`addDependency` /
`removeDependency`, JSON and TOML, `NS9003` duplicate rejection), the
`nopm add` / `nopm remove` commands with `--json` and `--dry-run`, resolve ->
edit -> lock -> install with rollback of both the manifest and the lockfile.

Acceptance tests: 9 added (5 edit tests, 4 CLI end-to-end tests). These were
written by the lead under a one-time exception from the principal; the usual
split still puts `orchestrator/tests/**` on Kilo.

**Next queued, not yet assigned:** the system package source - `sys:` entries
resolved through winget, brew, apt, dnf, or zypper, with the exact command
lines unit-tested and no real installs in CI.
