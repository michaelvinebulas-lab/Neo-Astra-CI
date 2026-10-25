//! Host output adapters.
//!
//! Nova keeps console access behind a small host boundary so tests, the CLI, and
//! later embedders can capture or redirect output without changing evaluation.

/// A host that receives built-in output.
pub trait Host {
    /// Writes to standard output without adding a newline.
    fn stdout(&mut self, text: &str);
    /// Writes to standard error without adding a newline.
    fn stderr(&mut self, text: &str);
}

/// The default host, backed by the process standard streams.
#[derive(Debug, Default, Clone, Copy)]
pub struct StdHost;

impl Host for StdHost {
    fn stdout(&mut self, text: &str) {
        use std::io::Write;
        let mut stdout = std::io::stdout().lock();
        let _ = stdout.write_all(text.as_bytes());
        let _ = stdout.flush();
    }

    fn stderr(&mut self, text: &str) {
        use std::io::Write;
        let mut stderr = std::io::stderr().lock();
        let _ = stderr.write_all(text.as_bytes());
        let _ = stderr.flush();
    }
}

/// An in-memory host used by tests and embedders.
#[derive(Debug, Default, Clone)]
pub struct BufferHost {
    pub stdout: String,
    pub stderr: String,
}

impl BufferHost {
    /// Creates an empty buffer host.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl Host for BufferHost {
    fn stdout(&mut self, text: &str) {
        self.stdout.push_str(text);
    }

    fn stderr(&mut self, text: &str) {
        self.stderr.push_str(text);
    }
}
