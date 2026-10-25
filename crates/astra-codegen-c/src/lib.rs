//! C backend boundary for native and Wasm targets.
//!
//! The C17 backend lands in `C-001`; this crate only fixes the boundary.

use astra_core::ir::Program;

/// Backend identifier used by the manifest and diagnostics.
pub const TARGET: &str = "c";

/// A backend failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendError {
    pub message: String,
}

impl BackendError {
    /// Creates the scaffold's not-implemented error.
    #[must_use]
    pub fn not_implemented() -> Self {
        Self {
            message: "The C backend is not implemented in the scaffold.".to_owned(),
        }
    }
}

/// Emits C source for a lowered program.
pub fn generate(_program: &Program) -> Result<String, BackendError> {
    Err(BackendError::not_implemented())
}
