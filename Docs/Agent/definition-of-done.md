# Definition of Done

This document is the completion gate. A task is not done because code compiles, a document exists, or the agent feels finished. It is done when the relevant checklist is satisfied and the evidence is recorded.

---

## 1. Universal DoD

Every task must satisfy all of these:

- [ ] The task goal is restated in concrete terms.
- [ ] The scope and out-of-scope items are clear.
- [ ] The change is consistent with D1–D25.
- [ ] No confirmed decision was silently changed.
- [ ] Unrelated files were not modified.
- [ ] The relevant checks were run, or the absence of checks is stated.
- [ ] Evidence is recorded in `../state.md`.
- [ ] `../state.md` reflects the new status.
- [ ] The next action is explicit.
- [ ] The final response separates facts from assumptions.

If any item is false, the task is not done.

---

## 2. Decision DoD

A decision is done when:

- [ ] The decision is recorded in the `../state.md` Decisions table (detailed rationale may live in `../archive/`).
- [ ] The chosen option is checked.
- [ ] The rejected options are still visible.
- [ ] Rationale is recorded.
- [ ] Consequences and tradeoffs are recorded.
- [ ] Related design notes are updated for consistency.
- [ ] Contradictions with earlier decisions are resolved or explicitly flagged.
- [ ] `project-context-glossary.md` is updated if terminology changed.
- [ ] `../state.md` records the decision.

A decision is not done if it exists only in chat.

---

## 3. Specification DoD

A specification section is done when:

- [ ] It states the rule, not just an example.
- [ ] It defines valid and invalid behavior.
- [ ] It defines error behavior.
- [ ] It defines target-specific behavior, if any.
- [ ] It defines interactions with modes and semantic policies.
- [ ] It includes at least one valid example and one invalid example.
- [ ] It has at least one conformance case or a plan for one.
- [ ] It does not contradict an earlier section.
- [ ] Ambiguous terms are defined in the glossary.
- [ ] The grammar is updated, if syntax changed.

---

## 4. Compiler feature DoD

A compiler feature is done when:

- [ ] The syntax is accepted by the lexer and parser.
- [ ] Invalid syntax produces a structured diagnostic.
- [ ] The resolver and type checker handle the feature.
- [ ] The behavior is represented in HIR/IR.
- [ ] At least one backend implements it.
- [ ] Other backends either implement it or reject it explicitly.
- [ ] Unit tests cover the happy path and at least one failure path.
- [ ] A conformance case exists if the feature is portable.
- [ ] The formatter handles the new syntax.
- [ ] Documentation is updated.
- [ ] `astra check --json` reports stable diagnostics.
- [ ] `../state.md` records the verification.

A feature is not done if it only works in one backend without an explicit target limitation.

---

## 5. Bug-fix DoD

A bug fix is done when:

- [ ] The bug is reproduced.
- [ ] The root cause is identified.
- [ ] The fix addresses the cause, not the symptom.
- [ ] A regression test exists.
- [ ] The original failing case now passes.
- [ ] Related tests still pass.
- [ ] The fix does not weaken a confirmed semantic guarantee.
- [ ] The change is recorded in `../state.md`.

A bug fix is not done if the test was deleted or skipped.

---

## 6. Backend DoD

A backend is done for a feature when:

- [ ] It consumes the shared IR, not a private parser.
- [ ] It preserves the documented semantics.
- [ ] It emits readable target code.
- [ ] It handles errors according to the target mapping.
- [ ] It handles memory according to the active policy.
- [ ] It rejects unsupported features with a clear diagnostic.
- [ ] It has generated-output tests.
- [ ] It has runtime tests where possible.
- [ ] It documents any target-specific boundary behavior.
- [ ] It does not introduce a hidden runtime unless documented.

---

## 6.1 Nova and NoPM DoD

A Nova or NoPM feature is done when:

- [ ] It consumes the shared frontend, manifest, and lockfile.
- [ ] It does not create a second parser or type checker.
- [ ] Nova remains development-only unless a confirmed decision changes its scope.
- [ ] Diagnostics use the same codes and JSON shape as `astra check`.
- [ ] Package resolution is deterministic and target-aware.
- [ ] Remote artifacts are integrity-checked.
- [ ] Lifecycle scripts are disabled by default.
- [ ] Unsupported targets or ecosystems fail with a clear diagnostic.
- [ ] The same lockfile works for Nova and compiled builds.
- [ ] Tests cover the happy path and at least one target/security failure.
- [ ] `../state.md` records the verification.

---

## 7. Documentation DoD

A documentation change is done when:

- [ ] The document has a clear purpose.
- [ ] It uses canonical naming.
- [ ] It links to related documents.
- [ ] It does not contain stale decisions.
- [ ] It distinguishes confirmed decisions from proposals.
- [ ] Examples are syntactically plausible.
- [ ] The table of contents or index is updated if needed.
- [ ] The state log is updated.

---

## 8. Agent-system DoD

A change to this folder is done when:

- [ ] The entrypoint still points to valid files.
- [ ] The read order is still coherent.
- [ ] The state log and todo list are consistent.
- [ ] No protocol contradicts `system-prompt.md`.
- [ ] No schema in `io-schemas.md` is contradicted.
- [ ] The change does not silently expand agent authority.
- [ ] The completion gate still matches the project phase.

---

## 9. Phase-gate DoD

A phase is complete when:

- [ ] Every task in the phase is `done` or explicitly deferred.
- [ ] The phase output exists.
- [ ] The phase gate in `runbook.md` is satisfied.
- [ ] Known risks are recorded.
- [ ] The next phase is unambiguous.
- [ ] `../state.md` records the transition.

Do not start the next phase because the current one is “mostly done.”

---

## 10. Evidence format

Record verification as:

```markdown
### Verification
- Command: `cargo test`
  Result: pass
  Evidence: 42 tests passed, 0 failed
- Command: `astra check --json examples/hello.ns`
  Result: not run
  Reason: CLI not implemented yet
```

Never write “tests pass” without naming the command or the test.

---

## 11. Status definitions

| Status | Meaning |
|---|---|
| `todo` | Not started |
| `in_progress` | Actively being worked |
| `partial` | Some work is complete, DoD not met |
| `blocked` | Cannot proceed without a decision, file, tool, or permission |
| `done` | DoD satisfied and evidence recorded |

Only one task should be `in_progress` at a time.

---

## 12. Stop rule

When the DoD is satisfied:

1. Update `../state.md`.
2. Update `../state.md`.
3. Report the result.
4. Stop.

Do not continue into the next task just because there is time left.
