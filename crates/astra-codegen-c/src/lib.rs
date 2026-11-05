//! C17 backend for Neo Astra.
//!
//! Emits a self-contained C17 translation unit from a checked module. The C
//! backend is the V1 bootstrap path for native and Wasm targets (design §6):
//! Astra → C17 → `clang` → native binary or Wasm module.
//!
//! The backend consumes the shared checked frontend ([`CModule`]: AST + HIR +
//! annotations + integer policy), the same typed-IR boundary Nova and the JS
//! backend use. The generated translation unit is freestanding-safe: it
//! includes only `<stdint.h>`, `<stdbool.h>`, and `<stddef.h>` and declares the
//! runtime as `extern`. The matching host shim ([`GeneratedC::host_shim`])
//! defines those externs using libc and is linked only when a runnable
//! executable is needed.
//!
//! The first slice supports the portable managed subset; unsupported features
//! are rejected with stable `NS80xx` diagnostics instead of emitting
//! incorrect C. See `C-001` in `Docs/archive/Agent/todo.md` for the supported scope and
//! boundaries.

mod emit;
pub mod runtime;
mod ty;

/// Backend identifier used by the manifest and diagnostics.
pub const TARGET: &str = "c";

/// Integer arithmetic policy selected by the manifest.
///
/// The compiler API maps its own policy enum onto this type when it builds a
/// [`CModule`], keeping this crate independent of the API crate.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum IntegerPolicy {
    /// Fixed-width operations wrap modulo 2^N (the project default).
    #[default]
    Wrapping,
    /// Overflow traps at runtime.
    SafeChecked,
    /// Arbitrary-precision behavior where the target supports it.
    Arbitrary,
}

/// The checked module payload the C backend consumes.
///
/// Mirrors the typed-IR fields the compiler API produces for source-to-source
/// backends: the parsed AST, resolved HIR, checked expression annotations, the
/// `@memory:manual` scopes (supported for alloc/free), and the integer
/// arithmetic policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CModule {
    /// Canonical module name (dotted) used in the emitted header comment.
    pub name: String,
    /// Parsed source declaration tree.
    pub ast: astra_core::ast::Module,
    /// Resolved HIR with imports, exports, and symbol tables.
    pub hir: astra_core::hir::Module,
    /// Checked expression types, in source order.
    pub annotations: Vec<astra_core::checker::TypeAnnotation>,
    /// Spans of `@memory:manual` scopes. The C backend supports `Memory.alloc`
    /// / `Memory.free` and pointer access (`ptr.read` / `ptr.write`) inside
    /// them; `ptr.as_slice` needs the still-unspecced `MutSlice` surface.
    pub manual_memory_scopes: Vec<astra_core::span::Span>,
    /// Selected integer arithmetic policy.
    pub integer_policy: IntegerPolicy,
}

/// A backend failure with a stable diagnostic code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendError {
    /// Stable `NS80xx` (or shared `NS8xxx`) diagnostic code.
    pub code: String,
    /// Human-readable explanation.
    pub message: String,
}

impl BackendError {
    /// Creates a backend error.
    #[must_use]
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    /// Creates an unsupported-request error.
    #[must_use]
    pub fn unsupported() -> Self {
        Self::new("NS8001", "The C backend does not support this request.")
    }
}

/// Output of a successful C backend run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedC {
    /// The freestanding-safe generated C17 translation unit.
    pub source: String,
    /// Public C header (`<module>.h`) with extern prototypes for exported
    /// symbols. `None` for standalone (single-module) builds.
    pub header: Option<String>,
    /// The hosted runtime implementation (`astra_host.c`), linked to run.
    pub host_shim: String,
    /// Individual translation units generated from inline `@script` blocks.
    pub script_sources: Vec<(String, String)>,
    /// The backend target identifier (`"c"`).
    pub target: String,
}

/// Emits C17 source for a checked module.
///
/// Returns a [`BackendError`] (carrying an `NS80xx` code) for any feature the
/// first slice does not support, so unsupported programs fail before code
/// generation instead of producing incorrect C.
pub fn generate(module: &CModule) -> Result<GeneratedC, BackendError> {
    emit::emit(module)
}

/// One entry in a project compile graph.
///
/// `dep_keys` lists the canonical module keys (as they appear in `import`
/// statements) that this module directly imports. The caller is responsible for
/// ordering `ProjectEntry` slices so each dependency appears before any module
/// that imports it.
#[derive(Debug, Clone)]
pub struct ProjectEntry {
    /// Compiled module payload (AST + HIR + annotations).
    pub module: CModule,
    /// Canonical key by which other modules import this module.
    pub key: String,
    /// Keys of modules that this module directly imports.
    pub dep_keys: Vec<String>,
}

/// What kind of artifact a project emission targets.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum EmitKind {
    /// A runnable program: the root module emits `int main(void)`.
    #[default]
    Executable,
    /// A library: no `int main(void)`; `pub` functions stay external so a C
    /// consumer can link them. Header artifacts are unchanged.
    Library,
}

/// Emits C17 source for a multi-module project.
///
/// Returns one [`GeneratedC`] per entry (with a `header` containing extern
/// prototypes for the module's exported functions) plus a shared host shim.
/// Each `GeneratedC::source` uses `#include "<dep>.h"` instead of inlining
/// the dependency, so the returned translation units must be compiled and
/// linked together.
pub fn generate_project(entries: &[ProjectEntry]) -> Result<Vec<GeneratedC>, BackendError> {
    emit::emit_project(entries, true)
}

/// Emits a multi-module project for the requested artifact kind.
///
/// [`EmitKind::Library`] suppresses the `int main(void)` wrapper so the output
/// can be archived or linked into a shared library. See `Docs/archive/Design/V1/11-c-abi.md`.
pub fn generate_project_with_kind(
    entries: &[ProjectEntry],
    kind: EmitKind,
) -> Result<Vec<GeneratedC>, BackendError> {
    emit::emit_project(entries, kind == EmitKind::Executable)
}
