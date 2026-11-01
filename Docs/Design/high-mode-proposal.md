# `high` mode - proposal

**Status: concept DECIDED 2026-11-01; every design detail below is PROPOSED
and not normative.** Recorded in `pending-decisions.md` addendum §8. Nothing
here is implemented, scheduled, or written into `Docs/spec-v0.1.md` until the
principal approves the details.

## What was decided

`high` is a new mode that makes Neo Astra read like a high-level language: it
**removes the low-level surface** from the developer and **grants high-level
benefits**. Boilerplate is gone, strings stay quoted.

```
# high
print("hello nova")
```

instead of

```
# app / core
pub fn main() -> i32 {
    println("hello, nova")
    return 0
}
```

More target/interop languages (Rust, Go, Lua, Ruby, Java) are a separate
roadmap item (addendum §7), not part of this mode.

## Proposed design

### Position in the mode system

Today `Mode` is a capability ladder (`crates/astra-core/src/mode.rs`):
`core` (no tags) < `app` (console, args, time, env) and
`app` + `web` (dom, fetch, worker, storage, websocket) or `app` + `system`
(fs, net, process, ffi, thread, ptr, memory, os); `web` and `system` are
incomparable.

**Proposed: `high` is an application profile, not a new capability rung.**
Capability-wise it is `app` (so `print`/console is available); it adds two
things on top: a **source-level ban list** and a **sugar profile**. That keeps
`Mode::allows` simple - treat `high` like `app` with an extra frontend gate -
and avoids a capability ladder that would have to order `high` against `web`
and `system`.

Alternative considered and not recommended: a fifth capability rung below
`core`. It would be capability-identical to `core` and still need an exception
for console/`print`, so the rung buys nothing.

### Proposed ban list (developer-facing low-level surface)

Inside a `high` module these are rejected with diagnostics, even where the
capability ladder would allow them:

- `Memory.*` (alloc/free/read/write) and `@memory:manual`;
- `Ptr` types and pointer arithmetic/dereference;
- `Region` and `Region.*`;
- `ffi` declarations and foreign calls;
- `@script` blocks (any language, including `@script:js`);
- `extern`/C-ABI export machinery (`@export_as`, `pub extern`).

Rationale: `Region` currently has no mode gate (2026-10-30 decision), so the
ladder alone cannot express "no low-level surface"; this list is a source-level
promise carried by the mode.

### Proposed sugar profile

Decided by the example:

- top-level statements; no `fn main` needed; the file's top-level statements
  become the program entry and exit `0`;
- `print(...)` builtin (proposed: alias of `println`, so `print("hello nova")`
  prints with a newline).

Proposed additions to approve or trim:

- implicit tail return: the last expression of a function body is its return
  value (explicit `return` still allowed);
- mixing top-level statements with an explicit `fn main` in the same module is
  an error;
- type annotations optional wherever inference can prove them (already true for
  `let`; proposed to extend to function return types only).

Not proposed (kept as-is): strings stay quoted, so the lexer is unchanged;
structs, enums, generics, error unions, `match`, and modules all stay
available - they are high-level features, not low-level ones.

### Proposed lowering / implementation shape

`high` desugars to the existing `app` surface before checking:

1. collect top-level statements and synthesize `fn main() -> i32` (append
   `return 0`);
2. rewrite tail expressions into `return` statements;
3. then run the normal resolver/checker/backends unchanged.

The parser needs to know the profile. **Proposed: the project's
`build.defaultMode` (or the selected target's mode) sets it for the entry
module**, and imported modules are parsed as ordinary modules. Alternative:
a file-level `@mode:high` annotation - rejected as more syntax to learn, which
defeats the point of the mode.

The ban list is a checker gate (single pass, like the system-mode side-effect
pass `NS5007`), so it can carry precise diagnostics such as "`Region` is not
available in `high`; use managed values".

### Compatibility

**Proposed:** `high` behaves like `app` for `Mode::allows` (binding from other
modules is allowed exactly as `app` is), because after lowering a high module
is an app module. The bans are a promise about what high *source* contains,
not a transitive guarantee about the call graph.

### Diagnostics and conformance

New codes to assign when approved (not before): one `NS2xxx` for high-profile
syntax mistakes (top-level statement mixed with `fn main`) and one `NS5xxx`
for a banned construct. Conformance cases: a high hello-world that runs on
every backend, plus one reject case per ban-list entry.

### Scheduling

Not in the V1 queue. Per the versioning policy (addendum §6) this is a new
mode, so it ships as a minor bump after 1.0.0 (e.g. `1.1.0`), design and
conformance first.

## Open questions for the principal (need answers before spec text)

1. Capability basis: `high = app + bans + sugar` (recommended), or strict
   `core` with a special `print` that bypasses the console capability?
2. Ban list: confirm all six groups (Memory/`@memory:manual`, `Ptr`, `Region`,
   `ffi`, `@script`, C-ABI/`@export_as`).
3. Sugar: confirm top-level statements + implicit `main`/exit 0 and
   `print`; keep or drop implicit tail return, the `fn main` mixing error, and
   optional return-type inference?
4. Profile selection: manifest `defaultMode: "high"` for the project's entry
   module (recommended), or another mechanism?
5. Is `high` meant to be importable by other modes (proposed: yes, as an
   app-level module), or is it strictly an application entry surface?
