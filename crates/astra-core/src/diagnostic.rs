//! Structured compiler diagnostics.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::span::Span;

/// Diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Note,
    Help,
}

impl Severity {
    /// Lowercase name used by the JSON protocol.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Note => "note",
            Self::Help => "help",
        }
    }
}

/// A stable, structured compiler diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub span: Option<Span>,
    pub hint: Option<String>,
}

impl Diagnostic {
    /// Creates an error diagnostic with a stable code.
    #[must_use]
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: Severity::Error,
            message: message.into(),
            span: None,
            hint: None,
        }
    }

    /// Attaches a source span.
    #[must_use]
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    /// Attaches a fix hint.
    #[must_use]
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for Diagnostic {}

#[cfg(test)]
mod tests {
    use super::{Diagnostic, Severity};
    use crate::span::Span;

    #[test]
    fn diagnostics_carry_code_and_span() {
        let diagnostic = Diagnostic::error("NS0001", "invalid token").with_span(Span::new(1, 3));

        assert_eq!(diagnostic.code, "NS0001");
        assert_eq!(diagnostic.severity, Severity::Error);
        assert_eq!(diagnostic.span, Some(Span::new(1, 3)));
        assert_eq!(diagnostic.to_string(), "NS0001: invalid token");
    }
}
