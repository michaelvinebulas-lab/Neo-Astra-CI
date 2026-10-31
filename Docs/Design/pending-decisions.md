# Pending decisions — 2026-10-31

**Status: every item below is proposed, none decided** (agent rule 10: do not
invent decisions). Each item gives its context from existing docs, the
options, and a proposed recommendation. Answer by quoting the numbers, e.g.
`1A 2B 3A 4A 5A 6A 7aC 7bB 8A`. Verdicts get recorded here and in
`Docs/Agent/state-log.md` **before** any implementation starts.

---

## 1. `[P11]` — call-argument evaluation order (spec §2)

**Context.** `Docs/spec-v0.1.md` §2 states no evaluation order for call
arguments. GC materialisation (`GC-001-proposal.md` §5.3) pins left-to-right
for *managed* arguments as an implementation detail, explicitly deferred to
this review by the `[P11]` verdict (proposal §10). The JS/TS backends evaluate
left-to-right because those languages do.

**Options.**

- **A — Left-to-right normative for all arguments.** Spec §2 states it; every
  backend must match; pin it with a test per backend (call with two
  side-effecting arguments, assert the order). *Pros:* one simple rule,
  matches JS/TS naturally, matches the GC mechanism already built, side-effect
  order becomes portable. *Cons:* the Nova interpreter and C emitter must be
  verified and pinned; one small spec addition.
- **B — Leave unspecified.** Spec explicitly says any order; programs may not
  rely on it. *Pros:* zero work, maximum backend freedom. *Cons:* a program
  whose arguments have side effects behaves differently per backend — a
  portability trap conformance cannot catch.
- **C — Normative only for managed arguments.** The GC-pinned subset becomes
  the rule; everything else stays free. *Pros:* minimal spec change covering
  the case that has a mechanism. *Cons:* two order rules in one expression
  class; still leaves the portability trap for unmanaged arguments.

**Proposed recommendation: A.** Every backend already does or can cheaply do
it; B is a documented portability hazard. Cost: one spec paragraph plus one
pinning test per backend.

---

## 2. Generic-forwarding instantiation gap (checker model, `NS6002`)

**Context.** `Memory.alloc<Wrap<T>>` inside a function whose own `T` is still
a type parameter is checked with `T` unresolved. Because the checker does not
re-check bodies per call instantiation (its model since Phase 5), a managed
element type smuggled through a generic wrapper can pass silently — `NS6002`
never sees a concrete type. Recorded in `todo.md` (C-001 limitations) and
`Docs/Agent/gc-001-workstream.md`.

**Options.**

- **A — Accept and document.** Spec notes that `NS6002` is checked only when
  the element type is concrete at the definition site, and generic forwarding
  is not covered. *Pros:* zero work, honest. *Cons:* leaves a real hole in a
  compile-time safety rule — a program the checker accepts can still put
  managed data in manual memory, which is exactly what `NS6002` exists to
  prevent.
- **B — Definition-site conservative rule.** Any `Memory.alloc` /
  `region.alloc` whose element type *mentions a type parameter* is rejected
  at the definition site, under `NS6002`'s umbrella (exact diagnostic code
  picked at implementation time). The forwarding form is refused; all
  concrete forms are untouched. *Pros:* closes the hole without changing the
  checker's instantiation model; small, testable checker change with a clear
  diagnostic ("element type depends on a generic parameter"). *Cons:*
  deliberately over-approximate — some generic forwarding that might be
  harmless in every real instantiation is also refused.
- **C — Instantiate-time re-checking.** Check generic bodies once per
  monomorphization. *Pros:* fixes the whole class of definition-site-silent
  checks, not just `NS6002`. *Cons:* touches the checker's core model (every
  diagnostic could duplicate per instantiation); a redesign, out of
  proportion to one gate.

**Proposed recommendation: B.** Smallest honest change that closes the
soundness hole while keeping the checker's model intact. C is the
principled long-term fix but is a checker redesign, not a gate fix.

---

## 3. `FFI-001` — generic functions across the C ABI

**Context.** `Docs/Design/V1/11-c-abi.md` §3 currently states that exported
generic functions are **not** exposed in headers and cannot be called across
the C boundary; each project-internal call site gets its own specialization.
`FFI-001` (polymorphic FFI) has no proposal. The question: is that refusal
the final V1 answer, or a placeholder?

**Options.**

- **A — Confirm the refusal as the V1 answer.** C consumers call only
  concrete `pub` functions; generics stay project-internal; `FFI-001`'s
  generic half shrinks to post-V1. *Pros:* already implemented and
  documented, zero work, ABI stays simple (no descriptors, no mangling
  contract for consumers). *Cons:* a C consumer that wants `list<i32>`
  helpers must ask for concrete wrapper functions in Neo Astra code.
- **B — Explicit instantiation exports (new language surface).** A way to
  say "export `f` for `i32` and `str`" — an export list with type arguments.
  Needs spec syntax plus `11-c-abi.md` mangling rules. *Pros:* real generics
  across the boundary without runtime descriptors. *Cons:* **new language
  surface** — must be specced and approved before any code; the mangling
  becomes part of the ABI contract.
- **C — Polymorphic entry points (runtime type descriptors).** One
  C-callable symbol per generic, taking type information at runtime.
  *Pros:* fully general. *Cons:* heavy runtime machinery (descriptors,
  boxing rules), ABI-v2 territory, fights the language's monomorphization
  model.

**Proposed recommendation: A for V1** — it is what the docs already say and
what is implemented — with **B recorded as the post-V1 candidate** if a real
C consumer ever asks. If you want B now, it opens with spec surface, not
code.

---

## 4. The unblocked `@script:c` boundary slice (near-term FFI work)

**Context.** The `[P9]` scoped-handle signature landed as `11-c-abi.md` §5.1
(`astra_gc_scope_enter` / `astra_gc_root` / `astra_gc_scope_exit`) *before*
any call site existed. The emitter is meant to emit it at the first
foreign-frame boundary that passes managed values into C code — no such
boundary is emitted yet, so the emission path and the `GC-001-proposal.md` §7
boundary-rooting test are unwritten.

**Options.**

- **A — Do it now.** Build the test case: a `@script:c` block receiving a
  managed value (str/slice/array), emit the registration triple, land the §7
  test proving the value survives a collection while the foreign frame is
  active. *Pros:* converts approved-on-paper API into a tested one; the only
  concrete unblocked implementation work on the queue; follows the routine
  hosted compile-and-run pattern already in `emit_c` tests. *Cons:* small;
  somewhat speculative until a real program needs it.
- **B — Wait for a real program** that needs managed values in C callbacks.
  *Pros:* no speculative work. *Cons:* §5.1 stays untested end-to-end; if it
  is wrong we find out when it is load-bearing.

**Proposed recommendation: A.** Decided design waiting for its first call
site; landing it closes the "signature landed, call site missing" gap.

---

## 5. `CONC-001` second slice — OS-threaded structured concurrency

**Context.** V1 shipped the single-threaded eager model (`spawn`/`Task`,
bounded `Channel`, deadlock panics), matching JS/Nova. The OS-threaded slice
is design-gated behind four decisions: threading model, blocking-wait
semantics (which changes the `channels` conformance case), task-local heaps
vs GC-001's single-heap collector, and cancellation safepoints.

**Options.**

- **A — Defer to post-V1.** V1 is complete and cross-backend consistent;
  record the second slice as post-V1 next to `WASM-001`. *Pros:* keeps the
  green tree green, no semantic churn in the conformance corpus. *Cons:* no
  real parallelism for users until post-V1.
- **B — Write the proposal now, implement later.** Draft
  `Docs/Design/CONC-001-proposal.md` covering the four decision areas with
  options, land it for approval; implementation after. *Pros:* design
  progress with no code risk; you approve concrete options instead of
  abstractions. *Cons:* options (especially conformance re-pinning) may
  shift as the codebase moves.
- **C — Proposal + implementation now.** *Pros:* threads land in V1.
  *Cons:* the largest work item on the board — touches GC (stop-the-world or
  per-task heaps), every channel test, and deadlock semantics; high risk
  against an all-green tree.

**Proposed recommendation: A**, promoting to B whenever you want the design
on paper. C only if OS threads are a launch requirement — that is a product
call, not an engineering one.

---

## 6. `HARD-001` — what "v0.1 hardening" means

**Context.** The placeholder has no definition anywhere (one backlog line).
Rule 6 (queue, not brainstorm) means it needs a scope decision before it can
be worked. Candidate scopes:

**Options.**

- **A — Robustness/fuzz pass.** Extend the recorded defect-trail method:
  property and fuzz tests for lexer/parser/formatter (random and mutated
  input never panics), panic-message and OOM-path audit, deterministic repro
  for every historical CI failure class. *Pros:* directly extends the method
  that has kept CI green; backend-neutral. *Cons:* only as good as the
  oracles written.
- **B — Release engineering.** Versioning policy, packaging/distribution,
  install and upgrade docs, changelog discipline. *Pros:* needed for any
  real v0.1 tag. *Cons:* no compiler behavior change.
- **C — Leave undefined**, folded into the v0.1 release checklist when a
  release date exists. *Pros:* no premature scope. *Cons:* the placeholder
  stays ambiguous.

**Proposed recommendation: A as the definition** (it matches the recorded
method), with B becoming its own task when a release is actually planned.
If neither is wanted now, choose C — but then the backlog line should say
"post-v0.1", not "undefined".

---

## 7. `PLAY-001` — two questions

**Context.** The expanded plan (slices A–E) exists; nothing design-blocks
it; the `ast` and `explain` API methods it wants are still stubs. The
unmerged branch `work/play-001-a` carries one commit (`a1ec67c`, "wasm
bridge scaffold") forked from an old master, before all of C-001/GC-001.

**7a — The `work/play-001-a` branch:**

- **A — Keep** it as the slice-A starting point (a scaffold rebase is cheap).
- **B — Drop** it (delete the branch; slice A is re-doable from the plan,
  and the tree has moved a lot since the fork).
- **C — Park** it (leave untouched until PLAY-001 actually starts; zero
  cost, decide then with better information).

**Proposed: C.** The branch costs nothing where it is.

**7b — Start PLAY-001 now, or later?**

- **A — Start now** (it is first in the up-next queue; the `ast`/`explain`
  stub work is ordinary compiler-api slices).
- **B — Later** — after item 4 (the boundary slice is smaller and closes
  approved design), then PLAY-001 becomes the active work.

**Proposed: B** — item 4 first regardless; PLAY-001 follows as the
multi-slice feature.

---

## 8. Confirm the deferral list

**Context.** Currently `[-]`: GC-001 Phase D (Wasm allocator), `WASM-001`,
fallible `Memory.alloc`/`region.alloc` entry-point policy, `MutSlice<T>`
surface, post-V1 pin release API, GC threshold adaptivity.

**Options.**

- **A — Confirm all six stay deferred** exactly where they are.
- **B — Pull one forward** (name it) — it becomes its own numbered item
  next round.

**Proposed: A.**

---

## Appendix — already decided, listed for completeness (no answer needed)

- **`Region` + `@memory:manual`:** decided 2026-10-30 — no gate; spec §8.4
  carries the clarification (scoped allocation, never-freed leak hazard).
  The stale todo entry was closed when this document landed.
- **`[P9]` signature, §5.8 pinning scope, per-case conformance mode:**
  decided and implemented (runs 29/35).
