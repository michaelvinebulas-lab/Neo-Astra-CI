//! Target-independent intermediate representation boundary.

use serde::{Deserialize, Serialize};

/// A lowered program ready for a backend.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Program {
    pub name: String,
}
