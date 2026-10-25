# Input/Output Data Schemas

These schemas keep agent work predictable. Use JSON for machine-readable records and Markdown for human-facing reports.

---

## 1. Global rules

- JSON must be valid UTF-8.
- JSON must use double quotes.
- JSON must not contain comments.
- Dates use ISO 8601 UTC: `YYYY-MM-DDTHH:MM:SSZ`.
- IDs use uppercase prefixes: `ASTRA-001`, `SPEC-001`, `D22`.
- Unknown fields are invalid unless a schema explicitly allows extensions.
- Arrays preserve order.
- `null` means “not applicable,” not “unknown.”
- Use `"unknown"` for an unknown value; do not omit a required field silently.
- Do not put secrets, tokens, or credentials in any record.

---

## 2. Task request

Used when the user or a planner assigns work.

```json
{
  "id": "SPEC-001",
  "title": "Create the spec-v0.1 skeleton",
  "phase": "specification",
  "goal": "Create a complete specification skeleton with stable section headings.",
  "scope": [
    "Create docs/spec-v0.1.md",
    "Add the table of contents",
    "Add section stubs for grammar, types, modes, memory, errors, concurrency, FFI, and conformance"
  ],
  "out_of_scope": [
    "Writing the full grammar",
    "Implementing the compiler"
  ],
  "inputs": [
    "docs/architecture.md",
    "docs/Agent/project-context-glossary.md"
  ],
  "acceptance": [
    "The file exists",
    "Every confirmed decision has a corresponding section",
    "The next task is unambiguous"
  ],
  "risk": "low"
}
```

### Field rules

| Field | Type | Required | Rule |
|---|---|---|---|
| `id` | string | yes | Unique task ID |
| `title` | string | yes | Short imperative title |
| `phase` | string | yes | Must match a runbook phase |
| `goal` | string | yes | One concrete outcome |
| `scope` | string[] | yes | At least one item |
| `out_of_scope` | string[] | yes | Prevents scope creep |
| `inputs` | string[] | yes | File paths or decisions |
| `acceptance` | string[] | yes | Testable outcomes |
| `risk` | enum | yes | `low`, `medium`, `high` |

---

## 3. Decision record

Used when a new architectural decision is proposed or confirmed.

```json
{
  "id": "D22",
  "status": "proposed",
  "title": "Diagnostic code namespace",
  "decision": "Use NS#### codes for source diagnostics.",
  "rationale": "Stable codes are easier to document, test, and filter.",
  "alternatives": [
    "Use free-form messages only",
    "Use per-backend code namespaces"
  ],
  "consequences": [
    "Diagnostics become part of the public contract",
    "Codes must be allocated centrally"
  ],
  "supersedes": [],
  "related": ["D16", "D20"]
}
```

### Status values

- `proposed`
- `confirmed`
- `rejected`
- `superseded`

A decision is not confirmed until it is recorded in `../architecture.md`.

---

## 4. Change report

Used at the end of a task.

```json
{
  "task_id": "SPEC-001",
  "status": "done",
  "summary": "Created the specification skeleton.",
  "files_changed": [
    {
      "path": "docs/spec-v0.1.md",
      "change": "created"
    }
  ],
  "verification": [
    {
      "command": "none",
      "result": "not_run",
      "reason": "Documentation-only task"
    }
  ],
  "risks": [],
  "follow_ups": [
    "SPEC-002"
  ],
  "next_action": "Write the lexical rules section."
}
```

### Status values

- `done`
- `partial`
- `blocked`

Do not use `done` if any DoD item is false.

---

## 5. Verification evidence

Used inside change reports and the state log.

```json
{
  "command": "cargo test",
  "result": "pass",
  "evidence": "42 tests passed, 0 failed",
  "duration_ms": 1200,
  "timestamp": "2026-10-23T12:00:00Z"
}
```

### Result values

- `pass`
- `fail`
- `not_run`
- `skipped`

`not_run` and `skipped` require a reason.

---

## 6. State update

Used to update `state-log.md` without rewriting the whole history.

```json
{
  "date": "2026-10-23",
  "phase": "specification",
  "completed": [
    "Created docs/spec-v0.1.md skeleton"
  ],
  "active": [],
  "next": [
    "Write lexical rules"
  ],
  "blockers": [],
  "decisions": [],
  "notes": "No code exists yet."
}
```

---

## 7. Todo item

Used in `todo.md` or a machine-readable task file.

```json
{
  "id": "SPEC-002",
  "title": "Define lexical rules",
  "status": "todo",
  "phase": "specification",
  "priority": "high",
  "depends_on": ["SPEC-001"],
  "dod": [
    "Identifiers, literals, and comments are defined",
    "Valid and invalid examples exist",
    "Conformance cases are listed"
  ]
}
```

### Status values

- `todo`
- `in_progress`
- `partial`
- `blocked`
- `done`

Only one item may be `in_progress`.

---

## 8. Compiler diagnostic

This is the target shape for `astra check --json` and compiler errors.

```json
{
  "code": "NS0001",
  "severity": "error",
  "message": "Block comments are not valid in Neo Astra source.",
  "file": "src/main.ns",
  "span": {
    "start": 42,
    "end": 58
  },
  "mode": "core",
  "target": "js",
  "hint": "Use // line comments instead.",
  "related": []
}
```

### Field rules

| Field | Type | Required | Rule |
|---|---|---|---|
| `code` | string | yes | Stable `NS####` code |
| `severity` | enum | yes | `error`, `warning`, `note` |
| `message` | string | yes | One sentence, no trailing period required |
| `file` | string | yes | Repository-relative path |
| `span` | object | yes | Byte offsets |
| `mode` | string | no | Resolved mode |
| `target` | string | no | Active target |
| `hint` | string | no | Actionable fix |
| `related` | object[] | no | Related spans |

Diagnostics must be stable enough to test.

### 8.1 Formatter CLI report

`astra fmt --json` returns one report after all files have been processed:

```json
{
  "check": true,
  "ok": false,
  "files": [
    {
      "path": "src/main.ns",
      "status": "would-reformat",
      "diagnostics": []
    }
  ],
  "diagnostics": [
    {
      "code": "NS2001",
      "severity": "error",
      "message": "Expected an identifier.",
      "hint": "Use a valid name.",
      "path": "src/main.ns",
      "line": 1,
      "column": 10
    }
  ]
}
```

File statuses are `formatted`, `unchanged`, `would-reformat`, or `error`. `ok` is
false when any file has an error, or when `--check` finds a difference.

---

## 9. Mode explanation record

Used by `astra explain` and inference tests.

```json
{
  "resolved_mode": "system",
  "source": "manifest target: native",
  "reasons": [
    {
      "file": "src/io.ns",
      "span": {
        "start": 120,
        "end": 160
      },
      "reason": "@requires(system) due to raw pointer usage"
    }
  ],
  "inferred": false,
  "cache_key": "mode-system-target-native"
}
```

---

## 10. Final response template

For human-facing work, use Markdown:

```markdown
## Result
What changed, in one or two sentences.

## Files changed
- `path` — why

## Verification
- Command: `...`
  Result: pass/fail/not run
  Evidence: ...

## Risks / open questions
Only if needed.

## Next
The single next action.
```

### Rules

- Lead with the result.
- Name every changed file.
- Do not claim a check ran if it did not.
- Put the next action last.
- Keep it proportional to the task.

---

## 11. Validation rules

A record is invalid if:

- A required field is missing.
- An unknown enum value is used.
- A path is absolute or escapes the repository, unless explicitly about the environment.
- A verification result is `pass` without evidence.
- A task is `done` while its DoD is incomplete.
- A decision is `confirmed` but absent from `../architecture.md`.
- A secret or credential appears anywhere.
