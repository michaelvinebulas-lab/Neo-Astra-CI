//! Byte spans into UTF-8 source text.

use serde::{Deserialize, Serialize};

/// A half-open byte range `[start, end)` in a source file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    /// Creates a span from byte offsets.
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Creates an empty span at `offset`.
    #[must_use]
    pub const fn empty(offset: usize) -> Self {
        Self::new(offset, offset)
    }

    /// Returns the number of bytes in the span.
    #[must_use]
    pub const fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Returns whether the span contains no bytes.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }

    /// Returns whether `offset` lies inside the half-open range.
    #[must_use]
    pub const fn contains(self, offset: usize) -> bool {
        self.start <= offset && offset < self.end
    }

    /// Returns the smallest span covering both spans.
    #[must_use]
    pub const fn join(self, other: Self) -> Self {
        let start = if self.start < other.start {
            self.start
        } else {
            other.start
        };
        let end = if self.end > other.end {
            self.end
        } else {
            other.end
        };
        Self::new(start, end)
    }
}

#[cfg(test)]
mod tests {
    use super::Span;

    #[test]
    fn contains_is_half_open() {
        let span = Span::new(2, 5);
        assert!(span.contains(2));
        assert!(span.contains(4));
        assert!(!span.contains(5));
    }

    #[test]
    fn join_covers_both_spans() {
        assert_eq!(Span::new(2, 4).join(Span::new(8, 9)), Span::new(2, 9));
    }
}
