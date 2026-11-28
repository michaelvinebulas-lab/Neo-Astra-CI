//! Recursive-descent parser for Neo Astra v0.1.
//!
//! The parser consumes lexer tokens, preserves byte spans, and returns
//! structured `NS2xxx` diagnostics. Newlines separate statements and items;
//! semicolons are optional soft separators and are required after scope
//! annotations.

use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::edition::Edition;
use crate::lexer::{
    Comment, Keyword, LexOptions, LexResult, Punctuation, Token, TokenKind, lex_with_options,
};
use crate::literal::{decode_byte_literal, decode_char_literal, decode_string_literal};
use crate::span::Span;

/// Result of a parser operation.
pub type ParseResult<T> = Result<T, Vec<Diagnostic>>;

const EXPECTED: &str = "NS2001";
const INVALID: &str = "NS2002";
const INVALID_TYPE: &str = "NS2003";
const INVALID_PATTERN: &str = "NS2004";
const INVALID_ITEM: &str = "NS2005";
const BOUNDARY: &str = "NS2006";
const BARE_TYPE_ARGUMENTS: &str = "NS2010";
const MALFORMED_TURBOFISH: &str = "NS2011";
const INVALID_TYPE_ARGUMENT: &str = "NS4018";
const DEPRECATED_SYNTAX: &str = "NS2015";
const INVALID_CONTROL_FLOW: &str = "NS4014";

/// Nesting limit for the recursive-descent parser (HARD-001). Beyond this depth
/// the parser reports `NS2002` and unwinds instead of risking a stack
/// overflow, which would abort the process rather than report diagnostics.
const MAX_NESTING_DEPTH: usize = 64;
const CONFLICTING_VISIBILITY: &str = "NS3002";
const UNKNOWN_SCRIPT_LANGUAGE: &str = "NS7001";
const INVALID_SCRIPT_EXPORTS: &str = "NS7003";
const MALFORMED_SCRIPT_BLOCK: &str = "NS7004";
const INVALID_SCRIPT_SIGNATURE: &str = "NS7005";

/// Parser options. Editions are threaded through now; syntax behavior changes
/// land in the next phase.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ParseOptions {
    pub edition: Edition,
}

/// Parses one source unit with the default options.
pub fn parse(source: &str) -> ParseResult<Module> {
    parse_with_options(source, ParseOptions::default())
}

/// Parses one source unit with explicit options.
pub fn parse_with_options(source: &str, options: ParseOptions) -> ParseResult<Module> {
    parse_with_comments_with_options(source, options).map(|(module, _)| module)
}

/// Parses one source unit and returns valid line comments as trivia.
pub fn parse_with_comments(source: &str) -> ParseResult<(Module, Vec<Comment>)> {
    parse_with_comments_with_options(source, ParseOptions::default())
}

/// Parses one source unit with explicit options and returns line comments.
pub fn parse_with_comments_with_options(
    source: &str,
    options: ParseOptions,
) -> ParseResult<(Module, Vec<Comment>)> {
    let LexResult {
        tokens,
        comments,
        diagnostics,
    } = lex_with_options(
        source,
        LexOptions {
            edition: options.edition,
        },
    );
    let mut parser = Parser {
        tokens,
        index: 0,
        diagnostics,
        pending_lambda: None,
        lambda_depth: 0,
        edition: options.edition,
        depth: 0,
        depth_reported: false,
        nesting_drained: false,
    };
    let module = parser.parse_module();

    if parser.diagnostics.is_empty() {
        Ok((module, comments))
    } else {
        Err(suppress_cascades(parser.diagnostics, source))
    }
}

/// Bounds parser cascades per source line.
///
/// Only parser codes are eligible: `NS2002` is the nesting-limit guard and is
/// always kept, and non-parser codes are never suppressed. Within one line we
/// keep at most two *distinct* parser codes; later repeats and a third distinct
/// code are consequences of the first mistake rather than independent errors.
/// Line numbers use raw byte offsets (`\n` is ASCII), so a span that is not on a
/// character boundary still gets a stable line.
fn suppress_cascades(diagnostics: Vec<Diagnostic>, source: &str) -> Vec<Diagnostic> {
    let mut per_line: std::collections::HashMap<usize, (usize, std::collections::HashSet<String>)> =
        std::collections::HashMap::new();
    diagnostics
        .into_iter()
        .filter(|diagnostic| {
            if !diagnostic.code.starts_with("NS2") || diagnostic.code == "NS2002" {
                return true;
            }
            let line = diagnostic.span.map_or(0, |span| {
                source.as_bytes()[..span.start.min(source.len())]
                    .iter()
                    .filter(|byte| **byte == b'\n')
                    .count()
            });
            let (count, codes) = per_line
                .entry(line)
                .or_insert((0, std::collections::HashSet::new()));
            if codes.contains(&diagnostic.code) {
                return false;
            }
            if *count >= 2 {
                return false;
            }
            codes.insert(diagnostic.code.clone());
            *count += 1;
            true
        })
        .collect()
}

struct PendingLambda {
    parameters: Vec<LambdaParameter>,
    body: Box<Expr>,
    span: Span,
}

struct LambdaParameter {
    name: String,
    ty: Option<Type>,
    span: Span,
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
    diagnostics: Vec<Diagnostic>,
    /// A lambda parsed by `parse_primary`, consumed by the immediately
    /// following call in `parse_postfix`.
    pending_lambda: Option<PendingLambda>,
    /// Nesting depth of `parse_expression` only, used to report an unconsumed
    /// lambda at the outermost expression.
    lambda_depth: usize,
    /// Selected language edition; consumed by the v0.2 parser rules in Phase 3.
    #[allow(dead_code)]
    edition: Edition,
    /// Current recursion depth of expression/type/block parsing.
    depth: usize,
    /// Whether the nesting-limit diagnostic was already reported.
    depth_reported: bool,
    /// Whether the guard has drained the token stream to EOF. Sticky: once
    /// set, speculative restores must not rewind the index, or unwinding
    /// speculation frames would each re-scan the tail (quadratic on deep
    /// nesting).
    nesting_drained: bool,
}

impl Parser {
    fn parse_module(&mut self) -> Module {
        let start = self.peek().span.start;
        let mut module = Module {
            span: Span::new(start, start),
            ..Module::default()
        };

        self.skip_separators();

        while !self.at_eof() {
            let before = self.index;
            let attributes = self.parse_attributes();

            if self.at_keyword(Keyword::Module) {
                let token = self.bump();
                module.name = self.parse_path();
                module.attributes = attributes;
                module.header_span = Span::new(token.span.start, self.last_end());
                self.finish_boundary(false);
                continue;
            }

            if let Some(item) = self.parse_item_with_attributes(attributes) {
                module.items.push(item);
            }

            self.finish_boundary(false);
            if self.index == before {
                self.bump();
            }
        }

        module.span = Span::new(start, self.last_end());
        module
    }

    fn parse_item_with_attributes(&mut self, attributes: Vec<Attribute>) -> Option<Item> {
        let start = self.peek().span.start;

        if self.at_punct(Punctuation::LBrace) && self.looks_like_script_block() {
            let script = self.parse_script_block();
            return Some(Item {
                attributes,
                visibility: Visibility::Normal,
                exported: false,
                kind: ItemKind::ScriptBlock(script.clone()),
                span: script.span,
            });
        }

        if self.at_canonical_scope() {
            let scoped = self.parse_canonical_scoped_items();
            return Some(Item {
                attributes,
                visibility: Visibility::Normal,
                exported: false,
                kind: ItemKind::ScopedItems(scoped.clone()),
                span: scoped.span,
            });
        }

        if self.at_punct(Punctuation::LBrace) && self.looks_like_scope_annotation() {
            let scoped = self.parse_scoped_items();
            return Some(Item {
                attributes,
                visibility: Visibility::Normal,
                exported: false,
                kind: ItemKind::ScopedItems(scoped.clone()),
                span: scoped.span,
            });
        }

        if self.at_keyword(Keyword::Import) {
            let import = self.parse_import();
            return Some(Item {
                attributes,
                visibility: Visibility::Normal,
                exported: false,
                kind: ItemKind::Import(import.clone()),
                span: import.span,
            });
        }

        if self.at_keyword(Keyword::Export) && self.export_is_declaration() {
            let export = self.parse_export();
            return Some(Item {
                attributes,
                visibility: Visibility::Normal,
                exported: false,
                kind: ItemKind::Export(export.clone()),
                span: export.span,
            });
        }

        self.parse_declaration_item(attributes, start)
    }

    fn parse_declaration_item(&mut self, attributes: Vec<Attribute>, start: usize) -> Option<Item> {
        let mut visibility = Visibility::Normal;
        let mut exported = false;
        let mut see_public = false;
        let mut see_private = false;
        let mut see_export = false;

        loop {
            if self.at_keyword(Keyword::Pub) {
                let token = self.bump();
                if see_public || see_private {
                    self.error(
                        CONFLICTING_VISIBILITY,
                        "Duplicate or conflicting visibility modifier.",
                        token.span,
                        "Use at most one of pub/public or private.",
                    );
                }
                see_public = true;
                visibility = Visibility::Public;
            } else if self.at_keyword(Keyword::Private) {
                let token = self.bump();
                if see_public || see_private {
                    self.error(
                        CONFLICTING_VISIBILITY,
                        "Duplicate or conflicting visibility modifier.",
                        token.span,
                        "Use at most one of pub/public or private.",
                    );
                }
                see_private = true;
                visibility = Visibility::Private;
            } else if self.at_keyword(Keyword::Export) {
                let token = self.bump();
                if see_export {
                    self.error(
                        CONFLICTING_VISIBILITY,
                        "Duplicate export modifier.",
                        token.span,
                        "Use a single export modifier.",
                    );
                }
                see_export = true;
                exported = true;
            } else {
                break;
            }
        }

        if see_private && (see_public || see_export) {
            self.error(
                CONFLICTING_VISIBILITY,
                "A private declaration cannot also be public or exported.",
                Span::new(start, self.last_end()),
                "Remove pub/export, or make the declaration non-private.",
            );
        }

        let kind = self.parse_declaration_kind()?;
        let span = Span::new(start, self.last_end());

        Some(Item {
            attributes,
            visibility,
            exported,
            kind,
            span,
        })
    }

    fn parse_declaration_kind(&mut self) -> Option<ItemKind> {
        if self.at_keyword(Keyword::Fn) {
            return Some(ItemKind::Function(self.parse_function_decl()));
        }

        if self.at_keyword(Keyword::Struct) {
            return Some(ItemKind::Struct(self.parse_struct_decl()));
        }

        if self.at_keyword(Keyword::Enum) {
            return Some(ItemKind::Enum(self.parse_enum_decl()));
        }

        if self.at_keyword(Keyword::Const) {
            return Some(ItemKind::Constant(self.parse_const_decl()));
        }

        if self.at_keyword(Keyword::Type) {
            return Some(ItemKind::TypeAlias(self.parse_type_alias_decl()));
        }

        if self.at_keyword(Keyword::Foreign) {
            return Some(ItemKind::ForeignModule(self.parse_foreign_module()));
        }

        self.error_current(
            INVALID_ITEM,
            "Expected a declaration.",
            "Use fn, struct, enum, const, type, foreign module, import, or export.",
        );

        if !self.at_eof()
            && !self.at(TokenKind::Newline)
            && !self.at_punct(Punctuation::Semicolon)
            && !self.at_punct(Punctuation::RBrace)
        {
            self.bump();
        }

        None
    }

    fn parse_attributes(&mut self) -> Vec<Attribute> {
        let mut attributes = Vec::new();

        while self.at_punct(Punctuation::At) {
            if self.at_canonical_scope() {
                break;
            }
            let start = self.bump().span.start;
            let name = self.expect_identifier().unwrap_or_default();
            let mut arguments = Vec::new();

            if self.eat_punct(Punctuation::LParen) {
                self.skip_newlines();

                if name == "export_as" {
                    let symbol_expr = self.parse_expression();
                    let symbol = match &symbol_expr.kind {
                        ExprKind::Literal(Literal::String(value)) => value.clone(),
                        _ => {
                            self.error(
                                EXPECTED,
                                "The first argument of '@export_as' must be a string literal naming the exported symbol.",
                                symbol_expr.span,
                                "Write @export_as(\"ns_module_fn_type\", T = i32).",
                            );
                            String::new()
                        }
                    };
                    let mut bindings = Vec::new();
                    loop {
                        if !self.eat_punct(Punctuation::Comma) {
                            break;
                        }
                        self.skip_newlines();
                        if self.at_punct(Punctuation::RParen) {
                            break;
                        }
                        let Some(parameter) = self.expect_identifier() else {
                            break;
                        };
                        self.expect_punct(Punctuation::Eq, "'='");
                        let ty = self.parse_type();
                        bindings.push((parameter, ty));
                    }
                    self.expect_punct(Punctuation::RParen, "')'");
                    let span = self.span_from(start);
                    attributes.push(Attribute {
                        name,
                        arguments: Vec::new(),
                        export_as: Some(ExportAsAttribute {
                            symbol,
                            bindings,
                            span,
                        }),
                        span,
                    });
                    self.skip_newlines();
                    continue;
                }

                if !self.at_punct(Punctuation::RParen) {
                    loop {
                        arguments.push(self.parse_expression());
                        if self.eat_punct(Punctuation::Comma) {
                            self.skip_newlines();
                            if self.at_punct(Punctuation::RParen) {
                                break;
                            }
                            continue;
                        }
                        break;
                    }
                }

                self.expect_punct(Punctuation::RParen, "')'");
            }

            attributes.push(Attribute {
                name,
                arguments,
                export_as: None,
                span: self.span_from(start),
            });
            self.skip_newlines();
        }

        attributes
    }

    fn parse_import(&mut self) -> ImportDecl {
        let start = self.bump().span.start;

        let form = if self.at(TokenKind::String) {
            ImportForm::SideEffect {
                specifier: self.expect_string(),
            }
        } else if self.eat_punct(Punctuation::Star) {
            self.expect_keyword(Keyword::As, "'as'");
            let alias = self.expect_identifier().unwrap_or_default();
            self.expect_keyword(Keyword::From, "'from'");
            let specifier = self.expect_string();
            ImportForm::Namespace { alias, specifier }
        } else if self.at_punct(Punctuation::LBrace) {
            let specifiers = self.parse_import_specifiers();
            self.expect_keyword(Keyword::From, "'from'");
            let specifier = self.expect_string();
            ImportForm::Named {
                specifiers,
                specifier,
            }
        } else if self.at(TokenKind::Identifier) {
            let first = self.bump().text;

            if self.eat_keyword(Keyword::From) {
                let specifier = self.expect_string();
                ImportForm::Default {
                    alias: first,
                    specifier,
                }
            } else if self.eat_punct(Punctuation::Comma) {
                let specifiers = self.parse_import_specifiers();
                self.expect_keyword(Keyword::From, "'from'");
                let specifier = self.expect_string();
                ImportForm::DefaultNamed {
                    default: first,
                    specifiers,
                    specifier,
                }
            } else {
                let mut path = vec![first];
                while self.eat_punct(Punctuation::Dot) {
                    path.push(self.expect_identifier().unwrap_or_default());
                }
                let alias = if self.eat_keyword(Keyword::As) {
                    Some(self.expect_identifier().unwrap_or_default())
                } else {
                    None
                };
                ImportForm::Path { path, alias }
            }
        } else {
            self.error_current(
                INVALID_ITEM,
                "Invalid import form.",
                "Use import path, import path as alias, or a string-specifier form.",
            );
            ImportForm::Path {
                path: Vec::new(),
                alias: None,
            }
        };

        ImportDecl {
            form,
            span: self.span_from(start),
        }
    }

    fn parse_import_specifiers(&mut self) -> Vec<ImportSpecifier> {
        let mut specifiers = Vec::new();
        self.expect_punct(Punctuation::LBrace, "'{'");
        self.skip_separators();

        while !self.at_punct(Punctuation::RBrace) && !self.at_eof() {
            let start = self.peek().span.start;
            let imported = self.parse_name_or_default();
            let local = if self.eat_keyword(Keyword::As) {
                self.expect_identifier().unwrap_or_default()
            } else {
                imported.clone()
            };

            specifiers.push(ImportSpecifier {
                imported,
                local,
                span: self.span_from(start),
            });

            self.skip_separators();
            if self.eat_punct(Punctuation::Comma) {
                self.skip_separators();
                continue;
            }
            if !self.at_punct(Punctuation::RBrace) {
                self.error_current(
                    EXPECTED,
                    "Expected ',' or '}' after import specifier.",
                    "Separate import names with commas.",
                );
                self.synchronize_until(&[Punctuation::Comma, Punctuation::RBrace]);
                self.eat_punct(Punctuation::Comma);
                self.skip_separators();
            }
        }

        self.expect_punct(Punctuation::RBrace, "'}'");
        specifiers
    }

    fn parse_export(&mut self) -> ExportDecl {
        let start = self.bump().span.start;

        let form = if self.at_punct(Punctuation::LBrace) {
            let specifiers = self.parse_export_specifiers();
            let from = if self.eat_keyword(Keyword::From) {
                Some(self.expect_string())
            } else {
                None
            };
            ExportForm::List { specifiers, from }
        } else if self.eat_punct(Punctuation::Star) {
            let alias = if self.eat_keyword(Keyword::As) {
                Some(self.parse_name_or_default())
            } else {
                None
            };
            self.expect_keyword(Keyword::From, "'from'");
            let from = self.expect_string();
            ExportForm::All { alias, from }
        } else if self.eat_keyword(Keyword::Default) {
            if self.at_keyword(Keyword::Fn) {
                ExportForm::DefaultFunction(self.parse_function_decl())
            } else {
                ExportForm::DefaultValue(self.parse_expression())
            }
        } else {
            self.error_current(
                INVALID_ITEM,
                "Invalid export form.",
                "Use export { ... }, export * from ..., or export default.",
            );
            ExportForm::List {
                specifiers: Vec::new(),
                from: None,
            }
        };

        ExportDecl {
            form,
            span: self.span_from(start),
        }
    }

    fn parse_export_specifiers(&mut self) -> Vec<ExportSpecifier> {
        let mut specifiers = Vec::new();
        self.expect_punct(Punctuation::LBrace, "'{'");
        self.skip_separators();

        while !self.at_punct(Punctuation::RBrace) && !self.at_eof() {
            let start = self.peek().span.start;
            let local = self.parse_name_or_default();
            let exported = if self.eat_keyword(Keyword::As) {
                self.parse_name_or_default()
            } else {
                local.clone()
            };

            specifiers.push(ExportSpecifier {
                local,
                exported,
                span: self.span_from(start),
            });

            self.skip_separators();
            if self.eat_punct(Punctuation::Comma) {
                self.skip_separators();
                continue;
            }
            if !self.at_punct(Punctuation::RBrace) {
                self.error_current(
                    EXPECTED,
                    "Expected ',' or '}' after export specifier.",
                    "Separate export names with commas.",
                );
                self.synchronize_until(&[Punctuation::Comma, Punctuation::RBrace]);
                self.eat_punct(Punctuation::Comma);
                self.skip_separators();
            }
        }

        self.expect_punct(Punctuation::RBrace, "'}'");
        specifiers
    }

    fn export_is_declaration(&self) -> bool {
        matches!(
            self.peek_at(1).map(|token| token.kind),
            Some(TokenKind::Punct(Punctuation::LBrace))
                | Some(TokenKind::Punct(Punctuation::Star))
                | Some(TokenKind::Keyword(Keyword::Default))
        )
    }

    fn parse_function_decl(&mut self) -> FunctionDecl {
        let start = self.bump().span.start;
        let name = self.expect_identifier().unwrap_or_default();
        let type_parameters = self.parse_type_parameters();
        let parameters = self.parse_parameters();
        let (return_type, error_set) = self.parse_return_and_errors();
        let body = self.parse_block();

        FunctionDecl {
            name,
            type_parameters,
            parameters,
            return_type,
            error_set,
            body,
            span: self.span_from(start),
        }
    }

    /// Skips to the next recovery anchor without emitting more diagnostics.
    ///
    /// A malformed item should not desynchronise the rest of the file. Anchors
    /// are delimiters that bound the item we were parsing.
    fn recover_to_anchor(&mut self, anchors: &[Punctuation]) {
        while !self.at_eof()
            && !self.at(TokenKind::Newline)
            && !anchors.iter().any(|anchor| self.at_punct(*anchor))
        {
            self.bump();
        }
    }

    fn parse_parameters(&mut self) -> Vec<Parameter> {
        let mut parameters = Vec::new();
        self.expect_punct(Punctuation::LParen, "'('");
        self.skip_newlines();

        if self.at_punct(Punctuation::RParen) {
            self.bump();
            return parameters;
        }

        let mut recovered = false;
        loop {
            let start = self.peek().span.start;
            if !self.at(TokenKind::Identifier) {
                self.error_current(
                    EXPECTED,
                    "Expected a parameter name.",
                    "Write name: Type inside the parentheses.",
                );
                self.recover_to_anchor(&[Punctuation::RParen, Punctuation::LBrace]);
                recovered = true;
                break;
            }
            let name = self.expect_identifier().unwrap_or_default();
            self.expect_punct(Punctuation::Colon, "':'");
            let ty = self.parse_type();
            parameters.push(Parameter {
                name,
                ty,
                span: self.span_from(start),
            });

            if self.eat_punct(Punctuation::Comma) {
                self.skip_newlines();
                if self.at_punct(Punctuation::RParen) {
                    break;
                }
                continue;
            }
            break;
        }

        self.skip_newlines();
        if recovered {
            self.eat_punct(Punctuation::RParen);
        } else {
            self.expect_punct(Punctuation::RParen, "')'");
        }
        parameters
    }

    fn parse_type_parameters(&mut self) -> Vec<String> {
        let mut parameters = Vec::new();
        if !self.eat_punct(Punctuation::Lt) {
            return parameters;
        }

        loop {
            parameters.push(self.expect_identifier().unwrap_or_default());
            if self.eat_punct(Punctuation::Comma) {
                self.skip_newlines();
                if self.edition == Edition::V0_2 && self.at_punct(Punctuation::Gt) {
                    break;
                }
                continue;
            }
            break;
        }

        self.skip_newlines();
        self.expect_punct(Punctuation::Gt, "'>'");
        parameters
    }

    fn parse_return_and_errors(&mut self) -> (Option<Type>, Vec<TypePath>) {
        let mut return_type = None;
        let mut error_set = Vec::new();

        if self.eat_punct(Punctuation::Arrow) {
            return_type = Some(self.parse_type());
            if self.at_punct(Punctuation::Bang) {
                error_set = self.parse_error_set();
            }
        } else if self.at_punct(Punctuation::Bang) {
            self.error_current(
                INVALID,
                "An error effect requires an explicit return type.",
                "Use '-> Type ! Error' instead of '! Error'.",
            );
            error_set = self.parse_error_set();
        }

        (return_type, error_set)
    }

    fn parse_error_set(&mut self) -> Vec<TypePath> {
        let mut errors = Vec::new();
        self.expect_punct(Punctuation::Bang, "'!'");

        if !self.at(TokenKind::Identifier) {
            self.error_current(
                EXPECTED,
                "Expected an error type after '!'.",
                "Write '! ErrorType' or '! First | Second'.",
            );
            return errors;
        }

        loop {
            errors.push(self.parse_type_path());
            if !self.eat_punct(Punctuation::Pipe) {
                break;
            }
        }

        errors
    }

    fn parse_struct_decl(&mut self) -> StructDecl {
        let start = self.bump().span.start;
        let name = self.expect_identifier().unwrap_or_default();
        let type_parameters = self.parse_type_parameters();

        let fields = if self.eat_punct(Punctuation::LBrace) {
            let mut fields = Vec::new();
            self.skip_newlines();

            while !self.at_punct(Punctuation::RBrace) && !self.at_eof() {
                let before = self.index;
                let field_start = self.peek().span.start;
                let field_name = self.expect_identifier().unwrap_or_default();
                self.expect_punct(Punctuation::Colon, "':'");
                let ty = self.parse_type();
                fields.push(FieldDecl {
                    name: field_name,
                    ty,
                    span: self.span_from(field_start),
                });

                if self.index == before {
                    self.bump();
                }
                self.skip_newlines();
                if self.eat_punct(Punctuation::Comma) {
                    self.skip_newlines();
                }
            }

            self.expect_punct(Punctuation::RBrace, "'}'");
            StructFields::Named(fields)
        } else if self.eat_punct(Punctuation::LParen) {
            StructFields::Tuple(self.parse_type_list_after_open())
        } else {
            self.error_current(
                EXPECTED,
                "Expected '{' or '(' after struct name.",
                "Declare named fields with { ... } or tuple fields with ( ... ).",
            );
            StructFields::Named(Vec::new())
        };

        StructDecl {
            name,
            type_parameters,
            fields,
            span: self.span_from(start),
        }
    }

    fn parse_enum_decl(&mut self) -> EnumDecl {
        let start = self.bump().span.start;
        let name = self.expect_identifier().unwrap_or_default();
        let type_parameters = self.parse_type_parameters();
        let mut variants = Vec::new();

        self.expect_punct(Punctuation::LBrace, "'{'");
        self.skip_newlines();

        while !self.at_punct(Punctuation::RBrace) && !self.at_eof() {
            let before = self.index;
            let variant_start = self.peek().span.start;
            let variant_name = self.expect_identifier().unwrap_or_default();
            let fields = if self.eat_punct(Punctuation::LParen) {
                let mut fields = Vec::new();
                self.skip_newlines();

                if !self.at_punct(Punctuation::RParen) {
                    loop {
                        fields.push(self.parse_variant_field());
                        self.skip_newlines();
                        if self.eat_punct(Punctuation::Comma) {
                            self.skip_newlines();
                            if self.at_punct(Punctuation::RParen) {
                                break;
                            }
                            continue;
                        }
                        break;
                    }
                }

                self.skip_newlines();
                self.expect_punct(Punctuation::RParen, "')'");
                Some(fields)
            } else {
                None
            };

            variants.push(EnumVariant {
                name: variant_name,
                fields,
                span: self.span_from(variant_start),
            });

            if self.index == before {
                self.bump();
            }
            self.skip_newlines();
            if self.eat_punct(Punctuation::Comma) {
                self.skip_newlines();
            }
        }

        self.expect_punct(Punctuation::RBrace, "'}'");

        EnumDecl {
            name,
            type_parameters,
            variants,
            span: self.span_from(start),
        }
    }

    fn parse_variant_field(&mut self) -> VariantField {
        let start = self.peek().span.start;
        let name = if self.at(TokenKind::Identifier)
            && self
                .peek_at(1)
                .is_some_and(|token| token.kind == TokenKind::Punct(Punctuation::Colon))
        {
            let name = self.bump().text;
            self.bump();
            Some(name)
        } else {
            None
        };

        VariantField {
            name,
            ty: self.parse_type(),
            span: self.span_from(start),
        }
    }

    fn parse_const_decl(&mut self) -> ConstantDecl {
        let start = self.bump().span.start;
        let name = self.expect_identifier().unwrap_or_default();
        self.expect_punct(Punctuation::Colon, "':'");
        let ty = self.parse_type();
        self.expect_punct(Punctuation::Eq, "'='");
        let value = self.parse_expression();

        ConstantDecl {
            name,
            ty,
            value,
            span: self.span_from(start),
        }
    }

    fn parse_type_alias_decl(&mut self) -> TypeAliasDecl {
        let start = self.bump().span.start;
        let name = self.expect_identifier().unwrap_or_default();
        let type_parameters = self.parse_type_parameters();
        self.expect_punct(Punctuation::Eq, "'='");
        let ty = self.parse_type();

        TypeAliasDecl {
            name,
            type_parameters,
            ty,
            span: self.span_from(start),
        }
    }

    fn parse_foreign_module(&mut self) -> ForeignModuleDecl {
        let start = self.bump().span.start;
        self.expect_keyword(Keyword::Module, "'module'");
        let name = self.expect_identifier().unwrap_or_default();
        let mut functions = Vec::new();

        self.expect_punct(Punctuation::LBrace, "'{'");
        self.skip_separators();

        while !self.at_punct(Punctuation::RBrace) && !self.at_eof() {
            if self.at_keyword(Keyword::Fn) {
                functions.push(self.parse_foreign_function());
            } else {
                self.error_current(
                    INVALID_ITEM,
                    "Expected a foreign function signature.",
                    "Foreign modules contain only fn declarations without bodies.",
                );
                self.bump();
            }
            self.skip_separators();
        }

        self.expect_punct(Punctuation::RBrace, "'}'");

        ForeignModuleDecl {
            name,
            functions,
            span: self.span_from(start),
        }
    }

    fn parse_foreign_function(&mut self) -> ForeignFunction {
        let start = self.bump().span.start;
        let name = self.expect_identifier().unwrap_or_default();
        let parameters = self.parse_parameters();
        let (return_type, error_set) = self.parse_return_and_errors();

        ForeignFunction {
            name,
            parameters,
            return_type,
            error_set,
            span: self.span_from(start),
        }
    }

    fn parse_script_block(&mut self) -> ScriptBlock {
        let block_start = self
            .expect_punct(Punctuation::LBrace, "'{'")
            .map_or(0, |t| t.span.start);
        self.expect_punct(Punctuation::At, "'@'");
        let _ = self.expect_identifier(); // "script"
        self.expect_punct(Punctuation::Colon, "':'");

        let lang_token = if self.at(TokenKind::Identifier) {
            Some(self.bump())
        } else {
            self.error_current(
                EXPECTED,
                "Expected script language.",
                "Specify a language such as js, ts, c, cpp, or python.",
            );
            None
        };
        let language = lang_token
            .as_ref()
            .map(|t| t.text.clone())
            .unwrap_or_default();
        let lang_span = lang_token
            .as_ref()
            .map(|t| t.span)
            .unwrap_or_else(|| self.span_from(block_start));
        match language.as_str() {
            "js" | "ts" | "c" | "cpp" | "python" => {}
            "java" => {
                self.error(
                    UNKNOWN_SCRIPT_LANGUAGE,
                    "Script language 'java' is reserved but unsupported until a JVM backend exists.",
                    lang_span,
                    "Supported languages in V1: js, ts, c, cpp, python.",
                );
            }
            _ => {
                self.error(
                    UNKNOWN_SCRIPT_LANGUAGE,
                    format!("Unknown or unsupported script language '{language}'."),
                    lang_span,
                    "Supported languages in V1: js, ts, c, cpp, python.",
                );
            }
        }

        if self.at_keyword(Keyword::As) || self.peek().text == "as" {
            self.bump();
        } else {
            self.error_current(
                EXPECTED,
                "Expected 'as'.",
                "Specify the script namespace using 'as <name>;'.",
            );
        }

        let name = self.expect_identifier().unwrap_or_default();
        self.expect_punct(Punctuation::Semicolon, "';'");
        let header_span = Span::new(block_start, self.last_end());

        let mut mode = String::new();
        let mut exports = Vec::new();
        let mut has_exports = false;

        self.skip_separators();
        while !self.at_eof() && !self.at_punct(Punctuation::RBrace) {
            if self.at_punct(Punctuation::At) {
                if self.peek_at(1).is_some_and(|t| t.text == "impl") {
                    break;
                }
                if self.peek_at(1).is_some_and(|t| t.text == "mode") {
                    self.bump(); // @
                    self.bump(); // mode
                    self.expect_punct(Punctuation::LParen, "'('");
                    mode = self.expect_identifier().unwrap_or_default();
                    self.expect_punct(Punctuation::RParen, "')'");
                    self.skip_separators();
                    continue;
                }
                if self.peek_at(1).is_some_and(|t| t.text == "exports") {
                    self.bump(); // @
                    self.bump(); // exports
                    has_exports = true;
                    self.expect_punct(Punctuation::LBrace, "'{'");
                    self.skip_separators();
                    let mut seen_exports = std::collections::BTreeSet::new();
                    while !self.at_punct(Punctuation::RBrace) && !self.at_eof() {
                        let before = self.index;
                        if self.at_keyword(Keyword::Fn) {
                            let fn_token = self.bump();
                            let fn_name = self.expect_identifier().unwrap_or_default();
                            if self.at_punct(Punctuation::Lt) {
                                self.error_current(
                                    INVALID_SCRIPT_SIGNATURE,
                                    "Script exports cannot be generic.",
                                    "Remove generic type parameters from the export signature.",
                                );
                                while !self.at_punct(Punctuation::Gt) && !self.at_eof() {
                                    self.bump();
                                }
                                if self.at_punct(Punctuation::Gt) {
                                    self.bump();
                                }
                            }
                            let parameters = self.parse_parameters();
                            let (return_type, error_set) = self.parse_return_and_errors();
                            let exp_span = Span::new(fn_token.span.start, self.last_end());
                            if !seen_exports.insert(fn_name.clone()) {
                                self.error(
                                    INVALID_SCRIPT_EXPORTS,
                                    format!("Duplicate export '{fn_name}' in script block."),
                                    exp_span,
                                    "Remove the duplicate export signature.",
                                );
                            }
                            exports.push(ScriptExport {
                                name: fn_name,
                                parameters,
                                return_type,
                                error_set,
                                span: exp_span,
                            });
                        } else {
                            self.error_current(
                                EXPECTED,
                                "Expected 'fn' export signature in script @exports block.",
                                "Use 'fn <name>(<params>) -> <type>'.",
                            );
                        }
                        self.finish_boundary(true);
                        if self.index == before {
                            self.bump();
                        }
                    }
                    self.expect_punct(Punctuation::RBrace, "'}'");
                    self.skip_separators();
                    continue;
                }
            }
            let tok = self.bump();
            self.error(
                MALFORMED_SCRIPT_BLOCK,
                format!("Unexpected token in script header: '{}'.", tok.text),
                tok.span,
                "Script header must declare @mode(...) and @exports { ... } before @impl.",
            );
        }

        if mode.is_empty() {
            self.error(
                MALFORMED_SCRIPT_BLOCK,
                "Malformed script block: missing @mode(...) declaration.",
                header_span,
                "Declare the minimum mode required by the foreign script, e.g. @mode(core).",
            );
            mode = "core".to_string();
        }

        if !has_exports {
            self.error(
                INVALID_SCRIPT_EXPORTS,
                "Missing @exports declaration in script block.",
                header_span,
                "Declare at least one exported function signature in @exports { ... }.",
            );
        } else if exports.is_empty() {
            self.error(
                INVALID_SCRIPT_EXPORTS,
                "Script block @exports declaration must contain at least one export signature.",
                header_span,
                "Add at least one function signature, e.g. fn add(a: i32, b: i32) -> i32.",
            );
        }

        if self.at_punct(Punctuation::At) && self.peek_at(1).is_some_and(|t| t.text == "impl") {
            self.bump();
            self.bump();
        } else {
            self.error_current(
                MALFORMED_SCRIPT_BLOCK,
                "Malformed script block: missing '@impl' marker.",
                "Mark the start of foreign implementation with '@impl'.",
            );
        }

        let (raw_source, body_span) = if self.at(TokenKind::RawSource) {
            let tok = self.bump();
            (tok.text, tok.span)
        } else {
            (String::new(), Span::new(self.last_end(), self.last_end()))
        };

        if self.at_punct(Punctuation::At) && self.peek_at(1).is_some_and(|t| t.text == "end") {
            self.bump();
            self.bump();
        } else {
            self.error_current(
                MALFORMED_SCRIPT_BLOCK,
                "Malformed script block: missing '@end' sentinel.",
                "End foreign raw source with an '@end' line.",
            );
        }

        self.skip_separators();
        if self.at_punct(Punctuation::RBrace) {
            self.bump();
        } else {
            self.error_current(
                MALFORMED_SCRIPT_BLOCK,
                "Malformed script block: missing closing '}'.",
                "Close the script block with '}'.",
            );
        }

        let total_span = Span::new(block_start, self.last_end());

        ScriptBlock {
            language,
            name,
            mode,
            exports,
            raw_source,
            span: total_span,
            header_span,
            body_span,
        }
    }

    fn parse_scoped_items(&mut self) -> ScopedItems {
        self.depth += 1;
        if self.depth > MAX_NESTING_DEPTH {
            self.nesting_limit_error("Scope nesting exceeds the supported depth.");
            let span = self.peek().span;
            self.depth -= 1;
            return ScopedItems {
                annotation: ScopeAnnotation {
                    kind: ScopeKind::Mode,
                    value: String::new(),
                    span,
                },
                items: Vec::new(),
                span,
            };
        }
        let scoped = self.parse_scoped_items_inner();
        self.depth -= 1;
        scoped
    }

    fn parse_scoped_items_inner(&mut self) -> ScopedItems {
        let start = self.bump().span.start;
        let annotation = self.parse_scope_annotation_body();
        let mut items = Vec::new();

        self.skip_separators();
        while !self.at_punct(Punctuation::RBrace) && !self.at_eof() {
            let before = self.index;
            let attributes = self.parse_attributes();
            if let Some(item) = self.parse_item_with_attributes(attributes) {
                items.push(item);
            }
            self.finish_boundary(true);
            if self.index == before {
                self.bump();
            }
        }

        self.expect_punct(Punctuation::RBrace, "'}'");

        ScopedItems {
            annotation,
            items,
            span: self.span_from(start),
        }
    }

    fn parse_scoped_statements(&mut self) -> ScopedStatements {
        self.depth += 1;
        if self.depth > MAX_NESTING_DEPTH {
            self.nesting_limit_error("Scope nesting exceeds the supported depth.");
            let span = self.peek().span;
            self.depth -= 1;
            return ScopedStatements {
                annotation: ScopeAnnotation {
                    kind: ScopeKind::Mode,
                    value: String::new(),
                    span,
                },
                statements: Vec::new(),
                span,
            };
        }
        let scoped = self.parse_scoped_statements_inner();
        self.depth -= 1;
        scoped
    }

    fn parse_scoped_statements_inner(&mut self) -> ScopedStatements {
        let start = self.bump().span.start;
        let annotation = self.parse_scope_annotation_body();
        let mut statements = Vec::new();

        self.skip_separators();
        while !self.at_punct(Punctuation::RBrace) && !self.at_eof() {
            let before = self.index;
            if self.looks_like_scope_annotation() {
                let scoped = self.parse_scoped_statements();
                statements.push(Stmt {
                    kind: StmtKind::Scoped(scoped.clone()),
                    span: scoped.span,
                });
            } else if let Some(statement) = self.parse_statement() {
                statements.push(statement);
            }
            self.finish_boundary(true);
            if self.index == before {
                self.bump();
            }
        }

        self.expect_punct(Punctuation::RBrace, "'}'");

        ScopedStatements {
            annotation,
            statements,
            span: self.span_from(start),
        }
    }

    fn parse_canonical_scoped_items(&mut self) -> ScopedItems {
        self.depth += 1;
        if self.depth > MAX_NESTING_DEPTH {
            self.nesting_limit_error("Scope nesting exceeds the supported depth.");
            let span = self.peek().span;
            self.depth -= 1;
            return ScopedItems {
                annotation: ScopeAnnotation {
                    kind: ScopeKind::Mode,
                    value: String::new(),
                    span,
                },
                items: Vec::new(),
                span,
            };
        }
        let start = self.peek().span.start;
        let annotation = self.parse_canonical_scope_annotation();
        self.expect_punct(Punctuation::LBrace, "'{'");
        let mut items = Vec::new();

        self.skip_separators();
        while !self.at_punct(Punctuation::RBrace) && !self.at_eof() {
            let before = self.index;
            let attributes = self.parse_attributes();
            if let Some(item) = self.parse_item_with_attributes(attributes) {
                items.push(item);
            }
            self.finish_boundary(true);
            if self.index == before {
                self.bump();
            }
        }

        self.expect_punct(Punctuation::RBrace, "'}'");
        self.depth -= 1;

        ScopedItems {
            annotation,
            items,
            span: self.span_from(start),
        }
    }

    fn parse_canonical_scoped_statements(&mut self) -> ScopedStatements {
        self.depth += 1;
        if self.depth > MAX_NESTING_DEPTH {
            self.nesting_limit_error("Scope nesting exceeds the supported depth.");
            let span = self.peek().span;
            self.depth -= 1;
            return ScopedStatements {
                annotation: ScopeAnnotation {
                    kind: ScopeKind::Mode,
                    value: String::new(),
                    span,
                },
                statements: Vec::new(),
                span,
            };
        }
        let start = self.peek().span.start;
        let annotation = self.parse_canonical_scope_annotation();
        self.expect_punct(Punctuation::LBrace, "'{'");
        let mut statements = Vec::new();

        self.skip_separators();
        while !self.at_punct(Punctuation::RBrace) && !self.at_eof() {
            let before = self.index;
            if let Some(statement) = self.parse_statement() {
                statements.push(statement);
            }
            self.finish_boundary(true);
            if self.index == before {
                self.bump();
            }
        }

        self.expect_punct(Punctuation::RBrace, "'}'");
        self.depth -= 1;

        ScopedStatements {
            annotation,
            statements,
            span: self.span_from(start),
        }
    }

    fn parse_canonical_scope_annotation(&mut self) -> ScopeAnnotation {
        let start = self
            .expect_punct(Punctuation::At, "'@'")
            .map_or_else(|| self.peek().span.start, |token| token.span.start);
        let name = self.expect_identifier().unwrap_or_default();
        self.expect_punct(Punctuation::LParen, "'('");
        self.skip_newlines();
        let value = self.expect_identifier().unwrap_or_default();
        self.skip_newlines();
        self.expect_punct(Punctuation::RParen, "')'");
        let kind = match name.as_str() {
            "mode" => ScopeKind::Mode,
            "memory" => ScopeKind::Memory,
            _ => {
                self.error_current(
                    INVALID,
                    format!("Unknown scope annotation '@{name}'."),
                    "Use @mode(...) or @memory(...).",
                );
                ScopeKind::Mode
            }
        };

        ScopeAnnotation {
            kind,
            value,
            span: self.span_from(start),
        }
    }

    fn parse_scope_annotation_body(&mut self) -> ScopeAnnotation {
        let start = self
            .expect_punct(Punctuation::At, "'@'")
            .map_or_else(|| self.peek().span.start, |token| token.span.start);
        let name = self.expect_identifier().unwrap_or_default();
        self.expect_punct(Punctuation::Colon, "':'");
        let value = self.expect_identifier().unwrap_or_default();
        self.expect_punct(Punctuation::Semicolon, "';'");
        let kind = match name.as_str() {
            "mode" => ScopeKind::Mode,
            "memory" => ScopeKind::Memory,
            _ => {
                self.error_current(
                    INVALID,
                    format!("Unknown scope annotation '@{name}'."),
                    "Use @mode:... or @memory:....",
                );
                ScopeKind::Mode
            }
        };

        ScopeAnnotation {
            kind,
            value,
            span: self.span_from(start),
        }
    }

    fn parse_block(&mut self) -> Block {
        self.depth += 1;
        if self.depth > MAX_NESTING_DEPTH {
            self.nesting_limit_error("Block nesting exceeds the supported depth.");
            // Consume the opening brace so the caller always makes progress.
            self.eat_punct(Punctuation::LBrace);
            let span = self.peek().span;
            self.depth -= 1;
            return Block {
                statements: Vec::new(),
                span,
            };
        }
        let block = self.parse_block_inner();
        self.depth -= 1;
        block
    }

    fn parse_block_inner(&mut self) -> Block {
        let start = self
            .expect_punct(Punctuation::LBrace, "'{'")
            .map_or_else(|| self.peek().span.start, |token| token.span.start);
        let mut statements = Vec::new();

        self.skip_separators();
        self.parse_block_statements(&mut statements);

        self.expect_punct(Punctuation::RBrace, "'}'");

        Block {
            statements,
            span: self.span_from(start),
        }
    }

    /// Parses block statements up to (but not including) the closing `}`.
    fn parse_block_statements(&mut self, statements: &mut Vec<Stmt>) {
        while !self.at_punct(Punctuation::RBrace) && !self.at_eof() {
            let before = self.index;
            if self.looks_like_scope_annotation() {
                let scoped = self.parse_scoped_statements();
                statements.push(Stmt {
                    kind: StmtKind::Scoped(scoped.clone()),
                    span: scoped.span,
                });
            } else if let Some(statement) = self.parse_statement() {
                statements.push(statement);
            }
            self.finish_boundary(true);
            if self.index == before {
                self.bump();
            }
        }
    }

    fn parse_statement(&mut self) -> Option<Stmt> {
        let start = self.peek().span.start;

        if self.at_canonical_scope() {
            let scoped = self.parse_canonical_scoped_statements();
            return Some(Stmt {
                kind: StmtKind::Scoped(scoped.clone()),
                span: scoped.span,
            });
        }

        if self.at_keyword(Keyword::Let) {
            return self.parse_let_statement();
        }

        if self.eat_keyword(Keyword::Return) {
            let value = if self.at(TokenKind::Newline)
                || self.at_punct(Punctuation::Semicolon)
                || self.at_punct(Punctuation::RBrace)
                || self.at_eof()
            {
                None
            } else {
                if self.edition == Edition::V0_2 && self.at_keyword(Keyword::Fail) {
                    self.error(
                        INVALID_CONTROL_FLOW,
                        "'return fail' is redundant in edition 0.2.",
                        self.peek().span,
                        "Remove `return`; `fail` already diverges.",
                    );
                }
                Some(self.parse_expression())
            };
            return Some(Stmt {
                kind: StmtKind::Return { value },
                span: self.span_from(start),
            });
        }

        if self.eat_keyword(Keyword::Break) {
            return Some(Stmt {
                kind: StmtKind::Break,
                span: self.span_from(start),
            });
        }

        if self.eat_keyword(Keyword::Continue) {
            return Some(Stmt {
                kind: StmtKind::Continue,
                span: self.span_from(start),
            });
        }

        if self.eat_keyword(Keyword::Defer) {
            return self.parse_statement().map(|statement| Stmt {
                kind: StmtKind::Defer(Box::new(statement)),
                span: self.span_from(start),
            });
        }

        if self.eat_keyword(Keyword::While) {
            if self.edition == Edition::V0_2 && self.at_keyword(Keyword::Let) {
                self.bump();
                let pattern = self.parse_pattern();
                self.expect_punct(Punctuation::Eq, "'='");
                let value = self.parse_expression();
                let body = self.parse_block();
                return Some(self.desugar_while_let(pattern, value, body, start));
            }
            let condition = self.parse_expression();
            let body = self.parse_block();
            return Some(Stmt {
                kind: StmtKind::While { condition, body },
                span: self.span_from(start),
            });
        }

        if self.eat_keyword(Keyword::For) {
            let binding = self.expect_identifier().unwrap_or_default();
            self.expect_keyword(Keyword::In, "'in'");
            let iterable = self.parse_expression();
            let body = self.parse_block();
            return Some(Stmt {
                kind: StmtKind::For {
                    binding,
                    iterable,
                    body,
                },
                span: self.span_from(start),
            });
        }

        if self.at(TokenKind::Newline)
            || self.at_punct(Punctuation::Semicolon)
            || self.at_punct(Punctuation::RBrace)
            || self.at_eof()
        {
            return None;
        }

        let expression = self.parse_expression();
        Some(Stmt {
            kind: StmtKind::Expr(expression),
            span: self.span_from(start),
        })
    }

    fn parse_let_statement(&mut self) -> Option<Stmt> {
        let start = self.bump().span.start;
        let mutable = self.eat_keyword(Keyword::Mut);
        let name = self.expect_identifier().unwrap_or_default();
        let ty = if self.eat_punct(Punctuation::Colon) {
            Some(self.parse_type())
        } else {
            None
        };

        if !self.eat_punct(Punctuation::Eq) {
            self.error_current(
                EXPECTED,
                "Expected '=' in let binding.",
                "Use 'let name = value' or 'let name: Type = value'.",
            );
            return None;
        }

        let value = self.parse_expression();

        Some(Stmt {
            kind: StmtKind::Let {
                mutable,
                name,
                ty,
                value,
            },
            span: self.span_from(start),
        })
    }

    fn parse_expression(&mut self) -> Expr {
        self.depth += 1;
        self.lambda_depth += 1;
        if self.depth > MAX_NESTING_DEPTH {
            self.lambda_depth -= 1;
            return self.nesting_limit_expression();
        }
        let depth = self.lambda_depth;
        let expression = self.parse_assignment();
        self.depth -= 1;
        self.lambda_depth -= 1;
        if depth == 1 && self.pending_lambda.is_some() {
            let lambda = self.pending_lambda.take().expect("pending lambda");
            self.error(
                EXPECTED,
                "A lambda must be invoked immediately.",
                lambda.span,
                "Write (fn(x) => body)(argument).",
            );
            return Expr {
                kind: ExprKind::Literal(Literal::Unit),
                span: lambda.span,
            };
        }
        expression
    }

    /// Bails out of a recursion deeper than `MAX_NESTING_DEPTH`: reports the
    /// limit once (`NS2002`), consumes a token so the caller always makes
    /// progress, and returns the same placeholder the "expected an expression"
    /// path produces.
    fn nesting_limit_expression(&mut self) -> Expr {
        let span = self.peek().span;
        self.nesting_limit_error("Expression nesting exceeds the supported depth.");
        if self.can_consume_for_progress() {
            self.bump();
        }
        self.depth -= 1;
        Expr {
            kind: ExprKind::Literal(Literal::Unit),
            span,
        }
    }

    /// Whether the parser may consume the current token to guarantee progress
    /// on an error path (stopper tokens stay put for the caller to handle).
    fn can_consume_for_progress(&self) -> bool {
        !self.at_eof()
            && !self.at(TokenKind::Newline)
            && !self.at_punct(Punctuation::Semicolon)
            && !self.at_punct(Punctuation::RBrace)
            && !self.at_punct(Punctuation::RParen)
            && !self.at_punct(Punctuation::RBracket)
            && !self.at_punct(Punctuation::Comma)
    }

    /// Reports the nesting limit once per parse and skips the rest of the
    /// input. Draining matters: without it the unwinding frames would keep
    /// parsing (and boxing) the remaining tokens into an ever deeper tree,
    /// and dropping that tree would overflow the stack again.
    fn nesting_limit_error(&mut self, message: &str) {
        if self.depth_reported {
            return;
        }
        self.depth_reported = true;
        // Leave the final EOF token in place for callers that expect it.
        self.index = self.tokens.len().saturating_sub(1);
        self.nesting_drained = true;
        let span = self.peek().span;
        self.error(
            INVALID,
            message,
            span,
            "Simplify the expression; the parser supports at most 64 levels of nesting.",
        );
    }

    fn parse_assignment(&mut self) -> Expr {
        let left = self.parse_range();

        if self.eat_punct(Punctuation::Eq) {
            self.skip_continuation_newlines();
            let value = self.parse_assignment();
            let span = Span::new(left.span.start, value.span.end);
            return Expr {
                kind: ExprKind::Assign {
                    target: Box::new(left),
                    value: Box::new(value),
                },
                span,
            };
        }

        left
    }

    /// Ranges bind looser than logical operators and tighter than assignment.
    fn parse_range(&mut self) -> Expr {
        let start = self.parse_logical_or();
        if self.edition != Edition::V0_2 {
            return start;
        }
        let inclusive = if self.eat_punct(Punctuation::DotDotEq) {
            true
        } else if self.eat_punct(Punctuation::DotDot) {
            false
        } else {
            return start;
        };
        self.skip_continuation_newlines();
        let end = self.parse_logical_or();
        let span = Span::new(start.span.start, end.span.end);
        Expr {
            kind: ExprKind::Range {
                start: Box::new(start),
                end: Box::new(end),
                inclusive,
            },
            span,
        }
    }

    fn parse_logical_or(&mut self) -> Expr {
        let mut expression = self.parse_logical_and();

        while self.eat_punct(Punctuation::PipePipe) {
            self.skip_continuation_newlines();
            let right = self.parse_logical_and();
            let span = Span::new(expression.span.start, right.span.end);
            expression = Expr {
                kind: ExprKind::Binary {
                    operator: BinaryOp::Or,
                    left: Box::new(expression),
                    right: Box::new(right),
                },
                span,
            };
        }

        expression
    }

    fn parse_logical_and(&mut self) -> Expr {
        let mut expression = self.parse_equality();

        while self.eat_punct(Punctuation::AmpAmp) {
            self.skip_continuation_newlines();
            let right = self.parse_equality();
            let span = Span::new(expression.span.start, right.span.end);
            expression = Expr {
                kind: ExprKind::Binary {
                    operator: BinaryOp::And,
                    left: Box::new(expression),
                    right: Box::new(right),
                },
                span,
            };
        }

        expression
    }

    /// `|` is bitwise-or here; error sets consume `|` in their own parser
    /// before expression parsing begins.
    fn parse_bit_or(&mut self) -> Expr {
        let mut expression = self.parse_bit_xor();

        while self.edition == Edition::V0_2 && self.eat_punct(Punctuation::Pipe) {
            self.skip_continuation_newlines();
            let right = self.parse_bit_xor();
            let span = Span::new(expression.span.start, right.span.end);
            expression = Expr {
                kind: ExprKind::Binary {
                    operator: BinaryOp::BitOr,
                    left: Box::new(expression),
                    right: Box::new(right),
                },
                span,
            };
        }

        expression
    }

    fn parse_bit_xor(&mut self) -> Expr {
        let mut expression = self.parse_bit_and();

        while self.edition == Edition::V0_2 && self.eat_punct(Punctuation::Caret) {
            self.skip_continuation_newlines();
            let right = self.parse_bit_and();
            let span = Span::new(expression.span.start, right.span.end);
            expression = Expr {
                kind: ExprKind::Binary {
                    operator: BinaryOp::BitXor,
                    left: Box::new(expression),
                    right: Box::new(right),
                },
                span,
            };
        }

        expression
    }

    fn parse_bit_and(&mut self) -> Expr {
        let mut expression = self.parse_shift();

        while self.edition == Edition::V0_2 && self.eat_punct(Punctuation::Amp) {
            self.skip_continuation_newlines();
            let right = self.parse_shift();
            let span = Span::new(expression.span.start, right.span.end);
            expression = Expr {
                kind: ExprKind::Binary {
                    operator: BinaryOp::BitAnd,
                    left: Box::new(expression),
                    right: Box::new(right),
                },
                span,
            };
        }

        expression
    }

    /// Shifts are recognized as two adjacent `<` / `>` tokens so closing
    /// nested generic arguments (`Option<Result<i32, str>>`) still parse.
    fn parse_shift(&mut self) -> Expr {
        let mut expression = self.parse_additive();

        loop {
            let operator = if self.edition != Edition::V0_2 {
                None
            } else {
                let current = self.peek();
                let next = self.peek_at(1);
                let adjacent = next.is_some_and(|token| token.span.start == current.span.end);
                if adjacent
                    && current.kind == TokenKind::Punct(Punctuation::Lt)
                    && next.is_some_and(|token| token.kind == TokenKind::Punct(Punctuation::Lt))
                {
                    Some(BinaryOp::Shl)
                } else if adjacent
                    && current.kind == TokenKind::Punct(Punctuation::Gt)
                    && next.is_some_and(|token| token.kind == TokenKind::Punct(Punctuation::Gt))
                {
                    Some(BinaryOp::Shr)
                } else {
                    None
                }
            };
            let Some(operator) = operator else {
                break;
            };
            self.bump();
            self.bump();
            self.skip_continuation_newlines();
            let right = self.parse_additive();
            let span = Span::new(expression.span.start, right.span.end);
            expression = Expr {
                kind: ExprKind::Binary {
                    operator,
                    left: Box::new(expression),
                    right: Box::new(right),
                },
                span,
            };
        }

        expression
    }

    fn parse_equality(&mut self) -> Expr {
        let mut expression = self.parse_comparison();

        loop {
            let operator = if self.eat_punct(Punctuation::EqEq) {
                Some(BinaryOp::Equal)
            } else if self.eat_punct(Punctuation::BangEq) {
                Some(BinaryOp::NotEqual)
            } else {
                None
            };

            let Some(operator) = operator else {
                break;
            };
            self.skip_continuation_newlines();

            let right = self.parse_comparison();
            let span = Span::new(expression.span.start, right.span.end);
            expression = Expr {
                kind: ExprKind::Binary {
                    operator,
                    left: Box::new(expression),
                    right: Box::new(right),
                },
                span,
            };
        }

        expression
    }

    fn parse_comparison(&mut self) -> Expr {
        let mut expression = self.parse_bit_or();

        loop {
            let operator = if self.eat_punct(Punctuation::Lt) {
                Some(BinaryOp::Less)
            } else if self.eat_punct(Punctuation::LtEq) {
                Some(BinaryOp::LessEqual)
            } else if self.eat_punct(Punctuation::Gt) {
                Some(BinaryOp::Greater)
            } else if self.eat_punct(Punctuation::GtEq) {
                Some(BinaryOp::GreaterEqual)
            } else {
                None
            };

            let Some(operator) = operator else {
                break;
            };
            self.skip_continuation_newlines();

            let right = self.parse_bit_or();
            let span = Span::new(expression.span.start, right.span.end);
            expression = Expr {
                kind: ExprKind::Binary {
                    operator,
                    left: Box::new(expression),
                    right: Box::new(right),
                },
                span,
            };
        }

        expression
    }

    fn parse_additive(&mut self) -> Expr {
        let mut expression = self.parse_multiplicative();

        loop {
            let operator = if self.eat_punct(Punctuation::Plus) {
                Some(BinaryOp::Add)
            } else if self.eat_punct(Punctuation::Minus) {
                Some(BinaryOp::Subtract)
            } else if self.eat_punct(Punctuation::PlusWrap) {
                Some(BinaryOp::AddWrap)
            } else if self.eat_punct(Punctuation::MinusWrap) {
                Some(BinaryOp::SubtractWrap)
            } else {
                None
            };

            let Some(operator) = operator else {
                break;
            };
            self.skip_continuation_newlines();

            let right = self.parse_multiplicative();
            let span = Span::new(expression.span.start, right.span.end);
            expression = Expr {
                kind: ExprKind::Binary {
                    operator,
                    left: Box::new(expression),
                    right: Box::new(right),
                },
                span,
            };
        }

        expression
    }

    fn parse_multiplicative(&mut self) -> Expr {
        let mut expression = self.parse_unary();

        loop {
            let operator = if self.eat_punct(Punctuation::Star) {
                Some(BinaryOp::Multiply)
            } else if self.eat_punct(Punctuation::Slash) {
                Some(BinaryOp::Divide)
            } else if self.eat_punct(Punctuation::Percent) {
                Some(BinaryOp::Remainder)
            } else if self.eat_punct(Punctuation::StarWrap) {
                Some(BinaryOp::MultiplyWrap)
            } else if self.eat_punct(Punctuation::SlashWrap) {
                Some(BinaryOp::DivideWrap)
            } else {
                None
            };

            let Some(operator) = operator else {
                break;
            };
            self.skip_continuation_newlines();

            let right = self.parse_unary();
            let span = Span::new(expression.span.start, right.span.end);
            expression = Expr {
                kind: ExprKind::Binary {
                    operator,
                    left: Box::new(expression),
                    right: Box::new(right),
                },
                span,
            };
        }

        expression
    }

    fn parse_unary(&mut self) -> Expr {
        let start = self.peek().span.start;

        let operator = if self.eat_punct(Punctuation::Minus) {
            Some(UnaryOp::Negate)
        } else if self.eat_keyword(Keyword::Not) {
            Some(UnaryOp::Not)
        } else {
            None
        };

        if let Some(operator) = operator {
            let operand = self.parse_unary();
            return Expr {
                kind: ExprKind::Unary {
                    operator,
                    operand: Box::new(operand),
                },
                span: self.span_from(start),
            };
        }

        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Expr {
        let mut expression = self.parse_primary();

        if let Some(lambda) = self.pending_lambda.take() {
            if self.eat_punct(Punctuation::LParen) {
                let arguments = self.parse_expression_list_after_open();
                let span = Span::new(lambda.span.start, self.last_end());
                expression = self.lower_lambda_call(lambda, arguments, span);
            } else {
                // Keep the pending lambda: it may be wrapped in parentheses,
                // with the call appearing after the closing parenthesis.
                self.pending_lambda = Some(lambda);
            }
        }

        loop {
            if self.edition == Edition::V0_2
                && self.at(TokenKind::Newline)
                && self.next_non_newline_is_punct(Punctuation::Dot)
            {
                self.skip_newlines();
            }

            if self.eat_punct(Punctuation::Dot) {
                let member = if self.at(TokenKind::Identifier) || self.at(TokenKind::Integer) {
                    self.bump().text
                } else {
                    self.error_current(
                        EXPECTED,
                        "Expected a field name or tuple index after '.'.",
                        "Use .name or .0.",
                    );
                    String::new()
                };

                let span = Span::new(expression.span.start, self.last_end());
                expression = Expr {
                    kind: ExprKind::Member {
                        target: Box::new(expression),
                        member,
                    },
                    span,
                };
                continue;
            }

            if self.eat_punct(Punctuation::LParen) {
                let arguments = self.parse_expression_list_after_open();
                let span = Span::new(expression.span.start, self.last_end());
                expression = Expr {
                    kind: ExprKind::Call {
                        callee: Box::new(expression),
                        arguments,
                    },
                    span,
                };
                continue;
            }

            if self.eat_punct(Punctuation::LBracket) {
                let index = self.parse_expression();
                self.expect_punct(Punctuation::RBracket, "']'");
                let span = Span::new(expression.span.start, self.last_end());
                if self.edition == Edition::V0_2 && matches!(index.kind, ExprKind::Range { .. }) {
                    let index_span = index.span;
                    let ExprKind::Range {
                        start,
                        end,
                        inclusive,
                    } = index.kind
                    else {
                        unreachable!("range was checked above");
                    };
                    // Checker annotations are keyed by span. Every synthetic
                    // node therefore needs a distinct span; reusing the whole
                    // index span would overwrite the receiver/argument types.
                    let callee = Expr {
                        kind: ExprKind::Member {
                            target: Box::new(expression),
                            member: "slice".to_owned(),
                        },
                        span: index_span,
                    };
                    let end = if inclusive {
                        let end_span = end.span;
                        Expr {
                            kind: ExprKind::Binary {
                                operator: BinaryOp::Add,
                                left: end,
                                right: Box::new(Expr {
                                    kind: ExprKind::Literal(Literal::Integer("1".to_owned())),
                                    span: Span::new(end_span.end, end_span.end),
                                }),
                            },
                            span: end_span,
                        }
                    } else {
                        *end
                    };
                    expression = Expr {
                        kind: ExprKind::Call {
                            callee: Box::new(callee),
                            arguments: vec![*start, end],
                        },
                        span,
                    };
                } else {
                    expression = Expr {
                        kind: ExprKind::Index {
                            target: Box::new(expression),
                            index: Box::new(index),
                        },
                        span,
                    };
                }
                continue;
            }

            if self.eat_punct(Punctuation::Bang) {
                let span = Span::new(expression.span.start, self.last_end());
                if self.edition == Edition::V0_2 {
                    self.error(
                        DEPRECATED_SYNTAX,
                        "Postfix `!` is not allowed in edition 0.2.",
                        span,
                        "Use `?` to propagate a failure.",
                    );
                }
                expression = Expr {
                    kind: ExprKind::Propagate(Box::new(expression)),
                    span,
                };
                continue;
            }

            if self.edition == Edition::V0_2 && self.eat_punct(Punctuation::Question) {
                let span = Span::new(expression.span.start, self.last_end());
                expression = Expr {
                    kind: ExprKind::Propagate(Box::new(expression)),
                    span,
                };
                continue;
            }

            if self.edition == Edition::V0_2 && self.at_punct(Punctuation::DoubleColon) {
                let start = expression.span.start;
                let turbofish_span = self.peek().span;
                if matches!(&expression.kind, ExprKind::TypeArguments { .. }) {
                    self.error(
                        MALFORMED_TURBOFISH,
                        "Only one turbofish is allowed in a postfix chain.",
                        turbofish_span,
                        "Remove the second turbofish.",
                    );
                    self.bump();
                    let _ = self.parse_type_arguments();
                    continue;
                }

                self.bump();
                let arguments = self.parse_type_arguments();
                let span = Span::new(start, self.last_end());
                let target_is_path = matches!(
                    &expression.kind,
                    ExprKind::Identifier(_) | ExprKind::Member { .. }
                );
                let followed_by_call = self.at_punct(Punctuation::LParen);
                let followed_by_struct_init =
                    self.at_punct(Punctuation::LBrace) && self.looks_like_struct_init();
                if !target_is_path {
                    self.error(
                        MALFORMED_TURBOFISH,
                        "A turbofish must follow a path.",
                        span,
                        "Use path::<...>(...) or path::<...> { ... }.",
                    );
                } else if !followed_by_call
                    && !followed_by_struct_init
                    && matches!(&expression.kind, ExprKind::Member { .. })
                {
                    self.error(
                        BARE_TYPE_ARGUMENTS,
                        "A qualified value path cannot carry expression-position type arguments.",
                        span,
                        "Use the bare variant name, such as None::<i32>.",
                    );
                }
                expression = Expr {
                    kind: ExprKind::TypeArguments {
                        target: Box::new(expression),
                        arguments,
                    },
                    span,
                };
                continue;
            }

            if self.edition == Edition::V0_2 && self.at_punct(Punctuation::Lt) {
                let lt_span = self.peek().span;
                if let Some(arguments) = self.try_type_arguments_for_call() {
                    self.error(
                        BARE_TYPE_ARGUMENTS,
                        "Expression-position type arguments require `::<...>` in edition 0.2.",
                        lt_span,
                        "Write path::<T>(value) instead of path<T>(value).",
                    );
                    let span = Span::new(expression.span.start, self.last_end());
                    expression = Expr {
                        kind: ExprKind::TypeArguments {
                            target: Box::new(expression),
                            arguments,
                        },
                        span,
                    };
                    continue;
                }
            }

            if self.edition == Edition::V0_1
                && self.at_punct(Punctuation::Lt)
                && let Some(arguments) = self.try_type_arguments_for_call()
            {
                let span = Span::new(expression.span.start, self.last_end());
                expression = Expr {
                    kind: ExprKind::TypeArguments {
                        target: Box::new(expression),
                        arguments,
                    },
                    span,
                };
                continue;
            }

            if self.at_punct(Punctuation::LBrace)
                && self.looks_like_struct_init()
                && let Some(path) = expression_to_type_path(&expression)
            {
                let fields = self.parse_struct_init_fields();
                let span = Span::new(expression.span.start, self.last_end());
                expression = Expr {
                    kind: ExprKind::StructInit { path, fields },
                    span,
                };
                continue;
            }

            break;
        }

        expression
    }

    /// Parses `fn(x: T, y) => body` and records it for `parse_postfix`, which
    /// requires an immediately following call and lowers the pair to a block.
    fn parse_lambda(&mut self) -> Expr {
        let start = self.bump().span.start;
        self.expect_punct(Punctuation::LParen, "'('");
        let mut parameters = Vec::new();
        if !self.at_punct(Punctuation::RParen) {
            loop {
                let token = self.peek().clone();
                let name = if self.at(TokenKind::Identifier) {
                    self.bump().text
                } else {
                    self.error_current(
                        EXPECTED,
                        "Expected a lambda parameter name.",
                        "Write fn(name) => expression.",
                    );
                    String::new()
                };
                let ty = if self.eat_punct(Punctuation::Colon) {
                    Some(self.parse_type())
                } else {
                    None
                };
                parameters.push(LambdaParameter {
                    name,
                    ty,
                    span: token.span,
                });
                if !self.eat_punct(Punctuation::Comma) {
                    break;
                }
                self.skip_newlines();
            }
        }
        self.expect_punct(Punctuation::RParen, "')'");
        self.expect_punct(Punctuation::FatArrow, "'=>'");
        self.skip_continuation_newlines();
        let body = Box::new(self.parse_assignment());
        let span = Span::new(start, body.span.end);
        self.pending_lambda = Some(PendingLambda {
            parameters,
            body,
            span,
        });
        Expr {
            kind: ExprKind::Identifier("__lambda".to_owned()),
            span,
        }
    }

    /// Lowers `(fn(x, y) => body)(a, b)` to `{ let x = a; let y = b; body }`.
    fn lower_lambda_call(
        &mut self,
        lambda: PendingLambda,
        arguments: Vec<Expr>,
        span: Span,
    ) -> Expr {
        let mut statements = Vec::new();
        let body = lambda.body;
        let body_span = body.span;
        let arity = lambda.parameters.len();
        if arguments.len() != arity {
            self.error(
                EXPECTED,
                format!(
                    "Lambda expects {arity} argument(s), but {} were provided.",
                    arguments.len()
                ),
                span,
                "Match the lambda parameter list.",
            );
        }
        for (index, parameter) in lambda.parameters.into_iter().enumerate() {
            let value = arguments.get(index).cloned().unwrap_or(Expr {
                kind: ExprKind::Literal(Literal::Unit),
                span: parameter.span,
            });
            statements.push(Stmt {
                kind: StmtKind::Let {
                    mutable: false,
                    name: parameter.name,
                    ty: parameter.ty,
                    value,
                },
                span: parameter.span,
            });
        }
        statements.push(Stmt {
            kind: StmtKind::Expr(*body),
            span: body_span,
        });
        Expr {
            kind: ExprKind::Block(Block { statements, span }),
            span,
        }
    }

    fn parse_primary(&mut self) -> Expr {
        let token = self.peek().clone();
        let start = token.span.start;

        match token.kind {
            TokenKind::Integer => {
                self.bump();
                Expr {
                    kind: ExprKind::Literal(Literal::Integer(token.text)),
                    span: token.span,
                }
            }
            TokenKind::Float => {
                self.bump();
                Expr {
                    kind: ExprKind::Literal(Literal::Float(token.text)),
                    span: token.span,
                }
            }
            TokenKind::String => {
                self.bump();
                let value = match decode_string_literal(&token.text) {
                    Ok(value) => value,
                    Err(message) => {
                        self.error(INVALID, message, token.span, "Fix the string escape.");
                        String::new()
                    }
                };
                Expr {
                    kind: ExprKind::Literal(Literal::String(value)),
                    span: token.span,
                }
            }
            TokenKind::ByteString => {
                self.bump();
                let value = match decode_byte_literal(&token.text) {
                    Ok(value) => value,
                    Err(message) => {
                        self.error(INVALID, message, token.span, "Fix the byte escape.");
                        Vec::new()
                    }
                };
                Expr {
                    kind: ExprKind::Literal(Literal::Bytes(value)),
                    span: token.span,
                }
            }
            TokenKind::Character => {
                self.bump();
                let value = match decode_char_literal(&token.text) {
                    Ok(value) => value,
                    Err(message) => {
                        self.error(INVALID, message, token.span, "Fix the character literal.");
                        '\0'
                    }
                };
                Expr {
                    kind: ExprKind::Literal(Literal::Character(value)),
                    span: token.span,
                }
            }
            TokenKind::Keyword(Keyword::True) => {
                self.bump();
                Expr {
                    kind: ExprKind::Literal(Literal::Bool(true)),
                    span: token.span,
                }
            }
            TokenKind::Keyword(Keyword::False) => {
                self.bump();
                Expr {
                    kind: ExprKind::Literal(Literal::Bool(false)),
                    span: token.span,
                }
            }
            TokenKind::Identifier => {
                self.bump();
                Expr {
                    kind: ExprKind::Identifier(token.text),
                    span: token.span,
                }
            }
            TokenKind::Punct(Punctuation::LParen) => self.parse_parenthesized(),
            TokenKind::Punct(Punctuation::LBracket) => self.parse_array_literal(),
            TokenKind::Punct(Punctuation::LBrace) => self.parse_brace_expression(),
            TokenKind::Punct(Punctuation::Bang) => {
                if self.edition == Edition::V0_2 {
                    self.error(
                        DEPRECATED_SYNTAX,
                        "Prefix `!` failure construction is not allowed in edition 0.2.",
                        token.span,
                        "Use `fail Error(...)` or `fail Error { field: value }`.",
                    );
                }
                self.parse_fail_expression()
            }
            TokenKind::Keyword(Keyword::Fail) => self.parse_fail_keyword_expression(),
            TokenKind::Keyword(Keyword::If) => self.parse_if_expression(),
            TokenKind::Keyword(Keyword::Match) => self.parse_match_expression(),
            TokenKind::Keyword(Keyword::Spawn) => self.parse_spawn_expression(),
            TokenKind::Keyword(Keyword::Panic) => {
                self.parse_message_expression(|message| ExprKind::Panic { message })
            }
            TokenKind::Keyword(Keyword::Abort) => {
                self.parse_message_expression(|message| ExprKind::Abort { message })
            }
            TokenKind::Keyword(Keyword::Fn) if self.edition == Edition::V0_2 => self.parse_lambda(),
            _ => {
                let (message, hint) =
                    if self.edition == Edition::V0_2 && is_binary_operator_token(&token.kind) {
                        (
                            "A binary operator cannot start an expression.",
                            "Add a left-hand operand before the operator.",
                        )
                    } else {
                        (
                            "Expected an expression.",
                            "Add a literal, identifier, call, block, if, or match expression.",
                        )
                    };
                self.error(INVALID, message, token.span, hint);
                if !self.at_eof()
                    && !self.at(TokenKind::Newline)
                    && !self.at_punct(Punctuation::Semicolon)
                    && !self.at_punct(Punctuation::RBrace)
                    && !self.at_punct(Punctuation::RParen)
                    && !self.at_punct(Punctuation::RBracket)
                {
                    self.bump();
                }
                Expr {
                    kind: ExprKind::Literal(Literal::Unit),
                    span: Span::new(start, self.last_end()),
                }
            }
        }
    }

    fn parse_parenthesized(&mut self) -> Expr {
        let start = self.bump().span.start;

        if self.eat_punct(Punctuation::RParen) {
            return Expr {
                kind: ExprKind::Literal(Literal::Unit),
                span: self.span_from(start),
            };
        }

        let first = self.parse_expression();
        if matches!(&first.kind, ExprKind::Identifier(name) if name == "__lambda")
            && let Some(lambda) = self.pending_lambda.as_mut()
        {
            // The call belongs to the parenthesized lambda, so extend its span
            // to the opening parenthesis for the lowered block.
            lambda.span = Span::new(start, lambda.span.end);
        }
        self.skip_newlines();

        if self.eat_punct(Punctuation::Comma) {
            let mut elements = vec![first];
            loop {
                self.skip_newlines();
                if self.edition == Edition::V0_2 && self.at_punct(Punctuation::RParen) {
                    break;
                }
                elements.push(self.parse_expression());
                self.skip_newlines();
                if self.eat_punct(Punctuation::Comma) {
                    continue;
                }
                break;
            }
            self.skip_newlines();
            self.expect_punct(Punctuation::RParen, "')'");
            return Expr {
                kind: ExprKind::Tuple(elements),
                span: self.span_from(start),
            };
        }

        self.expect_punct(Punctuation::RParen, "')'");
        let mut expression = first;
        expression.span = self.span_from(start);
        expression
    }

    fn parse_array_literal(&mut self) -> Expr {
        let start = self.bump().span.start;
        let mut elements = Vec::new();
        self.skip_newlines();

        if !self.at_punct(Punctuation::RBracket) {
            loop {
                elements.push(self.parse_expression());
                if self.eat_punct(Punctuation::Comma) {
                    self.skip_newlines();
                    if self.at_punct(Punctuation::RBracket) {
                        break;
                    }
                    continue;
                }
                break;
            }
        }

        self.skip_newlines();
        self.expect_punct(Punctuation::RBracket, "']'");
        Expr {
            kind: ExprKind::Array(elements),
            span: self.span_from(start),
        }
    }

    fn parse_brace_expression(&mut self) -> Expr {
        let saved_index = self.index;
        let saved_diagnostics = self.diagnostics.len();
        let start = self.bump().span.start;
        self.skip_newlines();

        if self.at_punct(Punctuation::RBrace) {
            if !self.nesting_drained {
                self.index = saved_index;
            }
            self.diagnostics.truncate(saved_diagnostics);
            return Expr {
                kind: ExprKind::Block(self.parse_block()),
                span: self.span_from(start),
            };
        }

        let first = self.parse_expression();

        if self.eat_punct(Punctuation::Colon) {
            let mut entries = Vec::new();
            let first_value = self.parse_expression();
            entries.push(MapEntry {
                key: first,
                value: first_value,
                span: self.span_from(start),
            });

            loop {
                self.skip_newlines();
                if self.eat_punct(Punctuation::Comma) {
                    self.skip_newlines();
                    if self.at_punct(Punctuation::RBrace) {
                        break;
                    }
                    let key_start = self.peek().span.start;
                    let key = self.parse_expression();
                    self.expect_punct(Punctuation::Colon, "':'");
                    let value = self.parse_expression();
                    entries.push(MapEntry {
                        key,
                        value,
                        span: self.span_from(key_start),
                    });
                    continue;
                }
                break;
            }

            self.expect_punct(Punctuation::RBrace, "'}'");
            return Expr {
                kind: ExprKind::Map(entries),
                span: self.span_from(start),
            };
        }

        // Clean speculation with no map colon: `first` is exactly what the
        // block's first statement parses to (`parse_statement` falls through
        // to `parse_expression` for non-keyword statements), so keep it
        // instead of restoring and re-parsing the same tokens. Restoring here
        // re-parsed the whole subtree a second time at every nesting level,
        // which made nested blocks exponential in depth (28 levels parsed for
        // minutes). Dirty speculations (keyword-led or malformed first
        // statements, scope annotations) still restore; they fail on their
        // first token, so nesting stays linear.
        if self.diagnostics.len() == saved_diagnostics {
            let statement_start = first.span.start;
            let statement = Stmt {
                kind: StmtKind::Expr(first),
                span: self.span_from(statement_start),
            };
            let mut statements = vec![statement];
            self.finish_boundary(true);
            self.parse_block_statements(&mut statements);
            self.expect_punct(Punctuation::RBrace, "'}'");

            return Expr {
                kind: ExprKind::Block(Block {
                    statements,
                    span: self.span_from(start),
                }),
                span: self.span_from(start),
            };
        }

        if !self.nesting_drained {
            self.index = saved_index;
        }
        self.diagnostics.truncate(saved_diagnostics);
        Expr {
            kind: ExprKind::Block(self.parse_block()),
            span: self.span_from(start),
        }
    }

    fn parse_if_expression(&mut self) -> Expr {
        let start = self.bump().span.start;

        // Edition 0.2 adds `if let PATTERN = EXPR`. It is desugared to the
        // existing match form so every backend inherits match semantics.
        if self.edition == Edition::V0_2 && self.at_keyword(Keyword::Let) {
            self.bump();
            let pattern = self.parse_pattern();
            self.expect_punct(Punctuation::Eq, "'='");
            let value = self.parse_expression();
            let then_branch = self.parse_block();
            let else_branch = if self.eat_keyword(Keyword::Else) {
                if self.at_keyword(Keyword::If) {
                    Some(ElseBranch::If(Box::new(self.parse_if_expression())))
                } else {
                    Some(ElseBranch::Block(self.parse_block()))
                }
            } else {
                None
            };
            return self.desugar_if_let(pattern, value, then_branch, else_branch, start);
        }

        let condition = self.parse_expression();
        if self.edition == Edition::V0_2 && expression_has_unparenthesized_struct_init(&condition) {
            self.error(
                EXPECTED,
                "Wrap a struct initializer in parentheses in a condition.",
                condition.span,
                "Write (User { field: value }) instead of User { field: value }.",
            );
        }
        let then_branch = self.parse_block();
        let else_branch = if self.eat_keyword(Keyword::Else) {
            if self.at_keyword(Keyword::If) {
                Some(ElseBranch::If(Box::new(self.parse_if_expression())))
            } else {
                Some(ElseBranch::Block(self.parse_block()))
            }
        } else {
            None
        };

        Expr {
            kind: ExprKind::If {
                condition: Box::new(condition),
                then_branch,
                else_branch,
            },
            span: self.span_from(start),
        }
    }

    /// Rewrites `if let P = V { T } else { E }` to `match V { P => T, _ => E }`.
    fn desugar_if_let(
        &self,
        pattern: Pattern,
        value: Expr,
        then_branch: Block,
        else_branch: Option<ElseBranch>,
        start: usize,
    ) -> Expr {
        let span = self.span_from(start);
        let else_body = match else_branch {
            Some(ElseBranch::Block(block)) => MatchArmBody::Block(block),
            Some(ElseBranch::If(expression)) => MatchArmBody::Expr(*expression),
            None => MatchArmBody::Expr(Expr {
                kind: ExprKind::Literal(Literal::Unit),
                span,
            }),
        };
        let arms = vec![
            MatchArm {
                pattern,
                body: MatchArmBody::Block(then_branch),
                span,
            },
            MatchArm {
                pattern: Pattern {
                    kind: PatternKind::Wildcard,
                    span,
                },
                body: else_body,
                span,
            },
        ];
        Expr {
            kind: ExprKind::Match {
                value: Box::new(value),
                arms,
            },
            span,
        }
    }

    /// Rewrites `while let P = V { B }` to a `while true` loop whose body
    /// matches `V`; a non-match breaks out of the loop.
    fn desugar_while_let(&self, pattern: Pattern, value: Expr, body: Block, start: usize) -> Stmt {
        let span = self.span_from(start);
        let match_expression = Expr {
            kind: ExprKind::Match {
                value: Box::new(value),
                arms: vec![
                    MatchArm {
                        pattern,
                        body: MatchArmBody::Block(body),
                        span,
                    },
                    MatchArm {
                        pattern: Pattern {
                            kind: PatternKind::Wildcard,
                            span,
                        },
                        body: MatchArmBody::Block(Block {
                            statements: vec![Stmt {
                                kind: StmtKind::Break,
                                span,
                            }],
                            span,
                        }),
                        span,
                    },
                ],
            },
            span,
        };
        Stmt {
            kind: StmtKind::While {
                condition: Expr {
                    kind: ExprKind::Literal(Literal::Bool(true)),
                    span,
                },
                body: Block {
                    statements: vec![Stmt {
                        kind: StmtKind::Expr(match_expression),
                        span,
                    }],
                    span,
                },
            },
            span,
        }
    }

    fn parse_match_expression(&mut self) -> Expr {
        let start = self.bump().span.start;
        let value = self.parse_expression();
        let mut arms = Vec::new();

        self.expect_punct(Punctuation::LBrace, "'{'");
        self.skip_newlines();

        while !self.at_punct(Punctuation::RBrace) && !self.at_eof() {
            let before = self.index;
            let arm_start = self.peek().span.start;
            let pattern = self.parse_pattern();
            self.expect_punct(Punctuation::FatArrow, "'=>'");
            let body = if self.at_punct(Punctuation::LBrace) {
                MatchArmBody::Block(self.parse_block())
            } else if self.at_keyword(Keyword::Return) {
                // `arm => return value` is a block-bodied arm with a single
                // return statement. The AST has no statement arm variant.
                let statement = self
                    .parse_statement()
                    .expect("a return keyword always parses a statement");
                let span = statement.span;
                MatchArmBody::Block(Block {
                    statements: vec![statement],
                    span,
                })
            } else {
                MatchArmBody::Expr(self.parse_expression())
            };
            let block_body = matches!(body, MatchArmBody::Block(_));
            arms.push(MatchArm {
                pattern,
                body,
                span: self.span_from(arm_start),
            });

            if self.index == before {
                self.bump();
            }
            if self.eat_punct(Punctuation::Comma) {
                self.skip_newlines();
            } else {
                let next = self.skip_newlines_at(self.index);
                let at_close = self
                    .tokens
                    .get(next)
                    .is_some_and(|token| token.kind == TokenKind::Punct(Punctuation::RBrace));
                if self.edition == Edition::V0_2 && !block_body && !at_close && !self.at_eof() {
                    self.error(
                        BOUNDARY,
                        "Expected ',' after a match arm.",
                        self.span_from(arm_start),
                        "Add a comma between match arms, or use a block body to omit it.",
                    );
                }
                self.skip_newlines();
            }
        }

        self.expect_punct(Punctuation::RBrace, "'}'");

        Expr {
            kind: ExprKind::Match {
                value: Box::new(value),
                arms,
            },
            span: self.span_from(start),
        }
    }

    fn parse_spawn_expression(&mut self) -> Expr {
        let start = self.bump().span.start;
        if self.edition == Edition::V0_2 && self.at_punct(Punctuation::LBrace) {
            let body = self.parse_block();
            return Expr {
                kind: ExprKind::Spawn { body },
                span: self.span_from(start),
            };
        }
        self.expect_punct(Punctuation::PipePipe, "'||'");
        let body = self.parse_block();
        Expr {
            kind: ExprKind::Spawn { body },
            span: self.span_from(start),
        }
    }

    fn parse_fail_expression(&mut self) -> Expr {
        let start = self.bump().span.start;
        self.parse_fail_tail(start, "!Error(args) or !Error { field: value }")
    }

    fn parse_fail_keyword_expression(&mut self) -> Expr {
        let start = self.bump().span.start;
        self.parse_fail_tail(start, "fail Error(args) or fail Error { field: value }")
    }

    fn parse_fail_tail(&mut self, start: usize, example: &str) -> Expr {
        let error = self.parse_type_path();

        let arguments = if self.eat_punct(Punctuation::LParen) {
            FailArguments::Tuple(self.parse_expression_list_after_open())
        } else if self.at_punct(Punctuation::LBrace) {
            FailArguments::Struct(self.parse_struct_init_fields())
        } else {
            self.error_current(
                EXPECTED,
                "Expected '(' or '{' after a failure type.",
                format!("Construct a failure with {example}."),
            );
            FailArguments::Tuple(Vec::new())
        };

        Expr {
            kind: ExprKind::Fail { error, arguments },
            span: self.span_from(start),
        }
    }

    fn parse_message_expression(&mut self, build: fn(Box<Expr>) -> ExprKind) -> Expr {
        let start = self.bump().span.start;
        self.expect_punct(Punctuation::LParen, "'('");
        let message = self.parse_expression();
        self.expect_punct(Punctuation::RParen, "')'");
        Expr {
            kind: build(Box::new(message)),
            span: self.span_from(start),
        }
    }

    fn parse_expression_list_after_open(&mut self) -> Vec<Expr> {
        let mut arguments = Vec::new();
        self.skip_newlines();

        if self.at_punct(Punctuation::RParen) {
            self.bump();
            return arguments;
        }

        loop {
            arguments.push(self.parse_expression());
            if self.eat_punct(Punctuation::Comma) {
                self.skip_newlines();
                if self.at_punct(Punctuation::RParen) {
                    break;
                }
                continue;
            }
            break;
        }

        self.skip_newlines();
        self.expect_punct(Punctuation::RParen, "')'");
        arguments
    }

    fn try_type_arguments_for_call(&mut self) -> Option<Vec<Type>> {
        let saved_index = self.index;
        let saved_diagnostics = self.diagnostics.len();
        let arguments = self.parse_type_arguments();

        let followed_by_call = self.at_punct(Punctuation::LParen);
        let followed_by_struct_init =
            self.at_punct(Punctuation::LBrace) && self.looks_like_struct_init();
        if (followed_by_call || followed_by_struct_init)
            && self.diagnostics.len() == saved_diagnostics
        {
            Some(arguments)
        } else {
            if !self.nesting_drained {
                self.index = saved_index;
            }
            self.diagnostics.truncate(saved_diagnostics);
            None
        }
    }

    fn parse_type_arguments(&mut self) -> Vec<Type> {
        let mut arguments = Vec::new();
        self.expect_punct(Punctuation::Lt, "'<'");

        if !self.at_punct(Punctuation::Gt) {
            loop {
                arguments.push(self.parse_type());
                if self.eat_punct(Punctuation::Comma) {
                    self.skip_newlines();
                    if self.edition == Edition::V0_2 && self.at_punct(Punctuation::Gt) {
                        break;
                    }
                    continue;
                }
                break;
            }
        }

        self.skip_newlines();
        self.expect_punct(Punctuation::Gt, "'>'");
        arguments
    }

    fn parse_struct_init_fields(&mut self) -> Vec<FieldInit> {
        let mut fields = Vec::new();
        self.expect_punct(Punctuation::LBrace, "'{'");
        self.skip_newlines();

        while !self.at_punct(Punctuation::RBrace) && !self.at_eof() {
            let start = self.peek().span.start;
            let name = self.expect_identifier().unwrap_or_default();
            self.expect_punct(Punctuation::Colon, "':'");
            let value = self.parse_expression();
            fields.push(FieldInit {
                name,
                value,
                span: self.span_from(start),
            });

            self.skip_newlines();
            if self.eat_punct(Punctuation::Comma) {
                self.skip_newlines();
                continue;
            }
            break;
        }

        self.skip_newlines();
        self.expect_punct(Punctuation::RBrace, "'}'");
        fields
    }

    fn parse_pattern(&mut self) -> Pattern {
        let token = self.peek().clone();
        let start = token.span.start;

        if token.kind == TokenKind::Punct(Punctuation::Underscore) {
            self.bump();
            return Pattern {
                kind: PatternKind::Wildcard,
                span: token.span,
            };
        }

        if token.kind == TokenKind::Punct(Punctuation::LParen) {
            self.bump();
            let mut patterns = Vec::new();
            self.skip_newlines();

            if !self.at_punct(Punctuation::RParen) {
                loop {
                    patterns.push(self.parse_pattern());
                    if self.eat_punct(Punctuation::Comma) {
                        self.skip_newlines();
                        if self.edition == Edition::V0_2 && self.at_punct(Punctuation::RParen) {
                            break;
                        }
                        continue;
                    }
                    break;
                }
            }

            self.expect_punct(Punctuation::RParen, "')'");
            return Pattern {
                kind: PatternKind::Tuple(patterns),
                span: self.span_from(start),
            };
        }

        if let Some(literal) = self.pattern_literal(&token) {
            self.bump();
            return Pattern {
                kind: PatternKind::Literal(literal),
                span: token.span,
            };
        }

        if token.kind == TokenKind::Identifier {
            self.bump();
            let first = token.text;

            if self.eat_punct(Punctuation::Dot) {
                let variant = self.expect_identifier().unwrap_or_default();
                let arguments = if self.eat_punct(Punctuation::LParen) {
                    Some(self.parse_pattern_list_after_open())
                } else {
                    None
                };
                let path = TypePath {
                    segments: vec![first],
                    arguments: Vec::new(),
                    span: token.span,
                };
                return Pattern {
                    kind: PatternKind::Variant {
                        path,
                        variant,
                        arguments,
                    },
                    span: self.span_from(start),
                };
            }

            if self.eat_punct(Punctuation::LParen) {
                let arguments = self.parse_pattern_list_after_open();
                let path = TypePath {
                    segments: vec![first],
                    arguments: Vec::new(),
                    span: token.span,
                };
                return Pattern {
                    kind: PatternKind::Constructor { path, arguments },
                    span: self.span_from(start),
                };
            }

            return Pattern {
                kind: PatternKind::Identifier(first),
                span: token.span,
            };
        }

        self.error(
            INVALID_PATTERN,
            "Expected a pattern.",
            token.span,
            "Use _, a binding, a literal, an enum variant, or a tuple pattern.",
        );
        if !self.at_eof()
            && !self.at(TokenKind::Newline)
            && !self.at_punct(Punctuation::Semicolon)
            && !self.at_punct(Punctuation::RBrace)
            && !self.at_punct(Punctuation::FatArrow)
        {
            self.bump();
        }

        Pattern {
            kind: PatternKind::Wildcard,
            span: Span::new(start, self.last_end()),
        }
    }

    fn parse_pattern_list_after_open(&mut self) -> Vec<Pattern> {
        let mut patterns = Vec::new();
        self.skip_newlines();

        if self.at_punct(Punctuation::RParen) {
            self.bump();
            return patterns;
        }

        loop {
            patterns.push(self.parse_pattern());
            if self.eat_punct(Punctuation::Comma) {
                self.skip_newlines();
                if self.at_punct(Punctuation::RParen) {
                    break;
                }
                continue;
            }
            break;
        }

        self.skip_newlines();
        self.expect_punct(Punctuation::RParen, "')'");
        patterns
    }

    fn pattern_literal(&self, token: &Token) -> Option<Literal> {
        match token.kind {
            TokenKind::Integer => Some(Literal::Integer(token.text.clone())),
            TokenKind::Float => Some(Literal::Float(token.text.clone())),
            TokenKind::String => decode_string_literal(&token.text).ok().map(Literal::String),
            TokenKind::ByteString => decode_byte_literal(&token.text).ok().map(Literal::Bytes),
            TokenKind::Character => decode_char_literal(&token.text)
                .ok()
                .map(Literal::Character),
            TokenKind::Keyword(Keyword::True) => Some(Literal::Bool(true)),
            TokenKind::Keyword(Keyword::False) => Some(Literal::Bool(false)),
            _ => None,
        }
    }

    fn parse_type(&mut self) -> Type {
        self.depth += 1;
        if self.depth > MAX_NESTING_DEPTH {
            let span = self.peek().span;
            self.nesting_limit_error("Type nesting exceeds the supported depth.");
            if self.can_consume_for_progress() {
                self.bump();
            }
            self.depth -= 1;
            return Type {
                kind: TypeKind::Unit,
                span,
            };
        }
        let ty = self.parse_type_inner();
        self.depth -= 1;
        ty
    }

    fn parse_type_inner(&mut self) -> Type {
        let token = self.peek().clone();
        let start = token.span.start;

        match token.kind {
            TokenKind::Punct(Punctuation::Underscore) if self.edition == Edition::V0_2 => {
                self.bump();
                self.error(
                    INVALID_TYPE_ARGUMENT,
                    "Wildcard type arguments are not supported in edition 0.2.",
                    token.span,
                    "Write an explicit type instead of `_`.",
                );
                Type {
                    kind: TypeKind::Unit,
                    span: token.span,
                }
            }
            TokenKind::Punct(Punctuation::LBracket) => {
                self.bump();
                let element = self.parse_type();

                if self.eat_punct(Punctuation::Semicolon) {
                    let length = self.expect_integer();
                    self.expect_punct(Punctuation::RBracket, "']'");
                    Type {
                        kind: TypeKind::FixedArray(Box::new(element), length),
                        span: self.span_from(start),
                    }
                } else {
                    self.expect_punct(Punctuation::RBracket, "']'");
                    Type {
                        kind: TypeKind::Array(Box::new(element)),
                        span: self.span_from(start),
                    }
                }
            }
            TokenKind::Punct(Punctuation::LBrace) => {
                self.bump();
                let key = self.parse_type();
                self.expect_punct(Punctuation::Colon, "':'");
                let value = self.parse_type();
                self.expect_punct(Punctuation::RBrace, "'}'");
                Type {
                    kind: TypeKind::Map(Box::new(key), Box::new(value)),
                    span: self.span_from(start),
                }
            }
            TokenKind::Punct(Punctuation::LParen) => {
                self.bump();

                if self.eat_punct(Punctuation::RParen) {
                    return Type {
                        kind: TypeKind::Unit,
                        span: self.span_from(start),
                    };
                }

                let mut elements = Vec::new();
                loop {
                    elements.push(self.parse_type());
                    if self.eat_punct(Punctuation::Comma) {
                        self.skip_newlines();
                        if self.edition == Edition::V0_2 && self.at_punct(Punctuation::RParen) {
                            break;
                        }
                        continue;
                    }
                    break;
                }
                self.expect_punct(Punctuation::RParen, "')'");
                Type {
                    kind: TypeKind::Tuple(elements),
                    span: self.span_from(start),
                }
            }
            TokenKind::Keyword(Keyword::Fn) => {
                self.bump();
                self.expect_punct(Punctuation::LParen, "'('");
                let parameters = self.parse_type_list_after_open();
                let (return_type, error_set) = self.parse_return_and_errors();

                Type {
                    kind: TypeKind::Function {
                        parameters,
                        return_type: return_type.map(Box::new),
                        error_set,
                    },
                    span: self.span_from(start),
                }
            }
            TokenKind::Identifier => Type {
                kind: TypeKind::Path(self.parse_type_path()),
                span: self.span_from(start),
            },
            TokenKind::Keyword(Keyword::Char) if self.edition == Edition::V0_2 => {
                self.bump();
                Type {
                    kind: TypeKind::Path(TypePath {
                        segments: vec!["char".to_owned()],
                        arguments: Vec::new(),
                        span: token.span,
                    }),
                    span: token.span,
                }
            }
            _ => {
                self.error(
                    INVALID_TYPE,
                    "Expected a type.",
                    token.span,
                    "Use a primitive, named, array, map, tuple, or function type.",
                );
                if !self.at_eof()
                    && !self.at(TokenKind::Newline)
                    && !self.at_punct(Punctuation::Semicolon)
                    && !self.at_punct(Punctuation::RBrace)
                    && !self.at_punct(Punctuation::RParen)
                    && !self.at_punct(Punctuation::RBracket)
                    && !self.at_punct(Punctuation::Comma)
                {
                    self.bump();
                }
                Type {
                    kind: TypeKind::Unit,
                    span: Span::new(start, self.last_end()),
                }
            }
        }
    }

    fn parse_type_path(&mut self) -> TypePath {
        let start = self.peek().span.start;
        let mut segments = Vec::new();

        if self.at(TokenKind::Identifier) {
            segments.push(self.bump().text);
        } else {
            self.error_current(
                INVALID_TYPE,
                "Expected a type name.",
                "Use an identifier such as i32, User, or Option<T>.",
            );
        }

        while self.eat_punct(Punctuation::Dot) {
            segments.push(self.expect_identifier().unwrap_or_default());
        }

        let arguments = if self.at_punct(Punctuation::Lt) {
            self.parse_type_arguments()
        } else {
            Vec::new()
        };

        TypePath {
            segments,
            arguments,
            span: self.span_from(start),
        }
    }

    fn parse_type_list_after_open(&mut self) -> Vec<Type> {
        let mut types = Vec::new();
        self.skip_newlines();

        if self.at_punct(Punctuation::RParen) {
            self.bump();
            return types;
        }

        loop {
            types.push(self.parse_type());
            if self.eat_punct(Punctuation::Comma) {
                self.skip_newlines();
                if self.at_punct(Punctuation::RParen) {
                    break;
                }
                continue;
            }
            break;
        }

        self.skip_newlines();
        self.expect_punct(Punctuation::RParen, "')'");
        types
    }

    fn parse_path(&mut self) -> Vec<String> {
        let mut path = Vec::new();

        if self.at(TokenKind::Identifier) {
            path.push(self.bump().text);
        } else {
            self.error_current(
                EXPECTED,
                "Expected a module path.",
                "Use a dotted path such as app.main.",
            );
            return path;
        }

        while self.eat_punct(Punctuation::Dot) {
            path.push(self.expect_identifier().unwrap_or_default());
        }

        path
    }

    fn parse_name_or_default(&mut self) -> String {
        if self.at_keyword(Keyword::Default) {
            self.bump().text
        } else {
            self.expect_identifier().unwrap_or_default()
        }
    }

    fn looks_like_script_block(&self) -> bool {
        self.at_punct(Punctuation::LBrace)
            && self
                .peek_at(1)
                .is_some_and(|token| token.kind == TokenKind::Punct(Punctuation::At))
            && self.peek_at(2).is_some_and(|token| token.text == "script")
            && self
                .peek_at(3)
                .is_some_and(|token| token.kind == TokenKind::Punct(Punctuation::Colon))
    }

    fn at_canonical_scope(&self) -> bool {
        if self.edition != Edition::V0_2 || !self.at_punct(Punctuation::At) {
            return false;
        }
        let mut index = self.index + 1;
        let Some(name) = self.tokens.get(index) else {
            return false;
        };
        if name.kind != TokenKind::Identifier || !matches!(name.text.as_str(), "mode" | "memory") {
            return false;
        }
        index += 1;
        if self.tokens.get(index).map(|token| &token.kind)
            != Some(&TokenKind::Punct(Punctuation::LParen))
        {
            return false;
        }
        index = self.skip_newlines_at(index + 1);
        if !self
            .tokens
            .get(index)
            .is_some_and(|token| token.kind == TokenKind::Identifier)
        {
            return false;
        }
        index = self.skip_newlines_at(index + 1);
        if self.tokens.get(index).map(|token| &token.kind)
            != Some(&TokenKind::Punct(Punctuation::RParen))
        {
            return false;
        }
        index = self.skip_newlines_at(index + 1);
        self.tokens.get(index).map(|token| &token.kind)
            == Some(&TokenKind::Punct(Punctuation::LBrace))
    }

    fn skip_continuation_newlines(&mut self) {
        if self.edition == Edition::V0_2 {
            self.skip_newlines();
        }
    }

    fn next_non_newline_is_punct(&self, punctuation: Punctuation) -> bool {
        let index = self.skip_newlines_at(self.index);
        self.tokens.get(index).map(|token| &token.kind) == Some(&TokenKind::Punct(punctuation))
    }

    fn skip_newlines_at(&self, mut index: usize) -> usize {
        while self
            .tokens
            .get(index)
            .is_some_and(|token| token.kind == TokenKind::Newline)
        {
            index += 1;
        }
        index
    }

    fn looks_like_scope_annotation(&self) -> bool {
        self.at_punct(Punctuation::LBrace)
            && self
                .peek_at(1)
                .is_some_and(|token| token.kind == TokenKind::Punct(Punctuation::At))
            && self
                .peek_at(2)
                .is_some_and(|token| token.kind == TokenKind::Identifier && token.text != "script")
            && self
                .peek_at(3)
                .is_some_and(|token| token.kind == TokenKind::Punct(Punctuation::Colon))
    }

    fn looks_like_struct_init(&self) -> bool {
        if !self.at_punct(Punctuation::LBrace) {
            return false;
        }
        let name = self.skip_newlines_at(self.index + 1);
        let colon = self.skip_newlines_at(name + 1);
        self.tokens
            .get(name)
            .is_some_and(|token| token.kind == TokenKind::Identifier)
            && self
                .tokens
                .get(colon)
                .is_some_and(|token| token.kind == TokenKind::Punct(Punctuation::Colon))
    }

    fn expect_identifier(&mut self) -> Option<String> {
        if self.at(TokenKind::Identifier) {
            Some(self.bump().text)
        } else {
            self.error_current(EXPECTED, "Expected an identifier.", "Use a valid name.");
            None
        }
    }

    fn expect_string(&mut self) -> String {
        if self.at(TokenKind::String) {
            let token = self.bump();
            match decode_string_literal(&token.text) {
                Ok(value) => value,
                Err(message) => {
                    self.error(INVALID, message, token.span, "Fix the string literal.");
                    String::new()
                }
            }
        } else {
            self.error_current(
                EXPECTED,
                "Expected a string module specifier.",
                "Use a quoted specifier such as \"./thing.ns\" or \"std:fs\".",
            );
            String::new()
        }
    }

    fn expect_integer(&mut self) -> String {
        if self.at(TokenKind::Integer) {
            self.bump().text
        } else {
            self.error_current(
                EXPECTED,
                "Expected an integer literal.",
                "Use a decimal, hexadecimal, or binary integer.",
            );
            String::new()
        }
    }

    fn expect_keyword(&mut self, keyword: Keyword, description: &str) -> Option<Token> {
        if self.at_keyword(keyword) {
            Some(self.bump())
        } else {
            self.error_current(
                EXPECTED,
                format!("Expected {description}."),
                format!("Add {}.", keyword.as_str()),
            );
            None
        }
    }

    fn expect_punct(&mut self, punctuation: Punctuation, description: &str) -> Option<Token> {
        if self.at_punct(punctuation) {
            Some(self.bump())
        } else {
            self.error_current(
                EXPECTED,
                format!("Expected {description}."),
                "Check the surrounding syntax.",
            );
            None
        }
    }

    fn at_keyword(&self, keyword: Keyword) -> bool {
        self.peek().kind == TokenKind::Keyword(keyword)
    }

    fn at_punct(&self, punctuation: Punctuation) -> bool {
        self.peek().kind == TokenKind::Punct(punctuation)
    }

    fn eat_keyword(&mut self, keyword: Keyword) -> bool {
        if self.at_keyword(keyword) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn eat_punct(&mut self, punctuation: Punctuation) -> bool {
        if self.at_punct(punctuation) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn at(&self, kind: TokenKind) -> bool {
        self.peek().kind == kind
    }

    fn at_eof(&self) -> bool {
        self.at(TokenKind::Eof)
    }

    fn peek(&self) -> &Token {
        let index = self.index.min(self.tokens.len().saturating_sub(1));
        &self.tokens[index]
    }

    fn peek_at(&self, offset: usize) -> Option<&Token> {
        self.tokens.get(self.index + offset)
    }

    fn bump(&mut self) -> Token {
        let token = self.peek().clone();
        if self.index < self.tokens.len().saturating_sub(1) {
            self.index += 1;
        }
        token
    }

    fn last_end(&self) -> usize {
        if self.index == 0 {
            self.peek().span.end
        } else {
            self.tokens[self.index - 1].span.end
        }
    }

    fn span_from(&self, start: usize) -> Span {
        Span::new(start, self.last_end())
    }

    fn skip_newlines(&mut self) {
        while self.at(TokenKind::Newline) {
            self.bump();
        }
    }

    fn skip_separators(&mut self) {
        while self.at(TokenKind::Newline) || self.at_punct(Punctuation::Semicolon) {
            self.bump();
        }
    }

    fn finish_boundary(&mut self, allow_rbrace: bool) {
        if self.at(TokenKind::Newline) || self.at_punct(Punctuation::Semicolon) {
            self.skip_separators();
            return;
        }

        if self.at_eof() || (allow_rbrace && self.at_punct(Punctuation::RBrace)) {
            return;
        }

        self.error_current(
            BOUNDARY,
            "Expected a newline or ';' between constructs.",
            "Put the next item or statement on a new line, or separate them with ';'.",
        );

        while !self.at_eof()
            && !self.at(TokenKind::Newline)
            && !self.at_punct(Punctuation::Semicolon)
            && !self.at_punct(Punctuation::RBrace)
        {
            self.bump();
        }
        self.skip_separators();
    }

    fn synchronize_until(&mut self, punctuation: &[Punctuation]) {
        while !self.at_eof() && !punctuation.iter().any(|item| self.at_punct(*item)) {
            self.bump();
        }
    }

    fn error(
        &mut self,
        code: &'static str,
        message: impl Into<String>,
        span: Span,
        hint: impl Into<String>,
    ) {
        self.diagnostics.push(
            Diagnostic::error(code, message)
                .with_span(span)
                .with_hint(hint),
        );
    }

    fn error_current(
        &mut self,
        code: &'static str,
        message: impl Into<String>,
        hint: impl Into<String>,
    ) {
        let span = self.peek().span;
        self.error(code, message, span, hint);
    }
}

fn is_binary_operator_token(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Punct(
            Punctuation::Plus
                | Punctuation::Minus
                | Punctuation::Star
                | Punctuation::Slash
                | Punctuation::Percent
                | Punctuation::PlusWrap
                | Punctuation::MinusWrap
                | Punctuation::StarWrap
                | Punctuation::SlashWrap
                | Punctuation::EqEq
                | Punctuation::BangEq
                | Punctuation::Lt
                | Punctuation::LtEq
                | Punctuation::Gt
                | Punctuation::GtEq
                | Punctuation::AmpAmp
                | Punctuation::Amp
                | Punctuation::Pipe
                | Punctuation::Caret
                | Punctuation::PipePipe
                | Punctuation::Eq
        )
    )
}

fn expression_has_unparenthesized_struct_init(expression: &Expr) -> bool {
    match &expression.kind {
        ExprKind::StructInit { .. } => true,
        ExprKind::Binary { left, right, .. } => {
            expression_has_unparenthesized_struct_init(left)
                || expression_has_unparenthesized_struct_init(right)
        }
        ExprKind::Assign { target, value } => {
            expression_has_unparenthesized_struct_init(target)
                || expression_has_unparenthesized_struct_init(value)
        }
        ExprKind::Unary { operand, .. } => expression_has_unparenthesized_struct_init(operand),
        _ => false,
    }
}

fn expression_to_type_path(expression: &Expr) -> Option<TypePath> {
    match &expression.kind {
        ExprKind::Identifier(name) => Some(TypePath {
            segments: vec![name.clone()],
            arguments: Vec::new(),
            span: expression.span,
        }),
        ExprKind::Member { target, member } => {
            let mut path = expression_to_type_path(target)?;
            path.segments.push(member.clone());
            path.span = expression.span;
            Some(path)
        }
        ExprKind::TypeArguments { target, arguments } => {
            let mut path = expression_to_type_path(target)?;
            if !path.arguments.is_empty() {
                return None;
            }
            path.arguments = arguments.clone();
            path.span = expression.span;
            Some(path)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ParseOptions, parse, parse_with_comments, parse_with_comments_with_options,
        parse_with_options,
    };
    use crate::ast::{
        ElseBranch, ExprKind, FailArguments, ImportForm, ItemKind, Literal, MatchArmBody,
        PatternKind, ScopeKind, StmtKind, StructFields, TypeKind, Visibility,
    };
    use crate::edition::Edition;

    fn parse_ok(source: &str) -> crate::ast::Module {
        parse(source).unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"))
    }

    fn parse_v02(source: &str) -> Result<crate::ast::Module, Vec<crate::diagnostic::Diagnostic>> {
        parse_with_options(
            source,
            ParseOptions {
                edition: Edition::V0_2,
            },
        )
    }

    #[test]
    fn v0_2_parses_fail_keyword_in_both_shapes() {
        let tuple = parse_v02(
            r#"
module app.main

fn run() -> i32 {
    fail Error("x")
}
"#,
        )
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
        let ItemKind::Function(function) = &tuple.items[0].kind else {
            panic!("expected function");
        };
        let StmtKind::Expr(expression) = &function.body.statements[0].kind else {
            panic!("expected expression statement");
        };
        let ExprKind::Fail { error, arguments } = &expression.kind else {
            panic!("expected fail expression");
        };
        assert_eq!(error.segments, vec!["Error"]);
        assert!(matches!(arguments, FailArguments::Tuple(items) if items.len() == 1));

        let named = parse_v02(
            r#"
module app.main

fn run() -> i32 {
    fail Error { message: "x" }
}
"#,
        )
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
        let ItemKind::Function(function) = &named.items[0].kind else {
            panic!("expected function");
        };
        let StmtKind::Expr(expression) = &function.body.statements[0].kind else {
            panic!("expected expression statement");
        };
        let ExprKind::Fail { arguments, .. } = &expression.kind else {
            panic!("expected fail expression");
        };
        assert!(matches!(arguments, FailArguments::Struct(fields) if fields.len() == 1));
    }

    #[test]
    fn v0_2_parses_postfix_question_as_propagation() {
        let kind = parse_v02(
            r#"
module app.main

fn run() -> i32 {
    work()?
}
"#,
        )
        .map(|module| {
            let ItemKind::Function(function) = &module.items[0].kind else {
                panic!("expected function");
            };
            let StmtKind::Expr(expression) = &function.body.statements[0].kind else {
                panic!("expected expression statement");
            };
            expression.kind.clone()
        })
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));

        let ExprKind::Propagate(inner) = kind else {
            panic!("expected propagation");
        };
        assert!(matches!(inner.kind, ExprKind::Call { .. }));
    }

    #[test]
    fn v0_2_rejects_prefix_and_postfix_bang_with_migration_hints() {
        let prefix = parse_v02(
            r#"
module app.main

fn run() -> i32 {
    !Error("x")
}
"#,
        )
        .unwrap_err();
        assert!(prefix.iter().any(|diagnostic| {
            diagnostic.code == "NS2015"
                && diagnostic
                    .hint
                    .as_deref()
                    .is_some_and(|hint| hint.contains("fail Error"))
        }));

        let postfix = parse_v02(
            r#"
module app.main

fn run() -> i32 {
    work()!
}
"#,
        )
        .unwrap_err();
        assert!(postfix.iter().any(|diagnostic| {
            diagnostic.code == "NS2015"
                && diagnostic
                    .hint
                    .as_deref()
                    .is_some_and(|hint| hint.contains("`?`"))
        }));
    }

    #[test]
    fn v0_2_rejects_return_fail_as_redundant() {
        let diagnostics = parse_v02(
            r#"
module app.main

fn run() -> i32 {
    return fail Error("x")
}
"#,
        )
        .unwrap_err();
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "NS4014"
                && diagnostic
                    .hint
                    .as_deref()
                    .is_some_and(|hint| hint.contains("Remove `return`"))
        }));
    }

    #[test]
    fn v0_1_keeps_bang_forms_and_treats_fail_as_an_identifier() {
        let prefix = parse_ok(
            r#"
module app.main

fn run() -> i32 {
    !Error("x")
}
"#,
        );
        let ItemKind::Function(function) = &prefix.items[0].kind else {
            panic!("expected function");
        };
        assert!(matches!(
            function.body.statements[0].kind,
            StmtKind::Expr(ref expression) if matches!(expression.kind, ExprKind::Fail { .. })
        ));

        let postfix = parse_ok(
            r#"
module app.main

fn run() -> i32 {
    work()!
}
"#,
        );
        let ItemKind::Function(function) = &postfix.items[0].kind else {
            panic!("expected function");
        };
        assert!(matches!(
            function.body.statements[0].kind,
            StmtKind::Expr(ref expression) if matches!(expression.kind, ExprKind::Propagate(_))
        ));

        let identifier = parse_ok(
            r#"
module app.main

fn run() -> i32 {
    fail
}
"#,
        );
        let ItemKind::Function(function) = &identifier.items[0].kind else {
            panic!("expected function");
        };
        assert!(matches!(
            function.body.statements[0].kind,
            StmtKind::Expr(ref expression)
                if matches!(&expression.kind, ExprKind::Identifier(name) if name == "fail")
        ));
    }

    #[test]
    fn v0_2_parses_turbofish_calls_constructors_and_unit_variants() {
        let call = parse_v02(
            r#"
module app.main

fn run() -> i32 {
    identity::<i32>(5)
}
"#,
        )
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
        let ItemKind::Function(function) = &call.items[0].kind else {
            panic!("expected function");
        };
        let StmtKind::Expr(expression) = &function.body.statements[0].kind else {
            panic!("expected expression statement");
        };
        let ExprKind::Call { callee, .. } = &expression.kind else {
            panic!("expected call");
        };
        assert!(matches!(
            &callee.kind,
            ExprKind::TypeArguments { arguments, .. } if arguments.len() == 1
        ));

        let constructor = parse_v02(
            r#"
module app.main

fn run() -> i32 {
    Box::<i32> { value: 1 }
}
"#,
        )
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
        let ItemKind::Function(function) = &constructor.items[0].kind else {
            panic!("expected function");
        };
        let StmtKind::Expr(expression) = &function.body.statements[0].kind else {
            panic!("expected expression statement");
        };
        let ExprKind::StructInit { path, .. } = &expression.kind else {
            panic!("expected struct initializer");
        };
        assert_eq!(path.segments, vec!["Box"]);
        assert_eq!(path.arguments.len(), 1);

        let unit_variant = parse_v02(
            r#"
module app.main

fn run() -> i32 {
    None::<i32>
}
"#,
        )
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
        let ItemKind::Function(function) = &unit_variant.items[0].kind else {
            panic!("expected function");
        };
        let StmtKind::Expr(expression) = &function.body.statements[0].kind else {
            panic!("expected expression statement");
        };
        assert!(matches!(expression.kind, ExprKind::TypeArguments { .. }));
    }

    #[test]
    fn v0_2_rejects_second_and_misplaced_turbofish() {
        let second = parse_v02(
            r#"
module app.main

fn run() -> i32 {
    identity::<i32>::<u8>()
}
"#,
        )
        .unwrap_err();
        assert!(second.iter().any(|diagnostic| {
            diagnostic.code == "NS2011" && diagnostic.message.contains("Only one turbofish")
        }));

        let indexed = parse_v02(
            r#"
module app.main

fn run() -> i32 {
    xs[0]::<i32>()
}
"#,
        )
        .unwrap_err();
        assert!(indexed.iter().any(|diagnostic| {
            diagnostic.code == "NS2011" && diagnostic.message.contains("must follow a path")
        }));

        let member = parse_v02(
            r#"
module app.main

fn run() -> i32 {
    math.add::<i32>
}
"#,
        )
        .unwrap_err();
        assert!(member.iter().any(|diagnostic| {
            diagnostic.code == "NS2010" && diagnostic.message.contains("qualified value path")
        }));
    }

    #[test]
    fn v0_1_keeps_the_bare_angle_type_argument_heuristic() {
        let module = parse_ok(
            r#"
module app.main

fn run() -> i32 {
    identity<i32>(5)
}
"#,
        );
        let ItemKind::Function(function) = &module.items[0].kind else {
            panic!("expected function");
        };
        let StmtKind::Expr(expression) = &function.body.statements[0].kind else {
            panic!("expected expression statement");
        };
        let ExprKind::Call { callee, .. } = &expression.kind else {
            panic!("expected call");
        };
        assert!(matches!(
            &callee.kind,
            ExprKind::TypeArguments { arguments, .. } if arguments.len() == 1
        ));
    }

    #[test]
    fn v0_2_parses_canonical_scope_blocks_at_item_and_statement_positions() {
        let items = parse_v02(
            r#"
module app.main

@mode(system) {
    pub fn helper() -> i32 {
        return 1
    }
}
"#,
        )
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
        let ItemKind::ScopedItems(scoped) = &items.items[0].kind else {
            panic!("expected scoped items");
        };
        assert_eq!(scoped.annotation.kind, ScopeKind::Mode);
        assert_eq!(scoped.annotation.value, "system");
        assert_eq!(scoped.items.len(), 1);

        let statements = parse_v02(
            r#"
module app.main

fn run() -> i32 {
    @memory(system) {
        let value = 1
        return value
    }
}
"#,
        )
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
        let ItemKind::Function(function) = &statements.items[0].kind else {
            panic!("expected function");
        };
        let StmtKind::Scoped(scoped) = &function.body.statements[0].kind else {
            panic!("expected scoped statements");
        };
        assert_eq!(scoped.annotation.kind, ScopeKind::Memory);
        assert_eq!(scoped.annotation.value, "system");
        assert_eq!(scoped.statements.len(), 2);
    }

    #[test]
    fn v0_2_allows_nested_canonical_scopes_without_semicolons() {
        let module = parse_v02(
            r#"
module app.main

fn run() -> i32 {
    @mode(system) {
        @memory(system) {
            return 1
        }
    }
}
"#,
        )
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
        let ItemKind::Function(function) = &module.items[0].kind else {
            panic!("expected function");
        };
        let StmtKind::Scoped(outer) = &function.body.statements[0].kind else {
            panic!("expected outer scope");
        };
        let StmtKind::Scoped(inner) = &outer.statements[0].kind else {
            panic!("expected inner scope");
        };
        assert_eq!(inner.annotation.kind, ScopeKind::Memory);
    }

    #[test]
    fn v0_1_keeps_the_braced_scope_block_form() {
        let module = parse_ok(
            r#"
module app.main

{@mode:system;
    pub fn helper() -> i32 {
        return 1
    }
}
"#,
        );
        let ItemKind::ScopedItems(scoped) = &module.items[0].kind else {
            panic!("expected scoped items");
        };
        assert_eq!(scoped.annotation.kind, ScopeKind::Mode);
        assert_eq!(scoped.annotation.value, "system");
    }

    #[test]
    fn v0_2_allows_trailing_commas_in_lists() {
        let module = parse_v02(
            r#"
module app.main

fn identity<T,>(value: T,) -> (T,) {
    let pair = (value, value,)
    match pair {
        (first, _,) => first,
    }
}
"#,
        )
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
        let ItemKind::Function(function) = &module.items[0].kind else {
            panic!("expected function");
        };
        assert_eq!(function.type_parameters, vec!["T"]);
        assert!(function.return_type.is_some());
        let StmtKind::Let { value, .. } = &function.body.statements[0].kind else {
            panic!("expected let");
        };
        assert!(matches!(value.kind, ExprKind::Tuple(ref elements) if elements.len() == 2));
    }

    #[test]
    fn v0_2_continues_after_a_trailing_binary_operator_and_before_a_dot() {
        let module = parse_v02(
            r#"
module app.main

fn run(user: User) -> i32 {
    let value = 1 +
        2
    let name = user
        .name
    return value
}
"#,
        )
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
        let ItemKind::Function(function) = &module.items[0].kind else {
            panic!("expected function");
        };
        let StmtKind::Let { value, .. } = &function.body.statements[0].kind else {
            panic!("expected let");
        };
        assert!(matches!(
            value.kind,
            ExprKind::Binary {
                operator: crate::ast::BinaryOp::Add,
                ..
            }
        ));
        let StmtKind::Let { value, .. } = &function.body.statements[1].kind else {
            panic!("expected let");
        };
        assert!(matches!(value.kind, ExprKind::Member { .. }));
    }

    #[test]
    fn v0_2_rejects_a_leading_binary_operator() {
        let diagnostics = parse_v02(
            r#"
module app.main

fn run() -> i32 {
    let value = * 2
    return value
}
"#,
        )
        .unwrap_err();
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "NS2002"
                && diagnostic
                    .message
                    .contains("binary operator cannot start an expression")
        }));
    }

    #[test]
    fn v0_2_enforces_match_arm_commas_except_after_blocks() {
        let missing = parse_v02(
            r#"
module app.main

fn run(value: i32) -> i32 {
    match value {
        1 => 1
        2 => 2
    }
}
"#,
        )
        .unwrap_err();
        assert!(missing.iter().any(|diagnostic| {
            diagnostic.code == "NS2006" && diagnostic.message.contains("match arm")
        }));

        let blocks = parse_v02(
            r#"
module app.main

fn run(value: i32) -> i32 {
    match value {
        1 => { return 1 }
        2 => { return 2 }
    }
}
"#,
        )
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
        let ItemKind::Function(function) = &blocks.items[0].kind else {
            panic!("expected function");
        };
        let StmtKind::Expr(expression) = &function.body.statements[0].kind else {
            panic!("expected match expression");
        };
        assert!(matches!(expression.kind, ExprKind::Match { ref arms, .. } if arms.len() == 2));
    }

    #[test]
    fn v0_1_does_not_continue_after_a_trailing_binary_operator() {
        assert!(
            parse(
                r#"
module app.main

fn run() -> i32 {
    let value = 1 +
        2
    return value
}
"#
            )
            .is_err()
        );
    }

    #[test]
    fn v0_2_ast_serializes_fail_and_propagation_contract() {
        let module = parse_v02(
            r#"
module app.main

fn run() -> i32 {
    let value = work()?
    fail Error { message: "x" }
}
"#,
        )
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
        let value = serde_json::to_value(&module).expect("AST must serialize");

        let statements = &value["items"][0]["kind"]["Function"]["body"]["statements"];
        let propagate = &statements[0]["kind"]["Let"]["value"]["kind"]["Propagate"];
        assert!(
            propagate.is_object(),
            "postfix `?` must serialize as Propagate"
        );
        let failure = &statements[1]["kind"]["Expr"]["kind"]["Fail"];
        assert!(
            failure["error"].is_object(),
            "fail type path must serialize"
        );
        assert!(
            failure["arguments"]["Struct"].is_array(),
            "named fail arguments must serialize as Struct"
        );
    }

    #[test]
    fn parse_with_comments_returns_comment_trivia() {
        let (module, comments) =
            parse_with_comments("// file\nmodule app.main\n// module doc\n").unwrap();
        assert_eq!(module.name, vec!["app", "main"]);
        assert_eq!(
            comments
                .iter()
                .map(|comment| comment.text.as_str())
                .collect::<Vec<_>>(),
            vec!["// file", "// module doc"]
        );
    }

    #[test]
    fn parses_module_function_and_return() {
        let module = parse_ok(
            r#"
module app.main

fn add(a: i32, b: i32) -> i32 {
    return a + b
}
"#,
        );

        assert_eq!(module.name, vec!["app", "main"]);
        assert_eq!(module.items.len(), 1);

        let ItemKind::Function(function) = &module.items[0].kind else {
            panic!("expected function");
        };
        assert_eq!(function.name, "add");
        assert_eq!(function.parameters.len(), 2);
        assert!(function.return_type.is_some());
        assert_eq!(function.body.statements.len(), 1);

        let StmtKind::Return { value: Some(value) } = &function.body.statements[0].kind else {
            panic!("expected return");
        };
        assert!(matches!(
            value.kind,
            ExprKind::Binary {
                operator: crate::ast::BinaryOp::Add,
                ..
            }
        ));
    }

    #[test]
    fn parses_imports_and_exports() {
        let module = parse_ok(
            r#"
module app.main

import std.io
import { read, write as write_file } from "std:fs"
import * as math from "std:math"
import fallback from "./thing.ns"
import "./effects.ns"

fn helper() -> i32 {
    return 1
}

export { helper, helper as alias }
export { read } from "std:fs"
export * from "./shared.ns"
export * as tools from "./tools.ns"
export default helper
"#,
        );

        assert_eq!(module.items.len(), 11);
        assert!(matches!(
            module.items[0].kind,
            ItemKind::Import(crate::ast::ImportDecl {
                form: ImportForm::Path { .. },
                ..
            })
        ));
        assert!(matches!(
            module.items[1].kind,
            ItemKind::Import(crate::ast::ImportDecl {
                form: ImportForm::Named { .. },
                ..
            })
        ));
        assert!(matches!(
            module.items[3].kind,
            ItemKind::Import(crate::ast::ImportDecl {
                form: ImportForm::Default { .. },
                ..
            })
        ));
        assert!(matches!(
            module.items[7].kind,
            ItemKind::Export(crate::ast::ExportDecl {
                form: crate::ast::ExportForm::List { from: Some(_), .. },
                ..
            })
        ));
        assert!(matches!(
            module.items[10].kind,
            ItemKind::Export(crate::ast::ExportDecl {
                form: crate::ast::ExportForm::DefaultValue(_),
                ..
            })
        ));
    }

    #[test]
    fn parses_declarations_and_foreign_modules() {
        let module = parse_ok(
            r#"
module app.main

pub struct User {
    id: u64
    name: str
}

private enum Status {
    Pending
    Active
    Suspended(reason: str)
}

const MAX: u32 = 1_000
type UserId = u64

foreign module math_engine {
    fn compute(data: [f64]) -> [f64]
}
"#,
        );

        assert_eq!(module.items.len(), 5);
        assert_eq!(module.items[0].visibility, Visibility::Public);

        let ItemKind::Struct(user) = &module.items[0].kind else {
            panic!("expected struct");
        };
        let StructFields::Named(fields) = &user.fields else {
            panic!("expected named fields");
        };
        assert_eq!(fields.len(), 2);

        assert_eq!(module.items[1].visibility, Visibility::Private);

        let ItemKind::Enum(status) = &module.items[1].kind else {
            panic!("expected enum");
        };
        assert_eq!(status.variants.len(), 3);
        assert!(status.variants[2].fields.is_some());

        let ItemKind::ForeignModule(foreign) = &module.items[4].kind else {
            panic!("expected foreign module");
        };
        assert_eq!(foreign.functions.len(), 1);
    }

    #[test]
    fn parses_control_flow_and_assignments() {
        let module = parse_ok(
            r#"
module app.main

fn count(items: [i32]) -> i32 {
    let mut total = 0

    for item in items {
        total = total + item
    }

    while total > 10 {
        total = total - 1
    }

    if total == 0 {
        return 0
    } else {
        return total
    }
}
"#,
        );

        let ItemKind::Function(function) = &module.items[0].kind else {
            panic!("expected function");
        };
        assert_eq!(function.body.statements.len(), 4);
        assert!(matches!(
            function.body.statements[1].kind,
            StmtKind::For { .. }
        ));
        assert!(matches!(
            function.body.statements[2].kind,
            StmtKind::While { .. }
        ));

        let StmtKind::Expr(if_expr) = &function.body.statements[3].kind else {
            panic!("expected if expression");
        };
        let ExprKind::If { else_branch, .. } = &if_expr.kind else {
            panic!("expected if expression kind");
        };
        assert!(matches!(else_branch, Some(ElseBranch::Block(_))));
    }

    #[test]
    fn parses_match_patterns() {
        let module = parse_ok(
            r#"
module app.main

fn classify(value: Option<i32>) -> str {
    match value {
        Some(0) => "zero"
        Some(n) => "number"
        None => "none"
    }
}
"#,
        );

        let ItemKind::Function(function) = &module.items[0].kind else {
            panic!("expected function");
        };
        let StmtKind::Expr(expression) = &function.body.statements[0].kind else {
            panic!("expected match expression");
        };
        let ExprKind::Match { arms, .. } = &expression.kind else {
            panic!("expected match");
        };
        assert_eq!(arms.len(), 3);
        assert!(matches!(
            arms[0].pattern.kind,
            PatternKind::Constructor { .. }
        ));
        assert!(matches!(
            arms[1].pattern.kind,
            PatternKind::Constructor { .. }
        ));
    }

    #[test]
    fn parses_scoped_blocks_and_semicolon_groups() {
        let module = parse_ok(
            r#"
module app.main
const A: i32 = 1; const B: i32 = 2

fn scoped() {
    {@mode:core;
        let x = 1; let y = 2
    }
}
"#,
        );

        assert_eq!(module.items.len(), 3);
        assert!(matches!(module.items[0].kind, ItemKind::Constant(_)));
        assert!(matches!(module.items[1].kind, ItemKind::Constant(_)));

        let ItemKind::Function(function) = &module.items[2].kind else {
            panic!("expected function");
        };
        assert!(matches!(
            function.body.statements[0].kind,
            StmtKind::Scoped(_)
        ));
    }

    #[test]
    fn parses_error_effects_and_failure_construction() {
        let module = parse_ok(
            r#"
module app.main

fn parse_age(input: str) -> i32 ! InvalidAge | ParseIntError {
    if input.is_empty() {
        return !InvalidAge("bad")
    }
    return input.to_int()!
}
"#,
        );

        let ItemKind::Function(function) = &module.items[0].kind else {
            panic!("expected function");
        };
        assert_eq!(function.error_set.len(), 2);
        assert!(matches!(
            function.return_type.as_ref().map(|ty| &ty.kind),
            Some(TypeKind::Path(_))
        ));
    }

    #[test]
    fn parses_generic_calls_and_struct_initializers() {
        let module = parse_ok(
            r#"
module app.main

fn main() {
    let user = User { id: 1 }
    let boxed = Box<i32> { value: 1 }
    let value = identity<i32>(1)
}
"#,
        );

        let ItemKind::Function(function) = &module.items[0].kind else {
            panic!("expected function");
        };
        let StmtKind::Let { value, .. } = &function.body.statements[0].kind else {
            panic!("expected let");
        };
        assert!(matches!(value.kind, ExprKind::StructInit { .. }));

        let StmtKind::Let { value, .. } = &function.body.statements[1].kind else {
            panic!("expected let");
        };
        let ExprKind::StructInit { path, .. } = &value.kind else {
            panic!("expected generic struct initializer");
        };
        assert_eq!(path.arguments.len(), 1);

        let StmtKind::Let { value, .. } = &function.body.statements[2].kind else {
            panic!("expected let");
        };
        assert!(matches!(value.kind, ExprKind::Call { .. }));
    }

    #[test]
    fn parses_composite_and_function_types() {
        let module = parse_ok(
            r#"
module app.main

fn uses_types(
    a: [i32],
    b: [i32; 4],
    c: {str: i32},
    d: (i32, str),
    e: fn(i32) -> str ! TypeError
) -> () {
    return ()
}
"#,
        );

        let ItemKind::Function(function) = &module.items[0].kind else {
            panic!("expected function");
        };
        assert_eq!(function.parameters.len(), 5);
        assert!(matches!(function.parameters[0].ty.kind, TypeKind::Array(_)));
        assert!(matches!(
            function.parameters[1].ty.kind,
            TypeKind::FixedArray(_, _)
        ));
        assert!(matches!(
            function.parameters[2].ty.kind,
            TypeKind::Map(_, _)
        ));
        assert!(matches!(function.parameters[3].ty.kind, TypeKind::Tuple(_)));

        let TypeKind::Function { error_set, .. } = &function.parameters[4].ty.kind else {
            panic!("expected function type");
        };
        assert_eq!(error_set.len(), 1);
    }

    #[test]
    fn parses_module_attributes_and_scoped_item_blocks() {
        let module = parse_ok(
            r#"
@requires(system)
@tags(fs, ffi)
module io

{@mode:core;
    fn portable() { }
}
"#,
        );

        assert_eq!(module.attributes.len(), 2);
        assert_eq!(module.attributes[0].name, "requires");
        assert_eq!(module.attributes[1].name, "tags");
        assert!(matches!(module.items[0].kind, ItemKind::ScopedItems(_)));
    }

    #[test]
    fn rejects_missing_semicolon_between_same_line_items() {
        let diagnostics = parse("module app.main\nconst A: i32 = 1 const B: i32 = 2\n")
            .expect_err("expected a boundary diagnostic");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "NS2006")
        );
    }

    #[test]
    fn reports_invalid_lexical_forms() {
        let diagnostics = parse("/* invalid */\n").expect_err("expected a lexical diagnostic");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "NS1001")
        );
    }

    #[test]
    fn reports_conflicting_visibility_modifiers() {
        let diagnostics =
            parse("module app.main\npub public fn f() {}\n").expect_err("expected NS3002");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "NS3002")
        );
    }

    #[test]
    fn slice_c_parse_error_stays_bounded() {
        let diagnostics =
            parse_with_comments_with_options("public fn broken( {\n", ParseOptions::default())
                .expect_err("expected a parse error");
        assert!(
            diagnostics.len() <= 2,
            "one malformed parameter list must not cascade: {diagnostics:?}"
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "NS2001")
        );
    }

    #[test]
    fn slice_c_malformed_struct_body_stays_bounded() {
        let diagnostics = parse_with_comments_with_options(
            "struct Broken {\n    name: ,\n}\n",
            ParseOptions::default(),
        )
        .expect_err("expected a parse error");
        assert!(
            diagnostics.len() <= 2,
            "one malformed field must not cascade: {diagnostics:?}"
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "NS2003")
        );
    }

    #[test]
    fn slice_c_depth_guard_is_never_suppressed() {
        let source = format!("fn f() -> i32 {{ return {} }}\n", "(".repeat(80));
        let diagnostics = parse_with_comments_with_options(&source, ParseOptions::default())
            .expect_err("expected a depth-limit error");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "NS2002")
        );
    }

    #[test]
    fn v0_2_if_let_desugars_to_match() {
        let module = parse_v02(
            r#"
module app.main

fn classify(value: Option<i32>) -> i32 {
    if let Some(inner) = value {
        return inner
    } else {
        return 0
    }
}
"#,
        )
        .expect("if let should parse in edition 0.2");
        let ItemKind::Function(function) = &module.items[0].kind else {
            panic!("expected function");
        };
        let StmtKind::Expr(expression) = &function.body.statements[0].kind else {
            panic!("expected expression statement");
        };
        let ExprKind::Match { value: _, arms } = &expression.kind else {
            panic!("expected match desugaring");
        };
        assert_eq!(arms.len(), 2);
        assert!(matches!(arms[0].body, MatchArmBody::Block(_)));
        assert!(matches!(arms[1].pattern.kind, PatternKind::Wildcard));
        assert!(matches!(arms[1].body, MatchArmBody::Block(_)));
    }

    #[test]
    fn v0_2_while_let_desugars_to_while_match_break() {
        let module = parse_v02(
            r#"
module app.main

fn drain(values: [Option<i32>]) -> i32 {
    while let Some(value) = values.pop() {
        return value
    }
    return 0
}
"#,
        )
        .expect("while let should parse in edition 0.2");
        let ItemKind::Function(function) = &module.items[0].kind else {
            panic!("expected function");
        };
        let StmtKind::While { condition, body } = &function.body.statements[0].kind else {
            panic!("expected while desugaring");
        };
        assert!(matches!(
            condition.kind,
            ExprKind::Literal(Literal::Bool(true))
        ));
        let StmtKind::Expr(expression) = &body.statements[0].kind else {
            panic!("expected match expression in loop body");
        };
        let ExprKind::Match { arms, .. } = &expression.kind else {
            panic!("expected match desugaring");
        };
        assert_eq!(arms.len(), 2);
        let MatchArmBody::Block(break_block) = &arms[1].body else {
            panic!("expected break arm");
        };
        assert!(matches!(break_block.statements[0].kind, StmtKind::Break));
    }

    #[test]
    fn v0_2_parses_exclusive_range_in_for() {
        let module = parse_v02(
            r#"
module app.main

fn count() -> i32 {
    let mut total = 0
    for value in 0..3 {
        total = total + value
    }
    return total
}
"#,
        )
        .expect("range should parse in edition 0.2");
        let ItemKind::Function(function) = &module.items[0].kind else {
            panic!("expected function");
        };
        let StmtKind::For { iterable, .. } = &function.body.statements[1].kind else {
            panic!("expected for loop");
        };
        let ExprKind::Range {
            start: _,
            end: _,
            inclusive,
        } = &iterable.kind
        else {
            panic!("expected range iterable");
        };
        assert!(!inclusive);
    }

    #[test]
    fn v0_2_parses_inclusive_range_in_for() {
        let module = parse_v02(
            r#"
module app.main

fn count() -> i32 {
    let mut total = 0
    for value in 0..=3 {
        total = total + value
    }
    return total
}
"#,
        )
        .expect("inclusive range should parse in edition 0.2");
        let ItemKind::Function(function) = &module.items[0].kind else {
            panic!("expected function");
        };
        let StmtKind::For { iterable, .. } = &function.body.statements[1].kind else {
            panic!("expected for loop");
        };
        assert!(matches!(
            &iterable.kind,
            ExprKind::Range {
                inclusive: true,
                ..
            }
        ));
    }

    #[test]
    fn v0_2_index_range_lowers_to_slice_call() {
        let module = parse_v02(
            r#"
module app.main

fn part(text: str) -> str ! SliceError {
    return text[0..2]?
}
"#,
        )
        .expect("index range should parse in edition 0.2");
        let ItemKind::Function(function) = &module.items[0].kind else {
            panic!("expected function");
        };
        let StmtKind::Return {
            value: Some(expression),
        } = &function.body.statements[0].kind
        else {
            panic!("expected return");
        };
        let ExprKind::Propagate(inner) = &expression.kind else {
            panic!("expected propagation");
        };
        let ExprKind::Call { callee, arguments } = &inner.kind else {
            panic!("expected slice call");
        };
        assert!(matches!(
            &callee.kind,
            ExprKind::Member { member, .. } if member == "slice"
        ));
        assert_eq!(arguments.len(), 2);
    }

    #[test]
    fn v0_1_still_rejects_if_let() {
        let source = r#"
module app.main

fn classify(value: Option<i32>) -> i32 {
    if let Some(inner) = value {
        return inner
    }
    return 0
}
"#;
        assert!(parse(source).is_err());
    }
}
