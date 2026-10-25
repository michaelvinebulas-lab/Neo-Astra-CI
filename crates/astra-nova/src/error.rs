//! Structured Nova runtime failures.
//!
//! Frontend failures keep the compiler's `Diagnostic` values unchanged. Runtime
//! failures have their own representation because they describe execution rather
//! than a source contract violation; `NOVA-002` maps both into CLI output.

use std::fmt;

use astra_core::diagnostic::Diagnostic;
use astra_core::span::Span;

use crate::value::Value;

/// One active function in a panic stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub function: String,
    pub span: Span,
    /// Canonical module containing the frame, when the execution is linked.
    pub module: Option<String>,
}

impl Frame {
    pub(crate) fn new(function: impl Into<String>, span: Span) -> Self {
        Self {
            function: function.into(),
            span,
            module: None,
        }
    }

    pub(crate) fn in_module(
        module: impl Into<String>,
        function: impl Into<String>,
        span: Span,
    ) -> Self {
        Self {
            function: function.into(),
            span,
            module: Some(module.into()),
        }
    }
}

/// A programmer-bug failure raised by `panic`, bounds checks, or assertions.
#[derive(Debug, Clone, PartialEq)]
pub struct Panic {
    pub message: String,
    pub span: Option<Span>,
    /// Innermost frame first.
    pub stack: Vec<Frame>,
}

impl Panic {
    pub(crate) fn new(message: impl Into<String>, span: Option<Span>, stack: Vec<Frame>) -> Self {
        Self {
            message: message.into(),
            span,
            stack,
        }
    }
}

/// An immediate termination raised by `abort`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Abort {
    pub message: String,
    pub span: Option<Span>,
}

/// A Nova failure.
#[derive(Debug, Clone, PartialEq)]
pub enum NovaError {
    /// Parse, resolve, or type-check diagnostics.
    Diagnostics(Vec<Diagnostic>),
    /// The program uses manual memory, which the first Nova engine does not run.
    ManualMemoryUnsupported { span: Span },
    /// The `main` entry point is missing.
    MissingEntry { name: String },
    /// The program reached a feature that has no Nova implementation yet.
    Unsupported { message: String, span: Option<Span> },
    /// A fallible `main` returned an unhandled error value.
    UnhandledFailure(Value),
    /// A programmer-bug failure.
    Panic(Panic),
    /// Immediate termination without unwinding.
    Abort(Abort),
    /// An invariant in the interpreter itself was violated.
    Internal(String),
    /// A foreign script block was encountered or invoked, which requires a production backend target.
    ForeignScriptUnsupported {
        name: String,
        language: String,
        span: Span,
    },
}

impl NovaError {
    /// A stable diagnostic-style code for tooling.
    ///
    /// These codes are Nova-specific; frontend diagnostics retain their
    /// `NSxxxx` codes.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Diagnostics(_) => "NV0000",
            Self::ManualMemoryUnsupported { .. } => "NV0001",
            Self::MissingEntry { .. } => "NV0002",
            Self::Unsupported { .. } => "NV0003",
            Self::UnhandledFailure(_) => "NV0004",
            Self::Panic(_) => "NV0005",
            Self::Abort(_) => "NV0006",
            Self::Internal(_) => "NV0007",
            Self::ForeignScriptUnsupported { .. } => "NV0008",
        }
    }
}

impl fmt::Display for NovaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Diagnostics(diagnostics) => {
                formatter.write_str("the program did not pass the frontend.")?;
                for diagnostic in diagnostics {
                    write!(formatter, "\n{diagnostic}")?;
                }
                Ok(())
            }
            Self::ManualMemoryUnsupported { .. } => formatter
                .write_str("Nova does not support '@memory:manual' scopes in its first engine."),
            Self::MissingEntry { name } => {
                write!(formatter, "Nova entry point '{name}' was not found.")
            }
            Self::Unsupported { message, .. } => formatter.write_str(message),
            Self::UnhandledFailure(value) => {
                write!(formatter, "unhandled error left 'main': {value}")
            }
            Self::Panic(panic) => {
                write!(formatter, "panic: {}", panic.message)?;
                for frame in &panic.stack {
                    match &frame.module {
                        Some(module) => write!(
                            formatter,
                            "\n  at {module}::{} ({}..{})",
                            frame.function, frame.span.start, frame.span.end
                        )?,
                        None => write!(
                            formatter,
                            "\n  at {} ({}..{})",
                            frame.function, frame.span.start, frame.span.end
                        )?,
                    }
                }
                Ok(())
            }
            Self::Abort(abort) => write!(formatter, "abort: {}", abort.message),
            Self::Internal(message) => write!(formatter, "Nova internal error: {message}"),
            Self::ForeignScriptUnsupported { name, language, .. } => write!(
                formatter,
                "foreign script '{name}' ({language}) requires a production target; Nova does not execute foreign scripts."
            ),
        }
    }
}

impl std::error::Error for NovaError {}

/// Internal control-flow fault used while evaluating.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Fault {
    Panic(Panic),
    Abort(Abort),
    Error(NovaError),
}

impl Fault {
    pub(crate) fn into_error(self) -> NovaError {
        match self {
            Self::Panic(panic) => NovaError::Panic(panic),
            Self::Abort(abort) => NovaError::Abort(abort),
            Self::Error(error) => error,
        }
    }
}

impl From<NovaError> for Fault {
    fn from(error: NovaError) -> Self {
        Self::Error(error)
    }
}
