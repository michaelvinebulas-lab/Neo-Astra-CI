//! Resolved high-level intermediate representation.
//!
//! HIR keeps source syntax close enough for type checking and Nova while adding
//! the symbol tables and export surfaces produced by resolution.

use std::collections::BTreeMap;

use crate::ast;
use crate::span::Span;
use crate::types::Type;

/// A resolved module.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Module {
    /// Dotted module name, or an empty string when the source has no header.
    pub name: String,
    /// Declarations in source order, including declarations introduced by
    /// `export default fn`.
    pub items: Vec<Item>,
    /// Import declarations in source order.
    pub imports: Vec<Import>,
    /// Local declarations and imported bindings by symbol namespace.
    pub symbols: SymbolTable,
    /// The module's public export surface.
    pub exports: ExportSurface,
}

/// A resolved top-level declaration.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Item {
    pub name: String,
    pub kind: ItemKind,
    /// Reserved for the type checker. Resolution leaves this as `None`.
    pub ty: Option<Type>,
    pub span: Span,
    pub visibility: ast::Visibility,
    /// True when the source used `pub`/`public` or `export` on the declaration.
    pub exported: bool,
    /// Source declaration consumed by the type checker and interpreter.
    pub declaration: Declaration,
}

/// The kind of a resolved top-level item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ItemKind {
    Function,
    Struct,
    Enum,
    Constant,
    TypeAlias,
    ForeignModule,
    ScriptBlock,
}

impl ItemKind {
    /// The namespace contribution of this declaration.
    #[must_use]
    pub const fn export_kind(self) -> ExportKind {
        match self {
            Self::Function | Self::Constant => ExportKind::Value,
            Self::Struct | Self::Enum => ExportKind::ValueAndType,
            Self::TypeAlias => ExportKind::Type,
            Self::ForeignModule | Self::ScriptBlock => ExportKind::Namespace,
        }
    }
}

/// Source declaration retained by HIR.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Declaration {
    Function(ast::FunctionDecl),
    Struct(ast::StructDecl),
    Enum(ast::EnumDecl),
    Constant(ast::ConstantDecl),
    TypeAlias(ast::TypeAliasDecl),
    ForeignModule(ast::ForeignModuleDecl),
    ScriptBlock(ast::ScriptBlock),
}

impl Declaration {
    /// The declaration kind.
    #[must_use]
    pub const fn kind(&self) -> ItemKind {
        match self {
            Self::Function(_) => ItemKind::Function,
            Self::Struct(_) => ItemKind::Struct,
            Self::Enum(_) => ItemKind::Enum,
            Self::Constant(_) => ItemKind::Constant,
            Self::TypeAlias(_) => ItemKind::TypeAlias,
            Self::ForeignModule(_) => ItemKind::ForeignModule,
            Self::ScriptBlock(_) => ItemKind::ScriptBlock,
        }
    }
}

/// A resolved import declaration.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Import {
    /// Canonical module key used during resolution.
    pub module: String,
    pub kind: ImportKind,
    pub span: Span,
}

/// The binding shape of an import.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ImportKind {
    SideEffect,
    Namespace {
        alias: String,
    },
    Named {
        bindings: Vec<ImportBinding>,
    },
    Default {
        local: String,
    },
    DefaultNamed {
        default: String,
        bindings: Vec<ImportBinding>,
    },
}

/// One named import binding.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ImportBinding {
    pub imported: String,
    pub local: String,
    pub span: Span,
}

/// Namespaces used by the resolver.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SymbolTable {
    pub values: BTreeMap<String, Symbol>,
    pub types: BTreeMap<String, Symbol>,
    pub namespaces: BTreeMap<String, Symbol>,
}

impl SymbolTable {
    /// Returns a symbol with `name` in any namespace, preferring values.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Symbol> {
        self.values
            .get(name)
            .or_else(|| self.types.get(name))
            .or_else(|| self.namespaces.get(name))
    }

    /// Returns whether `name` is bound in any namespace.
    #[must_use]
    pub fn contains_name(&self, name: &str) -> bool {
        self.values.contains_key(name)
            || self.types.contains_key(name)
            || self.namespaces.contains_key(name)
    }
}

/// A resolved local declaration or imported binding.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub visibility: ast::Visibility,
    /// True for declarations marked `pub`/`public` or `export`.
    pub exported: bool,
    pub span: Span,
    pub origin: SymbolOrigin,
}

/// Symbol categories visible to the resolver and type checker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    Constant,
    TypeAlias,
    ForeignModule,
    ScriptBlock,
    ImportedValue,
    ImportedType,
    ImportedNamespace,
}

impl SymbolKind {
    /// The export surface kind contributed by this symbol.
    #[must_use]
    pub const fn export_kind(self) -> ExportKind {
        match self {
            Self::Function | Self::Constant | Self::ImportedValue => ExportKind::Value,
            Self::Struct | Self::Enum => ExportKind::ValueAndType,
            Self::TypeAlias | Self::ImportedType => ExportKind::Type,
            Self::ForeignModule | Self::ImportedNamespace | Self::ScriptBlock => ExportKind::Namespace,
        }
    }
}

/// Where a resolved symbol came from.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SymbolOrigin {
    Local,
    Imported { module: String, name: String },
}

/// A module's public export surface.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ExportSurface {
    /// Named exports, sorted by exported name.
    pub named: BTreeMap<String, Export>,
    /// The module's single default export, when present.
    pub default: Option<Export>,
    /// Sources of `export * from` declarations that could not be expanded.
    pub stars: Vec<StarExport>,
}

impl ExportSurface {
    /// Returns a named export, excluding the default export.
    #[must_use]
    pub fn get_named(&self, name: &str) -> Option<&Export> {
        self.named.get(name)
    }
}

/// A retained `export * from` source.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StarExport {
    pub module: String,
    pub span: Span,
}

/// One resolved export.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Export {
    pub name: String,
    pub kind: ExportKind,
    pub target: ExportTarget,
    pub span: Span,
}

/// The value/type shape exported under a name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ExportKind {
    Value,
    Type,
    ValueAndType,
    Namespace,
}

impl ExportKind {
    /// Returns whether the export has a runtime value.
    #[must_use]
    pub const fn is_value(self) -> bool {
        matches!(self, Self::Value | Self::ValueAndType | Self::Namespace)
    }

    /// Returns whether the export has a type meaning.
    #[must_use]
    pub const fn is_type(self) -> bool {
        matches!(self, Self::Type | Self::ValueAndType)
    }
}

/// Where an export gets its value or type.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ExportTarget {
    /// A declaration or imported binding in the current module.
    Local(String),
    /// A named or default export of another module.
    Reexport { module: String, name: String },
    /// The namespace object of another module.
    Namespace { module: String },
    /// An anonymous default expression.
    Expression,
}
