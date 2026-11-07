//! Text and LSP position helpers shared by the server and editor features.

use astra_core::span::Span;

/// Converts a byte offset into an LSP `(line, character)` position.
///
/// LSP character offsets are UTF-16 code units, not bytes or Unicode scalar
/// values.
#[must_use]
pub fn offset_to_position(text: &str, offset: usize) -> (u32, u32) {
    let offset = offset.min(text.len());
    let mut offset = offset;
    while offset > 0 && !text.is_char_boundary(offset) {
        offset -= 1;
    }

    let mut line = 0u32;
    let mut character = 0u32;
    let mut index = 0usize;
    for ch in text.chars() {
        if index >= offset {
            break;
        }
        let len = ch.len_utf8();
        if index + len > offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            character = 0;
        } else if ch != '\r' {
            character += ch.len_utf16() as u32;
        }
        index += len;
    }
    (line, character)
}

/// Converts an LSP `(line, character)` position into a byte offset.
///
/// The result is clamped to the line and document boundaries, matching the
/// tolerant behaviour expected from language servers.
#[must_use]
pub fn position_to_offset(text: &str, line: u32, character: u32) -> usize {
    let mut current_line = 0u32;
    let mut index = 0usize;
    while current_line < line && index < text.len() {
        let ch = text[index..].chars().next().expect("in-bounds char");
        index += ch.len_utf8();
        if ch == '\n' {
            current_line += 1;
        }
    }
    if current_line < line {
        return text.len();
    }

    let mut units = 0u32;
    while index < text.len() {
        let ch = text[index..].chars().next().expect("in-bounds char");
        if ch == '\n' || ch == '\r' {
            break;
        }
        let width = ch.len_utf16() as u32;
        if units + width > character {
            break;
        }
        units += width;
        index += ch.len_utf8();
    }
    index
}

/// A word found in source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Word {
    pub text: String,
    pub start: usize,
    pub end: usize,
}

impl Word {
    #[must_use]
    pub fn span(&self) -> Span {
        Span::new(self.start, self.end)
    }
}

#[must_use]
pub fn is_identifier_char(ch: char) -> bool {
    ch == '_' || ch.is_alphanumeric()
}

/// Returns the identifier surrounding a byte offset.
///
/// A position at the end of an identifier still belongs to that identifier;
/// this makes hover work when the editor cursor is just after a name.
#[must_use]
pub fn identifier_at(text: &str, offset: usize) -> Option<Word> {
    let offset = offset.min(text.len());
    let mut start = offset;
    while start > 0 {
        let previous = text[..start].chars().next_back()?;
        if !is_identifier_char(previous) {
            break;
        }
        start -= previous.len_utf8();
    }

    let mut end = offset;
    while end < text.len() {
        let next = text[end..].chars().next()?;
        if !is_identifier_char(next) {
            break;
        }
        end += next.len_utf8();
    }

    if start == end {
        None
    } else {
        Some(Word {
            text: text[start..end].to_owned(),
            start,
            end,
        })
    }
}

/// Finds the identifier immediately before `offset`, allowing whitespace
/// between it and a preceding dot.
#[must_use]
pub fn identifier_before_dot(text: &str, offset: usize) -> Option<Word> {
    let mut before = text[..offset.min(text.len())].trim_end();
    if !before.ends_with('.') {
        return None;
    }
    before = before[..before.len() - 1].trim_end();
    identifier_at(before, before.len())
}

/// Locates `name` inside a declaration span.
///
/// Declaration spans cover the whole declaration (for example `fn foo(...)`),
/// while LSP navigation should land on the name itself. If the name cannot be
/// found the original span is returned.
#[must_use]
pub fn name_span(text: &str, span: Span, name: &str) -> Span {
    let start = span.start.min(text.len());
    let end = span.end.min(text.len());
    if start >= end || name.is_empty() {
        return span;
    }

    let slice = &text[start..end];
    for (relative, _) in slice.match_indices(name) {
        let name_start = start + relative;
        let name_end = name_start + name.len();
        let before_ok = text[..name_start]
            .chars()
            .next_back()
            .is_none_or(|ch| !is_identifier_char(ch));
        let after_ok = text[name_end..]
            .chars()
            .next()
            .is_none_or(|ch| !is_identifier_char(ch));
        if before_ok && after_ok {
            return Span::new(name_start, name_end);
        }
    }
    span
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_and_offset_roundtrip_utf16() {
        let text = "a😀b\ncd";
        for offset in [0, 1, 5, 6, 7, 8, 9] {
            let (line, character) = offset_to_position(text, offset);
            let roundtrip = position_to_offset(text, line, character);
            assert_eq!(roundtrip, offset);
        }
    }

    #[test]
    fn position_to_offset_clamps_past_line_and_document() {
        let text = "ab\ncd";
        assert_eq!(position_to_offset(text, 0, 99), 2);
        assert_eq!(position_to_offset(text, 1, 1), 4);
        assert_eq!(position_to_offset(text, 9, 0), text.len());
    }

    #[test]
    fn identifier_at_accepts_cursor_after_word() {
        let word = identifier_at("let value = 1", 9).expect("word");
        assert_eq!(word.text, "value");
        assert_eq!((word.start, word.end), (4, 9));
    }

    #[test]
    fn identifier_before_dot_skips_whitespace() {
        let word = identifier_before_dot("util . add", "util . ".len()).expect("word");
        assert_eq!(word.text, "util");
    }

    #[test]
    fn name_span_finds_the_declaration_name() {
        let span = Span::new(0, 20);
        assert_eq!(
            name_span("pub fn value() {}", span, "value"),
            Span::new(7, 12)
        );
    }
}
