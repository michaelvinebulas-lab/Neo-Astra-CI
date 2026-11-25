//! Name, module, and export resolution.
//!
//! Resolution is intentionally split from type checking. It establishes the
//! local symbol tables, import bindings, export surfaces, and visibility rules
//! that the type checker and Nova consume.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{
    self, ExportForm, ImportForm, ItemKind as AstItemKind, Type as AstType, TypeKind, TypePath,
    Visibility,
};
use crate::diagnostic::Diagnostic;
use crate::hir;
use crate::hir::{
    Declaration, Export, ExportKind, ExportSurface, ExportTarget, Import, ImportBinding,
    ImportKind, Item, ItemKind, StarExport, Symbol, SymbolKind, SymbolOrigin, SymbolTable,
};
use crate::span::Span;

const PRIVATE_EXPORT: &str = "NS3001";
const DUPLICATE_EXPORT: &str = "NS3003";
const DUPLICATE_DEFAULT_EXPORT: &str = "NS3004";
const UNKNOWN_EXPORT_TARGET: &str = "NS3005";
const DUPLICATE_IMPORT_BINDING: &str = "NS3006";
const MISSING_IMPORTED_NAME: &str = "NS3007";
const CONFLICTING_REEXPORTS: &str = "NS3008";
const DEFAULT_NOT_VALUE: &str = "NS3009";
const PRIVATE_LEAK: &str = "NS3010";
const DUPLICATE_DECLARATION: &str = "NS3011";
const UNRESOLVED_MODULE: &str = "NS3012";
const MODULE_CYCLE: &str = "NS3013";
const SCRIPT_NAMESPACE_COLLISION: &str = "NS7006";

/// Result of a resolver operation.
pub type ResolveResult<T> = Result<T, Vec<Diagnostic>>;

/// Resolves source modules into HIR.
pub trait Resolver {
    /// Resolves one module in standalone mode.
    fn resolve_module(&self, module: &ast::Module) -> ResolveResult<hir::Module>;
}

/// A parsed project module graph and its resolved import edges.
///
/// Module keys are canonical project identities, not raw source specifiers.
/// The import map records the exact target chosen for every importer/specifier
/// pair so relative paths from different directories cannot collide.
#[derive(Debug, Clone, Default)]
pub struct ModuleGraph {
    /// Parsed modules keyed by canonical module identity.
    pub modules: BTreeMap<String, ast::Module>,
    /// `(importer, raw specifier) -> canonical target`.
    pub imports: BTreeMap<(String, String), String>,
}

impl ModuleGraph {
    /// Inserts a parsed module under its canonical identity.
    pub fn insert_module(&mut self, id: impl Into<String>, module: ast::Module) {
        self.modules.insert(id.into(), module);
    }

    /// Records a resolved import edge.
    pub fn insert_import(
        &mut self,
        importer: impl Into<String>,
        specifier: impl Into<String>,
        target: impl Into<String>,
    ) {
        self.imports
            .insert((importer.into(), specifier.into()), target.into());
    }
}

/// A deterministic resolver over an optional set of source modules.
///
/// `standalone` accepts external imports as opaque modules because a single
/// source unit does not include the project module graph. `from_modules`
/// enables strict import validation against raw specifier keys.
/// `from_graph` uses canonical module identities and an explicit import map.
#[derive(Debug, Default, Clone)]
pub struct ModuleResolver {
    modules: BTreeMap<String, ast::Module>,
    imports: BTreeMap<(String, String), String>,
    strict_imports: bool,
    identity_imports: bool,
}

impl ModuleResolver {
    /// Creates a resolver for one module without a module graph.
    #[must_use]
    pub fn standalone() -> Self {
        Self::default()
    }

    /// Creates a strict resolver over a module graph keyed by resolved module
    /// paths or specifiers.
    ///
    /// This convenience form treats every raw specifier as its own module key.
    /// Use [`ModuleResolver::from_graph`] when canonical module identities
    /// differ from source specifiers.
    #[must_use]
    pub fn from_modules<I>(modules: I) -> Self
    where
        I: IntoIterator<Item = (String, ast::Module)>,
    {
        Self {
            modules: modules.into_iter().collect(),
            imports: BTreeMap::new(),
            strict_imports: true,
            identity_imports: true,
        }
    }

    /// Creates a strict resolver from a project module graph and its explicit
    /// `(importer, raw specifier) -> target` import map.
    #[must_use]
    pub fn from_graph(graph: &ModuleGraph) -> Self {
        Self {
            modules: graph.modules.clone(),
            imports: graph.imports.clone(),
            strict_imports: true,
            identity_imports: false,
        }
    }

    /// Resolves `module` as the root module.
    pub fn resolve(&self, module: &ast::Module) -> ResolveResult<hir::Module> {
        let key = module_key(module);
        let mut state = ResolutionState::new(self);
        state.stack.push(key.clone());
        let result = state.resolve_ast(&key, module);
        state.stack.pop();
        result
    }

    /// Resolves a module from the supplied graph by key.
    pub fn resolve_named(&self, name: &str) -> ResolveResult<hir::Module> {
        let Some(module) = self.modules.get(name).cloned() else {
            return Err(vec![unresolved_module(name, None)]);
        };

        let mut state = ResolutionState::new(self);
        state.stack.push(name.to_owned());
        let result = state.resolve_ast(name, &module);
        state.stack.pop();
        result
    }

    /// Resolves a root module by canonical project identity.
    ///
    /// This is an alias for [`Self::resolve_named`] that makes project-graph
    /// call sites explicit.
    pub fn resolve_root(&self, id: &str) -> ResolveResult<hir::Module> {
        self.resolve_named(id)
    }
}

impl Resolver for ModuleResolver {
    fn resolve_module(&self, module: &ast::Module) -> ResolveResult<hir::Module> {
        self.resolve(module)
    }
}

/// Resolves one standalone module.
pub fn resolve_module(module: &ast::Module) -> ResolveResult<hir::Module> {
    ModuleResolver::standalone().resolve_module(module)
}

/// A raw module specifier found in an import or re-export declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleSpecifier {
    /// The specifier exactly as written, except dotted paths are joined with
    /// `.` (for example `app.util`).
    pub specifier: String,
    /// Span of the import or export declaration.
    pub span: Span,
}

/// Collects every module specifier from top-level imports and re-exports.
///
/// The loader uses this to discover the project graph without duplicating AST
/// traversal. Local and external classification happens after collection.
#[must_use]
pub fn collect_module_specifiers(module: &ast::Module) -> Vec<ModuleSpecifier> {
    let mut specifiers = Vec::new();
    for item in &module.items {
        match &item.kind {
            AstItemKind::Import(import) => {
                let specifier = match &import.form {
                    ImportForm::Path { path, .. } => Some(path.join(".")),
                    ImportForm::SideEffect { specifier }
                    | ImportForm::Namespace { specifier, .. }
                    | ImportForm::Named { specifier, .. }
                    | ImportForm::Default { specifier, .. }
                    | ImportForm::DefaultNamed { specifier, .. } => Some(specifier.clone()),
                };
                if let Some(specifier) = specifier {
                    specifiers.push(ModuleSpecifier {
                        specifier,
                        span: import.span,
                    });
                }
            }
            AstItemKind::Export(export) => {
                let specifier = match &export.form {
                    ExportForm::List { from, .. } => from.clone(),
                    ExportForm::All { from, .. } => Some(from.clone()),
                    ExportForm::DefaultValue(_) | ExportForm::DefaultFunction(_) => None,
                };
                if let Some(specifier) = specifier {
                    specifiers.push(ModuleSpecifier {
                        specifier,
                        span: export.span,
                    });
                }
            }
            _ => {}
        }
    }
    specifiers
}

fn module_key(module: &ast::Module) -> String {
    if module.name.is_empty() {
        "<root>".to_owned()
    } else {
        module.name.join(".")
    }
}

fn unresolved_module(name: &str, span: Option<Span>) -> Diagnostic {
    let diagnostic = Diagnostic::error(
        UNRESOLVED_MODULE,
        format!("Imported module '{name}' could not be resolved."),
    )
    .with_hint("Add the module to the project module graph or fix the import specifier.");
    if let Some(span) = span {
        diagnostic.with_span(span)
    } else {
        diagnostic
    }
}

struct ResolutionState<'a> {
    resolver: &'a ModuleResolver,
    cache: BTreeMap<String, ResolveResult<hir::Module>>,
    stack: Vec<String>,
    reported: BTreeSet<String>,
}

impl<'a> ResolutionState<'a> {
    fn new(resolver: &'a ModuleResolver) -> Self {
        Self {
            resolver,
            cache: BTreeMap::new(),
            stack: Vec::new(),
            reported: BTreeSet::new(),
        }
    }

    fn resolve_ast(&mut self, key: &str, module: &ast::Module) -> ResolveResult<hir::Module> {
        let mut decls = Vec::new();
        let mut imports = Vec::new();
        let mut exports = Vec::new();
        collect_items(&module.items, &mut decls, &mut imports, &mut exports);

        let mut pass = ModulePass {
            state: self,
            key: key.to_owned(),
            module,
            diagnostics: Vec::new(),
            items: Vec::new(),
            resolved_imports: Vec::new(),
            symbols: SymbolTable::default(),
            resolved_exports: ExportSurface::default(),
            decls,
            imports_ast: imports,
            exports_ast: exports,
            declaration_bindings: BTreeMap::new(),
            import_bindings: BTreeMap::new(),
            default_function_names: BTreeSet::new(),
            script_declarations: BTreeSet::new(),
            pending_stars: Vec::new(),
        };

        pass.run(key)
    }

    fn resolve_named(&mut self, name: &str, span: Option<Span>) -> ResolveResult<hir::Module> {
        if let Some(cached) = self.cache.get(name) {
            return cached.clone();
        }

        if self.stack.iter().any(|entry| entry == name) {
            let diagnostic = Diagnostic::error(
                MODULE_CYCLE,
                format!("Cyclic module import involving '{name}'."),
            )
            .with_hint("Break the named import or re-export cycle.");
            let diagnostic = if let Some(span) = span {
                diagnostic.with_span(span)
            } else {
                diagnostic
            };
            return Err(vec![diagnostic]);
        }

        let Some(module) = self.resolver.modules.get(name).cloned() else {
            return Err(vec![unresolved_module(name, span)]);
        };

        self.stack.push(name.to_owned());
        let result = self.resolve_ast(name, &module);
        self.stack.pop();
        self.cache.insert(name.to_owned(), result.clone());
        result
    }
}

#[derive(Debug, Clone, Copy)]
enum DeclRef<'a> {
    Item(&'a ast::Item),
    DefaultFunction(&'a ast::FunctionDecl),
}

impl<'a> DeclRef<'a> {
    fn name(self) -> &'a str {
        match self {
            Self::Item(item) => match &item.kind {
                AstItemKind::Function(declaration) => &declaration.name,
                AstItemKind::Struct(declaration) => &declaration.name,
                AstItemKind::Enum(declaration) => &declaration.name,
                AstItemKind::Constant(declaration) => &declaration.name,
                AstItemKind::TypeAlias(declaration) => &declaration.name,
                AstItemKind::ForeignModule(declaration) => &declaration.name,
                AstItemKind::ScriptBlock(declaration) => &declaration.name,
                AstItemKind::Import(_) | AstItemKind::Export(_) | AstItemKind::ScopedItems(_) => "",
            },
            Self::DefaultFunction(function) => &function.name,
        }
    }

    fn span(self) -> Span {
        match self {
            Self::Item(item) => item.span,
            Self::DefaultFunction(function) => function.span,
        }
    }

    fn visibility(self) -> Visibility {
        match self {
            Self::Item(item) => item.visibility,
            Self::DefaultFunction(_) => Visibility::Normal,
        }
    }

    fn exported(self) -> bool {
        match self {
            Self::Item(item) => item.exported,
            Self::DefaultFunction(_) => true,
        }
    }

    fn is_default_function(self) -> bool {
        matches!(self, Self::DefaultFunction(_))
    }

    fn declaration(self) -> Declaration {
        match self {
            Self::Item(item) => match &item.kind {
                AstItemKind::Function(declaration) => Declaration::Function(declaration.clone()),
                AstItemKind::Struct(declaration) => Declaration::Struct(declaration.clone()),
                AstItemKind::Enum(declaration) => Declaration::Enum(declaration.clone()),
                AstItemKind::Constant(declaration) => Declaration::Constant(declaration.clone()),
                AstItemKind::TypeAlias(declaration) => Declaration::TypeAlias(declaration.clone()),
                AstItemKind::ForeignModule(declaration) => {
                    Declaration::ForeignModule(declaration.clone())
                }
                AstItemKind::ScriptBlock(declaration) => {
                    Declaration::ScriptBlock(declaration.clone())
                }
                AstItemKind::Import(_) | AstItemKind::Export(_) | AstItemKind::ScopedItems(_) => {
                    unreachable!("only declarations are collected as DeclRef")
                }
            },
            Self::DefaultFunction(function) => Declaration::Function(function.clone()),
        }
    }
}

fn collect_items<'a>(
    items: &'a [ast::Item],
    decls: &mut Vec<DeclRef<'a>>,
    imports: &mut Vec<&'a ast::ImportDecl>,
    exports: &mut Vec<&'a ast::ExportDecl>,
) {
    for item in items {
        match &item.kind {
            AstItemKind::Import(import) => imports.push(import),
            AstItemKind::Export(export) => {
                if let ExportForm::DefaultFunction(function) = &export.form {
                    decls.push(DeclRef::DefaultFunction(function));
                }
                exports.push(export);
            }
            AstItemKind::ScopedItems(scoped) => {
                collect_items(&scoped.items, decls, imports, exports);
            }
            _ => decls.push(DeclRef::Item(item)),
        }
    }
}

struct PendingStar {
    module: String,
    target: TargetSurface,
    span: Span,
}

#[derive(Debug)]
enum TargetSurface {
    Resolved(ExportSurface),
    Opaque,
    Missing,
}

struct LocalBinding {
    kind: ExportKind,
    visibility: Visibility,
}

struct ModulePass<'m, 's, 'r> {
    state: &'s mut ResolutionState<'r>,
    key: String,
    module: &'m ast::Module,
    diagnostics: Vec<Diagnostic>,
    items: Vec<Item>,
    resolved_imports: Vec<Import>,
    symbols: SymbolTable,
    resolved_exports: ExportSurface,
    decls: Vec<DeclRef<'m>>,
    imports_ast: Vec<&'m ast::ImportDecl>,
    exports_ast: Vec<&'m ast::ExportDecl>,
    declaration_bindings: BTreeMap<String, Span>,
    import_bindings: BTreeMap<String, Span>,
    default_function_names: BTreeSet<String>,
    script_declarations: BTreeSet<String>,
    pending_stars: Vec<PendingStar>,
}

impl<'m, 's, 'r> ModulePass<'m, 's, 'r> {
    fn run(&mut self, _key: &str) -> ResolveResult<hir::Module> {
        for declaration in self.decls.clone() {
            self.add_declaration(declaration);
        }

        for import in self.imports_ast.clone() {
            self.process_import(import);
        }

        self.add_declaration_exports();

        for export in self.exports_ast.clone() {
            self.process_export(export);
        }

        self.expand_star_exports();
        self.check_private_leaks();

        if !self.diagnostics.is_empty() {
            return Err(std::mem::take(&mut self.diagnostics));
        }

        Ok(hir::Module {
            name: self.module.name.join("."),
            items: std::mem::take(&mut self.items),
            imports: std::mem::take(&mut self.resolved_imports),
            symbols: std::mem::take(&mut self.symbols),
            exports: std::mem::take(&mut self.resolved_exports),
        })
    }

    fn error(&mut self, code: &str, message: impl Into<String>, span: Span, hint: &str) {
        self.diagnostics.push(
            Diagnostic::error(code, message)
                .with_span(span)
                .with_hint(hint),
        );
    }

    fn add_declaration(&mut self, declaration: DeclRef<'m>) {
        let name = declaration.name().to_owned();
        if name.is_empty() {
            return;
        }

        let span = declaration.span();
        let kind = declaration.declaration().kind();
        let is_script = kind == ItemKind::ScriptBlock;
        if let Some(previous) = self.declaration_bindings.get(&name) {
            let collision_with_script = is_script || self.script_declarations.contains(&name);
            let code = if collision_with_script {
                SCRIPT_NAMESPACE_COLLISION
            } else {
                DUPLICATE_DECLARATION
            };
            self.error(
                code,
                format!("Duplicate declaration '{name}'."),
                span,
                &format!(
                    "The first declaration starts at byte {}. Rename one of the declarations.",
                    previous.start
                ),
            );
            return;
        }

        if is_script {
            self.script_declarations.insert(name.clone());
        }

        self.declaration_bindings.insert(name.clone(), span);

        let visibility = declaration.visibility();
        let exported = declaration.exported() || visibility == Visibility::Public;
        let symbol = Symbol {
            name: name.clone(),
            kind: symbol_kind(kind),
            visibility,
            exported,
            span,
            origin: SymbolOrigin::Local,
        };

        match kind {
            ItemKind::Function | ItemKind::Constant => {
                self.symbols.values.insert(name.clone(), symbol);
            }
            ItemKind::Struct | ItemKind::Enum => {
                self.symbols.values.insert(name.clone(), symbol.clone());
                self.symbols.types.insert(name.clone(), symbol);
            }
            ItemKind::TypeAlias => {
                self.symbols.types.insert(name.clone(), symbol);
            }
            ItemKind::ForeignModule | ItemKind::ScriptBlock => {
                self.symbols.namespaces.insert(name.clone(), symbol);
            }
        }

        if declaration.is_default_function() {
            self.default_function_names.insert(name.clone());
        }

        self.items.push(Item {
            name,
            kind,
            ty: None,
            span,
            visibility,
            exported,
            declaration: declaration.declaration(),
        });
    }

    fn add_declaration_exports(&mut self) {
        let exports: Vec<(String, ExportKind, Span)> = self
            .items
            .iter()
            .filter(|item| !self.default_function_names.contains(&item.name))
            .filter(|item| item.visibility == Visibility::Public || item.exported)
            .map(|item| (item.name.clone(), item.kind.export_kind(), item.span))
            .collect();

        for (name, kind, span) in exports {
            self.add_named_export(name.clone(), kind, ExportTarget::Local(name), span);
        }
    }

    fn process_import(&mut self, import: &'m ast::ImportDecl) {
        match &import.form {
            ImportForm::Path { path, alias } => {
                let specifier = path.join(".");
                let Some(module) = self.canonical_target(&specifier, import.span) else {
                    return;
                };
                let alias = alias
                    .clone()
                    .or_else(|| path.last().cloned())
                    .unwrap_or_default();
                let available = self.target_exists(&module, import.span);
                if available && !alias.is_empty() {
                    self.add_import_namespace(&module, &alias, import.span);
                }
                self.resolved_imports.push(Import {
                    module,
                    kind: ImportKind::Namespace { alias },
                    span: import.span,
                });
            }
            ImportForm::SideEffect { specifier } => {
                let Some(module) = self.canonical_target(specifier, import.span) else {
                    return;
                };
                self.target_exists(&module, import.span);
                self.resolved_imports.push(Import {
                    module,
                    kind: ImportKind::SideEffect,
                    span: import.span,
                });
            }
            ImportForm::Namespace { alias, specifier } => {
                let Some(module) = self.canonical_target(specifier, import.span) else {
                    return;
                };
                let available = self.target_exists(&module, import.span);
                if available && !alias.is_empty() {
                    self.add_import_namespace(&module, alias, import.span);
                }
                self.resolved_imports.push(Import {
                    module,
                    kind: ImportKind::Namespace {
                        alias: alias.clone(),
                    },
                    span: import.span,
                });
            }
            ImportForm::Named {
                specifiers,
                specifier,
            } => {
                let Some(module) = self.canonical_target(specifier, import.span) else {
                    return;
                };
                let target = self.target_surface(&module, import.span);
                let mut bindings = Vec::new();
                for binding_ast in specifiers {
                    if let Some(binding) = self.import_named(&target, binding_ast, &module) {
                        bindings.push(binding);
                    }
                }
                self.resolved_imports.push(Import {
                    module,
                    kind: ImportKind::Named { bindings },
                    span: import.span,
                });
            }
            ImportForm::Default { alias, specifier } => {
                let Some(module) = self.canonical_target(specifier, import.span) else {
                    return;
                };
                let target = self.target_surface(&module, import.span);
                let _ = self.import_default(&target, &module, alias, import.span);
                self.resolved_imports.push(Import {
                    module,
                    kind: ImportKind::Default {
                        local: alias.clone(),
                    },
                    span: import.span,
                });
            }
            ImportForm::DefaultNamed {
                default,
                specifiers,
                specifier,
            } => {
                let Some(module) = self.canonical_target(specifier, import.span) else {
                    return;
                };
                let target = self.target_surface(&module, import.span);
                let _ = self.import_default(&target, &module, default, import.span);
                let mut bindings = Vec::new();
                for binding_ast in specifiers {
                    if let Some(binding) = self.import_named(&target, binding_ast, &module) {
                        bindings.push(binding);
                    }
                }
                self.resolved_imports.push(Import {
                    module,
                    kind: ImportKind::DefaultNamed {
                        default: default.clone(),
                        bindings,
                    },
                    span: import.span,
                });
            }
        }
    }

    /// Converts a raw source specifier into the canonical target chosen by the
    /// project loader. Standalone resolution keeps specifiers opaque, and the
    /// compatibility `from_modules` resolver uses identity keys.
    fn canonical_target(&mut self, specifier: &str, span: Span) -> Option<String> {
        if let Some(target) = self
            .state
            .resolver
            .imports
            .get(&(self.key.clone(), specifier.to_owned()))
        {
            return Some(target.clone());
        }

        if !self.state.resolver.strict_imports {
            return Some(specifier.to_owned());
        }

        if self.state.resolver.identity_imports
            && self.state.resolver.modules.contains_key(specifier)
        {
            return Some(specifier.to_owned());
        }

        self.diagnostics
            .push(unresolved_module(specifier, Some(span)));
        None
    }

    fn target_exists(&mut self, module: &str, span: Span) -> bool {
        if self.state.resolver.modules.contains_key(module) {
            return true;
        }

        if self.state.resolver.strict_imports {
            self.diagnostics.push(unresolved_module(module, Some(span)));
            return false;
        }

        true
    }

    fn target_surface(&mut self, module: &str, span: Span) -> TargetSurface {
        if !self.state.resolver.modules.contains_key(module) {
            if self.state.resolver.strict_imports {
                self.diagnostics.push(unresolved_module(module, Some(span)));
                return TargetSurface::Missing;
            }
            return TargetSurface::Opaque;
        }

        match self.state.resolve_named(module, Some(span)) {
            Ok(resolved) => TargetSurface::Resolved(resolved.exports),
            Err(diagnostics) => {
                if self.state.reported.insert(module.to_owned()) {
                    self.diagnostics.extend(diagnostics);
                }
                TargetSurface::Missing
            }
        }
    }

    fn add_import_namespace(&mut self, module: &str, alias: &str, span: Span) -> bool {
        if !self.claim_import_binding(alias, span) {
            return false;
        }

        self.symbols.namespaces.insert(
            alias.to_owned(),
            Symbol {
                name: alias.to_owned(),
                kind: SymbolKind::ImportedNamespace,
                visibility: Visibility::Normal,
                exported: false,
                span,
                origin: SymbolOrigin::Imported {
                    module: module.to_owned(),
                    name: "*".to_owned(),
                },
            },
        );
        true
    }

    fn import_named(
        &mut self,
        target: &TargetSurface,
        specifier: &ast::ImportSpecifier,
        module: &str,
    ) -> Option<ImportBinding> {
        if specifier.imported == "default" && specifier.local == "default" {
            self.error(
                DUPLICATE_IMPORT_BINDING,
                "The default import must be aliased to a local name.",
                specifier.span,
                "Write `default as localName`.",
            );
            return None;
        }

        let kind = match target {
            TargetSurface::Missing => return None,
            TargetSurface::Opaque => {
                if specifier.imported == "default" {
                    ExportKind::Value
                } else {
                    ExportKind::ValueAndType
                }
            }
            TargetSurface::Resolved(exports) => {
                if specifier.imported == "default" {
                    let Some(export) = &exports.default else {
                        self.error(
                            MISSING_IMPORTED_NAME,
                            format!("Module '{module}' has no default export."),
                            specifier.span,
                            "Import a named export or add a default export.",
                        );
                        return None;
                    };
                    if !export.kind.is_value() {
                        self.error(
                            MISSING_IMPORTED_NAME,
                            format!("Module '{module}' default export is not a value."),
                            specifier.span,
                            "Default exports must be values.",
                        );
                        return None;
                    }
                    export.kind
                } else {
                    let Some(export) = exports.get_named(&specifier.imported) else {
                        self.error(
                            MISSING_IMPORTED_NAME,
                            format!(
                                "Module '{module}' does not export '{}'.",
                                specifier.imported
                            ),
                            specifier.span,
                            "Check the target module's export surface.",
                        );
                        return None;
                    };
                    export.kind
                }
            }
        };

        self.add_import_binding(
            module,
            &specifier.imported,
            &specifier.local,
            kind,
            specifier.span,
        )
    }

    fn import_default(
        &mut self,
        target: &TargetSurface,
        module: &str,
        local: &str,
        span: Span,
    ) -> Option<ImportBinding> {
        if local.is_empty() {
            return None;
        }

        let kind = match target {
            TargetSurface::Missing => return None,
            TargetSurface::Opaque => ExportKind::Value,
            TargetSurface::Resolved(exports) => {
                let Some(export) = &exports.default else {
                    self.error(
                        MISSING_IMPORTED_NAME,
                        format!("Module '{module}' has no default export."),
                        span,
                        "Import a named export or add a default export.",
                    );
                    return None;
                };
                if !export.kind.is_value() {
                    self.error(
                        MISSING_IMPORTED_NAME,
                        format!("Module '{module}' default export is not a value."),
                        span,
                        "Default exports must be values.",
                    );
                    return None;
                }
                export.kind
            }
        };

        self.add_import_binding(module, "default", local, kind, span)
    }

    fn add_import_binding(
        &mut self,
        module: &str,
        imported: &str,
        local: &str,
        kind: ExportKind,
        span: Span,
    ) -> Option<ImportBinding> {
        if local.is_empty() || !self.claim_import_binding(local, span) {
            return None;
        }

        let origin = SymbolOrigin::Imported {
            module: module.to_owned(),
            name: imported.to_owned(),
        };
        let symbol = |kind| Symbol {
            name: local.to_owned(),
            kind,
            visibility: Visibility::Normal,
            exported: false,
            span,
            origin: origin.clone(),
        };

        match kind {
            ExportKind::Value => {
                self.symbols
                    .values
                    .insert(local.to_owned(), symbol(SymbolKind::ImportedValue));
            }
            ExportKind::Type => {
                self.symbols
                    .types
                    .insert(local.to_owned(), symbol(SymbolKind::ImportedType));
            }
            ExportKind::ValueAndType => {
                self.symbols
                    .values
                    .insert(local.to_owned(), symbol(SymbolKind::ImportedValue));
                self.symbols
                    .types
                    .insert(local.to_owned(), symbol(SymbolKind::ImportedType));
            }
            ExportKind::Namespace => {
                self.symbols
                    .namespaces
                    .insert(local.to_owned(), symbol(SymbolKind::ImportedNamespace));
            }
        }

        Some(ImportBinding {
            imported: imported.to_owned(),
            local: local.to_owned(),
            span,
        })
    }

    fn claim_import_binding(&mut self, name: &str, span: Span) -> bool {
        if self.symbols.contains_name(name) || self.import_bindings.contains_key(name) {
            self.error(
                DUPLICATE_IMPORT_BINDING,
                format!("Duplicate import binding '{name}'."),
                span,
                "Rename one of the imported bindings.",
            );
            return false;
        }

        self.import_bindings.insert(name.to_owned(), span);
        true
    }

    fn process_export(&mut self, export: &'m ast::ExportDecl) {
        match &export.form {
            ExportForm::List { specifiers, from } => {
                if let Some(specifier) = from {
                    let Some(module) = self.canonical_target(specifier, export.span) else {
                        return;
                    };
                    let target = self.target_surface(&module, export.span);
                    for specifier in specifiers {
                        self.export_from(&target, &module, specifier);
                    }
                } else {
                    for specifier in specifiers {
                        self.export_local(specifier);
                    }
                }
            }
            ExportForm::All { alias, from } => {
                let Some(module) = self.canonical_target(from, export.span) else {
                    return;
                };
                if let Some(alias) = alias {
                    if !self.target_exists(&module, export.span) {
                        return;
                    }
                    let target = ExportTarget::Namespace {
                        module: module.clone(),
                    };
                    if alias == "default" {
                        self.add_default_export(Export {
                            name: "default".to_owned(),
                            kind: ExportKind::Namespace,
                            target,
                            span: export.span,
                        });
                    } else {
                        self.add_named_export(
                            alias.clone(),
                            ExportKind::Namespace,
                            target,
                            export.span,
                        );
                    }
                } else {
                    let target = self.target_surface(&module, export.span);
                    self.resolved_exports.stars.push(StarExport {
                        module: module.clone(),
                        span: export.span,
                    });
                    self.pending_stars.push(PendingStar {
                        module,
                        target,
                        span: export.span,
                    });
                }
            }
            ExportForm::DefaultValue(expression) => {
                let target = if let ast::ExprKind::Identifier(name) = &expression.kind {
                    let Some(binding) = self.lookup_local(name) else {
                        self.error(
                            UNKNOWN_EXPORT_TARGET,
                            format!("Unknown default export target '{name}'."),
                            expression.span,
                            "Export a local value or use an expression.",
                        );
                        return;
                    };
                    if binding.visibility == Visibility::Private {
                        self.error(
                            PRIVATE_EXPORT,
                            format!("Private declaration '{name}' cannot be exported."),
                            expression.span,
                            "Remove `private` or export a different value.",
                        );
                        return;
                    }
                    if !binding.kind.is_value() {
                        self.error(
                            DEFAULT_NOT_VALUE,
                            "The default export target is not a value.",
                            expression.span,
                            "Export a function, constant, or struct constructor.",
                        );
                        return;
                    }
                    ExportTarget::Local(name.clone())
                } else {
                    ExportTarget::Expression
                };

                self.add_default_export(Export {
                    name: "default".to_owned(),
                    kind: ExportKind::Value,
                    target,
                    span: export.span,
                });
            }
            ExportForm::DefaultFunction(function) => {
                let target = if function.name.is_empty() {
                    ExportTarget::Expression
                } else {
                    ExportTarget::Local(function.name.clone())
                };
                self.add_default_export(Export {
                    name: "default".to_owned(),
                    kind: ExportKind::Value,
                    target,
                    span: function.span,
                });
            }
        }
    }

    fn export_local(&mut self, specifier: &ast::ExportSpecifier) {
        if specifier.local == "default" {
            self.error(
                UNKNOWN_EXPORT_TARGET,
                "Cannot export `default` without a source module.",
                specifier.span,
                "Use `export { local as default }` or add `from \"module\"`.",
            );
            return;
        }

        let Some(binding) = self.lookup_local(&specifier.local) else {
            self.error(
                UNKNOWN_EXPORT_TARGET,
                format!("Unknown export target '{}'.", specifier.local),
                specifier.span,
                "Export a local declaration or imported binding.",
            );
            return;
        };

        if binding.visibility == Visibility::Private {
            self.error(
                PRIVATE_EXPORT,
                format!(
                    "Private declaration '{}' cannot be exported.",
                    specifier.local
                ),
                specifier.span,
                "Remove `private` or export a different declaration.",
            );
            return;
        }

        if specifier.exported == "default" {
            if !binding.kind.is_value() {
                self.error(
                    DEFAULT_NOT_VALUE,
                    "The default export target is not a value.",
                    specifier.span,
                    "Default exports must be functions, constants, or instances.",
                );
                return;
            }
            self.add_default_export(Export {
                name: "default".to_owned(),
                kind: binding.kind,
                target: ExportTarget::Local(specifier.local.clone()),
                span: specifier.span,
            });
        } else {
            self.add_named_export(
                specifier.exported.clone(),
                binding.kind,
                ExportTarget::Local(specifier.local.clone()),
                specifier.span,
            );
        }
    }

    fn export_from(
        &mut self,
        target: &TargetSurface,
        module: &str,
        specifier: &ast::ExportSpecifier,
    ) {
        let kind = match target {
            TargetSurface::Missing => return,
            TargetSurface::Opaque => {
                if specifier.local == "default" {
                    ExportKind::Value
                } else {
                    ExportKind::ValueAndType
                }
            }
            TargetSurface::Resolved(exports) => {
                if specifier.local == "default" {
                    let Some(export) = &exports.default else {
                        self.error(
                            MISSING_IMPORTED_NAME,
                            format!("Module '{module}' has no default export."),
                            specifier.span,
                            "Re-export a named export or add a default export.",
                        );
                        return;
                    };
                    export.kind
                } else {
                    let Some(export) = exports.get_named(&specifier.local) else {
                        self.error(
                            MISSING_IMPORTED_NAME,
                            format!("Module '{module}' does not export '{}'.", specifier.local),
                            specifier.span,
                            "Check the target module's export surface.",
                        );
                        return;
                    };
                    export.kind
                }
            }
        };

        let target = ExportTarget::Reexport {
            module: module.to_owned(),
            name: specifier.local.clone(),
        };

        if specifier.exported == "default" {
            if !kind.is_value() {
                self.error(
                    DEFAULT_NOT_VALUE,
                    "The default export target is not a value.",
                    specifier.span,
                    "Default exports must be values.",
                );
                return;
            }
            self.add_default_export(Export {
                name: "default".to_owned(),
                kind,
                target,
                span: specifier.span,
            });
        } else {
            self.add_named_export(specifier.exported.clone(), kind, target, specifier.span);
        }
    }

    fn lookup_local(&self, name: &str) -> Option<LocalBinding> {
        let value = self.symbols.values.get(name);
        let ty = self.symbols.types.get(name);
        let namespace = self.symbols.namespaces.get(name);

        let kind = match (value, ty, namespace) {
            (Some(_), Some(_), _) => ExportKind::ValueAndType,
            (Some(symbol), None, None) => symbol.kind.export_kind(),
            (None, Some(symbol), None) => symbol.kind.export_kind(),
            (None, None, Some(symbol)) => symbol.kind.export_kind(),
            (None, None, None) => return None,
            _ => return None,
        };

        let visibility = value
            .into_iter()
            .chain(ty)
            .chain(namespace)
            .map(|symbol| symbol.visibility)
            .find(|visibility| *visibility == Visibility::Private)
            .or_else(|| {
                value
                    .into_iter()
                    .chain(ty)
                    .chain(namespace)
                    .map(|symbol| symbol.visibility)
                    .find(|visibility| *visibility == Visibility::Public)
            })
            .unwrap_or(Visibility::Normal);

        Some(LocalBinding { kind, visibility })
    }

    fn add_named_export(
        &mut self,
        name: String,
        kind: ExportKind,
        target: ExportTarget,
        span: Span,
    ) {
        if name.is_empty() {
            return;
        }

        if name == "default" {
            if !kind.is_value() {
                self.error(
                    DEFAULT_NOT_VALUE,
                    "The default export target is not a value.",
                    span,
                    "Default exports must be values.",
                );
                return;
            }
            self.add_default_export(Export {
                name,
                kind,
                target,
                span,
            });
            return;
        }

        if let Some(existing) = self.resolved_exports.named.get(&name) {
            if existing.target == target && existing.kind == kind {
                return;
            }

            self.error(
                DUPLICATE_EXPORT,
                format!("Duplicate export name '{name}'."),
                span,
                "Rename one export or remove the duplicate.",
            );
            return;
        }

        self.resolved_exports.named.insert(
            name.clone(),
            Export {
                name,
                kind,
                target,
                span,
            },
        );
    }

    fn add_default_export(&mut self, export: Export) {
        if self.resolved_exports.default.is_some() {
            self.error(
                DUPLICATE_DEFAULT_EXPORT,
                "A module may have only one default export.",
                export.span,
                "Remove the extra default export.",
            );
            return;
        }

        if !export.kind.is_value() {
            self.error(
                DEFAULT_NOT_VALUE,
                "The default export target is not a value.",
                export.span,
                "Default exports must be values.",
            );
            return;
        }

        let mut export = export;
        if export.kind == ExportKind::ValueAndType {
            export.kind = ExportKind::Value;
        }
        self.resolved_exports.default = Some(export);
    }

    fn expand_star_exports(&mut self) {
        let pending = std::mem::take(&mut self.pending_stars);
        let mut origins: BTreeMap<String, String> = BTreeMap::new();

        for star in pending {
            let TargetSurface::Resolved(exports) = star.target else {
                continue;
            };

            for (name, source) in exports.named {
                if let Some(previous) = origins.get(&name) {
                    if previous != &star.module {
                        self.error(
                            CONFLICTING_REEXPORTS,
                            format!("Re-exported name '{name}' is provided by multiple modules."),
                            star.span,
                            "Export the name explicitly from one module.",
                        );
                    }
                    continue;
                }

                if self.resolved_exports.named.contains_key(&name) {
                    continue;
                }

                origins.insert(name.clone(), star.module.clone());
                self.resolved_exports.named.insert(
                    name.clone(),
                    Export {
                        name: name.clone(),
                        kind: source.kind,
                        target: ExportTarget::Reexport {
                            module: star.module.clone(),
                            name,
                        },
                        span: star.span,
                    },
                );
            }
        }
    }

    fn check_private_leaks(&mut self) {
        let mut exported_locals = BTreeSet::new();
        for export in self
            .resolved_exports
            .named
            .values()
            .chain(self.resolved_exports.default.iter())
        {
            if let ExportTarget::Local(name) = &export.target {
                exported_locals.insert(name.clone());
            }
        }

        for item in self.items.clone() {
            let is_public = item.visibility == Visibility::Public
                || item.exported
                || exported_locals.contains(&item.name);
            if !is_public || item.visibility == Visibility::Private {
                continue;
            }

            self.check_declaration_leaks(&item.name, &item.declaration);
        }
    }

    fn check_declaration_leaks(&mut self, owner: &str, declaration: &Declaration) {
        match declaration {
            Declaration::Function(function) => {
                let generics = generic_names(&function.type_parameters);
                for parameter in &function.parameters {
                    self.check_type_leak(owner, &parameter.ty, &generics);
                }
                if let Some(return_type) = &function.return_type {
                    self.check_type_leak(owner, return_type, &generics);
                }
                for error in &function.error_set {
                    self.check_type_path_leak(owner, error, &generics);
                }
            }
            Declaration::Struct(declaration) => {
                let generics = generic_names(&declaration.type_parameters);
                match &declaration.fields {
                    ast::StructFields::Named(fields) => {
                        for field in fields {
                            self.check_type_leak(owner, &field.ty, &generics);
                        }
                    }
                    ast::StructFields::Tuple(fields) => {
                        for field in fields {
                            self.check_type_leak(owner, field, &generics);
                        }
                    }
                }
            }
            Declaration::Enum(declaration) => {
                let generics = generic_names(&declaration.type_parameters);
                for variant in &declaration.variants {
                    if let Some(fields) = &variant.fields {
                        for field in fields {
                            self.check_type_leak(owner, &field.ty, &generics);
                        }
                    }
                }
            }
            Declaration::Constant(constant) => {
                self.check_type_leak(owner, &constant.ty, &BTreeSet::new());
            }
            Declaration::TypeAlias(alias) => {
                let generics = generic_names(&alias.type_parameters);
                self.check_type_leak(owner, &alias.ty, &generics);
            }
            Declaration::ForeignModule(module) => {
                for function in &module.functions {
                    for parameter in &function.parameters {
                        self.check_type_leak(owner, &parameter.ty, &BTreeSet::new());
                    }
                    if let Some(return_type) = &function.return_type {
                        self.check_type_leak(owner, return_type, &BTreeSet::new());
                    }
                    for error in &function.error_set {
                        self.check_type_path_leak(owner, error, &BTreeSet::new());
                    }
                }
            }
            Declaration::ScriptBlock(script) => {
                for export in &script.exports {
                    for parameter in &export.parameters {
                        self.check_type_leak(owner, &parameter.ty, &BTreeSet::new());
                    }
                    if let Some(return_type) = &export.return_type {
                        self.check_type_leak(owner, return_type, &BTreeSet::new());
                    }
                    for error in &export.error_set {
                        self.check_type_path_leak(owner, error, &BTreeSet::new());
                    }
                }
            }
        }
    }

    fn check_type_leak(&mut self, owner: &str, ty: &AstType, generics: &BTreeSet<String>) {
        match &ty.kind {
            TypeKind::Path(path) => self.check_type_path_leak(owner, path, generics),
            TypeKind::Array(element) | TypeKind::FixedArray(element, _) => {
                self.check_type_leak(owner, element, generics);
            }
            TypeKind::Map(key, value) => {
                self.check_type_leak(owner, key, generics);
                self.check_type_leak(owner, value, generics);
            }
            TypeKind::Tuple(elements) => {
                for element in elements {
                    self.check_type_leak(owner, element, generics);
                }
            }
            TypeKind::Function {
                parameters,
                return_type,
                error_set,
            } => {
                for parameter in parameters {
                    self.check_type_leak(owner, parameter, generics);
                }
                if let Some(return_type) = return_type {
                    self.check_type_leak(owner, return_type, generics);
                }
                for error in error_set {
                    self.check_type_path_leak(owner, error, generics);
                }
            }
            TypeKind::Unit => {}
        }
    }

    fn check_type_path_leak(&mut self, owner: &str, path: &TypePath, generics: &BTreeSet<String>) {
        let name = &path.segments[0];
        if !generics.contains(name) {
            let private = if path.segments.len() == 1 {
                self.symbols
                    .types
                    .get(name)
                    .filter(|symbol| symbol.visibility == Visibility::Private)
            } else {
                self.symbols
                    .types
                    .get(name)
                    .into_iter()
                    .chain(self.symbols.namespaces.get(name))
                    .find(|symbol| symbol.visibility == Visibility::Private)
            };

            if let Some(symbol) = private {
                let diagnostic = Diagnostic::error(
                    PRIVATE_LEAK,
                    format!("Private declaration '{name}' leaks through public API '{owner}'."),
                )
                .with_span(path.span)
                .with_hint("Export the private declaration or keep the API private.")
                .with_related(String::new(), symbol.span, "Private declaration.");
                self.diagnostics.push(diagnostic);
            }
        }

        for argument in &path.arguments {
            self.check_type_leak(owner, argument, generics);
        }
    }
}

fn symbol_kind(kind: ItemKind) -> SymbolKind {
    match kind {
        ItemKind::Function => SymbolKind::Function,
        ItemKind::Struct => SymbolKind::Struct,
        ItemKind::Enum => SymbolKind::Enum,
        ItemKind::Constant => SymbolKind::Constant,
        ItemKind::TypeAlias => SymbolKind::TypeAlias,
        ItemKind::ForeignModule => SymbolKind::ForeignModule,
        ItemKind::ScriptBlock => SymbolKind::ScriptBlock,
    }
}

fn generic_names(parameters: &[String]) -> BTreeSet<String> {
    parameters.iter().cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::{ModuleGraph, ModuleResolver, ResolveResult, resolve_module};
    use crate::ast;
    use crate::hir::{ExportKind, ExportTarget, SymbolKind};
    use crate::parser::parse;

    fn parse_module(source: &str) -> ast::Module {
        match parse(source) {
            Ok(module) => module,
            Err(diagnostics) => panic!("test source did not parse: {diagnostics:?}"),
        }
    }

    fn resolve(source: &str) -> ResolveResult<crate::hir::Module> {
        resolve_module(&parse_module(source))
    }

    fn resolve_graph(sources: &[(&str, &str)], root: &str) -> ResolveResult<crate::hir::Module> {
        let modules = sources
            .iter()
            .map(|(key, source)| ((*key).to_owned(), parse_module(source)))
            .collect::<Vec<_>>();
        ModuleResolver::from_modules(modules).resolve_named(root)
    }

    fn error_codes(result: ResolveResult<crate::hir::Module>) -> Vec<String> {
        match result {
            Ok(_) => Vec::new(),
            Err(diagnostics) => diagnostics
                .into_iter()
                .map(|diagnostic| diagnostic.code)
                .collect(),
        }
    }

    #[test]
    fn graph_resolution_uses_canonical_import_targets() {
        let mut graph = ModuleGraph::default();
        graph.insert_module(
            "src/main.ns",
            parse_module("import { value } from \"./util.ns\"\nfn main() { value() }"),
        );
        graph.insert_module(
            "src/util.ns",
            parse_module("pub fn value() -> i32 { return 1 }"),
        );
        graph.insert_import("src/main.ns", "./util.ns", "src/util.ns");

        let resolved = ModuleResolver::from_graph(&graph)
            .resolve_root("src/main.ns")
            .expect("graph should resolve");
        assert_eq!(resolved.imports[0].module, "src/util.ns");
    }

    #[test]
    fn graph_resolution_binds_the_same_specifier_per_importer() {
        let mut graph = ModuleGraph::default();
        graph.insert_module(
            "src/main.ns",
            parse_module(
                "import { value } from \"./util.ns\"\nfn main() -> i32 { return value() }",
            ),
        );
        graph.insert_module(
            "tests/main.ns",
            parse_module(
                "import { value } from \"./util.ns\"\nfn main() -> i32 { return value() }",
            ),
        );
        graph.insert_module(
            "src/util.ns",
            parse_module("pub fn value() -> i32 { return 1 }"),
        );
        graph.insert_module(
            "tests/util.ns",
            parse_module("pub fn value() -> i32 { return 2 }"),
        );
        graph.insert_import("src/main.ns", "./util.ns", "src/util.ns");
        graph.insert_import("tests/main.ns", "./util.ns", "tests/util.ns");

        let resolver = ModuleResolver::from_graph(&graph);
        let source_main = resolver
            .resolve_root("src/main.ns")
            .expect("source main should resolve");
        let test_main = resolver
            .resolve_root("tests/main.ns")
            .expect("test main should resolve");
        assert_eq!(source_main.imports[0].module, "src/util.ns");
        assert_eq!(test_main.imports[0].module, "tests/util.ns");
    }

    #[test]
    fn graph_resolution_requires_every_import_mapping() {
        let mut graph = ModuleGraph::default();
        graph.insert_module(
            "main.ns",
            parse_module("import { value } from \"./util.ns\"\nfn main() {}"),
        );
        graph.insert_module(
            "util.ns",
            parse_module("pub fn value() -> i32 { return 1 }"),
        );

        let codes = error_codes(ModuleResolver::from_graph(&graph).resolve_root("main.ns"));
        assert_eq!(codes, vec!["NS3012".to_owned()]);
    }

    #[test]
    fn graph_resolution_maps_reexports_to_canonical_targets() {
        let mut graph = ModuleGraph::default();
        graph.insert_module(
            "main.ns",
            parse_module("import { value } from \"./barrel.ns\"\nfn main() { value() }"),
        );
        graph.insert_module(
            "barrel.ns",
            parse_module("export { value } from \"./util.ns\""),
        );
        graph.insert_module(
            "util.ns",
            parse_module("pub fn value() -> i32 { return 1 }"),
        );
        graph.insert_import("main.ns", "./barrel.ns", "barrel.ns");
        graph.insert_import("barrel.ns", "./util.ns", "util.ns");

        let resolved = ModuleResolver::from_graph(&graph)
            .resolve_root("barrel.ns")
            .expect("barrel should resolve");
        let export = resolved
            .exports
            .get_named("value")
            .expect("value should be exported");
        assert_eq!(
            export.target,
            ExportTarget::Reexport {
                module: "util.ns".to_owned(),
                name: "value".to_owned(),
            }
        );
    }

    #[test]
    fn declarations_and_public_exports_are_indexed() {
        let module = resolve(
            r#"
module app.main

pub fn run(value: i32) -> i32 { return value }
fn hidden() {}
private fn secret() {}
pub struct User { name: str }
pub const LIMIT: i32 = 10
pub type Id = i32
pub enum Status { Ready Failed }
"#,
        )
        .expect("module should resolve");

        assert_eq!(module.name, "app.main");
        assert_eq!(module.items.len(), 7);
        assert!(module.items[0].exported);
        assert!(module.symbols.values["run"].exported);
        assert!(!module.items[1].exported);
        assert_eq!(module.symbols.values["run"].kind, SymbolKind::Function);
        assert_eq!(module.symbols.values["hidden"].kind, SymbolKind::Function);
        assert_eq!(module.symbols.values["User"].kind, SymbolKind::Struct);
        assert_eq!(module.symbols.types["Id"].kind, SymbolKind::TypeAlias);
        assert_eq!(module.symbols.values["Status"].kind, SymbolKind::Enum);

        let exported = module.exports.named.keys().cloned().collect::<Vec<_>>();
        assert_eq!(
            exported,
            vec!["Id", "LIMIT", "Status", "User", "run"]
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        );
        assert_eq!(module.exports.named["User"].kind, ExportKind::ValueAndType);
        assert_eq!(module.exports.named["Id"].kind, ExportKind::Type);
        assert!(!module.exports.named.contains_key("hidden"));
        assert!(!module.exports.named.contains_key("secret"));
    }

    #[test]
    fn export_list_promotes_normal_declarations() {
        let module = resolve(
            r#"
module app.main
fn helper() {}
export { helper }
"#,
        )
        .expect("module should resolve");

        assert_eq!(
            module.symbols.values["helper"].visibility,
            ast::Visibility::Normal
        );
        assert_eq!(
            module.exports.named["helper"].target,
            ExportTarget::Local("helper".to_owned())
        );
    }

    #[test]
    fn private_declarations_cannot_be_exported() {
        let codes = error_codes(resolve(
            r#"
module app.main
private fn secret() {}
export { secret }
"#,
        ));

        assert_eq!(codes, vec!["NS3001"]);
    }

    #[test]
    fn duplicate_declarations_are_reported() {
        let codes = error_codes(resolve(
            r#"
module app.main
fn thing() {}
fn thing() {}
"#,
        ));

        assert_eq!(codes, vec!["NS3011"]);
    }

    #[test]
    fn duplicate_import_bindings_are_reported() {
        let codes = error_codes(resolve(
            r#"
module app.main
fn helper() {}
import { helper } from "./util.ns"
"#,
        ));

        assert_eq!(codes, vec!["NS3006"]);
    }

    #[test]
    fn module_graph_resolves_named_and_namespace_imports() {
        let util = r#"
module app.util
pub fn add(value: i32) -> i32 { return value }
pub struct User { name: str }
export default fn make() {}
"#;
        let main = r#"
module app.main
import { add, User } from "./util.ns"
import * as util from "./util.ns"
import app.util as pathUtil
import make from "./util.ns"
pub fn run() { add(1) }
"#;

        let module = resolve_graph(
            &[("app.main", main), ("./util.ns", util), ("app.util", util)],
            "app.main",
        )
        .expect("module graph should resolve");

        assert_eq!(module.symbols.values["add"].kind, SymbolKind::ImportedValue);
        assert_eq!(module.symbols.types["User"].kind, SymbolKind::ImportedType);
        assert_eq!(
            module.symbols.namespaces["util"].kind,
            SymbolKind::ImportedNamespace
        );
        assert_eq!(
            module.symbols.namespaces["pathUtil"].kind,
            SymbolKind::ImportedNamespace
        );
        assert_eq!(
            module.symbols.values["make"].kind,
            SymbolKind::ImportedValue
        );
        assert_eq!(module.imports.len(), 4);
        assert_eq!(module.imports[0].module, "./util.ns".to_owned());
    }

    #[test]
    fn unresolved_modules_are_reported_in_strict_mode() {
        let codes = error_codes(resolve_graph(
            &[(
                "app.main",
                r#"
module app.main
import "./missing.ns"
"#,
            )],
            "app.main",
        ));

        assert_eq!(codes, vec!["NS3012"]);
    }

    #[test]
    fn missing_imported_names_are_reported() {
        let codes = error_codes(resolve_graph(
            &[
                (
                    "app.main",
                    r#"
module app.main
import { missing } from "./util.ns"
"#,
                ),
                (
                    "./util.ns",
                    r#"
module app.util
pub fn present() {}
"#,
                ),
            ],
            "app.main",
        ));

        assert_eq!(codes, vec!["NS3007"]);
    }

    #[test]
    fn imports_can_be_reexported_locally() {
        let module = resolve_graph(
            &[
                (
                    "app.main",
                    r#"
module app.main
import { add } from "./util.ns"
export { add }
"#,
                ),
                (
                    "./util.ns",
                    r#"
module app.util
pub fn add() {}
"#,
                ),
            ],
            "app.main",
        )
        .expect("re-export should resolve");

        assert_eq!(
            module.exports.named["add"].target,
            ExportTarget::Local("add".to_owned())
        );
    }

    #[test]
    fn reexport_lists_and_namespace_exports_are_resolved() {
        let module = resolve_graph(
            &[
                (
                    "app.main",
                    r#"
module app.main
export { add as plus } from "./util.ns"
export { default as make } from "./util.ns"
export * as util from "./util.ns"
"#,
                ),
                (
                    "./util.ns",
                    r#"
module app.util
pub fn add() {}
export default fn make() {}
"#,
                ),
            ],
            "app.main",
        )
        .expect("re-exports should resolve");

        assert_eq!(module.exports.named["plus"].kind, ExportKind::Value);
        assert_eq!(module.exports.named["make"].kind, ExportKind::Value);
        assert_eq!(module.exports.named["util"].kind, ExportKind::Namespace);
        assert!(module.exports.default.is_none());
    }

    #[test]
    fn star_reexport_conflicts_are_reported_and_explicit_exports_win() {
        let codes = error_codes(resolve_graph(
            &[
                (
                    "app.main",
                    r#"
module app.main
export { left } from "./a.ns"
export * from "./a.ns"
export * from "./b.ns"
"#,
                ),
                (
                    "./a.ns",
                    r#"
module app.a
pub fn left() {}
pub fn shared() {}
"#,
                ),
                (
                    "./b.ns",
                    r#"
module app.b
pub fn shared() {}
pub fn right() {}
"#,
                ),
            ],
            "app.main",
        ));

        assert_eq!(codes, vec!["NS3008"]);
    }

    #[test]
    fn explicit_exports_take_precedence_over_star_reexports() {
        let module = resolve_graph(
            &[
                (
                    "app.main",
                    r#"
module app.main
export * from "./a.ns"
pub fn shared() {}
"#,
                ),
                (
                    "./a.ns",
                    r#"
module app.a
pub fn shared() {}
pub fn other() {}
"#,
                ),
            ],
            "app.main",
        )
        .expect("explicit export should win");

        assert_eq!(
            module.exports.named["shared"].target,
            ExportTarget::Local("shared".to_owned())
        );
        assert_eq!(
            module.exports.named["other"].target,
            ExportTarget::Reexport {
                module: "./a.ns".to_owned(),
                name: "other".to_owned(),
            }
        );
    }

    #[test]
    fn private_types_cannot_leak_through_public_apis() {
        let codes = error_codes(resolve(
            r#"
module app.main
private struct Secret {}
pub fn reveal(value: [Secret]) {}
pub struct Public { secret: Secret }
pub type Visible = Option<Secret>
"#,
        ));

        assert_eq!(codes, vec!["NS3010", "NS3010", "NS3010"]);
    }

    #[test]
    fn private_namespaces_cannot_leak_through_public_apis() {
        let codes = error_codes(resolve(
            r#"
module app.main
private foreign module native {
    fn open() -> i32
}
pub fn read(handle: native.Handle) {}
"#,
        ));

        assert_eq!(codes, vec!["NS3010"]);
    }

    #[test]
    fn generic_parameters_do_not_leak_private_types() {
        let module = resolve(
            r#"
module app.main
private struct Secret {}
pub fn identity<T>(value: T) -> T { return value }
pub struct Box<T> { value: T }
"#,
        )
        .expect("generic parameters should resolve");

        assert_eq!(
            module.symbols.types["Secret"].visibility,
            ast::Visibility::Private
        );
    }

    #[test]
    fn default_export_must_target_a_value() {
        let codes = error_codes(resolve(
            r#"
module app.main
type Alias = i32
export default Alias
"#,
        ));

        assert_eq!(codes, vec!["NS3009"]);
    }

    #[test]
    fn default_export_cannot_expose_a_private_declaration() {
        let codes = error_codes(resolve(
            r#"
module app.main
private fn secret() {}
export default secret
"#,
        ));

        assert_eq!(codes, vec!["NS3001"]);
    }

    #[test]
    fn modules_can_have_only_one_default_export() {
        let codes = error_codes(resolve(
            r#"
module app.main
export default 1
export default 2
"#,
        ));

        assert_eq!(codes, vec!["NS3004"]);
    }

    #[test]
    fn scoped_items_are_resolved_at_module_scope() {
        let module = resolve(
            r#"
module app.main
{ @mode:core;
    pub fn scoped() {}
}
export { scoped }
"#,
        )
        .expect("scoped items should resolve");

        assert!(module.symbols.values.contains_key("scoped"));
        assert_eq!(
            module.exports.named["scoped"].target,
            ExportTarget::Local("scoped".to_owned())
        );
    }

    #[test]
    fn unknown_export_targets_are_reported() {
        let codes = error_codes(resolve(
            r#"
module app.main
export { missing }
"#,
        ));

        assert_eq!(codes, vec!["NS3005"]);
    }

    #[test]
    fn duplicate_export_names_are_reported() {
        let codes = error_codes(resolve(
            r#"
module app.main
fn first() {}
fn second() {}
export { first as value }
export { second as value }
"#,
        ));

        assert_eq!(codes, vec!["NS3003"]);
    }

    #[test]
    fn missing_reexported_names_are_reported() {
        let codes = error_codes(resolve_graph(
            &[
                (
                    "app.main",
                    r#"
module app.main
export { missing } from "./util.ns"
"#,
                ),
                (
                    "./util.ns",
                    r#"
module app.util
pub fn present() {}
"#,
                ),
            ],
            "app.main",
        ));

        assert_eq!(codes, vec!["NS3007"]);
    }

    #[test]
    fn bare_default_imports_must_be_aliased() {
        let codes = error_codes(resolve(
            r#"
module app.main
import { default } from "./util.ns"
"#,
        ));

        assert_eq!(codes, vec!["NS3006"]);
    }

    #[test]
    fn star_reexports_preserve_type_only_exports() {
        let module = resolve_graph(
            &[
                (
                    "app.main",
                    r#"
module app.main
export * from "./util.ns"
"#,
                ),
                (
                    "./util.ns",
                    r#"
module app.util
pub type Id = i32
pub fn add() {}
"#,
                ),
            ],
            "app.main",
        )
        .expect("star reexport should resolve");

        assert_eq!(module.exports.named["Id"].kind, ExportKind::Type);
        assert_eq!(module.exports.named["add"].kind, ExportKind::Value);
    }

    #[test]
    fn module_cycles_are_reported() {
        let codes = error_codes(resolve_graph(
            &[
                (
                    "app.a",
                    r#"
module app.a
import { b } from "./b.ns"
pub fn a() {}
"#,
                ),
                (
                    "./b.ns",
                    r#"
module app.b
import { a } from "app.a"
pub fn b() {}
"#,
                ),
            ],
            "app.a",
        ));

        assert_eq!(codes, vec!["NS3013"]);
    }
}
