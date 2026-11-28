# Vision pillars

**Status:** Active proof backlog. One file per pillar, worked through one at a time.

This folder turns the founding idea into executable proofs. Each file records:

- the original promise, quoted from the founding record;
- what the tree can currently show, and an honest status;
- a concrete `.ns` / manifest proof program and the exact commands that run it;
- the gate that must pass before the pillar is called proven;
- a release code used to track it in the queue.

The codes are `PV-01` … `PV-09`. They are stable identifiers, **not** version commitments. A release slot is assigned when the proof is scheduled.

## Conformance cases

- [`PV-01` — one semantic contract](../../../conformance/vision/pv01-contract.ns): frozen stdout in
  [`pv01-contract.expected`](../../../conformance/vision/pv01-contract.expected), driver
  [`run-pv01.mjs`](../../../conformance/vision/run-pv01.mjs), and captured evidence
  [`pv01-report.md`](../../../conformance/vision/pv01-report.md).

## Index

| Code | Pillar | Status | Release target |
|---|---|---|---|
| `PV-01` | One semantic contract | Proven | TBD |
| `PV-02` | Modes as capabilities | Partial | TBD |
| `PV-03` | Semantic policies | Unproven | TBD |
| `PV-04` | GC + manual memory | Partial | TBD |
| `PV-05` | Structured concurrency | Partial | TBD |
| `PV-06` | Polymorphic FFI | Partial | TBD |
| `PV-07` | NoPM | Partial | TBD |
| `PV-08` | Nova parity | Unproven | TBD |
| `PV-09` | Explainability | Strong-ish | TBD |

## Rules

1. A pillar is not “done” because a feature exists. It is done when its proof program passes on every target the promise names.
2. Do not add a new `.ns` feature to a pillar file. These documents test the contract; `spec-v0.1.md` / `spec-v0.2.md` define the language.
3. When a proof passes, update its status, record the evidence, and add a link to the conformance case.
4. When a proof fails because of a real gap, the gap goes into `Docs/todo.md` under the pillar code, not into a vague “later” list.

## Relationship to 1.2

`1.2.0` (browser playground) is the first **frontend/tooling** proof. It covers the tooling half of `PV-09` and part of `PV-07`. It does not close `PV-01` through `PV-08`. See [`../Playground-v1.2.md`](../Playground-v1.2.md).
