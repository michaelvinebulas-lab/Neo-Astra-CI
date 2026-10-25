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
//! incorrect C. See `C-001` in `Docs/Agent/todo.md` for the supported scope and
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
/// `@memory:manual` scopes the managed subset rejects, and the integer
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
    /// Spans of `@memory:manual` scopes, which this slice rejects.
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

    /// Creates the scaffold's not-implemented error.
    #[must_use]
    pub fn not_implemented() -> Self {
        Self::new("NS8002", "The C backend does not support this request.")
    }
}

/// Output of a successful C backend run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedC {
    /// The freestanding-safe generated C17 translation unit.
    pub source: String,
    /// The hosted runtime implementation (`astra_host.c`), linked to run.
    pub host_shim: String,
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
