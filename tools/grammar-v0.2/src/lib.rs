//! Standalone canonical-LR(1) check for the v0.2 grammar.
//!
//! This crate is a Phase 0 gate, not a compiler dependency. It encodes the
//! v0.2 forms whose LR behavior must be settled before the language freezes:
//!
//! - type arguments in type position (`Option<i32>`) versus turbofish in
//!   expression position (`Option::<i32>`);
//! - `fail Type { ... }` and `fail Type(...)` versus ordinary calls;
//! - struct initializers (`Foo { ... }`) versus block syntax;
//! - both scope-block spellings (`@mode(core) { ... }` and
//!   `{ @mode : core ; ... }`);
//! - optional trailing commas in comma-separated lists.
//!
//! The grammar is deliberately focused on the v0.2 ambiguity set. Operator
//! precedence layers and control-flow expressions are omitted; they do not
//! participate in the ambiguity questions above. `build_lr` uses canonical
//! LR(1) construction with Pager merging, so a grammar that is LR(1) but not
//! LALR(1) is not falsely rejected.

use std::collections::HashMap;

use qana_grammar::{NtId, Sym, SynGrammar, TokenId};

const TOKENS: &[&str] = &[
    "fn", "let", "mut", "struct", "enum", "const", "module", "spawn", "fail", "return", "mode",
    "memory", "true", "false", "not", "IDENT", "INT", "STR", "+", "-", "*", "/", "%", "/%", "+%",
    "-%", "*%", "=", "==", "!=", "<", "<=", ">", ">=", "&&", "||", "!", "?", ".", ",", ":", ";",
    "(", ")", "[", "]", "{", "}", "->", "=>", "|", "@", "::", "::<",
];

const NONTERMINALS: &[&str] = &[
    "Program",
    "Items",
    "Item",
    "FnDecl",
    "StructDecl",
    "EnumDecl",
    "ConstDecl",
    "ModuleDecl",
    "ModulePath",
    "ScopeItems",
    "ScopeKind",
    "TypeParams",
    "IdentList",
    "OptComma",
    "ParamList",
    "Param",
    "RetType",
    "ErrorBody",
    "Type",
    "TypePath",
    "TypeArgs",
    "TypeList",
    "Turbofish",
    "Block",
    "StmtList",
    "Stmt",
    "ScopeStmt",
    "Expr",
    "Atom",
    "FailExpr",
    "SpawnExpr",
    "Suffix",
    "CallSuffix",
    "TurbofishSuffix",
    "IndexSuffix",
    "FieldSuffix",
    "FieldName",
    "TrySuffix",
    "StructInitSuffix",
    "FieldInit",
    "FieldInitList",
    "MapEntry",
    "MapEntryList",
    "Literal",
    "FieldList",
    "Field",
    "VariantList",
    "Variant",
    "VariantFields",
    "ExprList",
];

/// Build the focused v0.2 grammar.
pub fn v0_2_grammar() -> SynGrammar {
    let mut grammar = SynGrammar::new(
        "astra-v0.2",
        TOKENS.iter().map(|name| (*name).to_string()).collect(),
    );

    let tokens: HashMap<&'static str, TokenId> = TOKENS
        .iter()
        .enumerate()
        .map(|(index, name)| (*name, index as TokenId))
        .collect();
    let nts: HashMap<&'static str, NtId> = NONTERMINALS
        .iter()
        .map(|name| (*name, grammar.nt(name)))
        .collect();

    let t = |name: &str| {
        Sym::T(
            *tokens
                .get(name)
                .unwrap_or_else(|| panic!("missing token {name}")),
        )
    };
    let n = |name: &str| {
        Sym::N(
            *nts.get(name)
                .unwrap_or_else(|| panic!("missing nonterminal {name}")),
        )
    };

    macro_rules! p {
        ($lhs:literal, [$($sym:expr),* $(,)?]) => {
            grammar.prod(nts[$lhs], vec![$($sym),*]);
        };
    }

    // A comma-separated list. The empty, single, continuation, and trailing
    // comma productions are explicit so the LR(1) construction sees every
    // decision point.
    macro_rules! list {
        ($name:literal, $element:expr) => {
            p!($name, []);
            p!($name, [$element]);
            p!($name, [n($name), t(","), $element]);
        };
    }

    p!("Program", [n("Items")]);

    p!("Items", []);
    p!("Items", [n("Items"), n("Item")]);

    p!("Item", [n("FnDecl")]);
    p!("Item", [n("StructDecl")]);
    p!("Item", [n("EnumDecl")]);
    p!("Item", [n("ConstDecl")]);
    p!("Item", [n("ModuleDecl")]);
    p!("Item", [n("ScopeItems")]);

    // Function declarations. The four spellings keep generics and return
    // types non-nullable at the grammar level.
    p!(
        "FnDecl",
        [
            t("fn"),
            t("IDENT"),
            t("("),
            n("ParamList"),
            n("OptComma"),
            t(")"),
            n("Block")
        ]
    );
    p!(
        "FnDecl",
        [
            t("fn"),
            t("IDENT"),
            n("TypeParams"),
            t("("),
            n("ParamList"),
            n("OptComma"),
            t(")"),
            n("Block")
        ]
    );
    p!(
        "FnDecl",
        [
            t("fn"),
            t("IDENT"),
            t("("),
            n("ParamList"),
            n("OptComma"),
            t(")"),
            n("RetType"),
            n("Block")
        ]
    );
    p!(
        "FnDecl",
        [
            t("fn"),
            t("IDENT"),
            n("TypeParams"),
            t("("),
            n("ParamList"),
            n("OptComma"),
            t(")"),
            n("RetType"),
            n("Block")
        ]
    );

    p!(
        "StructDecl",
        [
            t("struct"),
            t("IDENT"),
            t("{"),
            n("FieldList"),
            n("OptComma"),
            t("}")
        ]
    );
    p!(
        "StructDecl",
        [
            t("struct"),
            t("IDENT"),
            n("TypeParams"),
            t("{"),
            n("FieldList"),
            n("OptComma"),
            t("}")
        ]
    );
    p!(
        "StructDecl",
        [
            t("struct"),
            t("IDENT"),
            t("("),
            n("TypeList"),
            n("OptComma"),
            t(")")
        ]
    );
    p!(
        "StructDecl",
        [
            t("struct"),
            t("IDENT"),
            n("TypeParams"),
            t("("),
            n("TypeList"),
            n("OptComma"),
            t(")")
        ]
    );

    p!(
        "EnumDecl",
        [
            t("enum"),
            t("IDENT"),
            t("{"),
            n("VariantList"),
            n("OptComma"),
            t("}")
        ]
    );
    p!(
        "EnumDecl",
        [
            t("enum"),
            t("IDENT"),
            n("TypeParams"),
            t("{"),
            n("VariantList"),
            n("OptComma"),
            t("}")
        ]
    );

    p!(
        "ConstDecl",
        [
            t("const"),
            t("IDENT"),
            t(":"),
            n("Type"),
            t("="),
            n("Expr"),
            t(";")
        ]
    );
    p!("ModuleDecl", [t("module"), n("ModulePath")]);
    p!("ModulePath", [t("IDENT")]);
    p!("ModulePath", [n("ModulePath"), t("."), t("IDENT")]);

    p!(
        "ScopeItems",
        [
            t("@"),
            n("ScopeKind"),
            t("("),
            t("IDENT"),
            t(")"),
            t("{"),
            n("Items"),
            t("}")
        ]
    );
    p!(
        "ScopeItems",
        [
            t("{"),
            t("@"),
            n("ScopeKind"),
            t(":"),
            t("IDENT"),
            t(";"),
            n("Items"),
            t("}")
        ]
    );
    p!("ScopeKind", [t("mode")]);
    p!("ScopeKind", [t("memory")]);

    p!(
        "TypeParams",
        [t("<"), n("IdentList"), n("OptComma"), t(">")]
    );
    p!("Param", [t("IDENT"), t(":"), n("Type")]);
    p!("RetType", [t("->"), n("Type")]);
    p!("RetType", [t("->"), n("Type"), t("!"), n("ErrorBody")]);
    p!("ErrorBody", [n("TypePath")]);
    p!("ErrorBody", [n("TypePath"), t("|"), n("ErrorBody")]);

    p!("Type", [n("TypePath")]);
    p!("Type", [t("["), n("Type"), t("]")]);
    p!("Type", [t("["), n("Type"), t(";"), t("INT"), t("]")]);
    p!("Type", [t("{"), n("Type"), t(":"), n("Type"), t("}")]);
    p!("Type", [t("("), n("TypeList"), n("OptComma"), t(")")]);

    p!("TypePath", [t("IDENT")]);
    p!("TypePath", [t("IDENT"), n("TypeArgs")]);
    p!("TypePath", [n("TypePath"), t("."), t("IDENT")]);
    p!("TypeArgs", [t("<"), n("TypeList"), n("OptComma"), t(">")]);
    p!(
        "Turbofish",
        [t("::<"), n("TypeList"), n("OptComma"), t(">")]
    );

    p!("Block", [t("{"), n("StmtList"), t("}")]);
    p!("StmtList", []);
    p!("StmtList", [n("StmtList"), n("Stmt")]);

    p!("Stmt", [t("let"), t("IDENT"), t("="), n("Expr"), t(";")]);
    p!(
        "Stmt",
        [
            t("let"),
            t("IDENT"),
            t(":"),
            n("Type"),
            t("="),
            n("Expr"),
            t(";")
        ]
    );
    p!(
        "Stmt",
        [t("let"), t("mut"), t("IDENT"), t("="), n("Expr"), t(";")]
    );
    p!(
        "Stmt",
        [
            t("let"),
            t("mut"),
            t("IDENT"),
            t(":"),
            n("Type"),
            t("="),
            n("Expr"),
            t(";")
        ]
    );
    p!("Stmt", [t("return"), t(";")]);
    p!("Stmt", [t("return"), n("Expr"), t(";")]);
    p!("Stmt", [n("ScopeStmt")]);
    p!("Stmt", [n("Expr"), t(";")]);
    p!("Stmt", [t(";")]);

    p!(
        "ScopeStmt",
        [
            t("@"),
            n("ScopeKind"),
            t("("),
            t("IDENT"),
            t(")"),
            t("{"),
            n("StmtList"),
            t("}")
        ]
    );
    p!(
        "ScopeStmt",
        [
            t("{"),
            t("@"),
            n("ScopeKind"),
            t(":"),
            t("IDENT"),
            t(";"),
            n("StmtList"),
            t("}")
        ]
    );

    // Expression and postfix forms. A bare atom and an atom with one or more
    // suffixes are separate productions; the suffix list itself is non-empty.
    p!("Expr", [n("Atom")]);
    p!("Expr", [n("Atom"), n("Suffix")]);

    p!("Suffix", [n("CallSuffix")]);
    p!("Suffix", [n("IndexSuffix")]);
    p!("Suffix", [n("FieldSuffix")]);
    p!("Suffix", [n("TrySuffix")]);
    p!("Suffix", [n("StructInitSuffix")]);
    p!("Suffix", [n("Turbofish"), n("TurbofishSuffix")]);

    p!("CallSuffix", [t("("), n("ExprList"), n("OptComma"), t(")")]);
    p!(
        "TurbofishSuffix",
        [t("("), n("ExprList"), n("OptComma"), t(")")]
    );
    p!(
        "TurbofishSuffix",
        [t("{"), n("FieldInitList"), n("OptComma"), t("}")]
    );
    p!("IndexSuffix", [t("["), n("Expr"), t("]")]);
    p!("FieldSuffix", [t("."), n("FieldName")]);
    p!("FieldName", [t("IDENT")]);
    p!("FieldName", [t("INT")]);
    p!("TrySuffix", [t("?")]);
    p!(
        "StructInitSuffix",
        [t("{"), n("FieldInitList"), n("OptComma"), t("}")]
    );
    p!("FieldInit", [t("IDENT"), t(":"), n("Expr")]);
    p!("MapEntry", [n("Expr"), t(":"), n("Expr")]);

    p!("Atom", [n("Literal")]);
    p!("Atom", [t("IDENT")]);
    p!("Atom", [t("("), n("Expr"), t(")")]);
    p!("Atom", [t("["), n("ExprList"), n("OptComma"), t("]")]);
    p!("Atom", [t("{"), n("MapEntryList"), n("OptComma"), t("}")]);
    p!("Atom", [n("FailExpr")]);
    p!("Atom", [n("SpawnExpr")]);

    p!(
        "FailExpr",
        [
            t("fail"),
            n("TypePath"),
            t("("),
            n("ExprList"),
            n("OptComma"),
            t(")")
        ]
    );
    p!(
        "FailExpr",
        [
            t("fail"),
            n("TypePath"),
            t("{"),
            n("FieldInitList"),
            n("OptComma"),
            t("}")
        ]
    );

    p!("SpawnExpr", [t("spawn"), n("Block")]);
    p!("SpawnExpr", [t("spawn"), t("||"), n("Block")]);

    p!("Literal", [t("INT")]);
    p!("Literal", [t("STR")]);
    p!("Literal", [t("true")]);
    p!("Literal", [t("false")]);

    list!("IdentList", t("IDENT"));
    list!("ParamList", n("Param"));
    list!("FieldList", n("Field"));
    list!("TypeList", n("Type"));
    list!("VariantList", n("Variant"));
    list!("ExprList", n("Expr"));
    list!("FieldInitList", n("FieldInit"));
    list!("MapEntryList", n("MapEntry"));

    p!("OptComma", []);
    p!("OptComma", [t(",")]);

    p!("Field", [t("IDENT"), t(":"), n("Type")]);
    p!("Variant", [t("IDENT")]);
    p!("Variant", [t("IDENT"), n("VariantFields")]);
    p!(
        "VariantFields",
        [t("("), n("TypeList"), n("OptComma"), t(")")]
    );

    grammar.start = nts["Program"];
    grammar
}

#[cfg(test)]
mod tests {
    use super::*;
    use qana_grammar::build_lr;

    #[test]
    fn v0_2_grammar_is_lr1_conflict_free() {
        let grammar = v0_2_grammar();
        let tables = build_lr(&grammar);
        assert!(
            tables.conflicts.is_empty(),
            "v0.2 grammar has {} conflict(s):\n{:#?}",
            tables.conflicts.len(),
            tables.conflicts
        );
    }

    #[test]
    fn checker_reports_a_known_shift_reduce_ambiguity() {
        // The classic dangling-else grammar. If the checker silently accepted
        // this, a green v0.2 result would mean nothing.
        let mut grammar = SynGrammar::new(
            "dangling-else",
            ["if", "else", "x", "(", ")"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        );
        let tokens: HashMap<&str, TokenId> = ["if", "else", "x", "(", ")"]
            .iter()
            .enumerate()
            .map(|(index, name)| (*name, index as TokenId))
            .collect();
        let statement = grammar.nt("Statement");
        let if_token = Sym::T(tokens["if"]);
        let else_token = Sym::T(tokens["else"]);
        let x_token = Sym::T(tokens["x"]);
        let open = Sym::T(tokens["("]);
        let close = Sym::T(tokens[")"]);

        grammar.prod(statement, vec![if_token, open, close, Sym::N(statement)]);
        grammar.prod(
            statement,
            vec![
                if_token,
                open,
                close,
                Sym::N(statement),
                else_token,
                Sym::N(statement),
            ],
        );
        grammar.prod(statement, vec![x_token]);
        grammar.start = statement;

        let tables = build_lr(&grammar);
        assert!(
            !tables.conflicts.is_empty(),
            "checker failed to report the dangling-else shift/reduce conflict"
        );
    }

    #[test]
    fn grammar_covers_the_v0_2_ambiguity_forms() {
        let grammar = v0_2_grammar();
        for token in ["fail", "spawn", "||", "::<", "mode", "memory"] {
            assert!(
                grammar.term_names.iter().any(|name| name == token),
                "missing token {token:?}"
            );
        }

        let displays: Vec<String> = (0..grammar.prods.len())
            .map(|index| grammar.prod_display(index))
            .collect();
        let joined = displays.join("\n");
        for needle in [
            "FailExpr",
            "SpawnExpr",
            "Turbofish",
            "ScopeItems",
            "ScopeStmt",
            "StructInitSuffix",
        ] {
            assert!(
                joined.contains(needle),
                "grammar does not cover {needle}:\n{joined}"
            );
        }
    }

    #[test]
    fn trailing_commas_are_represented() {
        let grammar = v0_2_grammar();
        let displays: Vec<String> = (0..grammar.prods.len())
            .map(|index| grammar.prod_display(index))
            .collect();
        assert!(
            displays.iter().any(|line| line == "OptComma → ,"),
            "the optional trailing comma production is missing:\n{}",
            displays.join("\n")
        );
        let uses = displays
            .iter()
            .filter(|line| line.contains("OptComma"))
            .count();
        assert!(
            uses >= 8,
            "expected the optional trailing comma at each list site, found {uses}:\n{}",
            displays.join("\n")
        );
    }
}
