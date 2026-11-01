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
  with the output tail reported.

## Status: OPEN - KILO (task: package add and remove)

Read the task below, write your `## Kilo log` entry (understanding, approach,
questions or "none, proceeding"), then code. When the gate is green, write the
report in the same block and set Status to `DONE - lead`.

---

# Task: package add and remove

## Goal

npm-style install UX that edits the package manifest without destroying its
formatting or comments, then updates the lockfile and store.

## Deliverables

- Manifest edit API: add and remove a dependency, canonical output, duplicate
  rejection. Comments are a TOML concern; for JSON, stable key order is the
  equivalent - do not invent a JSON comment syntax.
- `nopm add <source-qualified-id>[@version] [dir]`: resolve, edit the package
  manifest, rewrite the lockfile, install.
- `nopm remove <name>`.
- Flags: `--json` and `--dry-run` (dry-run writes nothing).
- Everything stays inside `orchestrator/**`.

## Acceptance

- Edit tests: comments survive, formatting is stable, duplicates are rejected,
  and both JSON and TOML manifests are covered.
- End-to-end tests for path, git, and npm sources: add and remove, correct
  lockfile diff, populated store, dry-run writes nothing.
- The full gate is green and the report includes the tail.

## Notes

- The two-manifest layout is landed: package keys live in the core manifest,
  build keys in the build manifest, and there is no fallback between them.
- Do not touch the root `package.json` or anything under `Docs/`.
- Optional small follow-up when convenient: `astra ast` and `astra explain`
  still route to `runNotImplemented`; the compiler API methods behind them are
  implemented and working.
