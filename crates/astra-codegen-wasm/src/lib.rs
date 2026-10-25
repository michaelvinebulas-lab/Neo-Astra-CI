//! Direct Wasm backend boundary.
//!
//! The direct backend lands after V1; the C-to-Wasm path is the V1 route.

use astra_core::ir::Program;

/// Backend identifier used by the manifest and diagnostics.
pub const TARGET: &str = "wasm32";

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
            message: "The direct Wasm backend is not implemented in the scaffold.".to_owned(),
        }
    }
}

/// Emits Wasm for a lowered program.
pub fn generate(_program: &Program) -> Result<Vec<u8>, BackendError> {
    Err(BackendError::not_implemented())
}
