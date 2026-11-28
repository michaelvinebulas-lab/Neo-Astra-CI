# Nova Project Module Loading and Linking

**Status:** Implemented in `NOVA-003` (2026-10-25); retained as the acceptance baseline  
**Depends on:** `TYPE-003` (project type surfaces, complete) and `TYPE-004` (multi-source compiler API, complete)  
**Related:** [`08-nova-and-nopm.md`](./08-nova-and-nopm.md), [`02-compiler-design.md`](./02-compiler-design.md)  
**Scope:** Project-wide Nova loading, resolved module identity, and runtime linking

---

## 1. Summary

Project linking should be implemented by adding a **module-aware runtime context**
to one `Interpreter`, not by creating one interpreter per module.

The loader parses every project file with `astra-core`, assigns each file a
canonical `ModuleId`, builds an explicit `(importer, raw specifier) -> target`
map, and resolves and type-checks each module through the existing Rust
frontend. The resolver must stop using raw specifiers as module keys once a
project graph is present; otherwise `"../shared.ns"` is ambiguous between two
modules in different directories.

Recommended sequence:

1. Add project type surfaces and cross-module checking (`TYPE-003`), then expose
   them through the multi-source compiler API (`TYPE-004`). Both are complete.
2. Refactor the interpreter to be module-aware without changing single-module
   behavior.
3. Add the loader, resolver import map, and project diagnostics.
4. Link functions and constants across modules.
5. Link structs, enums, namespace imports, re-exports, and default exports.
6. Integrate with the `nova` CLI.

Stages 1 and 2 can proceed in parallel if the APIs below are fixed first.

---

## 2. Current state and constraints

> **Implementation note:** this section records the pre-`NOVA-003` state that
> motivated the design. The module graph, canonical import map, project loader,
> module-aware interpreter, and CLI integration now exist; use the acceptance
> matrix in §9 as the living checklist.

Relevant current behavior:

- `astra_core::resolver::ModuleResolver` uses module keys directly from import
  specifiers (`resolver.rs`).
- `resolve_module` is standalone and treats external imports as opaque.
- `ModuleResolver::from_modules` is strict, but module keys are still raw
  specifier strings, not filesystem identities.
- `check_resolved` checks one AST/HIR pair. Imported values and types are
  inserted into `SymbolTable` but receive `Type::Unknown` in the checker.
- `astra_nova::Interpreter::new` accepts one `CheckResult` and builds one set of
  function/struct/enum/constant maps.
- `Value::Function` carries only a declaration. `FunctionValue` has no module
  identity.
- `Value::Imported { module, name }` exists, but `module` is the raw specifier and
  calling it returns `NV0003`.
- Expression annotations and manual-memory spans are keyed only by `Span`. Span
  offsets are relative to one source file, so different modules can collide.
- `Frame` and `Panic` carry a function name and one-module `Span`, but no module
  path.
- `hir::ExportTarget::Expression` retains no expression, so an anonymous
  `export default expression` cannot be evaluated from HIR.

Hard constraints:

- `astra-core` remains the only lexer, parser, resolver, and type checker.
- A module must be parsed exactly once.
- Runtime name resolution must not become a second type resolver.
- Existing single-module `run_source` behavior must remain valid.
- Diagnostics must identify both the module path and the source span.

---

## 3. Module identity and loader

### 3.1 `ModuleId`

Use a canonical project-relative path as the runtime identity:

```rust
pub struct ModuleId(String); // "src/main.ns", "src/util/strings.ns"
```

Rules:

- Forward slashes only, even on Windows.
- No `..`, no leading `./`, no drive letter, no symlink component.
- Derived after `canonicalize` and project-root containment checks.
- Case is preserved. On case-insensitive filesystems, reject ids that collide
  case-insensitively.
- The source `module app.util` header is metadata, not the module key. File paths
  are authoritative, matching spec §12.5.

This avoids collisions between modules with duplicate headers and gives every
diagnostic and annotation a stable owner.

### 3.2 Import mapping

Add a project graph to the resolver rather than passing raw specifiers through:

```rust
#[derive(Debug, Clone, Default)]
pub struct ModuleGraph {
    pub modules: BTreeMap<String, ast::Module>,
    pub imports: BTreeMap<(String, String), String>,
}

impl ModuleGraph {
    pub fn insert_module(&mut self, id: impl Into<String>, module: ast::Module);
    pub fn insert_import(
        &mut self,
        importer: impl Into<String>,
        specifier: impl Into<String>,
        target: impl Into<String>,
    );
}

impl ModuleResolver {
    pub fn from_graph(graph: &ModuleGraph) -> Self;
    pub fn resolve_root(&self, id: &str) -> ResolveResult<hir::Module>;
}
```

`ModuleResolver::from_modules` remains as a convenience wrapper with an identity
import map, preserving current tests.

Resolver changes:

- `ModulePass` stores its canonical module key.
- Every `ImportForm` and `ExportForm` specifier is converted through the import
  map before `target_exists`, `target_surface`, or `resolve_named`.
- `hir::Import.module` and `hir::ExportTarget::{Reexport, Namespace}` store the
  canonical target id.
- Dotted `import app.util` uses `"app.util"` as the raw map key.
- The standalone resolver keeps its current opaque-external behavior.
- Strict project resolution fails if an import map entry is missing.

This is a small, explicit resolver extension. It is preferable to rewriting AST
strings in Nova because the resolver owns module identity and cycle detection.

### 3.3 Loader algorithm

New module: `crates/astra-nova/src/project.rs`.

```rust
pub struct ProjectOptions {
    pub root: PathBuf,
    pub entry: PathBuf,
    pub mode: Mode,
    pub allow_root_escape: bool,
}

pub struct Project {
    pub root: PathBuf,
    pub entry: ModuleId,
    pub modules: BTreeMap<ModuleId, CompiledModule>,
}

pub struct CompiledModule {
    pub id: ModuleId,
    pub path: PathBuf,
    pub text: Rc<str>,
    pub ast: ast::Module,
    pub resolved: hir::Module,
    pub checked: CheckResult,
}

pub fn load_project(options: ProjectOptions) -> Result<Project, ProjectError>;
pub fn build_project_interpreter(
    project: Project,
    options: NovaOptions,
    host: Box<dyn Host>,
) -> Result<Interpreter, ProjectError>;
pub fn run_project(path: impl AsRef<Path>, options: NovaOptions) -> Result<Value, ProjectError>;
```

Algorithm:

1. Canonicalize root and entry. Reject an entry outside the root unless
   `allow_root_escape`.
2. Parse the entry with `astra_core::parser::parse`.
3. Walk its top-level imports and `export ... from` declarations. A small public
   `astra-core` specifier collector can avoid duplicating AST traversal.
4. Resolve each raw specifier to one of:
   - `Local(ModuleId)` for relative `.ns` paths and dotted project paths.
   - `External` for `std:` and bare package specifiers.
5. Queue local targets, parse each exactly once, and record every
   `(importer, raw specifier) -> target` edge.
6. Detect cycles during DFS before resolution. Reject direct and transitive
   cycles with `NS3013` and the importing span.
7. Validate root containment, `.ns` extension, duplicate canonical ids, and
   case-insensitive collisions.
8. Build one `ModuleGraph`, resolve every module with `ModuleResolver::from_graph`,
   and type-check each AST/HIR pair with `check_resolved`. Keep the original
   source text in `CompiledModule` so text and JSON diagnostics can compute
   one-based line and Unicode-scalar column positions without re-reading disk.
9. Preserve deterministic order by sorting modules by `ModuleId` for checking
   and diagnostics.

Path rules:

- `"./thing.ns"` and `"../shared.ns"` resolve from the importing file's
  directory.
- Dotted project paths resolve under the project root: `app.util` ->
  `app/util.ns`.
- `std:` and bare specifiers are `External`. V1 fails at load time with `NV0003`
  and a clear hint. NoPM and the standard library will supply these later.
- Absolute filesystem specifiers are rejected.
- A `path:`-style package specifier is not a module specifier; it belongs to
  NoPM.

### 3.4 Test-only source map

Add a constructor that bypasses the filesystem:

```rust
pub fn project_from_sources(
    entry: ModuleId,
    sources: BTreeMap<ModuleId, String>,
    options: NovaOptions,
) -> Result<Project, ProjectError>;
```

This makes runtime tests deterministic and Windows-independent. The filesystem
loader should build the same `Project` shape from disk.

---

## 4. Cross-module type checking

Project linking must not silently weaken the type contract.

Current per-module checking treats every imported symbol as `Unknown`. A project
build therefore accepts calls and property access that should be rejected. This
is the main semantic gap.

Recommended follow-up, `TYPE-003`:

- Build a `ProjectExports` map from each module's resolved HIR.
- Add a checker option such as:

  ```rust
  pub struct ProjectCheckOptions<'a> {
      pub mode: Mode,
      pub current_module: &'a str,
      pub exports: &'a ProjectExports,
  }
  ```

- In `register_imports`, resolve each imported value/type through the target
  module's export surface and use its resolved declaration type instead of
  `Type::Unknown`.
- Check modules in dependency order. Import cycles are already rejected, so a
  topological order is available.
- Keep `check_resolved` as the single-module API; add
  `check_project_module` rather than overloading it.

Until `TYPE-003` lands, Nova project linking can run, but it must be described as
"runtime-linked with opaque imports", not as fully type-checked multi-module
execution. The integration owner should not mark multi-module support complete
before `TYPE-003`.

---

## 5. Runtime model

### 5.1 One interpreter, many modules

Keep one `Interpreter` for the whole program. Replace single-module maps with
per-module tables and a current-module pointer:

```rust
pub(crate) struct RuntimeModule {
    id: ModuleId,
    path: PathBuf,
    globals: EnvRef,
    imports: ImportTable,
    functions: BTreeMap<String, Rc<ast::FunctionDecl>>,
    constants: BTreeMap<String, Rc<ast::ConstantDecl>>,
    constant_values: BTreeMap<String, Value>,
    constants_evaluating: BTreeSet<String>,
    structs: BTreeMap<String, Rc<ast::StructDecl>>,
    enums: BTreeMap<String, Rc<ast::EnumDecl>>,
    foreign_modules: BTreeSet<String>,
    annotations: HashMap<Span, Type>,
}

pub struct Interpreter {
    modules: BTreeMap<ModuleId, RuntimeModule>,
    current_module: ModuleId,
    entry: ModuleId,
    // host, options, call stack, builtins, program-wide caches
}
```

Why one interpreter:

- Shared host and task scopes.
- One deterministic scheduler.
- One panic stack spanning modules.
- No duplicate builtins or constant caches.
- Imported calls naturally remain inside structured concurrency.

Do not create one interpreter per module with isolated globals. That breaks
message/task ownership and makes cross-module panics and manual-memory preflight
inconsistent.

### 5.2 Function identity

`FunctionValue` must carry its defining module:

```rust
pub struct FunctionValue {
    pub(crate) module: ModuleId,
    pub(crate) declaration: Rc<ast::FunctionDecl>,
}
```

Build and cache one `Rc<FunctionValue>` per `(module, function)` so function
identity remains stable for equality checks.

`invoke_user_function`:

1. Save `current_module`.
2. Set `current_module` to the function's module.
3. Create the call scope as a child of that module's globals.
4. Evaluate the body.
5. Restore `current_module` before returning, including on error.

Annotations, constants, structs, enums, imports, and foreign modules are all
looked up through `current_module`.

### 5.3 Import resolution

Build an `ImportTable` from each module's HIR imports:

```rust
enum ImportTarget {
    Value { module: ModuleId, name: String },
    Default { module: ModuleId },
    Namespace { module: ModuleId },
}
```

`lookup_value` order per module:

1. Lexical scope.
2. Current module globals.
3. Current module constants.
4. `None`.
5. Current module imported values.
6. Builtins.
7. Imported namespaces.
8. Foreign modules.

Imported values must precede builtins so an import named `print` cannot silently
call the builtin. This matches the checker's current value table order.

Runtime imports resolve through the target module's export surface:

- `ExportTarget::Local(name)` -> local item or imported binding.
- `ExportTarget::Reexport { module, name }` -> follow recursively.
- `ExportTarget::Namespace { module }` -> `ModuleNamespace`.
- `ExportTarget::Expression` -> evaluate the module's cached default value; see
  §5.5.

`resolve_export(module, name)` is a runtime helper on the interpreter. It must
never consult source text or re-run name resolution.

### 5.4 Structs, enums, and namespaces

Add module-aware value forms:

```rust
Value::ModuleNamespace(ModuleId),
Value::EnumNamespace { module: ModuleId, enum_name: String },
```

Resolution rules:

- A named import of a tuple struct creates a `Constructor` carrying the defining
  `ModuleId` and struct name.
- `User { ... }` resolves `User` through the current module's local structs and
  imports. Imported named structs use the target module's `StructDecl` for field
  validation and construction.
- `Status.Pending` resolves `Status` as a local or imported enum, then selects the
  variant in the defining module.
- `import * as status from "./status.ns"` yields `ModuleNamespace`. Member access
  returns exported values, an `EnumNamespace` for an exported enum, or another
  namespace for a namespace re-export.
- Constructors and enum values carry the defining module in their identity where
  it affects lookup or equality.
- `struct_init` no longer returns `NV0003` for a local-project imported type; it
  uses the linked declaration. External/FFI types remain unsupported.

### 5.5 Constants and default exports

Constants are lazy and cached per module:

- `constant_values` and `constants_evaluating` move into `RuntimeModule`.
- Imported constants evaluate in their defining module and share its cache.
- A cross-module constant cycle is an internal error after type checking; the
  loader should reject module cycles first.

Default exports require one HIR change:

- Replace `ExportTarget::Expression` with
  `ExportTarget::Expression(Box<ast::Expr>)`, or retain
  `hir::Module.default_expression: Option<ast::Expr>`.
- The resolver already has the expression; storing it keeps Nova from walking AST
  export forms independently.
- Cache the evaluated default per module.
- Anonymous default expressions are then executable and re-exportable.

Until this change lands, support named default functions/constants and reject
anonymous default expressions with `NV0003` rather than silently treating them
as absent.

### 5.6 Initialization order

Neo Astra currently has no top-level executable statements. Module loading is
therefore validation plus lazy constant/function evaluation.

V1 rules:

- Side-effect imports ensure the module is loaded and checked; they run no
  initialization.
- Constants and functions initialize lazily on first use.
- If top-level statements are added later, add a `ModuleState` state machine and
  post-order initialization. Do not pretend it exists now.

Module cycles are rejected before resolution in V1. This keeps initialization
and re-export behavior deterministic while spec §17 proposal 16 is unresolved.

### 5.7 Tasks and defers

- A spawned task keeps the defining module in its captured runtime context.
- Task bodies execute with `current_module` set to the defining module.
- A task remains owned by the lexical scope where `spawn` appears, even when the
  function was called from another module.
- `defer` statements are AST nodes from their defining module and run before
  `current_module` is restored.
- A panic crossing module boundaries records a frame per module call.

---

## 6. Diagnostics and errors

Span-only errors are insufficient across files.

Add project-level diagnostics:

```rust
pub struct ProjectDiagnostic {
    pub module: ModuleId,
    pub path: PathBuf,
    pub diagnostic: Diagnostic,
}

pub enum ProjectError {
    Diagnostics(Vec<ProjectDiagnostic>),
    Runtime {
        module: ModuleId,
        path: PathBuf,
        error: NovaError,
    },
}
```

Rules:

- Loader and checker failures carry the module path and original span.
- `CompiledModule.text` supplies line and Unicode-scalar column calculation.
- Runtime failures carry the active module path.
- `NovaError` remains the single-module runtime error type; `ProjectError` adds
  location.
- `Frame` gains `module: Option<ModuleId>` so panic stacks can cross modules while
  preserving existing single-module construction.
- `manual_memory_scopes` are checked for every module before execution. Report
  the first offending module by deterministic `ModuleId` order.
- Codes:
  - Missing/unresolvable local module: `NS3012`.
  - Module cycle: `NS3013`.
  - Invalid or non-UTF-8 source: `NS9007`.
  - Unreadable path: `NS9008`.
  - External/std package module with no adapter: `NV0003`.
  - Manual memory: `NV0001`.
  - Panic/abort: `NV0005`/`NV0006`, with module-aware stack frames.
- JSON formatting remains a CLI concern. The project API exposes structured
  paths, spans, codes, and stacks.

---

## 7. API and file ownership

`astra-core`:

- `src/resolver.rs` — `ModuleGraph`, import map, canonical keys.
- `src/hir.rs` — default-expression retention (if not stored on `Module`).
- `src/checker.rs` + `TYPE-003` — project export surfaces and imported types.
- `src/ast.rs` or `src/resolver.rs` — specifier collection helper, if needed.

`astra-nova`:

- `src/project.rs` — `ModuleId`, `Project`, loader, `ProjectError`.
- `src/interpreter.rs` — module-aware runtime and import/export linking.
- `src/value.rs` — module-aware `FunctionValue`, namespace values.
- `src/error.rs` — module-aware `Frame` and project error wrapper.
- `src/lib.rs` — exports.
- `src/bin/nova/main.rs` — later CLI integration via `run_project`.
- `tests/project.rs` and `tests/fixtures/project/**` — linked-project tests.

Do not put filesystem discovery, manifest parsing, or CLI output in
`astra-core`.

---

## 8. Implementation sequence

### Stage 0 — `TYPE-003` project type surfaces

- Export function/constant/struct/enum signatures from each checked module.
- Resolve imports against target export surfaces.
- Check modules in dependency order.
- Keep imported types opaque only as a fallback for external modules.

Exit criterion: a project call with a wrong imported argument type is rejected
with `NS4001` and the correct source span.

### Stage 1 — Module-aware interpreter refactor

- Introduce `ModuleId`, `RuntimeModule`, and `current_module`.
- Move function/constant/struct/enum/annotation tables into `RuntimeModule`.
- Add module to `FunctionValue` and `Frame`.
- Keep `Interpreter::new` as a one-module adapter.
- No imported calls yet.

Exit criterion: all existing `astra-nova` tests pass without expectation changes.

### Stage 2 — Loader and resolver graph

- Add `ModuleGraph`/import map to `astra-core`.
- Add `project.rs` and source-map constructor.
- Implement path normalization, root containment, cycle detection, and
  deterministic ordering.
- Add `ProjectError` and path-aware diagnostics.

Exit criterion: loader tests cover nested relative imports, dotted paths, root
escape, missing files, duplicate ids, and cycles; each module is parsed once.

### Stage 3 — Functions and constants

- Resolve imported functions and constants through export surfaces.
- Evaluate imported constants in their defining module.
- Carry module context through calls, tasks, and defers.
- Add module-aware panic frames.

Exit criterion: functions with identical names in different modules do not
collide; imported and transitive calls work.

### Stage 4 — Types, namespaces, and re-exports

- Link struct constructors, named struct initialization, and enums.
- Add module/enum namespace values.
- Follow re-export chains and namespace exports.
- Add default expression storage/evaluation.

Exit criterion: imported structs, enums, namespace imports, and re-exported
values run correctly.

### Stage 5 — CLI integration

- `nova run` calls `run_project` rather than `run_source`.
- `nova eval` remains a single in-memory module.
- Diagnostics use module paths in text and JSON.
- `nova watch` can reuse the loader for change detection.
- `nova test` can load each test module through the same project path.

Exit criterion: a multi-file example runs through `nova run` with correct
diagnostics and panic stacks.

---

## 9. Test matrix

Loader:

- entry file imports one sibling
- nested relative imports from different directories
- identical raw specifier in two directories resolves to different modules
- dotted project path
- `../` root escape rejected
- absolute path rejected
- missing module reports `NS3012` at the import
- invalid UTF-8 reports `NS9007`
- unreadable path reports `NS9008`
- duplicate canonical module id rejected
- case-insensitive duplicate rejected on Windows
- direct and transitive cycle reports `NS3013`
- each file parsed once
- external `std:`/bare import fails with `NV0003`

Type checking:

- imported function argument mismatch rejected (`TYPE-003`)
- imported return type mismatch rejected
- imported struct field type checked
- private imported declaration rejected
- transitive re-export type preserved
- type-only import erased from runtime linkage

Runtime functions and constants:

- imported function call
- transitive imported call
- same function name in two modules
- module-local recursion
- imported constant evaluated once
- imported constant used from two modules
- constant referencing another module's constant
- side-effect import has no fake initialization
- panic across two modules has two module-aware frames
- deferred cleanup runs in the defining module
- spawned task in imported function remains scope-owned by that function
- manual memory in an imported module rejected at preflight

Runtime types and namespaces:

- imported tuple struct constructor
- imported named struct initializer
- imported enum unit variant
- imported enum payload variant and match
- `import * as` function call
- `import * as` enum variant access
- namespace re-export
- named re-export
- `export *` re-export
- default named function import
- default expression import after HIR support
- imported type used in `struct_init` resolves the correct declaration

Determinism:

- module diagnostic order independent of filesystem iteration
- stable function/value identity across repeated imports
- no second parse: instrument a test-only parse counter or assert the loader sees
  one source per module

---

## 10. Rejected alternatives

- **Parse modules inside Nova.** Violates the single-parser rule and duplicates
  spans, diagnostics, and syntax evolution.
- **Flatten all module declarations into one global map.** Causes collisions,
  crosses private boundaries, and cannot represent namespace imports or
  re-exports correctly.
- **One interpreter per module.** Breaks shared task scopes, host output order,
  constant caches, and multi-module panic stacks.
- **Use raw specifiers as module keys.** `"../shared.ns"` means different files
  from different importers.
- **Use module headers as canonical keys.** Duplicate headers and missing
  headers make identity ambiguous; the spec resolves by path.
- **Resolve imports from the process working directory.** Non-deterministic and
  breaks nested modules.
- **Load modules lazily on first call.** Delays unresolved-import errors, makes
  execution order path-dependent, and complicates watch mode.
- **Keep `Value::Imported` as the only imported form.** It cannot distinguish a
  function, enum namespace, struct constructor, or default export.
- **Use `Span` alone as an annotation key.** Byte offsets collide across files.
- **Implement module linking before `TYPE-003` and call it complete.** The runtime
  would work while cross-module type errors remain undetected.

---

## 11. Risks and open decisions

1. **`TYPE-003` scope.** Imported types are opaque today. This is the largest
   blocker to calling multi-module Nova type-safe.
2. **Import-cycle policy.** V1 rejects all cycles. Spec §17 proposal 16 must be
   resolved before allowing side-effect or re-export cycles.
3. **Default expressions.** HIR currently discards them. Named defaults work;
   anonymous default values need the small HIR extension in §5.5.
4. **External modules.** `std:` and bare package imports remain unsupported until
   the standard library and NoPM are available. They must fail clearly, not be
   silently ignored.
5. **Windows paths.** Canonical ids must be normalized and case collisions
   checked explicitly.
6. **Symlinks.** Canonicalization can produce a path outside the project even
   when the source path looked contained. Apply containment after
   canonicalization.
7. **Performance.** Switching module context per call is cheap, but avoid cloning
   module tables per call. Cache function values and clone `Rc` handles only.
8. **Span collisions.** Every annotation, manual-memory span, and diagnostic must
   be keyed by module plus span.
9. **Top-level initialization.** The current language has no top-level executable
   statements. Do not build a speculative initializer now, but leave
   `ModuleState` as a named future extension.
10. **Public API breakage.** Adding `module` to `Frame` and `FunctionValue` changes
   public surface pre-1.0. Keep additions optional where possible.

---

## 12. Acceptance check

- [x] No second parser: every source goes through `astra_core::parser`.
- [x] Import resolution is deterministic and path-based.
- [x] Runtime representation for imported functions, constants, structs, enums,
      namespaces, re-exports, and defaults is specified.
- [x] Module initialization and cycle behavior are specified.
- [x] Structured diagnostics include module paths and spans.
- [x] `TYPE-003` is identified as the cross-module type-safety prerequisite.
- [x] File ownership and an incremental implementation sequence are defined.
- [x] Test matrix covers nested imports, cycles, collisions, constructors,
      namespaces, panics, tasks, and manual memory.
