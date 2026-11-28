# Edge-Case Protocol

This document defines what the agent does when normal execution breaks. The goal is to prevent loops, prevent silent guesses, and preserve the project state.

---

## 1. Core rule

> **Change the approach, not the story.**

If something fails, do not repeatedly run the same command and hope. Do not claim success without evidence. Do not silently reinterpret the requirement.

Every failure gets one of three outcomes:

1. **Recovered** — the problem is fixed and verified.
2. **Blocked** — the task cannot proceed without information or a decision.
3. **Partial** — some work is complete, but the definition of done is not met.

Record the outcome in `../state.md`.

---

## 2. Retry and loop limits

| Situation | Maximum attempts | Then |
|---|---:|---|
| Transient network/API failure | 1 retry | Report the failure and use an offline path if possible |
| Build or test failure | 2 different fixes | Stop, inspect assumptions, report |
| Same exact error three times | 0 more attempts | Stop and escalate |
| Tool returns malformed output | 1 reformulation | Stop and report the malformed result |
| Package installation failure | 1 alternate source | Stop and report |

An “attempt” is a changed approach. Re-running the identical command is not a new attempt.

Before stopping, record:

- The command
- The exact error
- What was already tried
- The suspected cause
- The next plausible approach

---

## 3. If-then protocols

### EC-01 — Missing file or directory

**If** a required file does not exist:

1. Search the repository for the file or an equivalent.
2. Check whether the file is expected to be created in this phase.
3. If it is a generated file, check the generator or build command.
4. If it is user-authored and cannot be reconstructed safely, stop and ask.

**Do not** invent its contents and present them as existing.

### EC-02 — Ambiguous requirement

**If** the request has more than one reasonable interpretation:

1. Determine whether the ambiguity affects correctness or public API.
2. If it does, ask one focused question with options.
3. If it does not, choose the least-privilege interpretation, state it, and record it.

Do not ask three vague questions. Ask one question that unblocks the work.

### EC-03 — Conflicting decisions

**If** two confirmed decisions conflict:

1. Quote both decisions.
2. Explain the exact conflict.
3. Propose at least one resolution.
4. State which decision would need to change.
5. Stop and ask before implementing.

Do not silently pick the more convenient decision.

### EC-04 — New feature requested

**If** the user asks for a feature not covered by the spec:

1. Check whether it is already implied by a confirmed decision.
2. If not, treat it as a new proposal.
3. Add it to `../state.md` as a proposal.
4. Ask whether it should become a decision before implementation.

### EC-05 — Scope creep

**If** you notice a related improvement while working:

1. Do not implement it.
2. Add a todo item with a short description.
3. Continue the current task.

### EC-06 — Build or test failure

**If** a build or test fails:

1. Read the full error.
2. Reproduce it with the smallest command.
3. Identify whether it is caused by the current change.
4. Fix the cause, not the symptom.
5. Re-run the check.
6. If it still fails after two changed approaches, stop and report.

**Do not** delete a failing test to make the suite green.

### EC-07 — Semantic drift between targets

**If** native, Wasm, and JS behave differently:

1. Write or identify a conformance case.
2. Decide whether the difference is a bug or a documented boundary behavior.
3. Fix the backend, not the conformance test, unless the test itself is wrong.
4. Record the result.

### EC-08 — Generated code is ugly or non-idiomatic

**If** generated JavaScript or TypeScript is unreadable:

1. Inspect the IR and backend mapping.
2. Determine whether the ugliness is required for semantic correctness.
3. If not required, fix the backend.
4. If required, document why.
5. Do not sacrifice semantics for aesthetics.

### EC-09 — Missing package or toolchain

**If** a dependency, compiler, or package manager is missing:

1. Check the project documentation for the expected tool.
2. Check whether a local alternative exists.
3. Do not install global tools without asking.
4. If the task cannot proceed, report the missing tool and the exact command needed.

### EC-10 — Network or API failure

**If** an external API or network request fails:

1. Retry once if the failure is clearly transient.
2. If it still fails, switch to local repository information.
3. Do not fabricate remote data.
4. Record that the result may be stale.

### EC-11 — Destructive action

**If** a task requires deleting files, resetting state, force-pushing, or overwriting user work:

1. Stop.
2. List exactly what would be destroyed.
3. Ask for explicit confirmation.
4. Prefer a reversible alternative.

### EC-12 — Unfamiliar changes

**If** you find files or edits you did not create:

1. Treat them as user work.
2. Read them before changing them.
3. Do not revert or rewrite them without confirmation.
4. Mention them in the change report if they affect the task.

### EC-13 — No progress

**If** two consecutive work cycles produce no measurable progress:

1. Stop.
2. Summarize what is known.
3. Identify the smallest question that would unblock progress.
4. Ask the user or mark the task blocked.

### EC-14 — User asks to skip verification

**If** the user asks to skip tests or checks:

1. Comply only if the task is documentation-only or the user explicitly accepts the risk.
2. Record “not run” in the change report.
3. Do not use the word “done” without qualification.
4. State what remains unverified.

### EC-15 — User asks for an unsupported claim

**If** the user asks you to claim something the evidence does not support:

1. State what the evidence actually shows.
2. Offer the strongest accurate wording.
3. Do not fabricate confidence.

### EC-16 — Secrets or credentials

**If** a task requires secrets:

1. Never print secrets.
2. Never commit secrets.
3. Ask the user to provide them through the approved environment mechanism.
4. If unavailable, stop.

### EC-17 — Cross-platform path issues

**If** a path behaves differently on Windows:

1. Prefer repository-relative paths.
2. Use the workspace’s existing path style.
3. Do not assume `/tmp` exists.
4. Record platform-specific instructions.

### EC-18 — Tool output is too large

**If** a command produces too much output:

1. Filter for errors and relevant lines.
2. Save the full output to a temporary file if needed.
3. Report the filtered result and the location of the full output.

### EC-19 — Definition of done cannot be met

**If** the DoD cannot be met within the current scope:

1. Mark the task `partial` or `blocked`.
2. State which DoD items are missing.
3. Add a follow-up todo.
4. Do not mark it `done`.

### EC-20 — The next step is unclear

**If** the correct next action is not obvious:

1. Re-read `../state.md` and `../state.md`.
2. Re-read the relevant decision.
3. If still unclear, propose two options and ask.

---

## 4. Escalation format

When stopping, use this format:

```markdown
## Blocked
**Task:** <task id and title>
**Reason:** <one sentence>
**Evidence:** <command/error/file>
**Tried:**
- <attempt 1>
- <attempt 2>
**Options:**
1. <option> — <consequence>
2. <option> — <consequence>
**Recommendation:** <option>
**Needs from user:** <specific answer or action>
```

---

## 5. Never do these

- Never loop on the same failing command.
- Never hide a failed test.
- Never delete a test to make the suite pass.
- Never claim “done” without checking the DoD.
- Never silently change a confirmed decision.
- Never invent a file, API, package, or output.
- Never overwrite user work without confirmation.
- Never run destructive commands without explicit approval.
- Never use a workaround that breaks the semantic contract just to get green output.
