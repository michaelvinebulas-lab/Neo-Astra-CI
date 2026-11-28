use std::fmt;

/// A NoPM diagnostic carrying the same code, message, and path fields as the
/// TypeScript oracle. `path` is the optional diagnostic input value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NopmError {
    pub code: String,
    pub message: String,
    pub path: Option<String>,
}

impl NopmError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            path: None,
        }
    }

    pub fn with_path(
        code: impl Into<String>,
        message: impl Into<String>,
        path: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            path: Some(path.into()),
        }
    }

    pub fn with_optional_path(
        code: impl Into<String>,
        message: impl Into<String>,
        path: Option<String>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            path,
        }
    }
}

impl fmt::Display for NopmError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for NopmError {}
