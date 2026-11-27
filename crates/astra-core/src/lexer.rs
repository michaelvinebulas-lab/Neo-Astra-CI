//! Lexer and source spans.
//!
//! The lexer follows the v0.1 lexical contract: UTF-8 source, byte spans, `//`
//! comments only, Unicode identifiers, typed keywords, and explicit diagnostics
//! for invalid comments, escapes, and unterminated literals.

use unicode_ident::{is_xid_continue, is_xid_start};

use crate::diagnostic::Diagnostic;
use crate::edition::Edition;
use crate::span::Span;

/// Canonical reserved words. Keyword aliases map to these variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Keyword {
    Abort,
    As,
    Break,
    Const,
    Continue,
    Default,
    Defer,
    Else,
    Enum,
    Export,
    Fail,
    False,
    Fn,
    For,
    Foreign,
    From,
    If,
    Import,
    In,
    Let,
    Match,
    Module,
    Mut,
    Not,
    Panic,
    Private,
    Pub,
    Return,
    Spawn,
    Struct,
    True,
    Type,
    While,
}

impl Keyword {
    /// Canonical spelling used by the formatter.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Abort => "abort",
            Self::As => "as",
            Self::Break => "break",
            Self::Const => "const",
            Self::Continue => "continue",
            Self::Default => "default",
            Self::Defer => "defer",
            Self::Else => "else",
            Self::Enum => "enum",
            Self::Export => "export",
            Self::Fail => "fail",
            Self::False => "false",
            Self::Fn => "fn",
            Self::For => "for",
            Self::Foreign => "foreign",
            Self::From => "from",
            Self::If => "if",
            Self::Import => "import",
            Self::In => "in",
            Self::Let => "let",
            Self::Match => "match",
            Self::Module => "module",
            Self::Mut => "mut",
            Self::Not => "not",
            Self::Panic => "panic",
            Self::Private => "private",
            Self::Pub => "pub",
            Self::Return => "return",
            Self::Spawn => "spawn",
            Self::Struct => "struct",
            Self::True => "true",
            Self::Type => "type",
            Self::While => "while",
        }
    }
}

/// Punctuation and operator tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Punctuation {
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    PlusWrap,
    MinusWrap,
    StarWrap,
    SlashWrap,
    EqEq,
    BangEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    AmpAmp,
    Amp,
    PipePipe,
    Bang,
    Eq,
    Dot,
    DotDot,
    DotDotEq,
    Comma,
    Colon,
    DoubleColon,
    Semicolon,
    Arrow,
    FatArrow,
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    At,
    Pipe,
    Caret,
    Question,
    Underscore,
}

/// A lexical token category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    Identifier,
    Integer,
    Float,
    String,
    ByteString,
    RawSource,
    Keyword(Keyword),
    Punct(Punctuation),
    Newline,
    Eof,
}

/// A lexed token. `text` is the exact source slice for the token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
    pub span: Span,
}

/// A valid `//` line comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    pub text: String,
    pub span: Span,
}

/// The result of lexing one source unit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LexResult {
    pub tokens: Vec<Token>,
    pub comments: Vec<Comment>,
    pub diagnostics: Vec<Diagnostic>,
}

impl LexResult {
    /// Returns whether lexing produced at least one diagnostic.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        !self.diagnostics.is_empty()
    }
}

/// Lexer options. Editions are threaded through now; syntax behavior changes
/// land in the next phase.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LexOptions {
    pub edition: Edition,
}

/// Lexes one UTF-8 source unit with the default options.
#[must_use]
pub fn lex(source: &str) -> LexResult {
    lex_with_options(source, LexOptions::default())
}

/// Lexes one UTF-8 source unit with explicit options.
#[must_use]
pub fn lex_with_options(source: &str, options: LexOptions) -> LexResult {
    Lexer::new(source, options).run()
}

enum EscapeOutcome {
    Continue,
    Unterminated,
}

struct Lexer<'a> {
    source: &'a str,
    offset: usize,
    tokens: Vec<Token>,
    comments: Vec<Comment>,
    diagnostics: Vec<Diagnostic>,
    /// Selected language edition; consumed by the v0.2 lexer rules in Phase 2.
    #[allow(dead_code)]
    edition: Edition,
    /// Brace depth inside a `{@script:…}` header.
    /// 0 = not in a script header. Set to 1 when `{@script:` is entered;
    /// incremented for each inner `{`, decremented for each `}`. When it
    /// reaches 0 again the outer script brace has been closed.
    script_header_depth: u32,
    script_start: usize,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str, options: LexOptions) -> Self {
        Self {
            source,
            offset: 0,
            tokens: Vec::new(),
            comments: Vec::new(),
            diagnostics: Vec::new(),
            edition: options.edition,
            script_header_depth: 0,
            script_start: 0,
        }
    }

    fn run(mut self) -> LexResult {
        if self.source.starts_with('\u{feff}') {
            self.offset = '\u{feff}'.len_utf8();
        }

        while let Some(character) = self.peek() {
            let start = self.offset;

            match character {
                ' ' | '\t' | '\r' => {
                    self.bump();
                }
                '\n' => {
                    self.bump();
                    self.push(TokenKind::Newline, start);
                }
                '/' if self.starts_with("//") => self.line_comment(start),
                '/' if self.starts_with("/*") => self.invalid_block_comment(start),
                '#' => self.invalid_line_comment(start),
                '-' if self.starts_with("--") => self.invalid_line_comment(start),
                '"' => self.string(start, false),
                'b' if self.starts_with("b\"") => self.string(start, true),
                '0'..='9' => self.number(start),
                character if is_identifier_start(character) => self.identifier(start),
                _ => self.operator_or_error(start),
            }
        }

        self.push(TokenKind::Eof, self.offset);
        LexResult {
            tokens: self.tokens,
            comments: self.comments,
            diagnostics: self.diagnostics,
        }
    }

    fn peek(&self) -> Option<char> {
        self.source.get(self.offset..)?.chars().next()
    }

    fn peek_n(&self, index: usize) -> Option<char> {
        self.source.get(self.offset..)?.chars().nth(index)
    }

    fn starts_with(&self, text: &str) -> bool {
        self.source
            .get(self.offset..)
            .is_some_and(|rest| rest.starts_with(text))
    }

    fn bump(&mut self) -> Option<char> {
        let character = self.peek()?;
        self.offset += character.len_utf8();
        Some(character)
    }

    fn push(&mut self, kind: TokenKind, start: usize) {
        let text = self
            .source
            .get(start..self.offset)
            .unwrap_or_default()
            .to_owned();
        self.tokens.push(Token {
            kind,
            text,
            span: Span::new(start, self.offset),
        });
    }

    fn diagnostic(
        &mut self,
        code: &'static str,
        message: impl Into<String>,
        hint: impl Into<String>,
        start: usize,
        end: usize,
    ) {
        self.diagnostics.push(
            Diagnostic::error(code, message)
                .with_span(Span::new(start, end))
                .with_hint(hint),
        );
    }

    fn line_comment(&mut self, start: usize) {
        self.skip_to_newline();

        let mut end = self.offset;
        while end > start
            && matches!(
                self.source.as_bytes().get(end - 1),
                Some(b' ' | b'\t' | b'\r')
            )
        {
            end -= 1;
        }

        let text = self.source.get(start..end).unwrap_or_default().to_owned();
        self.comments.push(Comment {
            text,
            span: Span::new(start, end),
        });
    }

    fn skip_to_newline(&mut self) {
        while let Some(character) = self.peek() {
            if character == '\n' {
                break;
            }
            self.bump();
        }
    }

    fn invalid_line_comment(&mut self, start: usize) {
        self.skip_to_newline();

        self.diagnostic(
            "NS1001",
            "Invalid comment form.",
            "Only // line comments are valid in .ns source.",
            start,
            self.offset,
        );
    }

    fn invalid_block_comment(&mut self, start: usize) {
        self.bump();
        self.bump();

        while let Some(character) = self.peek() {
            if character == '*' && self.starts_with("*/") {
                self.bump();
                self.bump();
                break;
            }
            self.bump();
        }

        self.diagnostic(
            "NS1001",
            "Invalid block comment.",
            "Only // line comments are valid in .ns source.",
            start,
            self.offset,
        );
    }

    fn string(&mut self, start: usize, is_byte: bool) {
        if is_byte {
            self.bump();
        }
        self.bump();

        while let Some(character) = self.peek() {
            match character {
                '"' => {
                    self.bump();
                    let kind = if is_byte {
                        TokenKind::ByteString
                    } else {
                        TokenKind::String
                    };
                    self.push(kind, start);
                    return;
                }
                '\n' | '\r' => {
                    self.diagnostic(
                        "NS1003",
                        "Unterminated string literal.",
                        "Close the string before the end of the line.",
                        start,
                        self.offset,
                    );
                    self.skip_to_newline();
                    return;
                }
                '\\' => {
                    if matches!(self.escape(is_byte), EscapeOutcome::Unterminated) {
                        self.diagnostic(
                            "NS1003",
                            "Unterminated string literal.",
                            "Close the string before the end of the line.",
                            start,
                            self.offset,
                        );
                        self.skip_to_newline();
                        return;
                    }
                }
                character if is_byte && (character as u32) > 0xFF => {
                    let character_start = self.offset;
                    self.bump();
                    self.diagnostic(
                        "NS1002",
                        "Non-byte character in byte literal.",
                        "Use only values in the u8 range or a valid escape.",
                        character_start,
                        self.offset,
                    );
                }
                _ => {
                    self.bump();
                }
            }
        }

        self.diagnostic(
            "NS1003",
            "Unterminated string literal.",
            "Add a closing quote.",
            start,
            self.offset,
        );
    }

    fn escape(&mut self, is_byte: bool) -> EscapeOutcome {
        let start = self.offset;
        self.bump();

        let Some(escaped) = self.peek() else {
            return EscapeOutcome::Unterminated;
        };

        match escaped {
            '\\' | '"' | 'n' | 'r' | 't' | '0' => {
                self.bump();
                EscapeOutcome::Continue
            }
            'u' => {
                self.bump();
                if self.peek() != Some('{') {
                    self.diagnostic(
                        "NS1002",
                        "Invalid Unicode escape.",
                        "Use the form \\u{1F600}.",
                        start,
                        self.offset,
                    );
                    return EscapeOutcome::Continue;
                }
                self.bump();

                let digits_start = self.offset;
                let mut value = 0_u32;
                let mut digits = 0_usize;
                let mut closed = false;

                while let Some(character) = self.peek() {
                    if let Some(digit) = character.to_digit(16) {
                        self.bump();
                        value = value.saturating_mul(16).saturating_add(digit);
                        digits += 1;
                        continue;
                    }

                    if character == '}' {
                        self.bump();
                        closed = true;
                    }
                    break;
                }

                if !closed || digits == 0 || digits > 6 || char::from_u32(value).is_none() {
                    self.diagnostic(
                        "NS1002",
                        "Invalid Unicode escape.",
                        "Use one to six hexadecimal digits in \\u{...}.",
                        start,
                        self.offset.max(digits_start),
                    );
                } else if is_byte && value > 0xFF {
                    self.diagnostic(
                        "NS1002",
                        "Unicode escape does not fit a byte literal.",
                        "Byte literals accept values in the u8 range.",
                        start,
                        self.offset,
                    );
                }

                EscapeOutcome::Continue
            }
            '\n' | '\r' => EscapeOutcome::Unterminated,
            _ => {
                self.bump();
                self.diagnostic(
                    "NS1002",
                    "Invalid escape sequence.",
                    "Supported escapes are \\\\, \\\", \\n, \\r, \\t, \\0, and \\u{...}.",
                    start,
                    self.offset,
                );
                EscapeOutcome::Continue
            }
        }
    }

    fn number(&mut self, start: usize) {
        if self.starts_with("0x") || self.starts_with("0X") {
            self.bump();
            self.bump();
            let digits_start = self.offset;
            while let Some(character) = self.peek() {
                if character.is_ascii_hexdigit() || character == '_' {
                    self.bump();
                } else {
                    break;
                }
            }
            if self.offset == digits_start {
                self.diagnostic(
                    "NS1005",
                    "Invalid hexadecimal literal.",
                    "Add at least one hexadecimal digit after 0x.",
                    start,
                    self.offset,
                );
            }
            self.byte_suffix();
            self.push(TokenKind::Integer, start);
            return;
        }

        if self.starts_with("0b") || self.starts_with("0B") {
            self.bump();
            self.bump();
            let digits_start = self.offset;
            while let Some(character) = self.peek() {
                if character == '0' || character == '1' || character == '_' {
                    self.bump();
                } else {
                    break;
                }
            }
            if self.offset == digits_start {
                self.diagnostic(
                    "NS1005",
                    "Invalid binary literal.",
                    "Add at least one binary digit after 0b.",
                    start,
                    self.offset,
                );
            }
            self.byte_suffix();
            self.push(TokenKind::Integer, start);
            return;
        }

        while let Some(character) = self.peek() {
            if character.is_ascii_digit() || character == '_' {
                self.bump();
            } else {
                break;
            }
        }

        // After a postfix `.`, a digit is an integer-only tuple index. It must
        // not absorb the following dot as a float (`t.0.1` is two indexes).
        let tuple_index = self.edition == Edition::V0_2
            && matches!(
                self.tokens.last(),
                Some(token) if token.kind == TokenKind::Punct(Punctuation::Dot)
            );

        let mut is_float = false;

        if !tuple_index
            && self.peek() == Some('.')
            && self.peek_n(1).is_some_and(|c| c.is_ascii_digit())
        {
            is_float = true;
            self.bump();
            while let Some(character) = self.peek() {
                if character.is_ascii_digit() || character == '_' {
                    self.bump();
                } else {
                    break;
                }
            }
        }

        if !tuple_index && matches!(self.peek(), Some('e' | 'E')) {
            let has_digit = match self.peek_n(1) {
                Some('+' | '-') => self.peek_n(2).is_some_and(|c| c.is_ascii_digit()),
                Some(character) => character.is_ascii_digit(),
                None => false,
            };

            if has_digit {
                is_float = true;
                self.bump();
                if matches!(self.peek(), Some('+' | '-')) {
                    self.bump();
                }
                while let Some(character) = self.peek() {
                    if character.is_ascii_digit() || character == '_' {
                        self.bump();
                    } else {
                        break;
                    }
                }
            }
        }

        if is_float {
            self.push(TokenKind::Float, start);
        } else {
            if !tuple_index {
                self.byte_suffix();
            }
            self.push(TokenKind::Integer, start);
        }
    }

    fn byte_suffix(&mut self) {
        if self.starts_with("u8")
            && self
                .peek_n(2)
                .is_none_or(|character| !is_identifier_continue(character))
        {
            self.bump();
            self.bump();
        }
    }

    fn identifier(&mut self, start: usize) {
        self.bump();

        while let Some(character) = self.peek() {
            if is_identifier_continue(character) {
                self.bump();
            } else {
                break;
            }
        }

        let text = self.source.get(start..self.offset).unwrap_or_default();
        let kind = if text == "_" {
            TokenKind::Punct(Punctuation::Underscore)
        } else if let Some(keyword) = keyword(text) {
            if keyword == Keyword::Fail && self.edition != Edition::V0_2 {
                TokenKind::Identifier
            } else {
                TokenKind::Keyword(keyword)
            }
        } else {
            TokenKind::Identifier
        };

        let is_impl_in_script = self.script_header_depth > 0
            && text == "impl"
            && self
                .tokens
                .last()
                .is_some_and(|t| t.kind == TokenKind::Punct(Punctuation::At));

        self.push(kind, start);

        if is_impl_in_script {
            self.script_header_depth = 0;
            self.capture_raw_script_source();
        }
    }

    fn capture_raw_script_source(&mut self) {
        while let Some(ch) = self.peek() {
            if ch == ' ' || ch == '\t' || ch == '\r' {
                self.bump();
            } else if ch == '\n' {
                self.bump();
                break;
            } else {
                break;
            }
        }

        let raw_start = self.offset;
        let mut found_end = false;
        let mut end_line_start = self.offset;

        while self.offset < self.source.len() {
            let line_start = self.offset;
            let remaining = &self.source[line_start..];
            let trimmed = remaining.trim_start_matches([' ', '\t']);
            if let Some(after) = trimmed.strip_prefix("@end") {
                let trimmed_after = after.trim_start_matches([' ', '\t']);
                if trimmed_after.starts_with("\r\n")
                    || trimmed_after.starts_with('\n')
                    || trimmed_after.starts_with('}')
                    || trimmed_after.is_empty()
                {
                    found_end = true;
                    end_line_start = line_start;
                    break;
                }
            }

            while let Some(ch) = self.peek() {
                self.bump();
                if ch == '\n' {
                    break;
                }
            }
        }

        if !found_end {
            self.diagnostic(
                "NS7004",
                "Malformed script block: missing '@end'.",
                "Add an '@end' sentinel line before the closing '}'.",
                self.script_start,
                self.offset,
            );
            end_line_start = self.offset;
        }

        let raw_slice = &self.source[raw_start..end_line_start];
        let normalized = raw_slice.replace("\r\n", "\n");

        self.tokens.push(Token {
            kind: TokenKind::RawSource,
            text: normalized,
            span: Span::new(raw_start, end_line_start),
        });

        if found_end {
            self.offset = end_line_start;
            while let Some(ch) = self.peek() {
                if ch == ' ' || ch == '\t' {
                    self.bump();
                } else {
                    break;
                }
            }
        }
    }

    fn operator_or_error(&mut self, start: usize) {
        let Some(character) = self.bump() else {
            return;
        };

        let one = |lexer: &mut Self, kind| lexer.push(TokenKind::Punct(kind), start);

        match character {
            '+' => {
                if self.peek() == Some('%') {
                    self.bump();
                    one(self, Punctuation::PlusWrap);
                } else {
                    one(self, Punctuation::Plus);
                }
            }
            '-' => {
                if self.peek() == Some('%') {
                    self.bump();
                    one(self, Punctuation::MinusWrap);
                } else if self.peek() == Some('>') {
                    self.bump();
                    one(self, Punctuation::Arrow);
                } else {
                    one(self, Punctuation::Minus);
                }
            }
            '*' => {
                if self.peek() == Some('%') {
                    self.bump();
                    one(self, Punctuation::StarWrap);
                } else {
                    one(self, Punctuation::Star);
                }
            }
            '/' => {
                if self.peek() == Some('%') {
                    self.bump();
                    one(self, Punctuation::SlashWrap);
                } else {
                    one(self, Punctuation::Slash);
                }
            }
            '%' => one(self, Punctuation::Percent),
            '=' => {
                if self.peek() == Some('=') {
                    self.bump();
                    one(self, Punctuation::EqEq);
                } else if self.peek() == Some('>') {
                    self.bump();
                    one(self, Punctuation::FatArrow);
                } else {
                    one(self, Punctuation::Eq);
                }
            }
            '!' => {
                if self.peek() == Some('=') {
                    self.bump();
                    one(self, Punctuation::BangEq);
                } else {
                    one(self, Punctuation::Bang);
                }
            }
            '<' => {
                if self.peek() == Some('=') {
                    self.bump();
                    one(self, Punctuation::LtEq);
                } else {
                    one(self, Punctuation::Lt);
                }
            }
            '>' => {
                if self.peek() == Some('=') {
                    self.bump();
                    one(self, Punctuation::GtEq);
                } else {
                    one(self, Punctuation::Gt);
                }
            }
            '&' => {
                if self.peek() == Some('&') {
                    self.bump();
                    one(self, Punctuation::AmpAmp);
                } else if self.edition == Edition::V0_2 {
                    one(self, Punctuation::Amp);
                } else {
                    self.diagnostic(
                        "NS1005",
                        "Unexpected character '&'.",
                        "Use && for logical and.",
                        start,
                        self.offset,
                    );
                }
            }
            '^' => {
                if self.edition == Edition::V0_2 {
                    one(self, Punctuation::Caret);
                } else {
                    self.diagnostic(
                        "NS1005",
                        "Unexpected character '^'.",
                        "Bitwise xor is available in edition 0.2.",
                        start,
                        self.offset,
                    );
                }
            }
            '|' => {
                if self.peek() == Some('|') {
                    self.bump();
                    one(self, Punctuation::PipePipe);
                } else {
                    one(self, Punctuation::Pipe);
                }
            }
            '.' => {
                if self.edition == Edition::V0_2 && self.peek() == Some('.') {
                    self.bump();
                    if self.peek() == Some('=') {
                        self.bump();
                        one(self, Punctuation::DotDotEq);
                    } else {
                        one(self, Punctuation::DotDot);
                    }
                } else {
                    one(self, Punctuation::Dot);
                }
            }
            ',' => one(self, Punctuation::Comma),
            ':' => {
                if self.peek() == Some(':') && self.edition == Edition::V0_2 {
                    self.bump();
                    let rest = self.source.get(self.offset..).unwrap_or_default();
                    if self.peek() == Some('<') {
                        one(self, Punctuation::DoubleColon);
                    } else {
                        let hint = if rest.trim_start().starts_with('<') {
                            "Write `::<` with no space."
                        } else {
                            "A standalone `::` is not valid; use `::<...>` for type arguments."
                        };
                        self.diagnostic("NS2011", "Malformed turbofish.", hint, start, self.offset);
                        one(self, Punctuation::DoubleColon);
                    }
                } else {
                    one(self, Punctuation::Colon);
                }
            }
            ';' => one(self, Punctuation::Semicolon),
            '(' => one(self, Punctuation::LParen),
            ')' => one(self, Punctuation::RParen),
            '[' => one(self, Punctuation::LBracket),
            ']' => one(self, Punctuation::RBracket),
            '{' => {
                if self.script_header_depth > 0 {
                    self.script_header_depth += 1;
                }
                one(self, Punctuation::LBrace);
            }
            '}' => {
                if self.script_header_depth > 0 {
                    self.script_header_depth -= 1;
                }
                one(self, Punctuation::RBrace);
            }
            '@' => {
                if self.starts_with("script:") {
                    // Depth 1: the outer {@script:...} brace has already been
                    // consumed as a '{' before the '@', so we start at depth 1.
                    self.script_header_depth = 1;
                    self.script_start = self
                        .tokens
                        .last()
                        .filter(|t| t.kind == TokenKind::Punct(Punctuation::LBrace))
                        .map_or(start, |t| t.span.start);
                }
                one(self, Punctuation::At);
            }
            '?' if self.edition == Edition::V0_2 => one(self, Punctuation::Question),
            _ => {
                self.diagnostic(
                    "NS1005",
                    format!("Unexpected character '{character}'."),
                    "Remove the character or replace it with valid syntax.",
                    start,
                    self.offset,
                );
            }
        }
    }
}

fn is_identifier_start(character: char) -> bool {
    character == '_' || is_xid_start(character)
}

fn is_identifier_continue(character: char) -> bool {
    character == '_' || is_xid_continue(character)
}

fn keyword(text: &str) -> Option<Keyword> {
    Some(match text {
        "abort" => Keyword::Abort,
        "as" => Keyword::As,
        "break" => Keyword::Break,
        "const" | "constant" => Keyword::Const,
        "continue" => Keyword::Continue,
        "default" => Keyword::Default,
        "defer" => Keyword::Defer,
        "else" => Keyword::Else,
        "enum" => Keyword::Enum,
        "export" => Keyword::Export,
        "fail" => Keyword::Fail,
        "false" => Keyword::False,
        "fn" | "function" => Keyword::Fn,
        "for" => Keyword::For,
        "foreign" => Keyword::Foreign,
        "from" => Keyword::From,
        "if" => Keyword::If,
        "import" => Keyword::Import,
        "in" => Keyword::In,
        "let" | "var" => Keyword::Let,
        "match" => Keyword::Match,
        "module" => Keyword::Module,
        "mut" | "mutable" => Keyword::Mut,
        "not" => Keyword::Not,
        "panic" => Keyword::Panic,
        "private" => Keyword::Private,
        "pub" | "public" => Keyword::Pub,
        "return" => Keyword::Return,
        "spawn" => Keyword::Spawn,
        "struct" | "record" => Keyword::Struct,
        "true" => Keyword::True,
        "type" | "typedef" => Keyword::Type,
        "while" => Keyword::While,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::{Edition, Keyword, LexOptions, Punctuation, TokenKind, lex, lex_with_options};

    fn kinds(source: &str) -> Vec<TokenKind> {
        lex(source)
            .tokens
            .into_iter()
            .map(|token| token.kind)
            .collect()
    }

    fn errors(source: &str) -> Vec<String> {
        lex(source)
            .diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.code)
            .collect()
    }

    fn punct(punctuation: Punctuation) -> TokenKind {
        TokenKind::Punct(punctuation)
    }

    #[test]
    fn lexes_keywords_and_aliases_to_canonical_kinds() {
        let source =
            "fn function let var const constant type typedef struct record mut mutable pub public";
        assert_eq!(
            kinds(source),
            vec![
                TokenKind::Keyword(Keyword::Fn),
                TokenKind::Keyword(Keyword::Fn),
                TokenKind::Keyword(Keyword::Let),
                TokenKind::Keyword(Keyword::Let),
                TokenKind::Keyword(Keyword::Const),
                TokenKind::Keyword(Keyword::Const),
                TokenKind::Keyword(Keyword::Type),
                TokenKind::Keyword(Keyword::Type),
                TokenKind::Keyword(Keyword::Struct),
                TokenKind::Keyword(Keyword::Struct),
                TokenKind::Keyword(Keyword::Mut),
                TokenKind::Keyword(Keyword::Mut),
                TokenKind::Keyword(Keyword::Pub),
                TokenKind::Keyword(Keyword::Pub),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lexes_unicode_identifiers_and_byte_spans() {
        let result = lex("_x café _ 42");
        assert!(!result.has_errors());

        let cafe = &result.tokens[1];
        assert_eq!(cafe.kind, TokenKind::Identifier);
        assert_eq!(cafe.text, "café");
        assert_eq!(cafe.span.start, 3);
        assert_eq!(cafe.span.end, 8);

        assert_eq!(result.tokens[2].kind, punct(Punctuation::Underscore));
        assert_eq!(result.tokens[3].kind, TokenKind::Integer);
    }

    #[test]
    fn lexes_integer_and_float_literals() {
        let source = "0 42 1_000_000 0xFF 0b1010 1.0 1e9 1.5e-3 0xFFu8";
        assert_eq!(
            kinds(source),
            vec![
                TokenKind::Integer,
                TokenKind::Integer,
                TokenKind::Integer,
                TokenKind::Integer,
                TokenKind::Integer,
                TokenKind::Float,
                TokenKind::Float,
                TokenKind::Float,
                TokenKind::Integer,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lexes_strings_byte_strings_and_escapes() {
        let source = r#""hello\nworld" b"abc" "\u{1F600}" "\\" "\"" "#;
        let result = lex(source);
        assert!(!result.has_errors(), "{:?}", result.diagnostics);
        assert_eq!(
            result
                .tokens
                .into_iter()
                .map(|token| token.kind)
                .collect::<Vec<_>>(),
            vec![
                TokenKind::String,
                TokenKind::ByteString,
                TokenKind::String,
                TokenKind::String,
                TokenKind::String,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn reports_invalid_escape_sequences() {
        assert_eq!(errors(r#""\q""#), vec!["NS1002"]);
        assert_eq!(errors(r#""\u{110000}""#), vec!["NS1002"]);
        assert_eq!(errors(r#""\u{1234567}""#), vec!["NS1002"]);
        assert_eq!(errors(r#"b"\u{100}""#), vec!["NS1002"]);
    }

    #[test]
    fn reports_unterminated_strings() {
        assert_eq!(errors("\"hello"), vec!["NS1003"]);
        assert_eq!(errors("\"hello\nworld"), vec!["NS1003"]);
    }

    #[test]
    fn lexes_operators_with_longest_match() {
        let source =
            "+ +% - -% * *% / /% == != < <= > >= && || ! = . , : ; -> => ( ) [ ] { } @ | _";
        assert_eq!(
            kinds(source),
            vec![
                punct(Punctuation::Plus),
                punct(Punctuation::PlusWrap),
                punct(Punctuation::Minus),
                punct(Punctuation::MinusWrap),
                punct(Punctuation::Star),
                punct(Punctuation::StarWrap),
                punct(Punctuation::Slash),
                punct(Punctuation::SlashWrap),
                punct(Punctuation::EqEq),
                punct(Punctuation::BangEq),
                punct(Punctuation::Lt),
                punct(Punctuation::LtEq),
                punct(Punctuation::Gt),
                punct(Punctuation::GtEq),
                punct(Punctuation::AmpAmp),
                punct(Punctuation::PipePipe),
                punct(Punctuation::Bang),
                punct(Punctuation::Eq),
                punct(Punctuation::Dot),
                punct(Punctuation::Comma),
                punct(Punctuation::Colon),
                punct(Punctuation::Semicolon),
                punct(Punctuation::Arrow),
                punct(Punctuation::FatArrow),
                punct(Punctuation::LParen),
                punct(Punctuation::RParen),
                punct(Punctuation::LBracket),
                punct(Punctuation::RBracket),
                punct(Punctuation::LBrace),
                punct(Punctuation::RBrace),
                punct(Punctuation::At),
                punct(Punctuation::Pipe),
                punct(Punctuation::Underscore),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn collects_line_comments_and_keeps_newlines() {
        let result = lex("// doc  \r\n/// more\n42");
        assert!(!result.has_errors());
        assert_eq!(
            result
                .tokens
                .iter()
                .map(|token| token.kind)
                .collect::<Vec<_>>(),
            vec![
                TokenKind::Newline,
                TokenKind::Newline,
                TokenKind::Integer,
                TokenKind::Eof
            ]
        );
        assert_eq!(
            result
                .comments
                .iter()
                .map(|comment| comment.text.as_str())
                .collect::<Vec<_>>(),
            vec!["// doc", "/// more"]
        );
        assert_eq!(result.comments[0].span.start, 0);
        assert_eq!(result.comments[0].span.end, "// doc".len());
    }

    #[test]
    fn does_not_treat_comment_markers_inside_strings_as_comments() {
        let result = lex("\"// not a comment\" // comment");
        assert!(!result.has_errors());
        assert_eq!(result.comments.len(), 1);
        assert_eq!(result.comments[0].text, "// comment");
    }

    #[test]
    fn rejects_invalid_comment_forms() {
        assert_eq!(
            errors("/* block */\n# toml\n-- dash\n42"),
            vec!["NS1001", "NS1001", "NS1001"]
        );
    }

    #[test]
    fn reports_unexpected_characters() {
        assert_eq!(errors("?"), vec!["NS1005"]);
        assert_eq!(errors("&"), vec!["NS1005"]);
        assert_eq!(errors("0x"), vec!["NS1005"]);
        assert_eq!(errors("0b"), vec!["NS1005"]);
    }

    #[test]
    fn skips_a_leading_bom_and_preserves_byte_spans() {
        let result = lex("\u{feff}fn main");
        assert!(!result.has_errors());
        assert_eq!(result.tokens[0].kind, TokenKind::Keyword(Keyword::Fn));
        assert_eq!(result.tokens[0].span.start, 3);
        assert_eq!(result.tokens[0].span.end, 5);
    }

    #[test]
    fn distinguishes_b_identifier_from_byte_string() {
        assert_eq!(
            kinds("b b\"x\""),
            vec![TokenKind::Identifier, TokenKind::ByteString, TokenKind::Eof]
        );
    }

    fn v02(source: &str) -> super::LexResult {
        lex_with_options(
            source,
            LexOptions {
                edition: Edition::V0_2,
            },
        )
    }

    #[test]
    fn fail_is_a_keyword_only_in_edition_0_2() {
        assert_eq!(kinds("fail"), vec![TokenKind::Identifier, TokenKind::Eof]);
        let result = v02("fail");
        assert!(!result.has_errors());
        assert_eq!(result.tokens[0].kind, TokenKind::Keyword(Keyword::Fail));
    }

    #[test]
    fn question_is_a_token_only_in_edition_0_2() {
        assert!(lex("?").has_errors());
        let result = v02("x?");
        assert!(!result.has_errors());
        assert_eq!(
            result.tokens[1].kind,
            TokenKind::Punct(Punctuation::Question)
        );
    }

    #[test]
    fn double_colon_requires_a_following_angle_bracket() {
        let valid = v02("::<i32>");
        assert!(!valid.has_errors());
        assert_eq!(
            valid.tokens[0].kind,
            TokenKind::Punct(Punctuation::DoubleColon)
        );

        for source in ["::", ":: <i32>"] {
            let result = v02(source);
            assert!(
                result
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code == "NS2011"),
                "{source}: {:?}",
                result.diagnostics
            );
        }
    }

    #[test]
    fn v0_1_keeps_two_colons_for_a_standalone_double_colon() {
        assert_eq!(
            kinds("::"),
            vec![
                TokenKind::Punct(Punctuation::Colon),
                TokenKind::Punct(Punctuation::Colon),
                TokenKind::Eof
            ]
        );
    }

    #[test]
    fn tuple_indexes_are_integers_after_a_postfix_dot() {
        for (source, indexes) in [("t.0.1", 2), ("t.0.1.2", 3)] {
            let result = v02(source);
            assert!(!result.has_errors(), "{source}: {:?}", result.diagnostics);
            let integer_indexes = result
                .tokens
                .iter()
                .filter(|token| token.kind == TokenKind::Integer)
                .count();
            assert_eq!(integer_indexes, indexes, "{source}");
            assert!(
                result
                    .tokens
                    .iter()
                    .all(|token| token.kind != TokenKind::Float),
                "{source} must not produce a float"
            );
        }

        let float = lex("1.0");
        assert_eq!(
            float.tokens[0].kind,
            TokenKind::Float,
            "a leading digit still starts a float"
        );
    }

    #[test]
    fn v0_1_keeps_suffixed_postfix_numbers_as_one_token() {
        let result = lex("t.0u8");
        assert!(!result.has_errors(), "{:?}", result.diagnostics);
        let integers = result
            .tokens
            .iter()
            .filter(|token| token.kind == TokenKind::Integer)
            .count();
        assert_eq!(integers, 1, "t.0u8 must stay a single integer token");
    }

    #[test]
    fn triple_slash_stays_an_ordinary_comment() {
        let result = lex("/// doc\nfn main() {}");
        assert!(!result.has_errors());
        assert_eq!(result.comments.len(), 1);
        assert_eq!(result.comments[0].text, "/// doc");
    }
}
