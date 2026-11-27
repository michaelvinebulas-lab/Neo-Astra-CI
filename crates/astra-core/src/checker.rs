//! The `TYPE-002` type checker.
//!
//! The checker consumes the source AST plus resolver HIR. It annotates HIR item
//! types, records expression types for Nova and later lowering, and reports
//! structured `NS4xxx`/`NS5xxx` diagnostics. It never re-parses source and never
//! invents symbols for the resolver.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::ast;
use crate::diagnostic::Diagnostic;
use crate::edition::Edition;
use crate::hir;
use crate::literal::{float_suffix, integer_suffix};
use crate::mode::Mode;
use crate::parser::{ParseOptions, parse_with_options};
use crate::resolver::resolve_module;
use crate::span::Span;
use crate::type_surface::{ImportedTypeDef, ModuleTypeSurface};
use crate::types::{FunctionType, Type};

const TYPE_MISMATCH: &str = "NS4001";
const UNKNOWN_IDENTIFIER: &str = "NS4002";
const UNKNOWN_TYPE: &str = "NS4003";
const ARGUMENT_COUNT: &str = "NS4004";
const NOT_CALLABLE: &str = "NS4005";
const INVALID_OPERANDS: &str = "NS4006";
const MISSING_RETURN: &str = "NS4007";
const NOT_ASSIGNABLE: &str = "NS4009";
const NON_EXHAUSTIVE_MATCH: &str = "NS4010";
const UNHANDLED_ERROR: &str = "NS4011";
const UNDECLARED_ERROR: &str = "NS4012";
const INVALID_PROPAGATION: &str = "NS4013";
const INVALID_CONTROL_FLOW: &str = "NS4014";
const INVALID_MEMBER: &str = "NS4015";
const INVALID_STRUCT_FIELD: &str = "NS4016";
const INVALID_PATTERN: &str = "NS4017";
const INVALID_TYPE_ARGUMENT: &str = "NS4018";
const TASK_BOUNDARY: &str = "NS4019";
const BARE_TYPE_ARGUMENTS: &str = "NS2010";
const MODE_REQUIREMENT: &str = "NS5001";
const MODE_WIDENING: &str = "NS5002";
const CAPABILITY_REQUIREMENT: &str = "NS5003";
const MEMORY_REQUIREMENT: &str = "NS5004";
const INVALID_CAPABILITY_ANNOTATION: &str = "NS5005";
const MANUAL_ALLOC_IN_GC_SCOPE: &str = "NS6001";
const MANAGED_ELEMENT_IN_MANUAL_ALLOC: &str = "NS6002";
const UNSUPPORTED_SCRIPT_TARGET: &str = "NS7002";
const INVALID_SCRIPT_SIGNATURE: &str = "NS7005";
/// D27 system-mode side-effect rule: two or more state-changing subexpressions
/// in a target-native-order position (binary operands, array elements, struct
/// fields, map entries). Call arguments are exempt — they are ordered.
const NATIVE_ORDER_EFFECTS: &str = "NS5007";
/// `@export_as` applied to a non-generic or non-exported function.
const EXPORT_AS_TARGET: &str = "NS7010";
/// `@export_as` bindings do not bind every type parameter exactly once.
const EXPORT_AS_BINDINGS: &str = "NS7011";
/// A bound type is not C-boundary-safe.
const EXPORT_AS_TYPE: &str = "NS7012";
/// Duplicate symbol/binding list, or an invalid C identifier.
const EXPORT_AS_SYMBOL: &str = "NS7013";

/// Type-checker options.
#[derive(Debug, Clone, Default)]
pub struct CheckOptions {
    /// Language edition. Defaults to `0.1`; syntax behavior changes later.
    pub edition: Edition,
    /// The resolved capability mode selected by the CLI, environment, or
    /// manifest. Source annotations are validated against this mode.
    pub mode: Mode,
    /// Active target (e.g. "js", "ts", "native", "wasm").
    pub target: Option<String>,
    /// Type surfaces for modules that this module imports from.
    ///
    /// When populated, `register_imports` uses the surfaces to resolve real
    /// types for imported names instead of falling back to `Type::Unknown`.
    /// For standalone checks this is empty and imports remain opaque.
    pub module_surfaces: BTreeMap<String, ModuleTypeSurface>,
}

impl CheckOptions {
    /// Creates options for a selected mode, with no module surfaces.
    #[must_use]
    pub fn new(mode: Mode) -> Self {
        Self {
            edition: Edition::V0_1,
            mode,
            target: None,
            module_surfaces: BTreeMap::new(),
        }
    }

    /// Selects the language edition.
    #[must_use]
    pub fn with_edition(mut self, edition: Edition) -> Self {
        self.edition = edition;
        self
    }

    /// Sets the active compilation target.
    #[must_use]
    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }

    /// Creates options with a pre-populated module surface map.
    #[must_use]
    pub fn with_surfaces(mode: Mode, module_surfaces: BTreeMap<String, ModuleTypeSurface>) -> Self {
        Self {
            edition: Edition::V0_1,
            mode,
            target: None,
            module_surfaces,
        }
    }
}

impl PartialEq for CheckOptions {
    fn eq(&self, other: &Self) -> bool {
        self.edition == other.edition
            && self.mode == other.mode
            && self.target == other.target
            && self.module_surfaces == other.module_surfaces
    }
}

impl Eq for CheckOptions {}

/// One typed expression span recorded for later stages.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypeAnnotation {
    pub span: Span,
    pub ty: Type,
}

/// The successful output of type checking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    /// Resolved HIR with item types filled in.
    pub module: hir::Module,
    /// Expression annotations in deterministic source order.
    pub annotations: Vec<TypeAnnotation>,
    /// Spans of `@memory:manual` blocks. Each covers the whole
    /// `{@memory:manual; ...}` block, not just its header, so containment
    /// checks see the body as well.
    ///
    /// Used to reject `Memory.alloc` outside such a block (`NS6001`). Nova
    /// rejects manual blocks even when the selected mode grants manual memory
    /// because its first engine has no checked development allocator.
    pub manual_memory_scopes: Vec<Span>,
}

/// Parses, resolves, and type-checks one standalone source module.
pub fn check_source(source: &str, options: CheckOptions) -> Result<CheckResult, Vec<Diagnostic>> {
    let module = parse_with_options(
        source,
        ParseOptions {
            edition: options.edition,
        },
    )?;
    check_module(&module, options)
}

/// Resolves and type-checks one parsed source module.
pub fn check_module(
    source: &ast::Module,
    options: CheckOptions,
) -> Result<CheckResult, Vec<Diagnostic>> {
    let resolved = resolve_module(source)?;
    check_resolved(source, &resolved, options)
}

/// Type-checks an already-resolved module.
pub fn check_resolved(
    source: &ast::Module,
    resolved: &hir::Module,
    options: CheckOptions,
) -> Result<CheckResult, Vec<Diagnostic>> {
    let mut checker = Checker::new(source, resolved, options);
    checker.run();

    if checker.diagnostics.is_empty() {
        Ok(CheckResult {
            module: checker.resolved,
            annotations: checker.annotations,
            manual_memory_scopes: checker.manual_memory_scopes,
        })
    } else {
        Err(checker.diagnostics)
    }
}

#[derive(Debug, Clone)]
struct Local {
    ty: Type,
    mutable: bool,
    span: Span,
}

#[derive(Debug, Clone)]
struct ValueDef {
    ty: Type,
    /// Declaration site, used for secondary labels such as "defined here".
    span: Option<Span>,
}

impl ValueDef {
    fn new(ty: Type) -> Self {
        Self { ty, span: None }
    }

    fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }
}

#[derive(Debug, Clone)]
struct NamespaceDef {
    members: BTreeMap<String, ValueDef>,
    open: bool,
}

#[derive(Debug, Clone)]
enum TypeDef {
    Struct(StructDef),
    Enum(EnumDef),
    Alias(AliasDef),
    BuiltinNominal,
}

#[derive(Debug, Clone)]
struct StructDef {
    type_parameters: Vec<String>,
    fields: ast::StructFields,
}

#[derive(Debug, Clone)]
struct EnumDef {
    type_parameters: Vec<String>,
    variants: Vec<ast::EnumVariant>,
    span: Option<Span>,
}

#[derive(Debug, Clone)]
struct AliasDef {
    type_parameters: Vec<String>,
    ty: ast::Type,
}

#[derive(Debug, Clone)]
struct ResolvedField {
    name: Option<String>,
    ty: Type,
}

#[derive(Debug, Clone)]
struct ResolvedStruct {
    type_parameters: Vec<String>,
    fields: Vec<ResolvedField>,
}

#[derive(Debug, Clone)]
struct ResolvedVariant {
    name: String,
    fields: Option<Vec<ResolvedField>>,
}

#[derive(Debug, Clone)]
struct ResolvedEnum {
    type_parameters: Vec<String>,
    variants: Vec<ResolvedVariant>,
}

#[derive(Debug, Clone)]
struct FunctionContext {
    name: String,
    return_type: Type,
    declared_errors: Vec<Type>,
    span: Span,
    /// Span covering the `! E1 | E2` clause, used as the secondary label for
    /// unhandled or undeclared error effects.
    error_set_span: Option<Span>,
}

/// One recoverable error effect together with its construction or call site.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ErrorEffect {
    ty: Type,
    span: Span,
}

impl ErrorEffect {
    fn new(ty: Type, span: Span) -> Self {
        Self { ty, span }
    }
}

#[derive(Debug, Clone)]
struct ExprInfo {
    ty: Type,
    errors: Vec<ErrorEffect>,
}

impl ExprInfo {
    fn value(ty: Type) -> Self {
        Self {
            ty,
            errors: Vec::new(),
        }
    }

    fn with_errors(mut self, errors: Vec<ErrorEffect>) -> Self {
        append_effects(&mut self.errors, errors);
        self
    }
}

#[derive(Debug, Clone)]
struct LValueInfo {
    ty: Type,
    mutable: bool,
    errors: Vec<ErrorEffect>,
}

#[derive(Debug, Clone)]
struct StmtInfo {
    expression: Option<ExprInfo>,
    diverges: bool,
}

#[derive(Debug, Clone)]
struct BlockInfo {
    tail: Option<ExprInfo>,
    diverges: bool,
}

#[derive(Debug, Clone, Default)]
struct Coverage {
    some: bool,
    none: bool,
    ok: bool,
    err: bool,
    boolean: [bool; 2],
    variants: BTreeSet<String>,
    err_members: BTreeSet<String>,
    err_wildcard: bool,
}

enum MatchShape {
    Option,
    Result,
    Enum(ResolvedEnum),
    Bool,
    Tuple,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ConstructorKey {
    Bool(bool),
    Some,
    None,
    Ok,
    Err,
    Variant(String),
    Tuple,
    Unit,
}

struct Constructor {
    key: ConstructorKey,
    fields: Vec<Type>,
}

#[derive(Clone, Copy)]
enum RowPattern<'a> {
    Wildcard,
    Pattern(&'a ast::Pattern),
}

struct Checker<'a> {
    source: &'a ast::Module,
    resolved: hir::Module,
    options: CheckOptions,
    diagnostics: Vec<Diagnostic>,
    annotations: Vec<TypeAnnotation>,
    manual_memory_scopes: Vec<Span>,
    /// Spans of `Memory.alloc` / `region.alloc` member expressions whose mode
    /// and scope gates passed, so `check_call` can apply `NS6002` to the
    /// element type bound for `T` (spec 8.4; GC-001 proposal §11).
    manual_alloc_spans: Vec<Span>,
    /// Non-zero while checking inside a node that already reported NS5007, so
    /// nested order-sensitive nodes do not duplicate the diagnostic.
    native_order_suppress: usize,
    /// Hybrid NS6002 micro-pass: alloc nodes whose element type depends on a
    /// generic parameter, recorded per generic function (by name) at definition
    /// time so they can be re-checked at instantiation time.
    deferred_alloc_checks: BTreeMap<String, Vec<(Span, Type)>>,
    /// Hybrid NS6002 micro-pass: call sites of generic functions recorded
    /// during body checking, replayed after all bodies are checked so the
    /// callee's deferred alloc checks are available.
    pending_generic_call_checks: Vec<(Span, String, String, BTreeMap<String, Type>)>,
    mode: Mode,
    function_modes: HashMap<Span, Mode>,
    type_defs: BTreeMap<String, TypeDef>,
    imported_types: BTreeSet<String>,
    values: BTreeMap<String, ValueDef>,
    /// Maps a bare enum variant name to the enum that declares it. Variants are
    /// in value scope under edition 0.2 (`Just(...)`, `Nothing`).
    variant_owners: BTreeMap<String, String>,
    namespaces: BTreeMap<String, NamespaceDef>,
    script_modes: BTreeMap<String, Mode>,
    scopes: Vec<BTreeMap<String, Local>>,
    current_function: Option<FunctionContext>,
    current_type_parameters: Vec<String>,
    loop_depth: usize,
    alias_stack: Vec<String>,
}

impl<'a> Checker<'a> {
    fn new(source: &'a ast::Module, resolved: &hir::Module, options: CheckOptions) -> Self {
        let mode = options.mode;
        Self {
            source,
            resolved: resolved.clone(),
            options,
            diagnostics: Vec::new(),
            annotations: Vec::new(),
            manual_memory_scopes: Vec::new(),
            manual_alloc_spans: Vec::new(),
            native_order_suppress: 0,
            deferred_alloc_checks: BTreeMap::new(),
            pending_generic_call_checks: Vec::new(),
            mode,
            function_modes: HashMap::new(),
            type_defs: BTreeMap::new(),
            imported_types: BTreeSet::new(),
            values: BTreeMap::new(),
            variant_owners: BTreeMap::new(),
            namespaces: BTreeMap::new(),
            script_modes: BTreeMap::new(),
            scopes: Vec::new(),
            current_function: None,
            current_type_parameters: Vec::new(),
            loop_depth: 0,
            alias_stack: Vec::new(),
        }
    }

    fn run(&mut self) {
        self.register_type_names();
        // Imported type definitions must be available before local function
        // signatures are resolved. Otherwise an imported error type in `! E`
        // would be reported as unknown during signature construction.
        self.register_imports();
        self.register_values();
        self.check_mode_annotations();
        self.fill_type_definitions();
        self.check_constants();
        self.check_functions();
        self.check_deferred_generic_allocs();
    }

    fn error(
        &mut self,
        code: &str,
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

    fn annotate(&mut self, span: Span, ty: Type) {
        self.annotations.push(TypeAnnotation { span, ty });
    }

    fn register_type_names(&mut self) {
        let items = self.resolved.items.clone();
        for item in &items {
            match &item.declaration {
                hir::Declaration::Struct(declaration) => {
                    self.type_defs.insert(
                        declaration.name.clone(),
                        TypeDef::Struct(StructDef {
                            type_parameters: declaration.type_parameters.clone(),
                            fields: declaration.fields.clone(),
                        }),
                    );
                }
                hir::Declaration::Enum(declaration) => {
                    self.type_defs.insert(
                        declaration.name.clone(),
                        TypeDef::Enum(EnumDef {
                            type_parameters: declaration.type_parameters.clone(),
                            variants: declaration.variants.clone(),
                            span: Some(item.span),
                        }),
                    );
                }
                hir::Declaration::TypeAlias(declaration) => {
                    self.type_defs.insert(
                        declaration.name.clone(),
                        TypeDef::Alias(AliasDef {
                            type_parameters: declaration.type_parameters.clone(),
                            ty: declaration.ty.clone(),
                        }),
                    );
                }
                _ => {}
            }
        }

        for (name, def) in [
            ("ParseIntError", TypeDef::BuiltinNominal),
            ("OverflowError", TypeDef::BuiltinNominal),
            ("TaskError", TypeDef::BuiltinNominal),
            ("ChannelError", TypeDef::BuiltinNominal),
            ("SliceError", TypeDef::BuiltinNominal),
        ] {
            self.type_defs.entry(name.to_owned()).or_insert(def);
        }
    }

    fn register_values(&mut self) {
        let items = self.resolved.items.clone();
        for (index, item) in items.iter().enumerate() {
            match &item.declaration {
                hir::Declaration::Function(declaration) => {
                    let signature = self.function_signature(declaration);
                    self.values.insert(
                        declaration.name.clone(),
                        ValueDef::new(Type::Function(signature.clone())).with_span(item.span),
                    );
                    self.resolved.items[index].ty = Some(Type::Function(signature));
                }
                hir::Declaration::Struct(declaration) => {
                    let ty = Type::Named(
                        declaration.name.clone(),
                        declaration
                            .type_parameters
                            .iter()
                            .map(|parameter| Type::Generic(parameter.clone()))
                            .collect(),
                    );
                    if let ast::StructFields::Tuple(fields) = &declaration.fields {
                        let generics = generic_map(&declaration.type_parameters);
                        let parameters = fields
                            .iter()
                            .map(|field| self.resolve_type(field, &generics))
                            .collect::<Vec<_>>();
                        if !parameters.is_empty() {
                            let signature = FunctionType {
                                type_parameters: declaration.type_parameters.clone(),
                                parameters,
                                return_type: Box::new(ty.clone()),
                                errors: Vec::new(),
                            };
                            self.values.insert(
                                declaration.name.clone(),
                                ValueDef::new(Type::Function(signature)).with_span(item.span),
                            );
                        }
                    }
                    self.resolved.items[index].ty = Some(ty);
                }
                hir::Declaration::Enum(declaration) => {
                    let generics = generic_map(&declaration.type_parameters);
                    let ty = Type::Named(
                        declaration.name.clone(),
                        declaration
                            .type_parameters
                            .iter()
                            .map(|parameter| Type::Generic(parameter.clone()))
                            .collect(),
                    );
                    for variant in &declaration.variants {
                        if self.options.edition != Edition::V0_2 {
                            continue;
                        }
                        self.variant_owners
                            .insert(variant.name.clone(), declaration.name.clone());
                        let value_ty = match &variant.fields {
                            Some(fields) if !fields.is_empty() => {
                                let parameters = fields
                                    .iter()
                                    .map(|field| self.resolve_type(&field.ty, &generics))
                                    .collect::<Vec<_>>();
                                Type::Function(FunctionType {
                                    type_parameters: declaration.type_parameters.clone(),
                                    parameters,
                                    return_type: Box::new(ty.clone()),
                                    errors: Vec::new(),
                                })
                            }
                            _ => ty.clone(),
                        };
                        self.values
                            .entry(variant.name.clone())
                            .or_insert(ValueDef::new(value_ty));
                    }
                    self.resolved.items[index].ty = Some(ty);
                }
                hir::Declaration::Constant(declaration) => {
                    let generics = BTreeMap::new();
                    let ty = self.resolve_type(&declaration.ty, &generics);
                    self.values
                        .insert(declaration.name.clone(), ValueDef::new(ty.clone()));
                    self.resolved.items[index].ty = Some(ty);
                }
                hir::Declaration::TypeAlias(declaration) => {
                    let generics = generic_map(&declaration.type_parameters);
                    let ty = self.resolve_type(&declaration.ty, &generics);
                    self.resolved.items[index].ty = Some(ty);
                }
                hir::Declaration::ForeignModule(declaration) => {
                    let mut members = BTreeMap::new();
                    for function in &declaration.functions {
                        let signature = self.function_signature_parts(
                            function.name.clone(),
                            &[],
                            &function.parameters,
                            function.return_type.as_ref(),
                            &function.error_set,
                            function.span,
                        );
                        members.insert(
                            function.name.clone(),
                            ValueDef::new(Type::Function(signature)),
                        );
                    }
                    self.namespaces.insert(
                        declaration.name.clone(),
                        NamespaceDef {
                            members,
                            open: false,
                        },
                    );
                }
                hir::Declaration::ScriptBlock(declaration) => {
                    let mut members = BTreeMap::new();
                    for export in &declaration.exports {
                        let signature = self.function_signature_parts(
                            export.name.clone(),
                            &[],
                            &export.parameters,
                            export.return_type.as_ref(),
                            &export.error_set,
                            export.span,
                        );
                        members.insert(
                            export.name.clone(),
                            ValueDef::new(Type::Function(signature)),
                        );
                    }
                    self.namespaces.insert(
                        declaration.name.clone(),
                        NamespaceDef {
                            members,
                            open: false,
                        },
                    );
                }
            }
        }
    }

    fn register_imports(&mut self) {
        // Build a lookup: local name → (module_key, imported_name) from the
        // resolved import list so we can consult module surfaces.
        let mut local_to_import: BTreeMap<String, (String, String)> = BTreeMap::new();
        for import in &self.resolved.imports {
            match &import.kind {
                hir::ImportKind::Named { bindings } => {
                    for binding in bindings {
                        local_to_import.insert(
                            binding.local.clone(),
                            (import.module.clone(), binding.imported.clone()),
                        );
                    }
                }
                hir::ImportKind::Default { local } => {
                    local_to_import
                        .insert(local.clone(), (import.module.clone(), "default".to_owned()));
                }
                hir::ImportKind::DefaultNamed { default, bindings } => {
                    local_to_import.insert(
                        default.clone(),
                        (import.module.clone(), "default".to_owned()),
                    );
                    for binding in bindings {
                        local_to_import.insert(
                            binding.local.clone(),
                            (import.module.clone(), binding.imported.clone()),
                        );
                    }
                }
                hir::ImportKind::SideEffect | hir::ImportKind::Namespace { .. } => {}
            }
        }

        for (name, symbol) in &self.resolved.symbols.values {
            if self.values.contains_key(name) {
                continue;
            }
            if matches!(
                symbol.kind,
                hir::SymbolKind::ImportedValue | hir::SymbolKind::ImportedType
            ) {
                // Try to look up the real type from a module surface.
                let ty = local_to_import
                    .get(name)
                    .and_then(|(module_key, imported_name)| {
                        self.options
                            .module_surfaces
                            .get(module_key)
                            .and_then(|surface| surface.get_value(imported_name))
                            .cloned()
                    })
                    .unwrap_or(Type::Unknown);

                self.values.insert(name.clone(), ValueDef::new(ty));
            }
        }

        // Register imported type definitions from module surfaces.
        let mut imported_constructors = Vec::new();
        for name in self.resolved.symbols.types.keys() {
            if self.type_defs.contains_key(name) {
                continue;
            }
            // Try to find a real type definition from a module surface.
            let def_from_surface =
                local_to_import
                    .get(name)
                    .and_then(|(module_key, imported_name)| {
                        self.options
                            .module_surfaces
                            .get(module_key)
                            .and_then(|surface| surface.get_type_def(imported_name))
                            .cloned()
                    });

            if let Some(imported_def) = def_from_surface {
                match imported_def {
                    ImportedTypeDef::Struct {
                        name: struct_name,
                        type_parameters,
                        fields,
                    } => {
                        if let ast::StructFields::Tuple(tuple_fields) = &fields
                            && !tuple_fields.is_empty()
                        {
                            imported_constructors.push((
                                name.clone(),
                                struct_name.clone(),
                                type_parameters.clone(),
                                tuple_fields.clone(),
                            ));
                        }
                        self.type_defs.insert(
                            name.clone(),
                            TypeDef::Struct(StructDef {
                                type_parameters,
                                fields,
                            }),
                        );
                        // Also insert the constructor type for named uses.
                        let ty = Type::Named(struct_name, Vec::new());
                        self.values
                            .entry(name.clone())
                            .or_insert_with(|| ValueDef::new(ty));
                    }
                    ImportedTypeDef::Enum {
                        name: enum_name,
                        type_parameters,
                        variants,
                    } => {
                        self.type_defs.insert(
                            name.clone(),
                            TypeDef::Enum(EnumDef {
                                type_parameters,
                                variants,
                                span: None,
                            }),
                        );
                        let ty = Type::Named(enum_name, Vec::new());
                        self.values
                            .entry(name.clone())
                            .or_insert_with(|| ValueDef::new(ty));
                    }
                }
            } else {
                self.imported_types.insert(name.clone());
            }
        }

        // Tuple structs are callable constructors. Build their signatures after
        // every imported type definition is available so field types resolve.
        for (local_name, struct_name, type_parameters, fields) in imported_constructors {
            let generics = generic_map(&type_parameters);
            let parameters = fields
                .iter()
                .map(|field| self.resolve_type(field, &generics))
                .collect::<Vec<_>>();
            let return_type = Type::Named(
                struct_name,
                type_parameters
                    .iter()
                    .map(|parameter| Type::Generic(parameter.clone()))
                    .collect(),
            );
            let signature = FunctionType {
                type_parameters,
                parameters,
                return_type: Box::new(return_type),
                errors: Vec::new(),
            };
            self.values
                .insert(local_name, ValueDef::new(Type::Function(signature)));
        }

        for (name, import) in &self.resolved.symbols.namespaces {
            if let hir::SymbolOrigin::Imported {
                module,
                name: imported,
            } = &import.origin
                && let Some(surface) = self.options.module_surfaces.get(module)
            {
                if imported == "*" {
                    let members: BTreeMap<String, ValueDef> = surface
                        .values
                        .iter()
                        .map(|(export_name, ty)| (export_name.clone(), ValueDef::new(ty.clone())))
                        .collect();
                    self.namespaces
                        .entry(name.clone())
                        .or_insert_with(|| NamespaceDef {
                            members,
                            open: false,
                        });
                    continue;
                }
                if let Some(members) = surface.namespaces.get(imported) {
                    let members = members
                        .iter()
                        .map(|(member, ty)| (member.clone(), ValueDef::new(ty.clone())))
                        .collect();
                    self.namespaces
                        .entry(name.clone())
                        .or_insert_with(|| NamespaceDef {
                            members,
                            open: false,
                        });
                    continue;
                }
                // A named namespace from a surface that predates nested
                // namespace tracking. Keep member access opaque rather than
                // rejecting valid project modules.
                self.namespaces
                    .entry(name.clone())
                    .or_insert_with(|| NamespaceDef {
                        members: BTreeMap::new(),
                        open: true,
                    });
                continue;
            }
            self.namespaces
                .entry(name.clone())
                .or_insert_with(|| NamespaceDef {
                    members: BTreeMap::new(),
                    open: true,
                });
        }
    }

    fn fill_type_definitions(&mut self) {
        let items = self.resolved.items.clone();
        for (index, item) in items.iter().enumerate() {
            if let hir::Declaration::TypeAlias(declaration) = &item.declaration {
                let generics = generic_map(&declaration.type_parameters);
                let ty = self.resolve_type(&declaration.ty, &generics);
                self.resolved.items[index].ty = Some(ty);
            }
        }

        // Validate every field/variant type, even when the declaration is never
        // constructed or accessed. Unknown types must not hide until use.
        let names = self.type_defs.keys().cloned().collect::<Vec<_>>();
        for name in names {
            match self.type_defs.get(&name) {
                Some(TypeDef::Struct(_)) => {
                    let _ = self.resolve_struct(&name);
                }
                Some(TypeDef::Enum(_)) => {
                    let _ = self.resolve_enum(&name);
                }
                Some(TypeDef::Alias(_) | TypeDef::BuiltinNominal) | None => {}
            }
        }
    }

    fn check_constants(&mut self) {
        let items = self.resolved.items.clone();
        for item in &items {
            let hir::Declaration::Constant(declaration) = &item.declaration else {
                continue;
            };

            let previous_mode = self.mode;
            self.mode = self
                .function_modes
                .get(&declaration.span)
                .copied()
                .unwrap_or(self.options.mode);
            self.push_scope();
            let declared = self.resolve_type(&declaration.ty, &BTreeMap::new());
            let info = self.check_expr(&declaration.value, Some(&declared));
            self.report_effects(&info.errors);
            self.pop_scope();
            self.mode = previous_mode;
        }
    }

    fn check_functions(&mut self) {
        let items = self.resolved.items.clone();
        for item in &items {
            let hir::Declaration::Function(declaration) = &item.declaration else {
                continue;
            };

            let previous_mode = self.mode;
            self.mode = self
                .function_modes
                .get(&declaration.span)
                .copied()
                .unwrap_or(self.options.mode);

            let signature = match &item.ty {
                Some(Type::Function(signature)) => signature.clone(),
                _ => self.function_signature(declaration),
            };

            self.push_scope();
            for (parameter, parameter_type) in
                declaration.parameters.iter().zip(&signature.parameters)
            {
                self.insert_local(
                    parameter.name.clone(),
                    Local {
                        ty: parameter_type.clone(),
                        mutable: false,
                        span: parameter.span,
                    },
                );
            }

            self.current_type_parameters = declaration.type_parameters.clone();

            self.current_function = Some(FunctionContext {
                name: declaration.name.clone(),
                return_type: (*signature.return_type).clone(),
                declared_errors: signature.errors.clone(),
                span: declaration.span,
                error_set_span: declaration
                    .error_set
                    .first()
                    .zip(declaration.error_set.last())
                    .map(|(first, last)| Span::new(first.span.start, last.span.end)),
            });

            let body = self.check_block(&declaration.body, false, None);
            let return_type = (*signature.return_type).clone();
            if !return_type.accepts(&Type::Unit) && !body.diverges && !return_type.is_opaque() {
                self.error(
                    MISSING_RETURN,
                    format!(
                        "Function '{}' can reach the end of its body without returning '{}'.",
                        declaration.name, return_type
                    ),
                    declaration.span,
                    "Return a value on every path or change the return type to '()'.",
                );
            }

            self.current_function = None;
            self.current_type_parameters.clear();
            self.pop_scope();
            self.mode = previous_mode;
        }
    }

    fn check_deferred_generic_allocs(&mut self) {
        // Group pending generic call sites by their enclosing function so that
        // transitive forwarding can be replayed (P3: worklist to fixpoint).
        type PendingCall = (Span, String, BTreeMap<String, Type>);
        let mut by_enclosing: BTreeMap<String, Vec<PendingCall>> = BTreeMap::new();
        for (span, callee_name, enclosing, subs) in
            std::mem::take(&mut self.pending_generic_call_checks)
        {
            by_enclosing
                .entry(enclosing)
                .or_default()
                .push((span, callee_name, subs));
        }

        // Worklist of (call_span, callee_name, substitution). Start with every
        // recorded call site.
        let mut worklist: Vec<(Span, String, BTreeMap<String, Type>)> = by_enclosing
            .values()
            .flat_map(|v| v.iter().cloned())
            .collect();
        let mut seen: std::collections::HashSet<(Span, String, BTreeMap<String, Type>)> =
            std::collections::HashSet::new();

        while let Some((call_span, callee_name, substitutions)) = worklist.pop() {
            if !seen.insert((call_span, callee_name.clone(), substitutions.clone())) {
                continue;
            }
            let all_concrete =
                !substitutions.is_empty() && substitutions.values().all(|t| !contains_generic(t));

            if all_concrete
                && let Some(checks) = self.deferred_alloc_checks.get(&callee_name).cloned()
            {
                for (_alloc_span, element_type) in checks {
                    let concrete_element = element_type.substitute(&substitutions);
                    if matches!(concrete_element, Type::Error | Type::Unknown) {
                        continue;
                    }
                    if contains_generic(&concrete_element) {
                        continue;
                    }
                    if self.holds_managed_refs(&concrete_element) {
                        self.error(
                                MANAGED_ELEMENT_IN_MANUAL_ALLOC,
                                format!(
                                    "Manual allocation cannot use element type '{concrete_element}': it holds \
                                     GC-managed references, and 'Ptr<T>' is never a GC root (spec 8.4), \
                                     so a collection could reclaim data this pointer still points at."
                                ),
                                call_span,
                                "Allocate pointer-free types here (scalars, Ptr<U>, or a struct/enum of \
                                 those); str, bytes, [T], Map, Channel, Task, and composites containing \
                                 them belong to GC-managed memory.",
                            );
                    }
                }
            }

            // Transitive replay: if this callee is itself a generic function,
            // compose this call's substitution onto its own recorded inner calls
            // and enqueue them (P3 fixpoint).
            if all_concrete && let Some(inner_calls) = by_enclosing.get(&callee_name).cloned() {
                for (inner_span, inner_callee, inner_subs) in inner_calls {
                    // Compose: apply outer substitution to inner's substitution.
                    let mut composed = inner_subs.clone();
                    for v in composed.values_mut() {
                        *v = v.substitute(&substitutions);
                    }
                    // Also substitute any generic that maps directly via outer.
                    for (k, v) in &substitutions {
                        composed.entry(k.clone()).or_insert(v.clone());
                    }
                    worklist.push((inner_span, inner_callee, composed));
                }
            }
        }
    }

    fn function_signature(&mut self, declaration: &ast::FunctionDecl) -> FunctionType {
        self.function_signature_parts(
            declaration.name.clone(),
            &declaration.type_parameters,
            &declaration.parameters,
            declaration.return_type.as_ref(),
            &declaration.error_set,
            declaration.span,
        )
    }

    fn function_signature_parts(
        &mut self,
        name: String,
        type_parameters: &[String],
        parameters: &[ast::Parameter],
        return_type: Option<&ast::Type>,
        error_set: &[ast::TypePath],
        _span: Span,
    ) -> FunctionType {
        let generics = generic_map(type_parameters);
        let parameters = parameters
            .iter()
            .map(|parameter| self.resolve_type(&parameter.ty, &generics))
            .collect::<Vec<_>>();
        let return_type = return_type
            .map(|ty| self.resolve_type(ty, &generics))
            .unwrap_or(Type::Unit);
        let mut errors = Vec::new();
        for error in error_set {
            let resolved = self.resolve_type_path(error, &generics);
            if !matches!(resolved, Type::Named(_, _) | Type::Unknown | Type::Error) {
                self.error(
                    UNDECLARED_ERROR,
                    format!("The error set for '{name}' contains non-nominal type '{resolved}'."),
                    error.span,
                    "Declare a struct or enum error type and use its name after '!'.",
                );
            }
            append_unique(&mut errors, vec![resolved]);
        }

        FunctionType {
            type_parameters: type_parameters.to_vec(),
            parameters,
            return_type: Box::new(return_type),
            errors,
        }
    }

    // ---------------------------------------------------------------------
    // Scopes and modes
    // ---------------------------------------------------------------------

    fn push_scope(&mut self) {
        self.scopes.push(BTreeMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn insert_local(&mut self, name: String, local: Local) {
        if name.is_empty() {
            return;
        }

        if let Some(scope) = self.scopes.last_mut()
            && let Some(existing) = scope.get(&name)
        {
            let span = existing.span;
            self.error(
                UNKNOWN_IDENTIFIER,
                format!("Identifier '{name}' is already declared in this scope."),
                local.span,
                format!(
                    "The previous declaration is at bytes {}..{}.",
                    span.start, span.end
                ),
            );
            return;
        }

        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, local);
        }
    }

    fn lookup_local(&self, name: &str) -> Option<&Local> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    fn check_mode_annotations(&mut self) {
        let module_mode = self.options.mode;
        self.mode = module_mode;
        let attributes = self.source.attributes.clone();
        self.check_capability_attributes(&attributes, module_mode);

        let items = self.source.items.clone();
        // `@export_as` symbols live in the C ABI verbatim, so they must be
        // unique across the whole module, not just per declaration.
        self.check_export_symbol_uniqueness(&items);
        self.check_item_modes(&items, module_mode);
        self.mode = module_mode;
    }

    fn check_item_modes(&mut self, items: &[ast::Item], current_mode: Mode) {
        for item in items {
            if !matches!(item.kind, ast::ItemKind::Function(_))
                && item
                    .attributes
                    .iter()
                    .any(|attribute| attribute.export_as.is_some())
            {
                let span = item
                    .attributes
                    .iter()
                    .find(|attribute| attribute.export_as.is_some())
                    .map(|attribute| attribute.span)
                    .unwrap_or(item.span);
                self.error(
                    EXPORT_AS_TARGET,
                    "@export_as is only allowed on a generic function declaration.",
                    span,
                    "Move the attribute onto a `pub fn`/`export fn` with type parameters.",
                );
            }
            match &item.kind {
                ast::ItemKind::ScopedItems(scoped) => {
                    let scoped_mode =
                        self.check_scope_annotation(&scoped.annotation, scoped.span, current_mode);
                    self.check_item_modes(&scoped.items, scoped_mode);
                }
                ast::ItemKind::Function(function) => {
                    self.function_modes.insert(function.span, current_mode);
                    self.check_capability_attributes(&item.attributes, current_mode);
                    self.check_export_as(item, function);
                }
                ast::ItemKind::Constant(constant) => {
                    self.function_modes.insert(constant.span, current_mode);
                    self.check_capability_attributes(&item.attributes, current_mode);
                }
                ast::ItemKind::Struct(_) | ast::ItemKind::Enum(_) | ast::ItemKind::TypeAlias(_) => {
                    self.check_capability_attributes(&item.attributes, current_mode);
                }
                ast::ItemKind::ForeignModule(foreign) => {
                    if !current_mode.allows(Mode::System) {
                        self.error(
                            MODE_REQUIREMENT,
                            format!(
                                "Foreign module '{}' requires system mode, but the active mode is '{}'.",
                                foreign.name,
                                current_mode.as_str()
                            ),
                            foreign.span,
                            "Move the foreign declaration behind a system-mode boundary.",
                        );
                    }
                    self.check_capability_attributes(&item.attributes, current_mode);
                }
                ast::ItemKind::Import(_) | ast::ItemKind::Export(_) => {
                    self.check_capability_attributes(&item.attributes, current_mode);
                }
                ast::ItemKind::ScriptBlock(script) => {
                    self.check_capability_attributes(&item.attributes, current_mode);

                    // Validate target compatibility (NS7002)
                    if let Some(target) = self.options.target.as_deref() {
                        let supported = match target {
                            "js" | "ts" => matches!(script.language.as_str(), "js" | "ts"),
                            "native" | "c" | "llvm" => {
                                matches!(script.language.as_str(), "c" | "cpp")
                            }
                            _ => false,
                        };
                        if !supported {
                            self.error(
                                UNSUPPORTED_SCRIPT_TARGET,
                                format!(
                                    "Script language '{}' is not supported by the active target '{target}'.",
                                    script.language
                                ),
                                script.span,
                                format!("Target '{target}' does not support '{}' script blocks.", script.language),
                            );
                        }
                    }

                    // Register declared mode for caller verification
                    if let Some(mode) = Mode::parse(&script.mode) {
                        self.script_modes.insert(script.name.clone(), mode);
                    }

                    // Validate boundary safety of export signatures (NS7005)
                    for export in &script.exports {
                        for param in &export.parameters {
                            let ty = self.resolve_type(&param.ty, &BTreeMap::new());
                            if !self.is_boundary_safe(&ty) {
                                self.error(
                                    INVALID_SCRIPT_SIGNATURE,
                                    format!("Type '{ty}' is not boundary-safe for script exports."),
                                    param.span,
                                    "Use boundary-safe primitive types, bool, str, bytes, Option, slices, or structs.",
                                );
                            }
                        }
                        if let Some(ret) = &export.return_type {
                            let ty = self.resolve_type(ret, &BTreeMap::new());
                            if !self.is_boundary_safe(&ty) {
                                self.error(
                                    INVALID_SCRIPT_SIGNATURE,
                                    format!("Type '{ty}' is not boundary-safe for script exports."),
                                    ret.span,
                                    "Use boundary-safe primitive types, bool, str, bytes, Option, slices, or structs.",
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    /// Rejects one `@export_as` symbol used by two declarations in the module
    /// (`NS7013`). The symbol is written into C verbatim, so a repeat would be
    /// a duplicate definition rather than a mangled overload.
    fn check_export_symbol_uniqueness(&mut self, items: &[ast::Item]) {
        let mut symbols: BTreeMap<String, Span> = BTreeMap::new();
        self.check_export_symbols_in(items, &mut symbols);
    }

    fn check_export_symbols_in(
        &mut self,
        items: &[ast::Item],
        symbols: &mut BTreeMap<String, Span>,
    ) {
        for item in items {
            match &item.kind {
                ast::ItemKind::Function(function) => {
                    let exported = item.exported || item.visibility == ast::Visibility::Public;
                    if function.type_parameters.is_empty() || !exported {
                        continue;
                    }
                    for attribute in &item.attributes {
                        let Some(spec) = &attribute.export_as else {
                            continue;
                        };
                        if !is_valid_c_identifier(&spec.symbol) {
                            // Reported by the per-declaration pass.
                            continue;
                        }
                        if symbols.contains_key(&spec.symbol) {
                            self.error(
                                EXPORT_AS_SYMBOL,
                                format!(
                                    "Exported symbol '{}' is already used by another `@export_as` in this module.",
                                    spec.symbol
                                ),
                                spec.span,
                                "Each exported symbol must be unique within the artifact.",
                            );
                        } else {
                            symbols.insert(spec.symbol.clone(), spec.span);
                        }
                    }
                }
                ast::ItemKind::ScopedItems(scoped) => {
                    self.check_export_symbols_in(&scoped.items, symbols);
                }
                _ => {}
            }
        }
    }

    /// `@export_as` (P2): validate the attribute and its instantiation
    /// bindings. Emitting the specialized symbols is the C backend's job; the
    /// diagnostics NS7010-NS7013 live here.
    fn check_export_as(&mut self, item: &ast::Item, function: &ast::FunctionDecl) {
        let specs: Vec<ast::ExportAsAttribute> = item
            .attributes
            .iter()
            .filter_map(|attribute| attribute.export_as.clone())
            .collect();
        if specs.is_empty() {
            return;
        }
        let exported = item.exported || item.visibility == ast::Visibility::Public;
        let mut instantiations = BTreeSet::new();
        for spec in specs {
            let span = spec.span;
            if function.type_parameters.is_empty() {
                self.error(
                    EXPORT_AS_TARGET,
                    "@export_as is only allowed on a generic function.",
                    span,
                    "Remove the attribute or add type parameters to the function.",
                );
                continue;
            }
            if !exported {
                self.error(
                    EXPORT_AS_TARGET,
                    "@export_as is only allowed on an exported (`pub` or `export`) function.",
                    span,
                    "Mark the declaration `pub` or `export`, or remove the attribute.",
                );
                continue;
            }
            let mut bound: BTreeMap<&str, &ast::Type> = BTreeMap::new();
            let mut valid = true;
            for (parameter, ty) in &spec.bindings {
                if !function
                    .type_parameters
                    .iter()
                    .any(|candidate| candidate == parameter)
                {
                    self.error(
                        EXPORT_AS_BINDINGS,
                        format!("`@export_as` binds unknown type parameter '{parameter}'."),
                        span,
                        "Bind each type parameter of the function exactly once.",
                    );
                    valid = false;
                    continue;
                }
                if bound.insert(parameter.as_str(), ty).is_some() {
                    self.error(
                        EXPORT_AS_BINDINGS,
                        format!("`@export_as` binds type parameter '{parameter}' more than once."),
                        span,
                        "Bind each type parameter of the function exactly once.",
                    );
                    valid = false;
                }
            }
            for parameter in &function.type_parameters {
                if !bound.contains_key(parameter.as_str()) {
                    self.error(
                        EXPORT_AS_BINDINGS,
                        format!(
                            "`@export_as` is missing a binding for type parameter '{parameter}'."
                        ),
                        span,
                        "Bind each type parameter of the function exactly once.",
                    );
                    valid = false;
                }
            }
            if !valid {
                continue;
            }
            let mut resolved: BTreeMap<String, Type> = BTreeMap::new();
            for (parameter, ty) in &bound {
                let resolved_ty = self.resolve_type(ty, &BTreeMap::new());
                if matches!(resolved_ty, Type::Error | Type::Unknown) {
                    valid = false;
                    continue;
                }
                if contains_generic(&resolved_ty) {
                    self.error(
                        EXPORT_AS_TYPE,
                        format!(
                            "`@export_as` bound type '{resolved_ty}' mentions a type parameter; it must be concrete."
                        ),
                        span,
                        "Bind a concrete, C-boundary-safe type.",
                    );
                    valid = false;
                    continue;
                }
                resolved.insert((*parameter).to_string(), resolved_ty);
            }
            if !valid {
                continue;
            }
            let mut boundary_ok = true;
            for ty in resolved.values() {
                if !self.is_boundary_safe(ty) {
                    self.error(
                        EXPORT_AS_TYPE,
                        format!("`@export_as` bound type '{ty}' is not C-boundary-safe."),
                        span,
                        "Use primitives, structs, enums, and composites of those (spec 11.4).",
                    );
                    boundary_ok = false;
                }
            }
            if !boundary_ok {
                continue;
            }
            if !is_valid_c_identifier(&spec.symbol) {
                self.error(
                    EXPORT_AS_SYMBOL,
                    format!("'{}' is not a valid C identifier.", spec.symbol),
                    span,
                    "Use letters, digits, and underscores, starting with a letter or underscore.",
                );
                continue;
            }

            let key: Vec<String> = function
                .type_parameters
                .iter()
                .map(|parameter| resolved[parameter].to_string())
                .collect();
            if !instantiations.insert(key) {
                self.error(
                    EXPORT_AS_SYMBOL,
                    "This instantiation is exported more than once.",
                    span,
                    "Remove the duplicate `@export_as`.",
                );
            }
        }
    }

    fn check_capability_attributes(&mut self, attributes: &[ast::Attribute], current_mode: Mode) {
        for attribute in attributes {
            match attribute.name.as_str() {
                "requires" => {
                    for argument in &attribute.arguments {
                        let Some((name, span)) = annotation_name(argument) else {
                            self.error(
                                INVALID_CAPABILITY_ANNOTATION,
                                "Attribute '@requires' expects a mode name.",
                                argument.span,
                                "Use core, app, web, or system.",
                            );
                            continue;
                        };
                        let Some(required) = Mode::parse(&name) else {
                            self.error(
                                INVALID_CAPABILITY_ANNOTATION,
                                format!("Unknown mode '{name}' in '@requires'."),
                                span,
                                "Use core, app, web, or system.",
                            );
                            continue;
                        };
                        if !current_mode.allows(required) {
                            self.error(
                                MODE_REQUIREMENT,
                                format!(
                                    "This item requires '{}' mode, but the active mode is '{}'.",
                                    required.as_str(),
                                    current_mode.as_str()
                                ),
                                attribute.span,
                                format!(
                                    "Mark the surrounding module or block @requires({}) or run with --mode {}.",
                                    required.as_str(),
                                    required.as_str()
                                ),
                            );
                        }
                    }
                }
                "tags" => {
                    for argument in &attribute.arguments {
                        let Some((tag, span)) = annotation_name(argument) else {
                            self.error(
                                INVALID_CAPABILITY_ANNOTATION,
                                "Attribute '@tags' expects capability names.",
                                argument.span,
                                "Use identifiers such as fs, net, dom, or ffi.",
                            );
                            continue;
                        };
                        let Some(required) = Mode::requirement_for_capability(&tag) else {
                            continue;
                        };
                        if !current_mode.allows(required) {
                            self.error(
                                CAPABILITY_REQUIREMENT,
                                format!(
                                    "Capability '{tag}' requires '{}' mode, but the active mode is '{}'.",
                                    required.as_str(),
                                    current_mode.as_str()
                                ),
                                span,
                                format!(
                                    "Run with --mode {} or remove the capability tag.",
                                    required.as_str()
                                ),
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// True when `span` starts inside an `@memory:manual` block.
    ///
    /// Recorded scope spans cover the whole block, so this is a plain offset
    /// test. Scopes from other functions can never match: module spans are
    /// disjoint, and a scope is only reachable from code the checker visited
    /// while inside it.
    fn in_manual_memory_scope(&self, span: Span) -> bool {
        self.manual_memory_scopes
            .iter()
            .any(|scope| scope.contains(span.start))
    }

    /// Reports `NS6002` when a manual allocation carries GC-managed
    /// references into `Ptr<T>` (spec 8.4; GC-001 proposal §11, condition 2).
    ///
    /// Fires only for member spans registered as a gate-passed
    /// `Memory.alloc` / `region.alloc`, and only once the call's `T` is bound
    /// in the return type, so it covers explicit type arguments and inferred
    /// ones alike. `Error`/`Unknown`/`Generic` element types stay silent:
    /// those paths already reported their own diagnostic.
    fn check_manual_alloc_element(&mut self, callee: &ast::Expr, return_type: &Type, span: Span) {
        let target_span = match &callee.kind {
            ast::ExprKind::TypeArguments { target, .. } => target.span,
            _ => callee.span,
        };
        if !self.manual_alloc_spans.contains(&target_span) {
            return;
        }
        let Type::Named(name, arguments) = return_type else {
            return;
        };
        if name != "Ptr" {
            return;
        }
        let Some(element) = arguments.first() else {
            return;
        };
        if matches!(element, Type::Error | Type::Unknown) {
            return;
        }
        if contains_generic(element) {
            if let Some(current) = &self.current_function {
                self.deferred_alloc_checks
                    .entry(current.name.clone())
                    .or_default()
                    .push((span, element.clone()));
            }
            return;
        }
        if self.holds_managed_refs(element) {
            self.error(
                MANAGED_ELEMENT_IN_MANUAL_ALLOC,
                format!(
                    "Manual allocation cannot use element type '{element}': it holds \
                     GC-managed references, and 'Ptr<T>' is never a GC root (spec 8.4), \
                     so a collection could reclaim data this pointer still points at."
                ),
                span,
                "Allocate pointer-free types here (scalars, Ptr<U>, or a struct/enum of \
                 those); str, bytes, [T], Map, Channel, Task, and composites containing \
                 them belong to GC-managed memory.",
            );
        }
    }

    /// True when a value of this type contains references a precise collector
    /// would have to trace.
    ///
    /// This is the inverse of the pointer-free requirement for manual
    /// allocation and the same predicate GC-001's shape analysis will use
    /// (proposal §5.5/§11). `Error`, `Unknown`, and `Generic` return false:
    /// unresolvable or not-yet-bound types are silent here because a
    /// diagnostic for them is someone else's job.
    fn holds_managed_refs(&mut self, ty: &Type) -> bool {
        match ty {
            // Fat pointers into managed buffers, and the buffer types
            // themselves: their storage is a GC block or a view into one.
            Type::Str | Type::Bytes | Type::Array(_) | Type::Slice(_) | Type::Map(_, _) => true,
            // Fixed arrays are inline storage: pointer-free iff the element is.
            Type::FixedArray(element, _) => self.holds_managed_refs(element),
            Type::Tuple(elements) => elements
                .iter()
                .any(|element| self.holds_managed_refs(element)),
            Type::Option(inner) => self.holds_managed_refs(inner),
            Type::Result(ok, error) => {
                self.holds_managed_refs(ok) || self.holds_managed_refs(error)
            }
            Type::Named(name, arguments) => match name.as_str() {
                // Spec 8.4: `Ptr<T>` is not a GC reference - a raw address is
                // one machine word, whatever it points at. A `Region` handle
                // points at a manual block the collector never traces.
                "Ptr" | "Region" => false,
                // Handles whose payload is managed.
                "Channel" | "Task" => true,
                _ => {
                    if let Some(struct_def) = self.resolve_struct(name) {
                        let substitutions = substitutions(&struct_def.type_parameters, arguments);
                        struct_def.fields.iter().any(|field| {
                            self.holds_managed_refs(&field.ty.substitute(&substitutions))
                        })
                    } else if let Some(enum_def) = self.resolve_enum(name) {
                        let substitutions = substitutions(&enum_def.type_parameters, arguments);
                        enum_def.variants.iter().any(|variant| {
                            variant.fields.iter().flatten().any(|field| {
                                self.holds_managed_refs(&field.ty.substitute(&substitutions))
                            })
                        })
                    } else {
                        // Opaque or foreign nominal: no payload to inspect,
                        // and its construction surface is gated elsewhere.
                        false
                    }
                }
            },
            // Scalars, unit, never, function values, and the silent types.
            _ => false,
        }
    }

    /// True when a value of this type carries a `Task` handle, directly or
    /// inside a composite. A task handle has one owner and a heap-bound
    /// record, so it must never cross a task boundary as a copied value.
    fn contains_task(&mut self, ty: &Type) -> bool {
        match ty {
            Type::FixedArray(element, _)
            | Type::Array(element)
            | Type::Slice(element)
            | Type::Option(element) => self.contains_task(element),
            Type::Tuple(elements) => elements.iter().any(|element| self.contains_task(element)),
            Type::Result(ok, error) => self.contains_task(ok) || self.contains_task(error),
            Type::Map(key, value) => self.contains_task(key) || self.contains_task(value),
            Type::Named(name, arguments) => match name.as_str() {
                "Task" => true,
                // A channel handle is shared, but a channel whose element type
                // carries a task can move that task across a boundary.
                "Channel" => arguments
                    .iter()
                    .any(|argument| self.contains_task(argument)),
                "Ptr" | "Region" => false,
                _ => {
                    if let Some(struct_def) = self.resolve_struct(name) {
                        let substitutions = substitutions(&struct_def.type_parameters, arguments);
                        struct_def
                            .fields
                            .iter()
                            .any(|field| self.contains_task(&field.ty.substitute(&substitutions)))
                    } else if let Some(enum_def) = self.resolve_enum(name) {
                        let substitutions = substitutions(&enum_def.type_parameters, arguments);
                        enum_def.variants.iter().any(|variant| {
                            variant.fields.iter().flatten().any(|field| {
                                self.contains_task(&field.ty.substitute(&substitutions))
                            })
                        })
                    } else {
                        false
                    }
                }
            },
            _ => false,
        }
    }

    /// Rejects a channel whose element type carries a task handle. Messages
    /// are deep-copied across task heaps, and copying a task record would
    /// duplicate its thread handle and heap pointer.
    fn check_channel_element(&mut self, callee: &ast::Expr, return_type: &Type, span: Span) {
        let Type::Named(name, arguments) = return_type else {
            return;
        };
        if name != "Channel" || self.resolve_struct(name).is_some() {
            return;
        }
        if !channel_create_callee(callee) {
            return;
        }
        let Some(element) = arguments.first() else {
            return;
        };
        if self.contains_task(element) {
            self.error(
                TASK_BOUNDARY,
                "Channel<T> cannot carry Task handles or values containing them.".to_owned(),
                span,
                "Send the task result instead; a task handle has a single owner.",
            );
        }
    }

    /// Validates a scope header and returns the mode it selects.
    ///
    /// `scope_span` covers the whole `{@...; ...}` block, not just the header
    /// line: memory scopes are recorded against it so containment checks for
    /// manual-memory calls see the body too.
    fn check_scope_annotation(
        &mut self,
        annotation: &ast::ScopeAnnotation,
        scope_span: Span,
        current_mode: Mode,
    ) -> Mode {
        match annotation.kind {
            ast::ScopeKind::Mode => {
                let Some(scoped_mode) = Mode::parse(&annotation.value) else {
                    self.error(
                        INVALID_CAPABILITY_ANNOTATION,
                        format!("Unknown mode '{}' in a scope annotation.", annotation.value),
                        annotation.span,
                        "Use core, app, web, or system.",
                    );
                    return current_mode;
                };
                if !current_mode.allows(scoped_mode) {
                    self.error(
                        MODE_WIDENING,
                        format!(
                            "Mode widening from '{}' to '{}' is not allowed.",
                            current_mode.as_str(),
                            scoped_mode.as_str()
                        ),
                        annotation.span,
                        "A scoped block may narrow capabilities, not widen them.",
                    );
                    return current_mode;
                }
                scoped_mode
            }
            ast::ScopeKind::Memory => {
                if annotation.value == "manual" {
                    self.manual_memory_scopes.push(scope_span);
                }
                if annotation.value == "manual" && !current_mode.allows(Mode::System) {
                    self.error(
                        MEMORY_REQUIREMENT,
                        format!(
                            "Manual memory requires system mode, but the active mode is '{}'.",
                            current_mode.as_str()
                        ),
                        annotation.span,
                        "Use '@memory:manual' only inside a system-mode boundary.",
                    );
                }
                current_mode
            }
        }
    }

    fn is_boundary_safe(&mut self, ty: &Type) -> bool {
        match ty {
            Type::I8
            | Type::I16
            | Type::I32
            | Type::I64
            | Type::U8
            | Type::U16
            | Type::U32
            | Type::U64
            | Type::Usize
            | Type::Isize
            | Type::F32
            | Type::F64
            | Type::Bool
            | Type::Str
            | Type::Bytes
            | Type::Char
            | Type::Unit => true,
            Type::Option(inner) => self.is_boundary_safe(inner),
            Type::Array(inner)
            | Type::Slice(inner)
            | Type::Range(inner)
            | Type::FixedArray(inner, _) => self.is_boundary_safe(inner),
            Type::Named(name, args) => {
                if let Some(def) = self.resolve_struct(name) {
                    def.fields.iter().all(|f| self.is_boundary_safe(&f.ty))
                } else {
                    args.iter().all(|arg| self.is_boundary_safe(arg))
                }
            }
            Type::Result(ok, err) => self.is_boundary_safe(ok) && self.is_boundary_safe(err),
            Type::Generic(_)
            | Type::Function(_)
            | Type::Tuple(_)
            | Type::Map(_, _)
            | Type::Never => false,
            Type::Error | Type::Unknown => true,
        }
    }

    // ---------------------------------------------------------------------
    // Type resolution
    // ---------------------------------------------------------------------

    fn resolve_type(&mut self, ty: &ast::Type, generics: &BTreeMap<String, Type>) -> Type {
        match &ty.kind {
            ast::TypeKind::Path(path) => self.resolve_type_path(path, generics),
            ast::TypeKind::Array(element) => {
                Type::Array(Box::new(self.resolve_type(element, generics)))
            }
            ast::TypeKind::FixedArray(element, length) => {
                let length = length
                    .replace('_', "")
                    .parse::<usize>()
                    .unwrap_or_else(|_| {
                        self.error(
                            UNKNOWN_TYPE,
                            format!("Invalid fixed-array length '{length}'."),
                            ty.span,
                            "Use a non-negative decimal integer.",
                        );
                        0
                    });
                Type::FixedArray(Box::new(self.resolve_type(element, generics)), length)
            }
            ast::TypeKind::Map(key, value) => Type::Map(
                Box::new(self.resolve_type(key, generics)),
                Box::new(self.resolve_type(value, generics)),
            ),
            ast::TypeKind::Tuple(elements) => Type::Tuple(
                elements
                    .iter()
                    .map(|element| self.resolve_type(element, generics))
                    .collect(),
            ),
            ast::TypeKind::Function {
                parameters,
                return_type,
                error_set,
            } => Type::Function(FunctionType {
                type_parameters: Vec::new(),
                parameters: parameters
                    .iter()
                    .map(|parameter| self.resolve_type(parameter, generics))
                    .collect(),
                return_type: Box::new(
                    return_type
                        .as_ref()
                        .map(|ty| self.resolve_type(ty, generics))
                        .unwrap_or(Type::Unit),
                ),
                errors: error_set
                    .iter()
                    .map(|error| self.resolve_type_path(error, generics))
                    .collect(),
            }),
            ast::TypeKind::Unit => Type::Unit,
        }
    }

    fn resolve_type_path(
        &mut self,
        path: &ast::TypePath,
        generics: &BTreeMap<String, Type>,
    ) -> Type {
        let name = path.segments.join(".");
        let short_name = path
            .segments
            .last()
            .cloned()
            .unwrap_or_else(|| name.clone());
        let arguments = path
            .arguments
            .iter()
            .map(|argument| self.resolve_type(argument, generics))
            .collect::<Vec<_>>();

        if path.arguments.is_empty()
            && let Some(generic) = generics.get(&short_name)
        {
            return generic.clone();
        }

        if !path.arguments.is_empty() && generics.contains_key(&short_name) {
            self.error(
                INVALID_TYPE_ARGUMENT,
                format!("Generic parameter '{short_name}' cannot take type arguments."),
                path.span,
                "Remove the type argument list.",
            );
            return Type::Error;
        }

        match short_name.as_str() {
            "i8" => return primitive_type(&arguments, Type::I8, path.span, self),
            "i16" => return primitive_type(&arguments, Type::I16, path.span, self),
            "i32" => return primitive_type(&arguments, Type::I32, path.span, self),
            "i64" => return primitive_type(&arguments, Type::I64, path.span, self),
            "u8" => return primitive_type(&arguments, Type::U8, path.span, self),
            "u16" => return primitive_type(&arguments, Type::U16, path.span, self),
            "u32" => return primitive_type(&arguments, Type::U32, path.span, self),
            "u64" => return primitive_type(&arguments, Type::U64, path.span, self),
            "usize" => return primitive_type(&arguments, Type::Usize, path.span, self),
            "isize" => return primitive_type(&arguments, Type::Isize, path.span, self),
            "f32" => return primitive_type(&arguments, Type::F32, path.span, self),
            "f64" => return primitive_type(&arguments, Type::F64, path.span, self),
            "bool" => return primitive_type(&arguments, Type::Bool, path.span, self),
            "str" => return primitive_type(&arguments, Type::Str, path.span, self),
            "bytes" => return primitive_type(&arguments, Type::Bytes, path.span, self),
            "char" if self.options.edition == Edition::V0_2 => {
                return primitive_type(&arguments, Type::Char, path.span, self);
            }
            "Never" => return primitive_type(&arguments, Type::Never, path.span, self),
            "()" => return Type::Unit,
            "Option" => {
                if arguments.len() != 1 {
                    self.error(
                        INVALID_TYPE_ARGUMENT,
                        "Option requires exactly one type argument.",
                        path.span,
                        "Use Option<T>.",
                    );
                    return Type::Error;
                }
                return Type::Option(Box::new(arguments[0].clone()));
            }
            "Result" => {
                if arguments.len() != 2 {
                    self.error(
                        INVALID_TYPE_ARGUMENT,
                        "Result requires exactly two type arguments.",
                        path.span,
                        "Use Result<T, E>.",
                    );
                    return Type::Error;
                }
                return Type::Result(
                    Box::new(arguments[0].clone()),
                    Box::new(arguments[1].clone()),
                );
            }
            // `Region` (spec 8.4 manual API) is a non-generic handle: one
            // fixed block, no element type. `region.alloc<T>` supplies the
            // element type at the allocation site instead.
            "Region" => {
                if !arguments.is_empty() {
                    self.error(
                        INVALID_TYPE_ARGUMENT,
                        "'Region' takes no type arguments.",
                        path.span,
                        "Use 'Region' on its own.",
                    );
                    return Type::Error;
                }
                return Type::Named("Region".to_owned(), Vec::new());
            }
            "Task" | "Ptr" | "MutSlice" | "Channel" => {
                if arguments.len() != 1 {
                    self.error(
                        INVALID_TYPE_ARGUMENT,
                        format!("{short_name} requires exactly one type argument."),
                        path.span,
                        format!("Use {short_name}<T>."),
                    );
                    return Type::Error;
                }
                return Type::Named(short_name, arguments);
            }
            _ => {}
        }

        if let Some(def) = self.type_defs.get(&short_name).cloned() {
            return self.resolve_declared_type(&short_name, def, arguments, path.span);
        }

        if self.imported_types.contains(&short_name) {
            return Type::Unknown;
        }

        if path.segments.len() > 1 {
            return Type::Unknown;
        }

        self.error(
            UNKNOWN_TYPE,
            format!("Unknown type '{name}'."),
            path.span,
            "Declare the type, import it, or use a built-in type.",
        );
        Type::Error
    }

    fn resolve_declared_type(
        &mut self,
        name: &str,
        def: TypeDef,
        arguments: Vec<Type>,
        span: Span,
    ) -> Type {
        match def {
            TypeDef::Struct(struct_def) => {
                if !check_arity(&struct_def.type_parameters, &arguments, name, span, self) {
                    return Type::Error;
                }
                Type::Named(name.to_owned(), arguments)
            }
            TypeDef::Enum(enum_def) => {
                if !check_arity(&enum_def.type_parameters, &arguments, name, span, self) {
                    return Type::Error;
                }
                Type::Named(name.to_owned(), arguments)
            }
            TypeDef::Alias(alias) => {
                if !check_arity(&alias.type_parameters, &arguments, name, span, self) {
                    return Type::Error;
                }
                if self.alias_stack.iter().any(|active| active == name) {
                    self.error(
                        UNKNOWN_TYPE,
                        format!("Cyclic type alias involving '{name}'."),
                        span,
                        "Break the alias cycle by naming a struct or enum.",
                    );
                    return Type::Error;
                }

                let mut substitutions = BTreeMap::new();
                for (parameter, argument) in alias.type_parameters.iter().zip(arguments) {
                    substitutions.insert(parameter.clone(), argument);
                }

                self.alias_stack.push(name.to_owned());
                let resolved = self.resolve_type(&alias.ty, &substitutions);
                self.alias_stack.pop();
                resolved
            }
            TypeDef::BuiltinNominal => Type::Named(name.to_owned(), arguments),
        }
    }

    fn resolve_struct(&mut self, name: &str) -> Option<ResolvedStruct> {
        let TypeDef::Struct(def) = self.type_defs.get(name)?.clone() else {
            return None;
        };
        let generics = generic_map(&def.type_parameters);
        let fields = match &def.fields {
            ast::StructFields::Named(fields) => fields
                .iter()
                .map(|field| ResolvedField {
                    name: Some(field.name.clone()),
                    ty: self.resolve_type(&field.ty, &generics),
                })
                .collect(),
            ast::StructFields::Tuple(fields) => fields
                .iter()
                .enumerate()
                .map(|(index, field)| ResolvedField {
                    name: Some(index.to_string()),
                    ty: self.resolve_type(field, &generics),
                })
                .collect(),
        };

        Some(ResolvedStruct {
            type_parameters: def.type_parameters,
            fields,
        })
    }

    fn resolve_enum(&mut self, name: &str) -> Option<ResolvedEnum> {
        let TypeDef::Enum(def) = self.type_defs.get(name)?.clone() else {
            return None;
        };
        let generics = generic_map(&def.type_parameters);
        let variants = def
            .variants
            .iter()
            .map(|variant| {
                let fields = variant.fields.as_ref().map(|fields| {
                    fields
                        .iter()
                        .enumerate()
                        .map(|(index, field)| ResolvedField {
                            name: field.name.clone().or_else(|| Some(index.to_string())),
                            ty: self.resolve_type(&field.ty, &generics),
                        })
                        .collect::<Vec<_>>()
                });
                ResolvedVariant {
                    name: variant.name.clone(),
                    fields,
                }
            })
            .collect();

        Some(ResolvedEnum {
            type_parameters: def.type_parameters,
            variants,
        })
    }

    // ---------------------------------------------------------------------
    // Statements
    // ---------------------------------------------------------------------

    fn check_block(
        &mut self,
        block: &ast::Block,
        collect_tail: bool,
        expected: Option<&Type>,
    ) -> BlockInfo {
        self.push_scope();
        let mut diverges = false;
        let mut tail = None;

        for (index, statement) in block.statements.iter().enumerate() {
            let is_last = index + 1 == block.statements.len();
            let tail_expected = if collect_tail && is_last {
                expected
            } else {
                None
            };
            let info = self.check_stmt(statement, tail_expected);
            if info.diverges {
                diverges = true;
            }

            if let Some(expression) = info.expression {
                if collect_tail && is_last {
                    tail = Some(expression);
                } else {
                    self.report_effects(&expression.errors);
                }
            }
        }

        self.pop_scope();

        BlockInfo { tail, diverges }
    }

    fn check_stmt(&mut self, statement: &ast::Stmt, expected: Option<&Type>) -> StmtInfo {
        match &statement.kind {
            ast::StmtKind::Let {
                mutable,
                name,
                ty,
                value,
            } => {
                let declared = ty
                    .as_ref()
                    .map(|ty| self.resolve_type(ty, &BTreeMap::new()));
                let info = self.check_expr(value, declared.as_ref());
                self.report_effects(&info.errors);
                let local_ty = declared.unwrap_or(info.ty);
                self.insert_local(
                    name.clone(),
                    Local {
                        ty: local_ty,
                        mutable: *mutable,
                        span: statement.span,
                    },
                );
                StmtInfo {
                    expression: None,
                    diverges: false,
                }
            }
            ast::StmtKind::Return { value } => {
                let return_type = self
                    .current_function
                    .as_ref()
                    .map(|function| function.return_type.clone());
                let Some(return_type) = return_type else {
                    self.error(
                        INVALID_CONTROL_FLOW,
                        "'return' is only valid inside a function.",
                        statement.span,
                        "Move the return into a function body.",
                    );
                    return StmtInfo {
                        expression: None,
                        diverges: true,
                    };
                };

                let info = match value {
                    Some(value) => self.check_expr(value, Some(&return_type)),
                    None => {
                        let info = ExprInfo::value(Type::Unit);
                        if !return_type.accepts(&Type::Unit) {
                            self.type_mismatch(&return_type, &Type::Unit, statement.span);
                        }
                        info
                    }
                };
                self.report_effects(&info.errors);
                StmtInfo {
                    expression: None,
                    diverges: true,
                }
            }
            ast::StmtKind::Break => {
                if self.loop_depth == 0 {
                    self.error(
                        INVALID_CONTROL_FLOW,
                        "'break' is only valid inside a loop.",
                        statement.span,
                        "Move the break into a while or for body.",
                    );
                }
                StmtInfo {
                    expression: None,
                    diverges: true,
                }
            }
            ast::StmtKind::Continue => {
                if self.loop_depth == 0 {
                    self.error(
                        INVALID_CONTROL_FLOW,
                        "'continue' is only valid inside a loop.",
                        statement.span,
                        "Move the continue into a while or for body.",
                    );
                }
                StmtInfo {
                    expression: None,
                    diverges: true,
                }
            }
            ast::StmtKind::Defer(deferred) => {
                let info = self.check_stmt(deferred, None);
                if let Some(expression) = info.expression {
                    self.report_effects(&expression.errors);
                }
                StmtInfo {
                    expression: None,
                    diverges: false,
                }
            }
            ast::StmtKind::While { condition, body } => {
                let condition = self.check_expr(condition, Some(&Type::Bool));
                self.report_effects(&condition.errors);
                self.loop_depth += 1;
                let _ = self.check_block(body, false, None);
                self.loop_depth -= 1;
                StmtInfo {
                    expression: None,
                    diverges: false,
                }
            }
            ast::StmtKind::For {
                binding,
                iterable,
                body,
            } => {
                let iterable = self.check_expr(iterable, None);
                self.report_effects(&iterable.errors);
                let element = match &iterable.ty {
                    Type::Range(inner) => (**inner).clone(),
                    _ => self.iterable_element(&iterable.ty, statement.span),
                };
                self.push_scope();
                self.insert_local(
                    binding.clone(),
                    Local {
                        ty: element,
                        mutable: false,
                        span: statement.span,
                    },
                );
                self.loop_depth += 1;
                let _ = self.check_block(body, false, None);
                self.loop_depth -= 1;
                self.pop_scope();
                StmtInfo {
                    expression: None,
                    diverges: false,
                }
            }
            ast::StmtKind::Expr(expression) => {
                let info = self.check_expr(expression, expected);
                StmtInfo {
                    diverges: info.ty == Type::Never,
                    expression: Some(info),
                }
            }
            ast::StmtKind::Scoped(scoped) => {
                let previous_mode = self.mode;
                self.mode =
                    self.check_scope_annotation(&scoped.annotation, scoped.span, previous_mode);
                self.push_scope();
                for statement in &scoped.statements {
                    let info = self.check_stmt(statement, None);
                    if let Some(expression) = info.expression {
                        self.report_effects(&expression.errors);
                    }
                }
                self.pop_scope();
                self.mode = previous_mode;
                StmtInfo {
                    expression: None,
                    diverges: false,
                }
            }
        }
    }

    fn iterable_element(&mut self, ty: &Type, span: Span) -> Type {
        match ty {
            Type::Array(element) | Type::Slice(element) | Type::FixedArray(element, _) => {
                (**element).clone()
            }
            Type::Map(key, _) => (**key).clone(),
            Type::Str => Type::Str,
            Type::Bytes => Type::U8,
            Type::Unknown | Type::Error => Type::Unknown,
            _ => {
                self.error(
                    INVALID_OPERANDS,
                    format!("Type '{ty}' is not iterable."),
                    span,
                    "Iterate over an array, slice, map, str, or bytes.",
                );
                Type::Error
            }
        }
    }

    // ---------------------------------------------------------------------
    // Expressions
    // ---------------------------------------------------------------------

    fn check_expr(&mut self, expression: &ast::Expr, expected: Option<&Type>) -> ExprInfo {
        let info = self.check_expr_inner(expression, expected);
        if let Some(expected) = expected
            && !expected.accepts(&info.ty)
        {
            self.type_mismatch(expected, &info.ty, expression.span);
        }
        self.annotate(expression.span, info.ty.clone());
        info
    }

    /// D27: reject two or more state-changing subexpressions in a
    /// target-native-order position while checking in system mode. Reports at
    /// the outermost violating node and suppresses nested duplicates.
    fn check_native_order_effects(&mut self, expression: &ast::Expr) -> bool {
        if self.mode != Mode::System || self.native_order_suppress > 0 {
            return false;
        }
        let (effects, span) = match &expression.kind {
            ast::ExprKind::Binary { left, right, .. } => {
                let count = usize::from(expr_has_call(left)) + usize::from(expr_has_call(right));
                (count, expression.span)
            }
            ast::ExprKind::Array(elements) | ast::ExprKind::Tuple(elements) => (
                elements
                    .iter()
                    .filter(|element| expr_has_call(element))
                    .count(),
                expression.span,
            ),
            ast::ExprKind::Map(entries) => (
                entries
                    .iter()
                    .filter(|entry| expr_has_call(&entry.key) || expr_has_call(&entry.value))
                    .count(),
                expression.span,
            ),
            ast::ExprKind::StructInit { fields, .. } => (
                fields
                    .iter()
                    .filter(|field| expr_has_call(&field.value))
                    .count(),
                expression.span,
            ),
            _ => return false,
        };
        if effects < 2 {
            return false;
        }
        self.error(
            NATIVE_ORDER_EFFECTS,
            "This expression runs more than one state-changing call in a position \
             whose evaluation order is target-native, so the order is not portable \
             (spec 3.10).",
            span,
            "Hoist each call into its own `let` binding first; the bindings run in \
             source order on every backend.",
        );
        true
    }

    fn check_expr_inner(&mut self, expression: &ast::Expr, expected: Option<&Type>) -> ExprInfo {
        let flagged = self.check_native_order_effects(expression);
        if flagged {
            self.native_order_suppress += 1;
        }
        let info = self.check_expr_inner_body(expression, expected);
        if flagged {
            self.native_order_suppress -= 1;
        }
        info
    }

    fn check_expr_inner_body(
        &mut self,
        expression: &ast::Expr,
        expected: Option<&Type>,
    ) -> ExprInfo {
        match &expression.kind {
            ast::ExprKind::Literal(literal) => {
                ExprInfo::value(self.literal_type(literal, expected, expression.span))
            }
            ast::ExprKind::Identifier(name) => self.check_identifier(name, expression.span),
            ast::ExprKind::Unary { operator, operand } => {
                self.check_unary(*operator, operand, expression.span, expected)
            }
            ast::ExprKind::Binary {
                operator,
                left,
                right,
            } => self.check_binary(*operator, left, right, expression.span, expected),
            ast::ExprKind::Assign { target, value } => {
                self.check_assignment(target, value, expression.span)
            }
            ast::ExprKind::Call { callee, arguments } => {
                self.check_call(callee, arguments, expression.span, expected)
            }
            ast::ExprKind::Index { target, index } => {
                self.check_index(target, index, expression.span)
            }
            ast::ExprKind::Range {
                start,
                end,
                inclusive: _,
            } => self.check_range(start, end, expected),
            ast::ExprKind::Member { target, member } => {
                self.check_member(target, member, expression.span, expected)
            }
            ast::ExprKind::TypeArguments { target, arguments } => {
                for argument in arguments {
                    let _ = self.resolve_type(argument, &BTreeMap::new());
                }
                let info = self.check_expr(target, expected);
                if self.options.edition == Edition::V0_2 {
                    let unit_variant = match &target.kind {
                        ast::ExprKind::Identifier(name) => {
                            name == "None" || self.variant_owners.contains_key(name)
                        }
                        _ => false,
                    };
                    if !unit_variant {
                        self.error(
                            BARE_TYPE_ARGUMENTS,
                            "Type arguments on a value require a call or constructor.",
                            expression.span,
                            "Write path::<T>(value) or use a unit variant such as None::<T>.",
                        );
                    }
                }
                info
            }
            ast::ExprKind::Propagate(inner) => self.check_propagation(inner, expression.span),
            ast::ExprKind::Array(elements) => self.check_array(elements, expression.span, expected),
            ast::ExprKind::Map(entries) => self.check_map(entries, expression.span, expected),
            ast::ExprKind::Tuple(elements) => self.check_tuple(elements, expression.span, expected),
            ast::ExprKind::StructInit { path, fields } => {
                self.check_struct_init(path, fields, expression.span)
            }
            ast::ExprKind::Block(block) => self.check_block_expression(block, expected),
            ast::ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => self.check_if(
                condition,
                then_branch,
                else_branch.as_ref(),
                expression.span,
                expected,
            ),
            ast::ExprKind::Match { value, arms } => {
                self.check_match(value, arms, expression.span, expected)
            }
            ast::ExprKind::Spawn { body } => {
                let info = self.check_block(body, true, None);
                let value_type = info
                    .tail
                    .as_ref()
                    .map_or(Type::Unit, |tail| tail.ty.clone());
                if let Some(tail) = info.tail {
                    self.report_effects(&tail.errors);
                }
                // Spawn result cannot carry a Task handle.
                if self.contains_task(&value_type) {
                    self.error(
                        TASK_BOUNDARY,
                        "a spawn result cannot carry a Task handle; the child must join the task and return the value.".to_owned(),
                        expression.span,
                        "Return the task's result value instead of the handle.",
                    );
                }
                // Spawn captures: free names of the body after scope pop.
                // check_block already popped its scope, so enclosing scopes are active.
                let free = crate::free_names::spawn_free_names(body);
                let mut reported_capture = false;
                for name in &free {
                    if reported_capture {
                        break;
                    }
                    if let Some(local) = self.lookup_local(name) {
                        let ty = local.ty.clone();
                        if self.contains_task(&ty) {
                            self.error(
                                TASK_BOUNDARY,
                                format!(
                                    "spawn captures a Task handle via `{name}: {ty}`; task handles cannot cross a task boundary."
                                ),
                                expression.span,
                                "Join the task inside the child and capture the result instead.",
                            );
                            reported_capture = true;
                        }
                    }
                }
                ExprInfo::value(Type::Named("Task".to_owned(), vec![value_type]))
            }
            ast::ExprKind::Fail { error, arguments } => {
                self.check_fail(error, arguments, expression.span)
            }
            ast::ExprKind::Panic { message } | ast::ExprKind::Abort { message } => {
                let info = self.check_expr(message, Some(&Type::Str));
                ExprInfo {
                    ty: Type::Never,
                    errors: info.errors,
                }
            }
        }
    }

    fn check_identifier(&mut self, name: &str, span: Span) -> ExprInfo {
        if let Some(local) = self.lookup_local(name) {
            return ExprInfo::value(local.ty.clone());
        }
        if let Some(value) = self.values.get(name) {
            return ExprInfo::value(value.ty.clone());
        }
        if name == "None" {
            // The payload is unconstrained until context supplies a type, as
            // with any other empty collection literal.
            return ExprInfo::value(Type::Option(Box::new(Type::Unknown)));
        }
        if let Some(ty) = builtin_function(name) {
            return ExprInfo::value(ty);
        }
        if self.type_defs.contains_key(name) || self.imported_types.contains(name) {
            self.error(
                UNKNOWN_IDENTIFIER,
                format!("Type '{name}' cannot be used as a value."),
                span,
                "Use the type in a type position or construct a value with its initializer.",
            );
            return ExprInfo::value(Type::Error);
        }
        if self.namespaces.contains_key(name) {
            self.error(
                UNKNOWN_IDENTIFIER,
                format!("Namespace '{name}' cannot be used as a value."),
                span,
                "Access one of its members instead.",
            );
            return ExprInfo::value(Type::Error);
        }

        self.error(
            UNKNOWN_IDENTIFIER,
            format!("Unknown identifier '{name}'."),
            span,
            "Declare the name before using it.",
        );
        ExprInfo::value(Type::Error)
    }

    fn check_unary(
        &mut self,
        operator: ast::UnaryOp,
        operand: &ast::Expr,
        span: Span,
        expected: Option<&Type>,
    ) -> ExprInfo {
        match operator {
            ast::UnaryOp::Negate => {
                let info = self.check_expr(operand, expected.filter(|ty| ty.is_numeric()));
                if info.ty == Type::Never {
                    return info;
                }
                if !info.ty.is_numeric() && !info.ty.is_opaque() {
                    self.error(
                        INVALID_OPERANDS,
                        format!("Unary '-' expects a numeric operand, found '{}'.", info.ty),
                        span,
                        "Only integers and floats can be negated.",
                    );
                    return ExprInfo::value(Type::Error).with_errors(info.errors);
                }
                ExprInfo::value(info.ty).with_errors(info.errors)
            }
            ast::UnaryOp::Not => {
                let info = self.check_expr(operand, Some(&Type::Bool));
                ExprInfo::value(Type::Bool).with_errors(info.errors)
            }
        }
    }

    fn check_binary(
        &mut self,
        operator: ast::BinaryOp,
        left: &ast::Expr,
        right: &ast::Expr,
        span: Span,
        expected: Option<&Type>,
    ) -> ExprInfo {
        match operator {
            ast::BinaryOp::And | ast::BinaryOp::Or => {
                let left = self.check_expr(left, Some(&Type::Bool));
                let right = self.check_expr(right, Some(&Type::Bool));
                ExprInfo::value(Type::Bool).with_errors(merge_effects(&left.errors, &right.errors))
            }
            ast::BinaryOp::Add
            | ast::BinaryOp::Subtract
            | ast::BinaryOp::Multiply
            | ast::BinaryOp::Divide
            | ast::BinaryOp::Remainder
            | ast::BinaryOp::AddWrap
            | ast::BinaryOp::SubtractWrap
            | ast::BinaryOp::MultiplyWrap
            | ast::BinaryOp::DivideWrap => {
                let numeric_expected = expected.filter(|ty| ty.is_numeric());
                let (left_info, right_info) =
                    if is_numeric_literal(left) && !is_numeric_literal(right) {
                        let right_info = self.check_expr(right, numeric_expected);
                        let left_info = self.check_expr(left, Some(&right_info.ty));
                        (left_info, right_info)
                    } else {
                        let left_info = self.check_expr(left, numeric_expected);
                        let right_info = self.check_expr(right, Some(&left_info.ty));
                        (left_info, right_info)
                    };

                let errors = merge_effects(&left_info.errors, &right_info.errors);
                if left_info.ty == Type::Never || right_info.ty == Type::Never {
                    return ExprInfo {
                        ty: Type::Never,
                        errors,
                    };
                }
                if !left_info.ty.is_numeric() || !right_info.ty.is_numeric() {
                    if !left_info.ty.is_opaque() && !right_info.ty.is_opaque() {
                        self.error(
                            INVALID_OPERANDS,
                            format!(
                                "Operator requires numeric operands, found '{}' and '{}'.",
                                left_info.ty, right_info.ty
                            ),
                            span,
                            "Convert values explicitly; Neo Astra has no implicit numeric coercion.",
                        );
                    }
                    return ExprInfo::value(Type::Error).with_errors(errors);
                }
                if !left_info.ty.accepts(&right_info.ty) && !left_info.ty.is_opaque() {
                    self.type_mismatch(&left_info.ty, &right_info.ty, span);
                }
                ExprInfo::value(left_info.ty).with_errors(errors)
            }
            ast::BinaryOp::BitAnd
            | ast::BinaryOp::BitOr
            | ast::BinaryOp::BitXor
            | ast::BinaryOp::Shl
            | ast::BinaryOp::Shr => {
                let integer_expected = expected.filter(|ty| ty.is_integer());
                let (left_info, right_info) =
                    if is_numeric_literal(left) && !is_numeric_literal(right) {
                        let right_info = self.check_expr(right, integer_expected);
                        let left_info = self.check_expr(left, Some(&right_info.ty));
                        (left_info, right_info)
                    } else {
                        let left_info = self.check_expr(left, integer_expected);
                        let right_info = self.check_expr(right, Some(&left_info.ty));
                        (left_info, right_info)
                    };
                let errors = merge_effects(&left_info.errors, &right_info.errors);
                if left_info.ty == Type::Never || right_info.ty == Type::Never {
                    return ExprInfo {
                        ty: Type::Never,
                        errors,
                    };
                }
                if !left_info.ty.is_integer() || !right_info.ty.is_integer() {
                    if !left_info.ty.is_opaque() && !right_info.ty.is_opaque() {
                        self.error(
                            INVALID_OPERANDS,
                            format!(
                                "Bitwise operator requires integer operands, found '{}' and '{}'.",
                                left_info.ty, right_info.ty
                            ),
                            span,
                            "Bitwise operators are only defined for integers.",
                        );
                    }
                    return ExprInfo::value(Type::Error).with_errors(errors);
                }
                if !left_info.ty.accepts(&right_info.ty) && !left_info.ty.is_opaque() {
                    self.type_mismatch(&left_info.ty, &right_info.ty, span);
                }
                ExprInfo::value(left_info.ty).with_errors(errors)
            }
            ast::BinaryOp::Equal | ast::BinaryOp::NotEqual => {
                let left_info = self.check_expr(left, None);
                let right_info = self.check_expr(right, Some(&left_info.ty));
                if !left_info.ty.accepts(&right_info.ty)
                    && !left_info.ty.is_opaque()
                    && right_info.ty != Type::Never
                {
                    self.type_mismatch(&left_info.ty, &right_info.ty, span);
                }
                ExprInfo::value(Type::Bool)
                    .with_errors(merge_effects(&left_info.errors, &right_info.errors))
            }
            ast::BinaryOp::Less
            | ast::BinaryOp::LessEqual
            | ast::BinaryOp::Greater
            | ast::BinaryOp::GreaterEqual => {
                let left_info = self.check_expr(left, None);
                let right_info = self.check_expr(right, Some(&left_info.ty));
                let comparable = left_info.ty.is_numeric()
                    || matches!(left_info.ty, Type::Str | Type::Bytes)
                    || left_info.ty.is_opaque()
                    || left_info.ty == Type::Never;
                if !comparable {
                    self.error(
                        INVALID_OPERANDS,
                        format!("Type '{}' is not ordered.", left_info.ty),
                        span,
                        "Use numeric values, str, or bytes for ordered comparison.",
                    );
                } else if !left_info.ty.accepts(&right_info.ty) && !left_info.ty.is_opaque() {
                    self.type_mismatch(&left_info.ty, &right_info.ty, span);
                }
                ExprInfo::value(Type::Bool)
                    .with_errors(merge_effects(&left_info.errors, &right_info.errors))
            }
        }
    }

    fn check_assignment(&mut self, target: &ast::Expr, value: &ast::Expr, span: Span) -> ExprInfo {
        let target_info = self.check_lvalue(target);
        if !target_info.mutable && !target_info.ty.is_opaque() {
            self.error(
                NOT_ASSIGNABLE,
                "Assignment target is not mutable.",
                target.span,
                "Declare the binding with 'let mut'.",
            );
        }
        let value_info = self.check_expr(value, Some(&target_info.ty));
        let errors = merge_effects(&target_info.errors, &value_info.errors);
        if !target_info.ty.accepts(&value_info.ty) && !target_info.ty.is_opaque() {
            self.type_mismatch(&target_info.ty, &value_info.ty, value.span);
        }
        let _ = span;
        ExprInfo::value(Type::Unit).with_errors(errors)
    }

    fn check_lvalue(&mut self, expression: &ast::Expr) -> LValueInfo {
        match &expression.kind {
            ast::ExprKind::Identifier(name) => {
                if let Some(local) = self.lookup_local(name) {
                    let ty = local.ty.clone();
                    let mutable = local.mutable;
                    self.annotate(expression.span, ty.clone());
                    return LValueInfo {
                        ty,
                        mutable,
                        errors: Vec::new(),
                    };
                }
                if let Some(value) = self.values.get(name) {
                    let ty = value.ty.clone();
                    self.annotate(expression.span, ty.clone());
                    return LValueInfo {
                        ty,
                        mutable: false,
                        errors: Vec::new(),
                    };
                }
                self.error(
                    UNKNOWN_IDENTIFIER,
                    format!("Unknown assignment target '{name}'."),
                    expression.span,
                    "Declare the binding before assigning to it.",
                );
                LValueInfo {
                    ty: Type::Error,
                    mutable: false,
                    errors: Vec::new(),
                }
            }
            ast::ExprKind::Member { target, member } => {
                let base = self.check_lvalue(target);
                let ty = self.member_type(&base.ty, member, expression.span, None);
                self.annotate(expression.span, ty.clone());
                LValueInfo {
                    ty,
                    mutable: base.mutable,
                    errors: base.errors,
                }
            }
            ast::ExprKind::Index { target, index } => {
                let base = self.check_lvalue(target);
                let expected_index = expected_index_type(&base.ty);
                let index_info = self.check_expr(index, expected_index.as_ref());
                let tuple_index = integer_literal_value(index);
                let ty = self.index_type(&base.ty, &index_info.ty, tuple_index, expression.span);
                self.annotate(expression.span, ty.clone());
                LValueInfo {
                    ty,
                    mutable: base.mutable,
                    errors: merge_effects(&base.errors, &index_info.errors),
                }
            }
            _ => {
                self.error(
                    NOT_ASSIGNABLE,
                    "Expression is not a valid assignment target.",
                    expression.span,
                    "Assign to a mutable binding, field, or index.",
                );
                LValueInfo {
                    ty: Type::Error,
                    mutable: false,
                    errors: Vec::new(),
                }
            }
        }
    }

    fn check_call(
        &mut self,
        callee: &ast::Expr,
        arguments: &[ast::Expr],
        span: Span,
        expected: Option<&Type>,
    ) -> ExprInfo {
        if let Some(constructor) = builtin_constructor_name(callee) {
            return self.check_builtin_constructor(constructor, arguments, span, expected);
        }

        let (callee_info, explicit_arguments) = self.check_callee(callee);
        let mut errors = callee_info.errors.clone();
        let function = match callee_info.ty {
            Type::Function(function) => function,
            Type::Unknown => {
                for argument in arguments {
                    let info = self.check_expr(argument, None);
                    append_effects(&mut errors, info.errors);
                }
                return ExprInfo {
                    ty: Type::Unknown,
                    errors,
                };
            }
            Type::Error => {
                for argument in arguments {
                    let info = self.check_expr(argument, None);
                    append_effects(&mut errors, info.errors);
                }
                return ExprInfo {
                    ty: Type::Error,
                    errors,
                };
            }
            other => {
                self.error(
                    NOT_CALLABLE,
                    format!("Type '{other}' is not callable."),
                    span,
                    "Call a function value or a constructor.",
                );
                for argument in arguments {
                    let info = self.check_expr(argument, None);
                    append_effects(&mut errors, info.errors);
                }
                return ExprInfo {
                    ty: Type::Error,
                    errors,
                };
            }
        };

        if function.parameters.len() != arguments.len() {
            let mut diagnostic = Diagnostic::error(
                ARGUMENT_COUNT,
                format!(
                    "Function expects {} argument(s), but {} were provided.",
                    function.parameters.len(),
                    arguments.len()
                ),
            )
            .with_span(span)
            .with_hint("Match the declared function signature.");
            if let Some(name) = callee_identifier(callee)
                && let Some(declaration) = self.values.get(&name).and_then(|value| value.span)
            {
                diagnostic =
                    diagnostic.with_related(String::new(), declaration, "Function defined here.");
            }
            self.diagnostics.push(diagnostic);
        }

        let mut substitutions = BTreeMap::new();
        if let Some(arguments) = &explicit_arguments {
            if arguments.len() != function.type_parameters.len() {
                self.error(
                    INVALID_TYPE_ARGUMENT,
                    format!(
                        "Function expects {} type argument(s), but {} were provided.",
                        function.type_parameters.len(),
                        arguments.len()
                    ),
                    span,
                    "Match the declared type parameter list.",
                );
            }
            for (parameter, argument) in function.type_parameters.iter().zip(arguments) {
                substitutions.insert(parameter.clone(), argument.clone());
            }
        }

        let mut argument_infos = Vec::with_capacity(arguments.len());
        for (index, argument) in arguments.iter().enumerate() {
            let expected_argument = function.parameters.get(index).map(|parameter| {
                if explicit_arguments.is_none()
                    && !function.type_parameters.is_empty()
                    && contains_generic(parameter)
                {
                    None
                } else {
                    Some(parameter.substitute(&substitutions))
                }
            });
            let expected_argument = expected_argument.flatten();
            // Check the argument against the parameter type without reporting
            // the mismatch here; the explicit parameter loop below reports it
            // once, with the callee declaration as a related label.
            let info = self.check_expr_inner(argument, expected_argument.as_ref());
            self.annotate(argument.span, info.ty.clone());
            append_effects(&mut errors, info.errors.clone());
            argument_infos.push(info);
        }

        if explicit_arguments.is_none() && !function.type_parameters.is_empty() {
            for (parameter, argument) in function.parameters.iter().zip(&argument_infos) {
                infer_substitutions(parameter, &argument.ty, &mut substitutions);
            }
            if let Some(expected) = expected {
                infer_substitutions(&function.return_type, expected, &mut substitutions);
            }
            for parameter in &function.type_parameters {
                if !substitutions.contains_key(parameter) {
                    self.error(
                        INVALID_TYPE_ARGUMENT,
                        format!("Cannot infer type parameter '{parameter}'."),
                        span,
                        "Add explicit type arguments at the call site.",
                    );
                    substitutions.insert(parameter.clone(), Type::Unknown);
                }
            }
        }

        for (index, (parameter, argument)) in
            function.parameters.iter().zip(&argument_infos).enumerate()
        {
            if index >= arguments.len() {
                break;
            }
            let parameter = parameter.substitute(&substitutions);
            if !parameter.accepts(&argument.ty) && !parameter.is_opaque() {
                let related = callee_identifier(callee)
                    .and_then(|name| self.values.get(&name).and_then(|value| value.span))
                    .map(|declaration| (declaration, "Function defined here."));
                self.type_mismatch_with_related(
                    &parameter,
                    &argument.ty,
                    arguments[index].span,
                    related,
                );
            }
        }

        // Hybrid NS6002 micro-pass: record this generic call site so the
        // deferred alloc checks of the callee can be re-run with the concrete
        // substitution after all function bodies are checked.
        if !function.type_parameters.is_empty() {
            let callee_name = match &callee.kind {
                ast::ExprKind::Identifier(name) => Some(name.clone()),
                ast::ExprKind::TypeArguments { target, .. } => match &target.kind {
                    ast::ExprKind::Identifier(name) => Some(name.clone()),
                    _ => None,
                },
                _ => None,
            };
            if let Some(name) = callee_name {
                let enclosing = self
                    .current_function
                    .as_ref()
                    .map(|f| f.name.clone())
                    .unwrap_or_default();
                self.pending_generic_call_checks.push((
                    span,
                    name,
                    enclosing,
                    substitutions.clone(),
                ));
            }
        }

        let return_type = function.return_type.substitute(&substitutions);
        let function_errors = function
            .errors
            .iter()
            .map(|error| ErrorEffect::new(error.substitute(&substitutions), span))
            .collect::<Vec<_>>();
        append_effects(&mut errors, function_errors);

        // NS6002: once T is bound (explicitly or by inference), manual
        // allocation must not carry GC-managed references into `Ptr<T>`.
        self.check_manual_alloc_element(callee, &return_type, span);
        self.check_channel_element(callee, &return_type, span);

        ExprInfo {
            ty: return_type,
            errors,
        }
    }

    fn check_callee(&mut self, callee: &ast::Expr) -> (ExprInfo, Option<Vec<Type>>) {
        if let ast::ExprKind::TypeArguments { target, arguments } = &callee.kind {
            let generics = generic_map(&self.current_type_parameters);
            let explicit = arguments
                .iter()
                .map(|argument| self.resolve_type(argument, &generics))
                .collect::<Vec<_>>();
            let info = self.check_expr(target, None);
            return (info, Some(explicit));
        }

        (self.check_expr(callee, None), None)
    }

    /// A range is a typed iterable boundary pair. It is currently consumed by
    /// `for` loops; index-position ranges are lowered to `.slice(...)` by the
    /// parser before they reach the checker.
    fn check_range(
        &mut self,
        start: &ast::Expr,
        end: &ast::Expr,
        expected: Option<&Type>,
    ) -> ExprInfo {
        let start_info = self.check_expr(start, expected);
        let end_info = self.check_expr(end, Some(&start_info.ty));
        let element = if start_info.ty.accepts(&end_info.ty) {
            start_info.ty.clone()
        } else if end_info.ty.accepts(&start_info.ty) {
            end_info.ty.clone()
        } else {
            self.type_mismatch(&start_info.ty, &end_info.ty, end.span);
            Type::Error
        };
        if !element.is_integer() && !matches!(element, Type::Error | Type::Unknown) {
            self.error(
                INVALID_OPERANDS,
                format!("Range bounds must be integers, found '{element}'."),
                end.span,
                "Use integer bounds such as 0..10 or 0u8..=8u8.",
            );
        }
        ExprInfo {
            ty: Type::Range(Box::new(element)),
            errors: merge_effects(&start_info.errors, &end_info.errors),
        }
    }

    fn check_index(&mut self, target: &ast::Expr, index: &ast::Expr, span: Span) -> ExprInfo {
        let target_info = self.check_expr(target, None);
        let expected_index = expected_index_type(&target_info.ty);
        let index_info = self.check_expr(index, expected_index.as_ref());
        let tuple_index = integer_literal_value(index);
        let ty = self.index_type(&target_info.ty, &index_info.ty, tuple_index, span);
        ExprInfo {
            ty,
            errors: merge_effects(&target_info.errors, &index_info.errors),
        }
    }

    fn index_type(
        &mut self,
        target: &Type,
        index: &Type,
        tuple_index: Option<usize>,
        span: Span,
    ) -> Type {
        match target {
            Type::Array(element) | Type::Slice(element) | Type::FixedArray(element, _) => {
                if !index.accepts(&Type::Usize) && !index.is_opaque() {
                    self.type_mismatch(&Type::Usize, index, span);
                }
                (**element).clone()
            }
            Type::Bytes => {
                if !index.accepts(&Type::Usize) && !index.is_opaque() {
                    self.type_mismatch(&Type::Usize, index, span);
                }
                Type::U8
            }
            Type::Map(key, value) => {
                if !key.accepts(index) && !key.is_opaque() {
                    self.type_mismatch(key, index, span);
                }
                (**value).clone()
            }
            Type::Tuple(elements) => {
                if let Some(index) = tuple_index
                    && let Some(element) = elements.get(index)
                {
                    return element.clone();
                }
                self.error(
                    INVALID_MEMBER,
                    "Tuple index is invalid.".to_owned(),
                    span,
                    format!("Use an integer index in 0..{}.", elements.len()),
                );
                Type::Error
            }
            Type::Unknown | Type::Error => Type::Unknown,
            _ => {
                self.error(
                    INVALID_MEMBER,
                    format!("Type '{target}' cannot be indexed."),
                    span,
                    "Use an array, slice, map, or tuple.",
                );
                Type::Error
            }
        }
    }

    fn check_member(
        &mut self,
        target: &ast::Expr,
        member: &str,
        span: Span,
        expected: Option<&Type>,
    ) -> ExprInfo {
        if let ast::ExprKind::Identifier(name) = &target.kind
            && self.lookup_local(name).is_none()
        {
            if let Some(enum_def) = self.resolve_enum(name) {
                return self.enum_member(name, &enum_def, member, span, expected);
            }
            if let Some(namespace) = self.namespaces.get(name).cloned() {
                if let Some(&script_mode) = self.script_modes.get(name)
                    && !self.mode.allows(script_mode)
                {
                    self.error(
                        MODE_REQUIREMENT,
                        format!(
                            "Script block '{name}' requires '{}' mode, but the active mode is '{}'.",
                            script_mode.as_str(),
                            self.mode.as_str()
                        ),
                        span,
                        format!(
                            "Move the call into a {}-mode function or raise the mode.",
                            script_mode.as_str()
                        ),
                    );
                }
                if let Some(value) = namespace.members.get(member) {
                    return ExprInfo::value(value.ty.clone());
                }
                if namespace.open {
                    return ExprInfo::value(Type::Unknown);
                }
                self.error(
                    INVALID_MEMBER,
                    format!("Foreign module '{name}' has no member '{member}'."),
                    span,
                    "Declare the function inside the foreign module block.",
                );
                return ExprInfo::value(Type::Error);
            }
            // Built-in namespace constructors: `Channel.create<T>()` and
            // `bytes.new(length)`. They are only recognized when the name is
            // not shadowed by a local, enum, namespace, or user type.
            if member == "create" && name == "Channel" && self.resolve_struct(name).is_none() {
                return ExprInfo::value(Type::Function(FunctionType {
                    type_parameters: vec!["T".to_owned()],
                    parameters: Vec::new(),
                    return_type: Box::new(Type::Named(
                        "Channel".to_owned(),
                        vec![Type::Generic("T".to_owned())],
                    )),
                    errors: Vec::new(),
                }));
            }
            if member == "new" && name == "bytes" && self.resolve_struct(name).is_none() {
                return ExprInfo::value(Type::Function(FunctionType {
                    type_parameters: Vec::new(),
                    parameters: vec![Type::Usize],
                    return_type: Box::new(Type::Bytes),
                    errors: Vec::new(),
                }));
            }
            // Manual-memory builtins: `Memory.alloc<T>(count)` and
            // `Memory.free(ptr)` (03-runtime-and-memory.md §5.2). Manual memory
            // is available only in a mode that grants system capability.
            if name == "Memory" && self.resolve_struct(name).is_none() {
                if !self.mode.allows(Mode::System) {
                    self.error(
                        MEMORY_REQUIREMENT,
                        format!(
                            "Manual memory requires system mode, but the active mode is '{}'.",
                            self.mode.as_str()
                        ),
                        span,
                        "Use 'Memory' only inside a system-mode boundary.",
                    );
                    return ExprInfo::value(Type::Error);
                }
                if member == "alloc" {
                    // Spec 8.4: "`Memory.alloc` in a GC-managed scope is a
                    // compile-time error." The mode gate above covers *which
                    // mode* grants manual memory; this covers *where* it may be
                    // used. Only `alloc` is named by the rule - `Memory.free`
                    // and `ptr.*` have no scope requirement in the spec.
                    if !self.in_manual_memory_scope(span) {
                        self.error(
                            MANUAL_ALLOC_IN_GC_SCOPE,
                            "Memory.alloc is only allowed inside an '@memory:manual' scope, \
                             but this expression is in a GC-managed scope.",
                            span,
                            "Wrap this statement in {@memory:manual; ...} inside a system-mode boundary.",
                        );
                        return ExprInfo::value(Type::Error);
                    }
                    // Mode and scope both hold: record the member span so the
                    // call site can apply NS6002 to the element type (spec 8.4:
                    // `Ptr<T>` must not hold GC-managed references).
                    self.manual_alloc_spans.push(span);
                    return ExprInfo::value(Type::Function(FunctionType {
                        type_parameters: vec!["T".to_owned()],
                        parameters: vec![Type::Usize],
                        return_type: Box::new(Type::Named(
                            "Ptr".to_owned(),
                            vec![Type::Generic("T".to_owned())],
                        )),
                        errors: Vec::new(),
                    }));
                }
                if member == "free" {
                    return ExprInfo::value(Type::Function(FunctionType {
                        type_parameters: vec!["T".to_owned()],
                        parameters: vec![Type::Named(
                            "Ptr".to_owned(),
                            vec![Type::Generic("T".to_owned())],
                        )],
                        return_type: Box::new(Type::Unit),
                        errors: Vec::new(),
                    }));
                }
            }
            // `Region.new(bytes)` (spec 8.4): a fixed-capacity manual block.
            // Like `Memory.*`, region use is manual memory and needs a mode
            // that grants system capability. The scope rule in spec 8.4 names
            // only `Memory.alloc`, so `Region.new` carries the mode gate
            // alone; whether it should also require an `@memory:manual` scope
            // is recorded as a design question in Docs/archive/Agent/todo.md.
            if name == "Region" && member == "new" && self.resolve_struct(name).is_none() {
                if !self.mode.allows(Mode::System) {
                    self.error(
                        MEMORY_REQUIREMENT,
                        format!(
                            "Manual memory requires system mode, but the active mode is '{}'.",
                            self.mode.as_str()
                        ),
                        span,
                        "Use 'Region' only inside a system-mode boundary.",
                    );
                    return ExprInfo::value(Type::Error);
                }
                return ExprInfo::value(Type::Function(FunctionType {
                    type_parameters: Vec::new(),
                    parameters: vec![Type::Usize],
                    return_type: Box::new(Type::Named("Region".to_owned(), Vec::new())),
                    errors: Vec::new(),
                }));
            }
        }

        let target_info = self.check_expr(target, None);
        // `region.alloc<T>` on any receiver of type Region records its span as
        // well, so the call site can apply NS6002 to the element type. If the
        // mode gate inside `member_type` rejects the call first, the callee
        // types as Error and `check_call` returns before NS6002 could fire -
        // one program, one violated condition.
        if member == "alloc" && matches!(&target_info.ty, Type::Named(name, _) if name == "Region")
        {
            self.manual_alloc_spans.push(span);
        }
        let ty = self.member_type(&target_info.ty, member, target.span, expected);
        ExprInfo {
            ty,
            errors: target_info.errors,
        }
    }

    fn enum_member(
        &mut self,
        enum_name: &str,
        enum_def: &ResolvedEnum,
        member: &str,
        span: Span,
        expected: Option<&Type>,
    ) -> ExprInfo {
        let Some(variant) = enum_def
            .variants
            .iter()
            .find(|variant| variant.name == member)
            .cloned()
        else {
            self.error(
                INVALID_MEMBER,
                format!("Enum '{enum_name}' has no variant '{member}'."),
                span,
                "Use a declared variant name.",
            );
            return ExprInfo::value(Type::Error);
        };

        let substitutions = generic_map(&enum_def.type_parameters);
        let generic_return = Type::Named(
            enum_name.to_owned(),
            enum_def
                .type_parameters
                .iter()
                .map(|parameter| Type::Generic(parameter.clone()))
                .collect(),
        );
        let concrete_return = expected
            .cloned()
            .filter(|ty| matches!(ty, Type::Named(name, _) if name == enum_name));

        match variant.fields {
            Some(fields) => {
                let parameters = fields
                    .iter()
                    .map(|field| field.ty.substitute(&substitutions))
                    .collect::<Vec<_>>();
                ExprInfo::value(Type::Function(FunctionType {
                    type_parameters: enum_def.type_parameters.clone(),
                    parameters,
                    return_type: Box::new(generic_return),
                    errors: Vec::new(),
                }))
            }
            None => ExprInfo::value(concrete_return.unwrap_or(generic_return)),
        }
    }

    fn member_type(
        &mut self,
        ty: &Type,
        member: &str,
        span: Span,
        expected: Option<&Type>,
    ) -> Type {
        match ty {
            Type::Named(name, arguments) => {
                if let Some(struct_def) = self.resolve_struct(name) {
                    let substitutions = substitutions(&struct_def.type_parameters, arguments);
                    if let Some(field) = struct_def
                        .fields
                        .iter()
                        .find(|field| field.name.as_deref() == Some(member))
                    {
                        return field.ty.substitute(&substitutions);
                    }
                    if let Some(method) = self.struct_method(name, member, ty, span) {
                        return method;
                    }
                    self.error(
                        INVALID_MEMBER,
                        format!("Struct '{name}' has no field or method '{member}'."),
                        span,
                        "Check the field name or the struct declaration.",
                    );
                    return Type::Error;
                }
                // Pointer access is manual memory, so it obeys the same
                // system-mode gate as `Memory.alloc`: what makes a file
                // `@requires(system)` is raw pointer *usage*, not only the
                // allocation call (06-tooling-and-workflow.md).
                if name == "Ptr"
                    && matches!(member, "read" | "write")
                    && !self.mode.allows(Mode::System)
                {
                    self.error(
                        MEMORY_REQUIREMENT,
                        format!(
                            "Manual memory requires system mode, but the active mode is '{}'.",
                            self.mode.as_str()
                        ),
                        span,
                        "Use pointer access only inside a system-mode boundary.",
                    );
                    return Type::Error;
                }
                // Region methods are manual memory too: `region.alloc` hands
                // out raw pointers and `region.free` releases manual blocks,
                // so both carry the same system-mode gate as `Memory.alloc`
                // and `ptr.*` (usage, not only allocation).
                if name == "Region"
                    && matches!(member, "alloc" | "free")
                    && !self.mode.allows(Mode::System)
                {
                    self.error(
                        MEMORY_REQUIREMENT,
                        format!(
                            "Manual memory requires system mode, but the active mode is '{}'.",
                            self.mode.as_str()
                        ),
                        span,
                        "Use region allocation only inside a system-mode boundary.",
                    );
                    return Type::Error;
                }
                if let Some(method) = builtin_method(ty, member) {
                    return method;
                }
                if ty.is_opaque() {
                    return Type::Unknown;
                }
                let _ = expected;
                self.error(
                    INVALID_MEMBER,
                    format!("Type '{ty}' has no member '{member}'."),
                    span,
                    "Check the member name.",
                );
                Type::Error
            }
            Type::Tuple(elements) => {
                if let Ok(index) = member.parse::<usize>()
                    && let Some(element) = elements.get(index)
                {
                    return element.clone();
                }
                self.error(
                    INVALID_MEMBER,
                    "Tuple index is invalid.".to_owned(),
                    span,
                    format!("Use an index in 0..{}.", elements.len()),
                );
                Type::Error
            }
            Type::Unknown | Type::Error => Type::Unknown,
            _ => {
                if let Some(method) = builtin_method(ty, member) {
                    return method;
                }
                self.error(
                    INVALID_MEMBER,
                    format!("Type '{ty}' has no member '{member}'."),
                    span,
                    "Check the member name or add a type annotation.",
                );
                Type::Error
            }
        }
    }

    fn struct_method(
        &mut self,
        _name: &str,
        _member: &str,
        _ty: &Type,
        _span: Span,
    ) -> Option<Type> {
        None
    }

    fn check_builtin_constructor(
        &mut self,
        constructor: &str,
        arguments: &[ast::Expr],
        span: Span,
        expected: Option<&Type>,
    ) -> ExprInfo {
        match constructor {
            "Some" => {
                let argument_expected = match expected {
                    Some(Type::Option(inner)) => Some(&**inner),
                    _ => None,
                };
                if arguments.len() != 1 {
                    self.error(
                        ARGUMENT_COUNT,
                        "Some expects exactly one argument.",
                        span,
                        "Use Some(value).",
                    );
                }
                let mut errors = Vec::new();
                let mut value_type = Type::Never;
                for (index, argument) in arguments.iter().enumerate() {
                    let expected = if index == 0 { argument_expected } else { None };
                    let info = self.check_expr(argument, expected);
                    if index == 0 {
                        value_type = info.ty;
                    }
                    append_effects(&mut errors, info.errors);
                }
                ExprInfo {
                    ty: Type::Option(Box::new(value_type)),
                    errors,
                }
            }
            "Ok" | "Err" => {
                let (ok_expected, err_expected) = match expected {
                    Some(Type::Result(ok, error)) => (Some(&**ok), Some(&**error)),
                    _ => (None, None),
                };
                if arguments.len() != 1 {
                    self.error(
                        ARGUMENT_COUNT,
                        format!("{constructor} expects exactly one argument."),
                        span,
                        format!("Use {constructor}(value)."),
                    );
                }
                let expected_argument = if constructor == "Ok" {
                    ok_expected
                } else {
                    err_expected
                };
                let mut errors = Vec::new();
                let mut value_type = Type::Never;
                for (index, argument) in arguments.iter().enumerate() {
                    let info = self
                        .check_expr(argument, if index == 0 { expected_argument } else { None });
                    if index == 0 {
                        value_type = info.ty;
                    }
                    append_effects(&mut errors, info.errors);
                }
                let ty = if constructor == "Ok" {
                    Type::Result(Box::new(value_type), Box::new(Type::Never))
                } else {
                    Type::Result(Box::new(Type::Never), Box::new(value_type))
                };
                ExprInfo { ty, errors }
            }
            _ => {
                self.error(
                    NOT_CALLABLE,
                    format!("Unknown built-in constructor '{constructor}'."),
                    span,
                    "Use Some, Ok, or Err.",
                );
                ExprInfo::value(Type::Error)
            }
        }
    }

    fn check_propagation(&mut self, inner: &ast::Expr, span: Span) -> ExprInfo {
        let info = self.check_expr(inner, None);
        if !info.errors.is_empty() {
            for error in &info.errors {
                self.record_error(error);
            }
            return ExprInfo::value(info.ty);
        }

        if let Type::Result(ok, error) = info.ty {
            self.record_error(&ErrorEffect::new((*error).clone(), span));
            return ExprInfo::value((*ok).clone());
        }

        if !info.ty.is_opaque() {
            self.error(
                INVALID_PROPAGATION,
                format!("Expression of type '{}' is not fallible.", info.ty),
                span,
                if self.options.edition == Edition::V0_2 {
                    "Use '?' only on fallible calls or Result values."
                } else {
                    "Use '!' only on fallible calls or Result values."
                },
            );
        }
        ExprInfo::value(info.ty)
    }

    fn check_array(
        &mut self,
        elements: &[ast::Expr],
        span: Span,
        expected: Option<&Type>,
    ) -> ExprInfo {
        let (element_expected, fixed_length) = match expected {
            Some(Type::Array(inner)) | Some(Type::Slice(inner)) => (Some(&**inner), None),
            Some(Type::FixedArray(inner, length)) => (Some(&**inner), Some(*length)),
            _ => (None, None),
        };

        if let Some(length) = fixed_length
            && length != elements.len()
        {
            self.error(
                TYPE_MISMATCH,
                format!(
                    "Fixed array expects {length} element(s), but {} were provided.",
                    elements.len()
                ),
                span,
                "Match the declared fixed-array length.",
            );
        }

        let mut element_type = element_expected.cloned().unwrap_or(Type::Never);
        let mut errors = Vec::new();
        for element in elements {
            let expected_element = if element_type == Type::Never {
                element_expected
            } else {
                Some(&element_type)
            };
            let info = self.check_expr(element, expected_element);
            append_effects(&mut errors, info.errors);
            if element_type == Type::Never {
                element_type = info.ty.clone();
            } else if !element_type.accepts(&info.ty) && !element_type.is_opaque() {
                self.type_mismatch(&element_type, &info.ty, element.span);
            }
        }

        let ty = match fixed_length {
            Some(length) => Type::FixedArray(Box::new(element_type), length),
            None => Type::Array(Box::new(element_type)),
        };
        ExprInfo { ty, errors }
    }

    fn check_map(
        &mut self,
        entries: &[ast::MapEntry],
        _span: Span,
        expected: Option<&Type>,
    ) -> ExprInfo {
        let (key_expected, value_expected) = match expected {
            Some(Type::Map(key, value)) => (Some(&**key), Some(&**value)),
            _ => (None, None),
        };

        let mut key_type = key_expected.cloned().unwrap_or(Type::Never);
        let mut value_type = value_expected.cloned().unwrap_or(Type::Never);
        let mut errors = Vec::new();

        for entry in entries {
            let key_expected_item = if key_type == Type::Never {
                key_expected
            } else {
                Some(&key_type)
            };
            let value_expected_item = if value_type == Type::Never {
                value_expected
            } else {
                Some(&value_type)
            };
            let key = self.check_expr(&entry.key, key_expected_item);
            let value = self.check_expr(&entry.value, value_expected_item);
            append_effects(&mut errors, key.errors);
            append_effects(&mut errors, value.errors);
            if key_type == Type::Never {
                key_type = key.ty.clone();
            } else if !key_type.accepts(&key.ty) && !key_type.is_opaque() {
                self.type_mismatch(&key_type, &key.ty, entry.key.span);
            }
            if value_type == Type::Never {
                value_type = value.ty.clone();
            } else if !value_type.accepts(&value.ty) && !value_type.is_opaque() {
                self.type_mismatch(&value_type, &value.ty, entry.value.span);
            }
        }

        ExprInfo {
            ty: Type::Map(Box::new(key_type), Box::new(value_type)),
            errors,
        }
    }

    fn check_tuple(
        &mut self,
        elements: &[ast::Expr],
        _span: Span,
        expected: Option<&Type>,
    ) -> ExprInfo {
        let expected_elements = match expected {
            Some(Type::Tuple(elements)) => Some(elements.as_slice()),
            _ => None,
        };
        if let Some(expected_elements) = expected_elements
            && expected_elements.len() != elements.len()
        {
            self.error(
                TYPE_MISMATCH,
                format!(
                    "Tuple expects {} element(s), but {} were provided.",
                    expected_elements.len(),
                    elements.len()
                ),
                _span,
                "Match the declared tuple type.",
            );
        }

        let mut types = Vec::with_capacity(elements.len());
        let mut errors = Vec::new();
        for (index, element) in elements.iter().enumerate() {
            let expected_element = expected_elements.and_then(|elements| elements.get(index));
            let info = self.check_expr(element, expected_element);
            append_effects(&mut errors, info.errors);
            types.push(info.ty);
        }

        ExprInfo {
            ty: Type::Tuple(types),
            errors,
        }
    }

    fn check_struct_init(
        &mut self,
        path: &ast::TypePath,
        fields: &[ast::FieldInit],
        span: Span,
    ) -> ExprInfo {
        let arguments = path
            .arguments
            .iter()
            .map(|argument| self.resolve_type(argument, &BTreeMap::new()))
            .collect::<Vec<_>>();
        let name = path
            .segments
            .last()
            .cloned()
            .unwrap_or_else(|| path.segments.join("."));

        let Some(struct_def) = self.resolve_struct(&name) else {
            if self.imported_types.contains(&name) {
                let mut errors = Vec::new();
                for field in fields {
                    let info = self.check_expr(&field.value, None);
                    append_effects(&mut errors, info.errors);
                }
                return ExprInfo {
                    ty: Type::Unknown,
                    errors,
                };
            }
            self.error(
                UNKNOWN_TYPE,
                format!("Unknown struct type '{name}'."),
                span,
                "Declare the struct before constructing it.",
            );
            return ExprInfo::value(Type::Error);
        };

        // With no explicit type arguments, infer them from field values like
        // function calls do. This makes `Wrapper { value: 9 }` valid without
        // spelling `::<i32>`.
        let infer = arguments.is_empty() && !struct_def.type_parameters.is_empty();
        let mut inferred = BTreeMap::new();
        if infer {
            let generics = generic_map(&struct_def.type_parameters);
            for field in fields {
                let Some(declared) = struct_def
                    .fields
                    .iter()
                    .find(|candidate| candidate.name.as_deref() == Some(field.name.as_str()))
                else {
                    continue;
                };
                let generic_ty = declared.ty.substitute(&generics);
                let info = self.check_expr(&field.value, None);
                infer_substitutions(&generic_ty, &info.ty, &mut inferred);
            }
            for parameter in &struct_def.type_parameters {
                if !inferred.contains_key(parameter) {
                    self.error(
                        INVALID_TYPE_ARGUMENT,
                        format!("Cannot infer type parameter '{parameter}' for '{name}'."),
                        span,
                        "Add explicit type arguments after the struct name.",
                    );
                    inferred.insert(parameter.clone(), Type::Unknown);
                }
            }
        } else if !check_arity(&struct_def.type_parameters, &arguments, &name, span, self) {
            return ExprInfo::value(Type::Error);
        }

        let substitutions = if infer {
            inferred
        } else {
            substitutions(&struct_def.type_parameters, &arguments)
        };
        let arguments = if infer {
            struct_def
                .type_parameters
                .iter()
                .map(|parameter| {
                    substitutions
                        .get(parameter)
                        .cloned()
                        .unwrap_or(Type::Unknown)
                })
                .collect::<Vec<_>>()
        } else {
            arguments
        };
        let mut errors = Vec::new();
        let mut seen = BTreeSet::new();

        for field in fields {
            if !seen.insert(field.name.clone()) {
                self.error(
                    INVALID_STRUCT_FIELD,
                    format!("Field '{}' is initialized more than once.", field.name),
                    field.span,
                    "Remove the duplicate initializer.",
                );
            }
            let declared = struct_def
                .fields
                .iter()
                .find(|candidate| candidate.name.as_deref() == Some(field.name.as_str()));
            match declared {
                Some(declared) => {
                    if !infer {
                        let expected = declared.ty.substitute(&substitutions);
                        let info = self.check_expr(&field.value, Some(&expected));
                        append_effects(&mut errors, info.errors);
                    }
                }
                None => {
                    self.error(
                        INVALID_STRUCT_FIELD,
                        format!("Struct '{name}' has no field '{}'.", field.name),
                        field.span,
                        "Use a declared field name.",
                    );
                    let info = self.check_expr(&field.value, None);
                    append_effects(&mut errors, info.errors);
                }
            }
        }

        for field in &struct_def.fields {
            if let Some(field_name) = &field.name
                && !seen.contains(field_name)
            {
                self.error(
                    INVALID_STRUCT_FIELD,
                    format!("Field '{field_name}' is missing from the initializer."),
                    span,
                    "Initialize every field of the struct.",
                );
            }
        }

        ExprInfo {
            ty: Type::Named(name, arguments),
            errors,
        }
    }

    fn check_block_expression(&mut self, block: &ast::Block, expected: Option<&Type>) -> ExprInfo {
        let info = self.check_block(block, true, expected);
        let (ty, errors) = if info.diverges {
            (Type::Never, Vec::new())
        } else {
            match info.tail {
                Some(tail) => (tail.ty, tail.errors),
                None => (Type::Unit, Vec::new()),
            }
        };
        ExprInfo { ty, errors }
    }

    fn check_if(
        &mut self,
        condition: &ast::Expr,
        then_branch: &ast::Block,
        else_branch: Option<&ast::ElseBranch>,
        span: Span,
        expected: Option<&Type>,
    ) -> ExprInfo {
        let condition_info = self.check_expr(condition, Some(&Type::Bool));
        let then_info = self.check_block(then_branch, true, expected);
        let then_type = block_value_type(&then_info);
        let then_errors = block_tail_errors(&then_info);

        let (else_type, else_errors) = match else_branch {
            Some(ast::ElseBranch::Block(block)) => {
                let info = self.check_block(block, true, expected.or(Some(&then_type)));
                (block_value_type(&info), block_tail_errors(&info))
            }
            Some(ast::ElseBranch::If(expression)) => {
                let info = self.check_expr(expression, expected.or(Some(&then_type)));
                (info.ty, info.errors)
            }
            None => (Type::Unit, Vec::new()),
        };

        let ty = if then_type == Type::Never && else_type == Type::Never {
            Type::Never
        } else if then_type.accepts(&else_type) {
            then_type
        } else if else_type.accepts(&then_type) {
            else_type
        } else {
            self.type_mismatch(&then_type, &else_type, span);
            Type::Error
        };

        ExprInfo {
            ty,
            errors: merge_effects(
                &merge_effects(&condition_info.errors, &then_errors),
                &else_errors,
            ),
        }
    }

    fn check_match(
        &mut self,
        value: &ast::Expr,
        arms: &[ast::MatchArm],
        span: Span,
        expected: Option<&Type>,
    ) -> ExprInfo {
        let scrutinee = self.check_expr(value, None);
        let effects = scrutinee.errors.clone();
        let effect_types = effects
            .iter()
            .map(|effect| effect.ty.clone())
            .collect::<Vec<_>>();
        let shape = self.match_shape(&scrutinee.ty, &effect_types);

        let mut coverage = Coverage::default();
        // Opaque expectations (notably `println`'s `Unknown` parameter) must not
        // become the match result type; infer from the arms instead.
        let mut result_type: Option<Type> = expected.filter(|ty| !ty.is_opaque()).cloned();
        let mut diverges = !arms.is_empty();
        let mut errors = Vec::new();

        for arm in arms {
            self.push_scope();
            self.check_pattern(&arm.pattern, &scrutinee.ty, &effect_types, &mut coverage);
            let expected_arm = result_type.as_ref();
            let (arm_type, arm_errors) = match &arm.body {
                ast::MatchArmBody::Expr(expression) => {
                    let info = self.check_expr(expression, expected_arm);
                    (info.ty, info.errors)
                }
                ast::MatchArmBody::Block(block) => {
                    let info = self.check_block(block, true, expected_arm);
                    (block_value_type(&info), block_tail_errors(&info))
                }
            };
            self.pop_scope();
            append_effects(&mut errors, arm_errors);

            if arm_type != Type::Never {
                diverges = false;
                if let Some(result_type) = &mut result_type {
                    if !result_type.accepts(&arm_type) && !result_type.is_opaque() {
                        self.type_mismatch(result_type, &arm_type, arm.span);
                    }
                } else {
                    result_type = Some(arm_type);
                }
            }
        }

        let scrutinee_type =
            if effect_types.is_empty() || matches!(scrutinee.ty, Type::Result(_, _)) {
                scrutinee.ty.clone()
            } else {
                let error = if effect_types.len() == 1 {
                    effect_types[0].clone()
                } else {
                    Type::Unknown
                };
                Type::Result(Box::new(scrutinee.ty.clone()), Box::new(error))
            };
        let top_level_catch_all = arms
            .iter()
            .any(|arm| pattern_is_catch_all(&arm.pattern, &scrutinee_type));
        let declared_members = error_members(&scrutinee.ty, &effect_types);
        if !coverage.err_members.is_empty()
            && !coverage.err_wildcard
            && !top_level_catch_all
            && !declared_members.is_empty()
        {
            let missing_members = declared_members
                .iter()
                .filter_map(type_name)
                .filter(|name| !coverage.err_members.contains(*name))
                .collect::<Vec<_>>();
            if !missing_members.is_empty() {
                let mut diagnostic = Diagnostic::error(
                    NON_EXHAUSTIVE_MATCH,
                    format!(
                        "Non-exhaustive match: missing Err member(s) {}.",
                        missing_members.join(", ")
                    ),
                )
                .with_span(span)
                .with_hint(
                    "Add an Err arm for every declared error member, or a final wildcard arm.",
                );
                if let Some(declaration) = self.enum_declaration_span(&scrutinee.ty) {
                    diagnostic =
                        diagnostic.with_related(String::new(), declaration, "Enum declared here.");
                }
                self.diagnostics.push(diagnostic);
            }
        }

        let patterns = arms.iter().map(|arm| &arm.pattern).collect::<Vec<_>>();
        if !self.patterns_are_exhaustive(&patterns, &scrutinee_type) {
            let missing = coverage.missing(&shape);
            let mut diagnostic = Diagnostic::error(
                NON_EXHAUSTIVE_MATCH,
                format!("Non-exhaustive match: missing {}.", missing.join(", ")),
            )
            .with_span(span)
            .with_hint("Add the missing case(s) or a wildcard arm.");
            if let Some(declaration) = self.enum_declaration_span(&scrutinee_type) {
                diagnostic =
                    diagnostic.with_related(String::new(), declaration, "Enum declared here.");
            }
            self.diagnostics.push(diagnostic);
        }

        let ty = if diverges {
            Type::Never
        } else {
            result_type.unwrap_or(Type::Unit)
        };
        ExprInfo { ty, errors }
    }

    fn patterns_are_exhaustive(&mut self, patterns: &[&ast::Pattern], ty: &Type) -> bool {
        let rows = patterns
            .iter()
            .map(|pattern| vec![RowPattern::Pattern(pattern)])
            .collect::<Vec<_>>();
        self.rows_are_exhaustive(&rows, std::slice::from_ref(ty))
    }

    fn rows_are_exhaustive(&mut self, rows: &[Vec<RowPattern<'_>>], types: &[Type]) -> bool {
        if types.is_empty() {
            return !rows.is_empty();
        }
        if rows.is_empty() {
            return false;
        }
        let ty = &types[0];
        if matches!(
            ty,
            Type::Never | Type::Unknown | Type::Error | Type::Generic(_)
        ) {
            return true;
        }

        // Bindings and wildcards are catch-all patterns for any type. Normalize
        // them before looking for constructors so nested bindings behave like
        // wildcards in the pattern matrix.
        let normalized = rows
            .iter()
            .map(|row| {
                let mut row = row.to_vec();
                if let Some(first) = row.first_mut()
                    && let RowPattern::Pattern(pattern) = first
                    && pattern_is_catch_all(pattern, ty)
                {
                    *first = RowPattern::Wildcard;
                }
                row
            })
            .collect::<Vec<_>>();
        let rows = &normalized;

        // A wildcard covers the entire column. Strip it without expanding nested
        // constructors so recursive types terminate.
        if rows
            .iter()
            .all(|row| matches!(row.first(), Some(RowPattern::Wildcard)))
        {
            let next = rows.iter().map(|row| row[1..].to_vec()).collect::<Vec<_>>();
            return self.rows_are_exhaustive(&next, &types[1..]);
        }

        let constructors = self.constructors_for(ty);
        let remaining_types = &types[1..];
        if constructors.is_empty() {
            // Values with no enumerable constructors, such as integers or
            // strings, are covered only by wildcards.
            let next = rows
                .iter()
                .filter(|row| matches!(row.first(), Some(RowPattern::Wildcard)))
                .map(|row| row[1..].to_vec())
                .collect::<Vec<_>>();
            return self.rows_are_exhaustive(&next, remaining_types);
        }

        for constructor in constructors {
            let mut next_rows = Vec::new();
            for row in rows {
                match row.first() {
                    Some(RowPattern::Wildcard) => {
                        let mut expanded = vec![RowPattern::Wildcard; constructor.fields.len()];
                        expanded.extend(row[1..].iter().copied());
                        next_rows.push(expanded);
                    }
                    Some(RowPattern::Pattern(pattern)) => {
                        if let Some(mut expanded) =
                            pattern_row(pattern, ty, &constructor.key, constructor.fields.len())
                        {
                            expanded.extend(row[1..].iter().copied());
                            next_rows.push(expanded);
                        }
                    }
                    None => {}
                }
            }

            let mut next_types = constructor.fields.clone();
            next_types.extend(remaining_types.iter().cloned());
            if !self.rows_are_exhaustive(&next_rows, &next_types) {
                return false;
            }
        }

        true
    }

    fn constructors_for(&mut self, ty: &Type) -> Vec<Constructor> {
        match ty {
            Type::Option(inner) => vec![
                Constructor {
                    key: ConstructorKey::Some,
                    fields: vec![(**inner).clone()],
                },
                Constructor {
                    key: ConstructorKey::None,
                    fields: Vec::new(),
                },
            ],
            Type::Result(ok, error) => vec![
                Constructor {
                    key: ConstructorKey::Ok,
                    fields: vec![(**ok).clone()],
                },
                Constructor {
                    key: ConstructorKey::Err,
                    fields: vec![(**error).clone()],
                },
            ],
            Type::Bool => vec![
                Constructor {
                    key: ConstructorKey::Bool(false),
                    fields: Vec::new(),
                },
                Constructor {
                    key: ConstructorKey::Bool(true),
                    fields: Vec::new(),
                },
            ],
            Type::Tuple(elements) => vec![Constructor {
                key: ConstructorKey::Tuple,
                fields: elements.clone(),
            }],
            Type::Unit => vec![Constructor {
                key: ConstructorKey::Unit,
                fields: Vec::new(),
            }],
            Type::Named(name, arguments) => {
                let Some(enum_def) = self.resolve_enum(name) else {
                    return Vec::new();
                };
                let substitutions = substitutions(&enum_def.type_parameters, arguments);
                enum_def
                    .variants
                    .iter()
                    .map(|variant| Constructor {
                        key: ConstructorKey::Variant(variant.name.clone()),
                        fields: variant
                            .fields
                            .as_ref()
                            .map(|fields| {
                                fields
                                    .iter()
                                    .map(|field| field.ty.substitute(&substitutions))
                                    .collect()
                            })
                            .unwrap_or_default(),
                    })
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    fn match_shape(&mut self, ty: &Type, effects: &[Type]) -> MatchShape {
        if !effects.is_empty() {
            return MatchShape::Result;
        }
        match ty {
            Type::Option(_) => MatchShape::Option,
            Type::Result(_, _) => MatchShape::Result,
            Type::Bool => MatchShape::Bool,
            Type::Tuple(_) => MatchShape::Tuple,
            Type::Named(name, _) => {
                if let Some(enum_def) = self.resolve_enum(name) {
                    MatchShape::Enum(enum_def)
                } else {
                    MatchShape::Other
                }
            }
            _ => MatchShape::Other,
        }
    }

    fn check_pattern(
        &mut self,
        pattern: &ast::Pattern,
        ty: &Type,
        effects: &[Type],
        coverage: &mut Coverage,
    ) {
        match &pattern.kind {
            ast::PatternKind::Wildcard => {}
            ast::PatternKind::Identifier(name) => {
                if name == "None" && matches!(ty, Type::Option(_)) {
                    coverage.none = true;
                    return;
                }
                self.insert_local(
                    name.clone(),
                    Local {
                        ty: ty.clone(),
                        mutable: false,
                        span: pattern.span,
                    },
                );
            }
            ast::PatternKind::Literal(literal) => {
                let literal_type = self.literal_type(literal, Some(ty), pattern.span);
                if !ty.accepts(&literal_type) && !ty.is_opaque() {
                    self.type_mismatch(ty, &literal_type, pattern.span);
                }
                if let ast::Literal::Bool(value) = literal {
                    coverage.boolean[usize::from(*value)] = true;
                }
            }
            ast::PatternKind::Tuple(patterns) => {
                let Type::Tuple(elements) = ty else {
                    if !ty.is_opaque() {
                        self.error(
                            INVALID_PATTERN,
                            format!("Tuple pattern cannot match type '{ty}'."),
                            pattern.span,
                            "Use a tuple pattern only for tuple values.",
                        );
                    }
                    return;
                };
                if patterns.len() != elements.len() {
                    self.error(
                        INVALID_PATTERN,
                        format!(
                            "Tuple pattern has {} element(s), but type '{ty}' has {}.",
                            patterns.len(),
                            elements.len()
                        ),
                        pattern.span,
                        "Match every tuple element.",
                    );
                }
                for (child, element) in patterns.iter().zip(elements) {
                    let mut nested = Coverage::default();
                    self.check_pattern(child, element, &[], &mut nested);
                }
            }
            ast::PatternKind::Constructor { path, arguments } => {
                let name = path
                    .segments
                    .last()
                    .cloned()
                    .unwrap_or_else(|| path.segments.join("."));
                match name.as_str() {
                    "Some" => {
                        coverage.some = true;
                        let Type::Option(inner) = ty else {
                            if !ty.is_opaque() {
                                self.error(
                                    INVALID_PATTERN,
                                    format!("Pattern 'Some' cannot match type '{ty}'."),
                                    pattern.span,
                                    "Use Some only for Option<T> values.",
                                );
                            }
                            return;
                        };
                        self.check_pattern_arguments(
                            arguments,
                            std::slice::from_ref(&**inner),
                            pattern.span,
                            coverage,
                        );
                    }
                    "Ok" => {
                        coverage.ok = true;
                        let (ok, _) = result_parts(ty, effects);
                        self.check_pattern_arguments(arguments, &[ok], pattern.span, coverage);
                    }
                    "Err" => {
                        coverage.err = true;
                        let members = error_members(ty, effects);
                        let union = if members.len() == 1 {
                            members[0].clone()
                        } else {
                            Type::Unknown
                        };

                        if arguments.len() != 1 {
                            self.check_pattern_arguments(
                                arguments,
                                &[union],
                                pattern.span,
                                coverage,
                            );
                            return;
                        }

                        match &arguments[0].kind {
                            ast::PatternKind::Wildcard => {
                                coverage.err_wildcard = true;
                            }
                            ast::PatternKind::Identifier(name) => {
                                coverage.err_wildcard = true;
                                self.insert_local(
                                    name.clone(),
                                    Local {
                                        ty: union,
                                        mutable: false,
                                        span: arguments[0].span,
                                    },
                                );
                            }
                            ast::PatternKind::Constructor {
                                path,
                                arguments: inner,
                            } => {
                                let member_name = path.segments.last().cloned().unwrap_or_default();
                                if let Some(member_type) = members
                                    .iter()
                                    .find(|candidate| {
                                        type_name(candidate) == Some(member_name.as_str())
                                    })
                                    .cloned()
                                {
                                    coverage.err_members.insert(member_name);
                                    self.check_error_member_constructor(
                                        &member_type,
                                        inner,
                                        arguments[0].span,
                                        coverage,
                                    );
                                } else {
                                    self.error(
                                        INVALID_PATTERN,
                                        format!(
                                            "Error member '{member_name}' is not declared by the matched expression."
                                        ),
                                        arguments[0].span,
                                        error_member_hint(&members),
                                    );
                                }
                            }
                            ast::PatternKind::Variant {
                                path,
                                variant,
                                arguments: inner,
                            } => {
                                let member_name = path.segments.last().cloned().unwrap_or_default();
                                if let Some(member_type) = members
                                    .iter()
                                    .find(|candidate| {
                                        type_name(candidate) == Some(member_name.as_str())
                                    })
                                    .cloned()
                                {
                                    coverage.err_members.insert(member_name);
                                    self.check_error_member_variant(
                                        &member_type,
                                        variant,
                                        inner.as_deref().unwrap_or(&[]),
                                        arguments[0].span,
                                        coverage,
                                    );
                                } else {
                                    self.error(
                                        INVALID_PATTERN,
                                        format!(
                                            "Error member '{member_name}' is not declared by the matched expression."
                                        ),
                                        arguments[0].span,
                                        error_member_hint(&members),
                                    );
                                }
                            }
                            _ => {
                                self.check_pattern(&arguments[0], &union, &[], coverage);
                            }
                        }
                    }
                    _ => {
                        if let Some(owner) = self.variant_owners.get(&name).cloned()
                            && let Some(enum_def) = self.resolve_enum(&owner)
                            && let Some(variant) = enum_def
                                .variants
                                .iter()
                                .find(|variant| variant.name == name)
                        {
                            coverage.variants.insert(name.clone());
                            let substitutions = match ty {
                                Type::Named(_, arguments) => {
                                    substitutions(&enum_def.type_parameters, arguments)
                                }
                                _ => BTreeMap::new(),
                            };
                            let fields = variant
                                .fields
                                .clone()
                                .unwrap_or_default()
                                .iter()
                                .map(|field| field.ty.substitute(&substitutions))
                                .collect::<Vec<_>>();
                            self.check_pattern_arguments(
                                arguments,
                                &fields,
                                pattern.span,
                                coverage,
                            );
                            return;
                        }
                        self.error(
                            INVALID_PATTERN,
                            format!("Unknown constructor pattern '{name}'."),
                            pattern.span,
                            "Use Some, Ok, Err, or a declared enum variant.",
                        );
                    }
                }
            }
            ast::PatternKind::Variant {
                path,
                variant,
                arguments,
            } => {
                let enum_name = path
                    .segments
                    .last()
                    .cloned()
                    .unwrap_or_else(|| path.segments.join("."));
                if enum_name == "Option" || enum_name == "Result" {
                    let mut constructor_path = path.clone();
                    constructor_path.segments = vec![variant.clone()];
                    let constructor = ast::Pattern {
                        kind: ast::PatternKind::Constructor {
                            path: constructor_path,
                            arguments: arguments.clone().unwrap_or_default(),
                        },
                        span: pattern.span,
                    };
                    self.check_pattern(&constructor, ty, effects, coverage);
                    return;
                }

                let Some(enum_def) = self.resolve_enum(&enum_name) else {
                    self.error(
                        UNKNOWN_TYPE,
                        format!("Unknown enum type '{enum_name}'."),
                        pattern.span,
                        "Declare the enum before matching on it.",
                    );
                    return;
                };
                let Some(variant_def) = enum_def
                    .variants
                    .iter()
                    .find(|candidate| candidate.name == *variant)
                    .cloned()
                else {
                    self.error(
                        INVALID_PATTERN,
                        format!("Enum '{enum_name}' has no variant '{variant}'."),
                        pattern.span,
                        "Use a declared variant name.",
                    );
                    return;
                };
                coverage.variants.insert(variant.clone());

                let Type::Named(actual_name, actual_arguments) = ty else {
                    if !ty.is_opaque() {
                        self.error(
                            INVALID_PATTERN,
                            format!("Variant pattern cannot match type '{ty}'."),
                            pattern.span,
                            "Use a variant pattern only for enum values.",
                        );
                    }
                    return;
                };
                if actual_name != &enum_name {
                    self.type_mismatch(ty, &Type::Named(enum_name, Vec::new()), pattern.span);
                    return;
                }
                let substitutions = substitutions(&enum_def.type_parameters, actual_arguments);
                let field_types = variant_def
                    .fields
                    .unwrap_or_default()
                    .iter()
                    .map(|field| field.ty.substitute(&substitutions))
                    .collect::<Vec<_>>();
                self.check_pattern_arguments(
                    arguments.as_deref().unwrap_or(&[]),
                    &field_types,
                    pattern.span,
                    coverage,
                );
            }
        }
    }

    fn check_pattern_arguments(
        &mut self,
        patterns: &[ast::Pattern],
        types: &[Type],
        span: Span,
        _coverage: &mut Coverage,
    ) {
        if patterns.len() != types.len() {
            self.error(
                INVALID_PATTERN,
                format!(
                    "Pattern binds {} value(s), but the constructor has {} field(s).",
                    patterns.len(),
                    types.len()
                ),
                span,
                "Match every constructor field.",
            );
        }
        for (pattern, ty) in patterns.iter().zip(types) {
            let mut nested = Coverage::default();
            self.check_pattern(pattern, ty, &[], &mut nested);
        }
    }

    fn check_error_member_constructor(
        &mut self,
        member_type: &Type,
        patterns: &[ast::Pattern],
        span: Span,
        coverage: &mut Coverage,
    ) {
        let Some(name) = type_name(member_type).map(str::to_owned) else {
            self.error(
                INVALID_PATTERN,
                format!("Error member '{member_type}' is not nominal."),
                span,
                "Use a declared struct or enum error type.",
            );
            return;
        };

        if let Some(struct_def) = self.resolve_struct(&name) {
            let fields = struct_def
                .fields
                .iter()
                .map(|field| field.ty.clone())
                .collect::<Vec<_>>();
            self.check_pattern_arguments(patterns, &fields, span, coverage);
            return;
        }

        if let Some(enum_def) = self.resolve_enum(&name) {
            if enum_def.variants.len() == 1 {
                let fields = enum_def.variants[0]
                    .fields
                    .clone()
                    .unwrap_or_default()
                    .iter()
                    .map(|field| field.ty.clone())
                    .collect::<Vec<_>>();
                self.check_pattern_arguments(patterns, &fields, span, coverage);
            } else {
                self.error(
                    INVALID_PATTERN,
                    format!("Error member '{name}' is an enum; select a variant."),
                    span,
                    "Write Err(Enum.Variant(...)).",
                );
            }
            return;
        }

        self.error(
            INVALID_PATTERN,
            format!("Unknown error member type '{name}'."),
            span,
            "Declare the error type before matching it.",
        );
    }

    fn check_error_member_variant(
        &mut self,
        member_type: &Type,
        variant: &str,
        patterns: &[ast::Pattern],
        span: Span,
        coverage: &mut Coverage,
    ) {
        let Some(name) = type_name(member_type).map(str::to_owned) else {
            self.error(
                INVALID_PATTERN,
                format!("Error member '{member_type}' is not nominal."),
                span,
                "Use a declared enum error type.",
            );
            return;
        };
        let Some(enum_def) = self.resolve_enum(&name) else {
            self.error(
                INVALID_PATTERN,
                format!("Error member '{name}' is not an enum."),
                span,
                "Use a variant pattern only for enum error members.",
            );
            return;
        };
        let Some(definition) = enum_def
            .variants
            .iter()
            .find(|candidate| candidate.name == variant)
        else {
            self.error(
                INVALID_PATTERN,
                format!("Enum '{name}' has no variant '{variant}'."),
                span,
                "Use a declared variant name.",
            );
            return;
        };
        let fields = definition
            .fields
            .clone()
            .unwrap_or_default()
            .iter()
            .map(|field| field.ty.clone())
            .collect::<Vec<_>>();
        self.check_pattern_arguments(patterns, &fields, span, coverage);
    }

    fn check_fail(
        &mut self,
        error: &ast::TypePath,
        arguments: &ast::FailArguments,
        span: Span,
    ) -> ExprInfo {
        let error_type = self.resolve_type_path(error, &BTreeMap::new());
        if !matches!(error_type, Type::Named(_, _) | Type::Unknown | Type::Error) {
            self.error(
                UNDECLARED_ERROR,
                format!("Failure type '{error_type}' is not nominal."),
                span,
                "Use a struct or enum error type.",
            );
        }
        self.record_error(&ErrorEffect::new(error_type.clone(), span));

        let mut errors = Vec::new();
        match arguments {
            ast::FailArguments::Tuple(arguments) => {
                self.check_constructor_arguments(&error_type, arguments, span, &mut errors);
            }
            ast::FailArguments::Struct(fields) => {
                self.check_failure_fields(&error_type, fields, span, &mut errors);
            }
        }

        ExprInfo {
            ty: Type::Never,
            errors,
        }
    }

    fn check_constructor_arguments(
        &mut self,
        error_type: &Type,
        arguments: &[ast::Expr],
        span: Span,
        errors: &mut Vec<ErrorEffect>,
    ) {
        if let Type::Named(name, _) = error_type {
            if let Some(struct_def) = self.resolve_struct(name) {
                let fields = struct_def.fields;
                if fields.len() != arguments.len() {
                    self.error(
                        ARGUMENT_COUNT,
                        format!(
                            "Failure type '{name}' expects {} argument(s), but {} were provided.",
                            fields.len(),
                            arguments.len()
                        ),
                        span,
                        "Match the declared error type fields.",
                    );
                }
                for (field, argument) in fields.iter().zip(arguments) {
                    let info = self.check_expr(argument, Some(&field.ty));
                    append_effects(errors, info.errors);
                }
                return;
            }
            if let Some(enum_def) = self.resolve_enum(name) {
                if enum_def.variants.len() != 1 {
                    self.error(
                        UNDECLARED_ERROR,
                        format!(
                            "Failure type '{name}' has {} variants, so '!{name}(...)' is ambiguous.",
                            enum_def.variants.len()
                        ),
                        span,
                        "Use a dedicated single-variant error enum or a struct error type.",
                    );
                }
                let Some(variant) = enum_def.variants.first() else {
                    self.error(
                        UNDECLARED_ERROR,
                        format!("Failure type '{name}' has no constructible variant."),
                        span,
                        "Declare at least one enum variant.",
                    );
                    return;
                };
                let types = variant
                    .fields
                    .clone()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|field| field.ty)
                    .collect::<Vec<_>>();
                if types.len() != arguments.len() {
                    self.error(
                        ARGUMENT_COUNT,
                        format!(
                            "Failure type '{name}' expects {} argument(s), but {} were provided.",
                            types.len(),
                            arguments.len()
                        ),
                        span,
                        "Match the declared error variant fields.",
                    );
                }
                for (ty, argument) in types.iter().zip(arguments) {
                    let info = self.check_expr(argument, Some(ty));
                    append_effects(errors, info.errors);
                }
                return;
            }
            self.error(
                UNDECLARED_ERROR,
                format!("Failure type '{name}' is not a constructible struct or enum."),
                span,
                "Declare the error type as a struct or enum before constructing it.",
            );
            return;
        }

        for argument in arguments {
            let info = self.check_expr(argument, None);
            append_effects(errors, info.errors);
        }
    }

    fn check_failure_fields(
        &mut self,
        error_type: &Type,
        fields: &[ast::FieldInit],
        span: Span,
        errors: &mut Vec<ErrorEffect>,
    ) {
        let Type::Named(name, _) = error_type else {
            for field in fields {
                let info = self.check_expr(&field.value, None);
                append_effects(errors, info.errors);
            }
            return;
        };
        let Some(struct_def) = self.resolve_struct(name) else {
            if self.resolve_enum(name).is_some() {
                self.error(
                    INVALID_STRUCT_FIELD,
                    format!(
                        "Failure type '{name}' is an enum; use positional construction '!{name}(...)'."
                    ),
                    span,
                    "Construct enum errors with tuple arguments, not named fields.",
                );
            } else {
                self.error(
                    UNDECLARED_ERROR,
                    format!("Failure type '{name}' is not a constructible struct or enum."),
                    span,
                    "Declare the error type as a struct or enum before constructing it.",
                );
            }
            for field in fields {
                let info = self.check_expr(&field.value, None);
                append_effects(errors, info.errors);
            }
            return;
        };

        let mut seen = std::collections::BTreeSet::new();
        for field in fields {
            if !seen.insert(field.name.as_str()) {
                self.error(
                    INVALID_STRUCT_FIELD,
                    format!(
                        "Failure field '{}' is initialized more than once.",
                        field.name
                    ),
                    field.span,
                    "Remove the duplicate initializer.",
                );
            }
            match struct_def
                .fields
                .iter()
                .find(|candidate| candidate.name.as_deref() == Some(field.name.as_str()))
            {
                Some(declared) => {
                    let info = self.check_expr(&field.value, Some(&declared.ty));
                    append_effects(errors, info.errors);
                }
                None => {
                    self.error(
                        INVALID_STRUCT_FIELD,
                        format!("Failure type '{name}' has no field '{}'.", field.name),
                        field.span,
                        "Use a declared field name.",
                    );
                    let info = self.check_expr(&field.value, None);
                    append_effects(errors, info.errors);
                }
            }
        }
        for field in &struct_def.fields {
            if let Some(field_name) = &field.name
                && !fields.iter().any(|candidate| candidate.name == *field_name)
            {
                self.error(
                    INVALID_STRUCT_FIELD,
                    format!("Field '{field_name}' is missing from the failure value."),
                    span,
                    "Initialize every field of the error type.",
                );
            }
        }
    }

    // ---------------------------------------------------------------------
    // Literals, effects, and helpers
    // ---------------------------------------------------------------------

    fn literal_type(
        &mut self,
        literal: &ast::Literal,
        expected: Option<&Type>,
        span: Span,
    ) -> Type {
        match literal {
            ast::Literal::Integer(text) => {
                let (raw, suffix) = integer_suffix(text);
                let actual = suffix
                    .or_else(|| expected.filter(|ty| ty.is_integer()).cloned())
                    .unwrap_or(Type::I32);

                if let Some(value) = parse_integer(raw)
                    && !integer_fits(value, &actual)
                {
                    self.error(
                        TYPE_MISMATCH,
                        format!("Integer literal '{text}' does not fit in '{actual}'."),
                        span,
                        "Use a wider integer type or a smaller literal.",
                    );
                }
                actual
            }
            ast::Literal::Float(text) => {
                let (suffixed, suffix) = float_suffix(text);
                let actual = suffix
                    .or_else(|| expected.filter(|ty| ty.is_float()).cloned())
                    .unwrap_or(Type::F64);
                let raw = suffixed.replace('_', "");
                if let Ok(value) = raw.parse::<f64>()
                    && actual == Type::F32
                    && (value as f32).is_infinite()
                    && value.is_finite()
                {
                    self.error(
                        TYPE_MISMATCH,
                        format!("Float literal '{text}' does not fit in f32."),
                        span,
                        "Use f64 for larger floating-point values.",
                    );
                }
                actual
            }
            ast::Literal::String(_) => Type::Str,
            ast::Literal::Bytes(_) => Type::Bytes,
            ast::Literal::Character(_) => Type::Char,
            ast::Literal::Bool(_) => Type::Bool,
            ast::Literal::Unit => Type::Unit,
        }
    }

    fn enum_declaration_span(&self, ty: &Type) -> Option<Span> {
        let name = type_name(ty)?;
        match self.type_defs.get(name) {
            Some(TypeDef::Enum(definition)) => definition.span,
            _ => None,
        }
    }

    fn type_mismatch(&mut self, expected: &Type, actual: &Type, span: Span) {
        self.type_mismatch_with_related(expected, actual, span, None);
    }

    fn type_mismatch_with_related(
        &mut self,
        expected: &Type,
        actual: &Type,
        span: Span,
        related: Option<(Span, &'static str)>,
    ) {
        let mut diagnostic = Diagnostic::error(
            TYPE_MISMATCH,
            format!("Type mismatch: expected '{expected}', found '{actual}'."),
        )
        .with_span(span)
        .with_hint("Convert the value explicitly; Neo Astra has no implicit coercion.");
        if let Some((declaration, message)) = related {
            diagnostic = diagnostic.with_related(String::new(), declaration, message);
        }
        self.diagnostics.push(diagnostic);
    }

    fn report_effects(&mut self, errors: &[ErrorEffect]) {
        for error in errors {
            if error.ty.is_opaque() {
                continue;
            }
            let mut diagnostic = Diagnostic::error(
                UNHANDLED_ERROR,
                format!("Unhandled error effect: {}.", error.ty),
            )
            .with_span(error.span)
            .with_hint(if self.options.edition == Edition::V0_2 {
                "Propagate with '?' or handle the error with match."
            } else {
                "Propagate with '!' or handle the error with match."
            });
            if let Some(function) = &self.current_function {
                let declaration = function.error_set_span.unwrap_or(function.span);
                diagnostic =
                    diagnostic.with_related(String::new(), declaration, "Declared error set.");
            }
            self.diagnostics.push(diagnostic);
        }
    }

    fn record_error(&mut self, error: &ErrorEffect) {
        if error.ty.is_opaque() {
            return;
        }

        let Some(function) = self.current_function.as_ref().cloned() else {
            self.error(
                INVALID_PROPAGATION,
                "Error propagation and failure construction are only valid inside a function.",
                error.span,
                "Move the operation into a fallible function.",
            );
            return;
        };

        if !effect_is_compatible(&function.declared_errors, &error.ty) {
            let declaration = function.error_set_span.unwrap_or(function.span);
            let diagnostic = Diagnostic::error(
                UNDECLARED_ERROR,
                format!(
                    "Error type '{}' is not declared by function '{}'.",
                    error.ty, function.name
                ),
            )
            .with_span(error.span)
            .with_hint(format!(
                "Add '! {}' to the error set of '{}'.",
                error.ty, function.name
            ))
            .with_related(String::new(), declaration, "Declared error set.");
            self.diagnostics.push(diagnostic);
        }
    }
}

impl Coverage {
    fn missing(&self, shape: &MatchShape) -> Vec<String> {
        let mut missing = Vec::new();
        match shape {
            MatchShape::Option => {
                if !self.some {
                    missing.push("Some(_)".to_owned());
                }
                if !self.none {
                    missing.push("None".to_owned());
                }
            }
            MatchShape::Result => {
                if !self.ok {
                    missing.push("Ok(_)".to_owned());
                }
                if !self.err {
                    missing.push("Err(_)".to_owned());
                }
            }
            MatchShape::Enum(enum_def) => {
                for variant in &enum_def.variants {
                    if !self.variants.contains(&variant.name) {
                        missing.push(variant.name.clone());
                    }
                }
            }
            MatchShape::Bool => {
                if !self.boolean[0] {
                    missing.push("false".to_owned());
                }
                if !self.boolean[1] {
                    missing.push("true".to_owned());
                }
            }
            MatchShape::Tuple => missing.push("a tuple wildcard arm".to_owned()),
            MatchShape::Other => missing.push("a wildcard arm".to_owned()),
        }

        if missing.is_empty() {
            missing.push("a wildcard arm".to_owned());
        }
        missing
    }
}

fn generic_map(type_parameters: &[String]) -> BTreeMap<String, Type> {
    type_parameters
        .iter()
        .map(|parameter| (parameter.clone(), Type::Generic(parameter.clone())))
        .collect()
}

fn substitutions(type_parameters: &[String], arguments: &[Type]) -> BTreeMap<String, Type> {
    type_parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| (parameter.clone(), argument.clone()))
        .collect()
}

fn check_arity(
    type_parameters: &[String],
    arguments: &[Type],
    name: &str,
    span: Span,
    checker: &mut Checker<'_>,
) -> bool {
    if type_parameters.is_empty() && arguments.is_empty() {
        return true;
    }
    if type_parameters.len() == arguments.len() {
        return true;
    }
    checker.error(
        INVALID_TYPE_ARGUMENT,
        format!(
            "Type '{name}' expects {} type argument(s), but {} were provided.",
            type_parameters.len(),
            arguments.len()
        ),
        span,
        "Match the declared type parameter list.",
    );
    false
}

fn primitive_type(arguments: &[Type], ty: Type, span: Span, checker: &mut Checker<'_>) -> Type {
    if !arguments.is_empty() {
        checker.error(
            INVALID_TYPE_ARGUMENT,
            format!("Primitive type '{ty}' cannot take type arguments."),
            span,
            "Remove the type argument list.",
        );
        return Type::Error;
    }
    ty
}

fn builtin_function(name: &str) -> Option<Type> {
    let signature = match name {
        "print" | "println" | "dbg" => FunctionType {
            type_parameters: Vec::new(),
            parameters: vec![Type::Unknown],
            return_type: Box::new(Type::Unit),
            errors: Vec::new(),
        },
        "assert" => FunctionType {
            type_parameters: Vec::new(),
            parameters: vec![Type::Bool],
            return_type: Box::new(Type::Unit),
            errors: Vec::new(),
        },
        _ => return None,
    };
    Some(Type::Function(signature))
}

/// The index type expected by indexed reads and writes for `target`, if the
/// target supports index syntax. Sequence-like targets (arrays, slices, fixed
/// arrays, and `bytes`) index by `usize`; maps index by their key type.
fn expected_index_type(ty: &Type) -> Option<Type> {
    match ty {
        Type::Array(_) | Type::Slice(_) | Type::FixedArray(_, _) | Type::Bytes => Some(Type::Usize),
        Type::Map(key, _) => Some((**key).clone()),
        _ => None,
    }
}

fn callee_identifier(callee: &ast::Expr) -> Option<String> {
    match &callee.kind {
        ast::ExprKind::Identifier(name) => Some(name.clone()),
        ast::ExprKind::TypeArguments { target, .. } => match &target.kind {
            ast::ExprKind::Identifier(name) => Some(name.clone()),
            _ => None,
        },
        _ => None,
    }
}

fn builtin_constructor_name(callee: &ast::Expr) -> Option<&str> {
    match &callee.kind {
        ast::ExprKind::Identifier(name) if matches!(name.as_str(), "Some" | "Ok" | "Err") => {
            Some(name)
        }
        _ => None,
    }
}

/// True for the built-in `Channel.create<T>()` callee, with or without explicit
/// type arguments.
fn channel_create_callee(callee: &ast::Expr) -> bool {
    match &callee.kind {
        ast::ExprKind::Member { target, member } => {
            member == "create"
                && matches!(&target.kind, ast::ExprKind::Identifier(name) if name == "Channel")
        }
        ast::ExprKind::TypeArguments { target, .. } => channel_create_callee(target),
        _ => false,
    }
}

fn builtin_method(ty: &Type, member: &str) -> Option<Type> {
    let signature = match ty {
        Type::Str => match member {
            "is_empty" => FunctionType {
                type_parameters: Vec::new(),
                parameters: Vec::new(),
                return_type: Box::new(Type::Bool),
                errors: Vec::new(),
            },
            "len" => FunctionType {
                type_parameters: Vec::new(),
                parameters: Vec::new(),
                return_type: Box::new(Type::Usize),
                errors: Vec::new(),
            },
            "to_int" => FunctionType {
                type_parameters: Vec::new(),
                parameters: Vec::new(),
                return_type: Box::new(Type::I32),
                errors: vec![Type::Named("ParseIntError".to_owned(), Vec::new())],
            },
            "char_count" => FunctionType {
                type_parameters: Vec::new(),
                parameters: Vec::new(),
                return_type: Box::new(Type::Usize),
                errors: Vec::new(),
            },
            "bytes" => FunctionType {
                type_parameters: Vec::new(),
                parameters: Vec::new(),
                return_type: Box::new(Type::Array(Box::new(Type::U8))),
                errors: Vec::new(),
            },
            "chars" => FunctionType {
                type_parameters: Vec::new(),
                parameters: Vec::new(),
                return_type: Box::new(Type::Array(Box::new(Type::Str))),
                errors: Vec::new(),
            },
            "byte_at" => FunctionType {
                type_parameters: Vec::new(),
                parameters: vec![Type::Usize],
                return_type: Box::new(Type::Option(Box::new(Type::U8))),
                errors: Vec::new(),
            },
            "char_at" => FunctionType {
                type_parameters: Vec::new(),
                parameters: vec![Type::Usize],
                return_type: Box::new(Type::Option(Box::new(Type::Str))),
                errors: Vec::new(),
            },
            "slice" => FunctionType {
                type_parameters: Vec::new(),
                parameters: vec![Type::Usize, Type::Usize],
                return_type: Box::new(Type::Str),
                errors: vec![Type::Named("SliceError".to_owned(), Vec::new())],
            },
            _ => return None,
        },
        Type::Bytes => match member {
            "is_empty" => FunctionType {
                type_parameters: Vec::new(),
                parameters: Vec::new(),
                return_type: Box::new(Type::Bool),
                errors: Vec::new(),
            },
            "len" => FunctionType {
                type_parameters: Vec::new(),
                parameters: Vec::new(),
                return_type: Box::new(Type::Usize),
                errors: Vec::new(),
            },
            "slice" => FunctionType {
                type_parameters: Vec::new(),
                parameters: vec![Type::Usize, Type::Usize],
                return_type: Box::new(Type::Bytes),
                errors: Vec::new(),
            },
            _ => return None,
        },
        Type::Array(element) | Type::Slice(element) | Type::FixedArray(element, _) => {
            match member {
                "len" => FunctionType {
                    type_parameters: Vec::new(),
                    parameters: Vec::new(),
                    return_type: Box::new(Type::Usize),
                    errors: Vec::new(),
                },
                "push" => FunctionType {
                    type_parameters: Vec::new(),
                    parameters: vec![(**element).clone()],
                    return_type: Box::new(Type::Unit),
                    errors: Vec::new(),
                },
                "get" => FunctionType {
                    type_parameters: Vec::new(),
                    parameters: vec![Type::Usize],
                    return_type: Box::new(Type::Option(element.clone())),
                    errors: Vec::new(),
                },
                "slice" => FunctionType {
                    type_parameters: Vec::new(),
                    parameters: vec![Type::Usize, Type::Usize],
                    return_type: Box::new(Type::Array(element.clone())),
                    errors: Vec::new(),
                },
                _ => return None,
            }
        }
        Type::Map(key, value) => match member {
            "len" => FunctionType {
                type_parameters: Vec::new(),
                parameters: Vec::new(),
                return_type: Box::new(Type::Usize),
                errors: Vec::new(),
            },
            "get" => FunctionType {
                type_parameters: Vec::new(),
                parameters: vec![(**key).clone()],
                return_type: Box::new(Type::Option(value.clone())),
                errors: Vec::new(),
            },
            "contains" => FunctionType {
                type_parameters: Vec::new(),
                parameters: vec![(**key).clone()],
                return_type: Box::new(Type::Bool),
                errors: Vec::new(),
            },
            "insert" => FunctionType {
                type_parameters: Vec::new(),
                parameters: vec![(**key).clone(), (**value).clone()],
                return_type: Box::new(Type::Option(value.clone())),
                errors: Vec::new(),
            },
            _ => return None,
        },
        Type::Option(inner) => match member {
            "is_some" | "is_none" => FunctionType {
                type_parameters: Vec::new(),
                parameters: Vec::new(),
                return_type: Box::new(Type::Bool),
                errors: Vec::new(),
            },
            "unwrap" => FunctionType {
                type_parameters: Vec::new(),
                parameters: Vec::new(),
                return_type: inner.clone(),
                errors: Vec::new(),
            },
            "unwrap_or" => FunctionType {
                type_parameters: Vec::new(),
                parameters: vec![(**inner).clone()],
                return_type: inner.clone(),
                errors: Vec::new(),
            },
            _ => return None,
        },
        Type::Result(ok, error) => match member {
            "is_ok" | "is_err" => FunctionType {
                type_parameters: Vec::new(),
                parameters: Vec::new(),
                return_type: Box::new(Type::Bool),
                errors: Vec::new(),
            },
            "unwrap" => FunctionType {
                type_parameters: Vec::new(),
                parameters: Vec::new(),
                return_type: ok.clone(),
                errors: Vec::new(),
            },
            "unwrap_or" => FunctionType {
                type_parameters: Vec::new(),
                parameters: vec![(**ok).clone()],
                return_type: ok.clone(),
                errors: Vec::new(),
            },
            "error" => FunctionType {
                type_parameters: Vec::new(),
                parameters: Vec::new(),
                return_type: error.clone(),
                errors: Vec::new(),
            },
            _ => return None,
        },
        Type::Named(name, arguments) if name == "Task" => match member {
            "join" => FunctionType {
                type_parameters: Vec::new(),
                parameters: Vec::new(),
                return_type: Box::new(arguments.first().cloned().unwrap_or(Type::Unit)),
                errors: Vec::new(),
            },
            "cancel" => FunctionType {
                type_parameters: Vec::new(),
                parameters: Vec::new(),
                return_type: Box::new(Type::Unit),
                errors: Vec::new(),
            },
            "canceled" => FunctionType {
                type_parameters: Vec::new(),
                parameters: Vec::new(),
                return_type: Box::new(Type::Bool),
                errors: Vec::new(),
            },
            _ => return None,
        },
        Type::Named(name, arguments) if name == "Ptr" => {
            let element = arguments.first().cloned().unwrap_or(Type::Unknown);
            match member {
                // `ptr.read(index)` / `ptr.write(index, value)` (spec §8.4).
                // Both are unchecked: a manual pointer carries no length, so
                // the compiler has nothing to bounds-check against, and the
                // spec makes out-of-bounds access undefined in manual mode.
                "read" => FunctionType {
                    type_parameters: Vec::new(),
                    parameters: vec![Type::Usize],
                    return_type: Box::new(element),
                    errors: Vec::new(),
                },
                "write" => FunctionType {
                    type_parameters: Vec::new(),
                    parameters: vec![Type::Usize, element],
                    return_type: Box::new(Type::Unit),
                    errors: Vec::new(),
                },
                _ => return None,
            }
        }
        Type::Named(name, _) if name == "Region" => match member {
            // `region.alloc<T>(count)` / `region.free()` (spec 8.4). A region
            // is a fixed block, so `alloc` is fallible in the spec (`!
            // AllocError`); like `Memory.alloc`, the frontend does not track
            // that effect yet and the C backend panics on exhaustion until
            // the fallible entry-point policy lands.
            "alloc" => FunctionType {
                type_parameters: vec!["T".to_owned()],
                parameters: vec![Type::Usize],
                return_type: Box::new(Type::Named(
                    "Ptr".to_owned(),
                    vec![Type::Generic("T".to_owned())],
                )),
                errors: Vec::new(),
            },
            "free" => FunctionType {
                type_parameters: Vec::new(),
                parameters: Vec::new(),
                return_type: Box::new(Type::Unit),
                errors: Vec::new(),
            },
            _ => return None,
        },
        Type::Named(name, arguments) if name == "Channel" => {
            let element = arguments.first().cloned().unwrap_or(Type::Unknown);
            match member {
                // `send` and `receive` are fallible: their error effect is
                // `ChannelError` (closed channel, or a would-block operation
                // the runtime cannot service). Runtime backends surface them
                // as Result-shaped values, like `to_int`.
                "send" => FunctionType {
                    type_parameters: Vec::new(),
                    parameters: vec![element],
                    return_type: Box::new(Type::Unit),
                    errors: vec![Type::Named("ChannelError".to_owned(), Vec::new())],
                },
                "receive" => FunctionType {
                    type_parameters: Vec::new(),
                    parameters: Vec::new(),
                    return_type: Box::new(element),
                    errors: vec![Type::Named("ChannelError".to_owned(), Vec::new())],
                },
                "close" => FunctionType {
                    type_parameters: Vec::new(),
                    parameters: Vec::new(),
                    return_type: Box::new(Type::Unit),
                    errors: Vec::new(),
                },
                _ => return None,
            }
        }
        _ if ty.is_integer() => match member {
            "wrapping_add" | "wrapping_sub" | "wrapping_mul" | "wrapping_div" => FunctionType {
                type_parameters: Vec::new(),
                parameters: vec![ty.clone()],
                return_type: Box::new(ty.clone()),
                errors: Vec::new(),
            },
            "checked_add" | "checked_sub" | "checked_mul" | "checked_div" => FunctionType {
                type_parameters: Vec::new(),
                parameters: vec![ty.clone()],
                return_type: Box::new(ty.clone()),
                errors: vec![Type::Named("OverflowError".to_owned(), Vec::new())],
            },
            _ => return None,
        },
        _ => return None,
    };
    Some(Type::Function(signature))
}

fn contains_generic(ty: &Type) -> bool {
    match ty {
        Type::Generic(_) => true,
        Type::Option(inner) | Type::Array(inner) | Type::Slice(inner) => contains_generic(inner),
        Type::FixedArray(inner, _) => contains_generic(inner),
        Type::Result(ok, error) | Type::Map(ok, error) => {
            contains_generic(ok) || contains_generic(error)
        }
        Type::Tuple(elements) => elements.iter().any(contains_generic),
        Type::Function(function) => {
            function.parameters.iter().any(contains_generic)
                || contains_generic(&function.return_type)
                || function.errors.iter().any(contains_generic)
        }
        Type::Named(_, arguments) => arguments.iter().any(contains_generic),
        _ => false,
    }
}

fn infer_substitutions(
    parameter: &Type,
    argument: &Type,
    substitutions: &mut BTreeMap<String, Type>,
) {
    if argument.is_opaque() || argument == &Type::Never {
        return;
    }
    match (parameter, argument) {
        (Type::Generic(name), actual) => {
            substitutions
                .entry(name.clone())
                .or_insert_with(|| actual.clone());
        }
        (Type::Option(parameter), Type::Option(argument)) => {
            infer_substitutions(parameter, argument, substitutions);
        }
        (
            Type::Result(parameter_ok, parameter_error),
            Type::Result(argument_ok, argument_error),
        ) => {
            infer_substitutions(parameter_ok, argument_ok, substitutions);
            infer_substitutions(parameter_error, argument_error, substitutions);
        }
        (Type::Array(parameter), Type::Array(argument))
        | (Type::Array(parameter), Type::Slice(argument))
        | (Type::Slice(parameter), Type::Array(argument))
        | (Type::Slice(parameter), Type::Slice(argument)) => {
            infer_substitutions(parameter, argument, substitutions);
        }
        (Type::FixedArray(parameter, _), Type::FixedArray(argument, _)) => {
            infer_substitutions(parameter, argument, substitutions);
        }
        (Type::Map(parameter_key, parameter_value), Type::Map(argument_key, argument_value)) => {
            infer_substitutions(parameter_key, argument_key, substitutions);
            infer_substitutions(parameter_value, argument_value, substitutions);
        }
        (Type::Tuple(parameters), Type::Tuple(arguments)) => {
            for (parameter, argument) in parameters.iter().zip(arguments) {
                infer_substitutions(parameter, argument, substitutions);
            }
        }
        (Type::Named(parameter_name, parameters), Type::Named(argument_name, arguments))
            if parameter_name == argument_name =>
        {
            for (parameter, argument) in parameters.iter().zip(arguments) {
                infer_substitutions(parameter, argument, substitutions);
            }
        }
        (Type::Function(parameter), Type::Function(argument)) => {
            for (parameter, argument) in parameter.parameters.iter().zip(&argument.parameters) {
                infer_substitutions(parameter, argument, substitutions);
            }
            infer_substitutions(&parameter.return_type, &argument.return_type, substitutions);
        }
        _ => {}
    }
}

fn type_name(ty: &Type) -> Option<&str> {
    match ty {
        Type::Named(name, _) => Some(name),
        _ => None,
    }
}

/// The declared error members of a fallible scrutinee. Explicit effects win;
/// a concrete `Result` error type is the fallback for library values.
fn error_members(ty: &Type, effects: &[Type]) -> Vec<Type> {
    if !effects.is_empty() {
        return effects.to_vec();
    }
    if let Type::Result(_, error) = ty
        && !error.is_opaque()
    {
        return vec![(**error).clone()];
    }
    Vec::new()
}

fn error_member_hint(members: &[Type]) -> String {
    if members.is_empty() {
        "The matched expression declares no error members.".to_owned()
    } else {
        format!(
            "Declared error members: {}.",
            members
                .iter()
                .map(|member| member.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

/// The single compatibility rule for a nominal effect and a declared error set.
///
/// Failure construction and propagation both flow through `record_error`, so
/// this helper keeps membership checks consistent for both paths.
fn effect_is_compatible(declared: &[Type], error: &Type) -> bool {
    declared.iter().any(|candidate| candidate == error)
}

fn append_unique(target: &mut Vec<Type>, values: Vec<Type>) {
    for value in values {
        if !target.iter().any(|existing| existing == &value) {
            target.push(value);
        }
    }
}

fn append_effects(target: &mut Vec<ErrorEffect>, values: Vec<ErrorEffect>) {
    for value in values {
        if !target.iter().any(|existing| existing.ty == value.ty) {
            target.push(value);
        }
    }
}

fn merge_effects(left: &[ErrorEffect], right: &[ErrorEffect]) -> Vec<ErrorEffect> {
    let mut merged = left.to_vec();
    append_effects(&mut merged, right.to_vec());
    merged
}

fn block_value_type(info: &BlockInfo) -> Type {
    if info.diverges {
        return Type::Never;
    }
    info.tail
        .as_ref()
        .map(|tail| tail.ty.clone())
        .unwrap_or(Type::Unit)
}

fn block_tail_errors(info: &BlockInfo) -> Vec<ErrorEffect> {
    info.tail
        .as_ref()
        .map(|tail| tail.errors.clone())
        .unwrap_or_default()
}

fn pattern_is_catch_all(pattern: &ast::Pattern, ty: &Type) -> bool {
    match &pattern.kind {
        ast::PatternKind::Wildcard => true,
        ast::PatternKind::Identifier(name) => !(name == "None" && matches!(ty, Type::Option(_))),
        _ => false,
    }
}

fn pattern_row<'a>(
    pattern: &'a ast::Pattern,
    ty: &Type,
    key: &ConstructorKey,
    field_count: usize,
) -> Option<Vec<RowPattern<'a>>> {
    let (pattern_key, arguments) = match &pattern.kind {
        ast::PatternKind::Wildcard => {
            return Some(vec![RowPattern::Wildcard; field_count]);
        }
        ast::PatternKind::Identifier(name) => {
            if name == "None" && matches!(ty, Type::Option(_)) {
                (ConstructorKey::None, Vec::new())
            } else {
                return Some(vec![RowPattern::Wildcard; field_count]);
            }
        }
        ast::PatternKind::Literal(ast::Literal::Bool(value)) => {
            (ConstructorKey::Bool(*value), Vec::new())
        }
        ast::PatternKind::Literal(ast::Literal::Unit) if matches!(ty, Type::Unit) => {
            (ConstructorKey::Unit, Vec::new())
        }
        ast::PatternKind::Literal(_) => return None,
        ast::PatternKind::Tuple(patterns) if matches!(ty, Type::Tuple(_)) => (
            ConstructorKey::Tuple,
            patterns.iter().map(RowPattern::Pattern).collect(),
        ),
        ast::PatternKind::Tuple(_) => return None,
        ast::PatternKind::Constructor { path, arguments } => {
            let name = path.segments.last().map(String::as_str).unwrap_or_default();
            match (name, ty) {
                ("Some", Type::Option(_)) => (
                    ConstructorKey::Some,
                    arguments.iter().map(RowPattern::Pattern).collect(),
                ),
                ("Ok", Type::Result(_, _)) => (
                    ConstructorKey::Ok,
                    arguments.iter().map(RowPattern::Pattern).collect(),
                ),
                ("Err", Type::Result(_, _)) => (
                    ConstructorKey::Err,
                    arguments.iter().map(RowPattern::Pattern).collect(),
                ),
                _ => return None,
            }
        }
        ast::PatternKind::Variant {
            path,
            variant,
            arguments,
        } => {
            let enum_name = path.segments.last().map(String::as_str).unwrap_or_default();
            let nested = arguments
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .map(RowPattern::Pattern)
                .collect::<Vec<_>>();
            match enum_name {
                "Option" if variant == "Some" => (ConstructorKey::Some, nested),
                "Option" if variant == "None" => (ConstructorKey::None, Vec::new()),
                "Result" if variant == "Ok" => (ConstructorKey::Ok, nested),
                "Result" if variant == "Err" => (ConstructorKey::Err, nested),
                _ => (ConstructorKey::Variant(variant.clone()), nested),
            }
        }
    };

    if &pattern_key != key {
        return None;
    }

    let mut row = arguments;
    row.truncate(field_count);
    while row.len() < field_count {
        row.push(RowPattern::Wildcard);
    }
    Some(row)
}

fn result_parts(ty: &Type, effects: &[Type]) -> (Type, Type) {
    if let Type::Result(ok, error) = ty {
        return ((**ok).clone(), (**error).clone());
    }
    if !effects.is_empty() {
        return (
            ty.clone(),
            if effects.len() == 1 {
                effects[0].clone()
            } else {
                Type::Unknown
            },
        );
    }
    (ty.clone(), Type::Unknown)
}

fn annotation_name(expression: &ast::Expr) -> Option<(String, Span)> {
    match &expression.kind {
        ast::ExprKind::Identifier(name) => Some((name.clone(), expression.span)),
        ast::ExprKind::Literal(ast::Literal::String(name)) => Some((name.clone(), expression.span)),
        _ => None,
    }
}

fn is_numeric_literal(expression: &ast::Expr) -> bool {
    matches!(
        expression.kind,
        ast::ExprKind::Literal(ast::Literal::Integer(_) | ast::Literal::Float(_))
    )
}

fn integer_literal_value(expression: &ast::Expr) -> Option<usize> {
    let ast::ExprKind::Literal(ast::Literal::Integer(text)) = &expression.kind else {
        return None;
    };
    let (raw, _) = integer_suffix(text);
    usize::try_from(parse_integer(raw)?).ok()
}

fn parse_integer(text: &str) -> Option<u128> {
    let cleaned = text.replace('_', "");
    if let Some(hex) = cleaned
        .strip_prefix("0x")
        .or_else(|| cleaned.strip_prefix("0X"))
    {
        u128::from_str_radix(hex, 16).ok()
    } else if let Some(binary) = cleaned
        .strip_prefix("0b")
        .or_else(|| cleaned.strip_prefix("0B"))
    {
        u128::from_str_radix(binary, 2).ok()
    } else {
        cleaned.parse::<u128>().ok()
    }
}

fn integer_fits(value: u128, ty: &Type) -> bool {
    match ty {
        Type::I8 => value <= i8::MAX as u128,
        Type::I16 => value <= i16::MAX as u128,
        Type::I32 => value <= i32::MAX as u128,
        Type::I64 => value <= i64::MAX as u128,
        Type::Isize => value <= i64::MAX as u128,
        Type::U8 => value <= u8::MAX as u128,
        Type::U16 => value <= u16::MAX as u128,
        Type::U32 => value <= u32::MAX as u128,
        Type::U64 => value <= u64::MAX as u128,
        Type::Usize => value <= u64::MAX as u128,
        _ => true,
    }
}

/// True when evaluating `expression` can run user code (a call, a spawned
/// block, or a block/control-flow expression containing one). Used by the D27
/// system-mode side-effect rule.
fn expr_has_call(expression: &ast::Expr) -> bool {
    match &expression.kind {
        ast::ExprKind::Call { .. } => true,
        ast::ExprKind::Unary { operand, .. } => expr_has_call(operand),
        ast::ExprKind::Binary { left, right, .. } => expr_has_call(left) || expr_has_call(right),
        ast::ExprKind::Assign { target, value } => expr_has_call(target) || expr_has_call(value),
        ast::ExprKind::Index { target, index } => expr_has_call(target) || expr_has_call(index),
        ast::ExprKind::Range { start, end, .. } => expr_has_call(start) || expr_has_call(end),
        ast::ExprKind::Member { target, .. } => expr_has_call(target),
        ast::ExprKind::TypeArguments { target, .. } => expr_has_call(target),
        ast::ExprKind::Propagate(inner) => expr_has_call(inner),
        ast::ExprKind::Array(elements) | ast::ExprKind::Tuple(elements) => {
            elements.iter().any(expr_has_call)
        }
        ast::ExprKind::Map(entries) => entries
            .iter()
            .any(|entry| expr_has_call(&entry.key) || expr_has_call(&entry.value)),
        ast::ExprKind::StructInit { fields, .. } => {
            fields.iter().any(|field| expr_has_call(&field.value))
        }
        ast::ExprKind::Block(block) | ast::ExprKind::Spawn { body: block } => {
            block_has_calls(block)
        }
        ast::ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_has_call(condition)
                || block_has_calls(then_branch)
                || match else_branch {
                    Some(ast::ElseBranch::Block(block)) => block_has_calls(block),
                    Some(ast::ElseBranch::If(inner)) => expr_has_call(inner),
                    None => false,
                }
        }
        ast::ExprKind::Match { value, arms } => {
            expr_has_call(value)
                || arms.iter().any(|arm| match &arm.body {
                    ast::MatchArmBody::Expr(inner) => expr_has_call(inner),
                    ast::MatchArmBody::Block(block) => block_has_calls(block),
                })
        }
        ast::ExprKind::Fail { arguments, .. } => match arguments {
            ast::FailArguments::Tuple(values) => values.iter().any(expr_has_call),
            ast::FailArguments::Struct(fields) => {
                fields.iter().any(|field| expr_has_call(&field.value))
            }
        },
        ast::ExprKind::Panic { message } | ast::ExprKind::Abort { message } => {
            expr_has_call(message)
        }
        ast::ExprKind::Literal(_) | ast::ExprKind::Identifier(_) => false,
    }
}

fn block_has_calls(block: &ast::Block) -> bool {
    block.statements.iter().any(stmt_has_calls)
}

/// A valid C identifier: `[A-Za-z_][A-Za-z0-9_]*`.
fn is_valid_c_identifier(symbol: &str) -> bool {
    let mut characters = symbol.chars();
    match characters.next() {
        Some(first) if first == '_' || first.is_ascii_alphabetic() => {}
        _ => return false,
    }
    characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn stmt_has_calls(statement: &ast::Stmt) -> bool {
    match &statement.kind {
        ast::StmtKind::Let { value, .. } => expr_has_call(value),
        ast::StmtKind::Return { value } => value.as_ref().is_some_and(expr_has_call),
        ast::StmtKind::Break | ast::StmtKind::Continue => false,
        ast::StmtKind::Defer(inner) => stmt_has_calls(inner),
        ast::StmtKind::While { condition, body } => {
            expr_has_call(condition) || block_has_calls(body)
        }
        ast::StmtKind::For { iterable, body, .. } => {
            expr_has_call(iterable) || block_has_calls(body)
        }
        ast::StmtKind::Expr(inner) => expr_has_call(inner),
        ast::StmtKind::Scoped(scoped) => scoped.statements.iter().any(stmt_has_calls),
    }
}

#[cfg(test)]
mod tests {
    use crate::diagnostic::Diagnostic;
    use crate::mode::Mode;

    use super::{CheckOptions, check_source};

    fn diagnostics(source: &str) -> Vec<Diagnostic> {
        check_source(source, CheckOptions::new(Mode::Core))
            .err()
            .unwrap_or_default()
    }

    fn check(source: &str) -> Result<super::CheckResult, Vec<String>> {
        check_with_mode(source, Mode::Core)
    }

    fn check_with_mode(source: &str, mode: Mode) -> Result<super::CheckResult, Vec<String>> {
        check_source(source, CheckOptions::new(mode)).map_err(|diagnostics| {
            diagnostics
                .into_iter()
                .map(|diagnostic| diagnostic.code)
                .collect()
        })
    }

    fn assert_code(source: &str, code: &str) {
        let result = check(source);
        assert!(
            result
                .as_ref()
                .is_err_and(|codes| codes.contains(&code.to_owned())),
            "expected {code}, got {result:?}"
        );
    }

    #[test]
    fn infers_local_bindings_and_checks_returns() {
        let result = check(
            r#"
fn add(a: i32, b: i32) -> i32 {
    let total = a + b
    return total
}
"#,
        );
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn rejects_implicit_numeric_conversion() {
        let result = check(
            r#"
fn bad() -> u64 {
    let value: u32 = 1
    return value
}
"#,
        );
        assert!(result.is_err_and(|codes| codes.contains(&"NS4001".to_owned())));
    }

    #[test]
    fn requires_exhaustive_option_matches() {
        let result = check(
            r#"
fn value(input: Option<i32>) -> i32 {
    return match input {
        Some(number) => number,
    }
}
"#,
        );
        assert!(
            result
                .as_ref()
                .is_err_and(|codes| codes.contains(&"NS4010".to_owned())),
            "{result:?}"
        );
    }

    #[test]
    fn accepts_exhaustive_option_matches() {
        let result = check(
            r#"
fn value(input: Option<i32>) -> i32 {
    return match input {
        Some(number) => number,
        None => 0,
    }
}
"#,
        );
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn requires_fallible_calls_to_be_handled() {
        let result = check(
            r#"
fn parse(input: str) -> i32 ! ParseIntError {
    return input.to_int()!
}

fn caller(input: str) -> i32 {
    return input.to_int()
}
"#,
        );
        assert!(result.is_err_and(|codes| codes.contains(&"NS4011".to_owned())));
    }

    #[test]
    fn checks_declared_error_sets() {
        let result = check(
            r#"
fn parse(input: str) -> i32 {
    return input.to_int()!
}
"#,
        );
        assert!(result.is_err_and(|codes| codes.contains(&"NS4012".to_owned())));
    }

    #[test]
    fn reports_unhandled_effect_at_the_call_site() {
        let source = r#"
struct E(str)
fn parse() -> i32 ! E {
    return !E("bad")
}
fn caller() {
    let value = parse()
}
"#;
        let diagnostics = diagnostics(source);
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "NS4011")
            .expect("unhandled effect");
        let span = diagnostic.span.expect("effect span");
        assert_eq!(&source[span.start..span.end], "parse()");
    }

    #[test]
    fn deduplicates_the_same_effect_in_one_expression() {
        let source = r#"
struct E(str)
fn parse() -> i32 ! E {
    return !E("bad")
}
fn caller() {
    let value = parse() + parse()
}
"#;
        let count = diagnostics(source)
            .iter()
            .filter(|diagnostic| diagnostic.code == "NS4011")
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn reports_effects_inside_arguments_and_collections() {
        let source = r#"
struct E(str)
fn parse() -> i32 ! E {
    return !E("bad")
}
fn consume(value: i32) {}
fn caller() {
    consume(parse())
    let values = [parse(), parse()]
}
"#;
        let diagnostics = diagnostics(source);
        let spans = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "NS4011")
            .filter_map(|diagnostic| diagnostic.span)
            .map(|span| &source[span.start..span.end])
            .collect::<Vec<_>>();
        assert_eq!(spans, ["parse()", "parse()"], "{diagnostics:?}");
    }

    #[test]
    fn reports_multiple_distinct_effects() {
        let source = r#"
struct A(str)
struct B(str)
fn first() -> i32 ! A {
    return !A("a")
}
fn second() -> i32 ! B {
    return !B("b")
}
fn caller() {
    let value = first() + second()
}
"#;
        let count = diagnostics(source)
            .iter()
            .filter(|diagnostic| diagnostic.code == "NS4011")
            .count();
        assert_eq!(count, 2);
    }

    #[test]
    fn non_exhaustive_match_reports_only_exhaustiveness() {
        let source = r#"
struct E(str)
fn parse() -> i32 ! E {
    return !E("bad")
}
fn caller() {
    match parse() {
        Ok(value) => value,
    }
}
"#;
        let codes = diagnostics(source)
            .into_iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>();
        assert!(codes.contains(&"NS4010".to_owned()), "{codes:?}");
        assert!(!codes.contains(&"NS4011".to_owned()), "{codes:?}");
    }

    #[test]
    fn exhaustive_match_handles_a_fallible_expression() {
        let result = check(
            r#"
struct E(str)
fn parse() -> i32 ! E {
    return !E("bad")
}
fn caller() {
    match parse() {
        Ok(value) => value,
        Err(_) => 0,
    }
}
"#,
        );
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn checks_errors_from_function_value_calls() {
        let result = check(
            r#"
struct E(str)
fn parse() -> i32 ! E {
    return !E("bad")
}
fn invoke(f: fn() -> i32 ! E) -> i32 ! E {
    return f()!
}
fn main() {
    match invoke(parse) {
        Ok(value) => value,
        Err(_) => 0,
    }
}
"#,
        );
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn rejects_undeclared_errors_from_function_value_calls() {
        let source = r#"
struct E(str)
fn invoke(f: fn() -> i32 ! E) -> i32 {
    return f()!
}
"#;
        assert_code(source, "NS4012");
    }

    #[test]
    fn validates_named_failure_fields_at_the_value() {
        let source = r#"
struct E {
    message: str,
}
fn bad() -> i32 ! E {
    return !E { message: 1 }
}
"#;
        let diagnostics = diagnostics(source);
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "NS4001")
            .expect("field type mismatch");
        let span = diagnostic.span.expect("field span");
        assert_eq!(&source[span.start..span.end], "1");
        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.code == "NS4001")
                .count(),
            1
        );
    }

    #[test]
    fn rejects_ambiguous_enum_error_construction() {
        let source = r#"
enum E {
    A(str),
    B(i32),
}
fn bad() -> i32 ! E {
    return !E("bad")
}
"#;
        assert_code(source, "NS4012");
    }

    #[test]
    fn checks_struct_fields_and_construction() {
        let result = check(
            r#"
struct User {
    name: str,
    age: i32,
}

fn make() -> User {
    return User { name: "Ada", age: 36 }
}
"#,
        );
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn rejects_missing_struct_fields() {
        let result = check(
            r#"
struct User {
    name: str,
    age: i32,
}

fn make() -> User {
    return User { name: "Ada" }
}
"#,
        );
        assert!(result.is_err_and(|codes| codes.contains(&"NS4016".to_owned())));
    }

    #[test]
    fn rejects_mode_widening() {
        let result = check(
            r#"
{@mode:system;
    fn escape() {}
}
"#,
        );
        assert!(result.is_err_and(|codes| codes.contains(&"NS5002".to_owned())));
    }

    #[test]
    fn checks_composite_types_and_members() {
        let result = check(
            r#"
type Age = i32

struct Point {
    x: i32,
    y: i32,
}

enum Status {
    Active,
    Suspended(str),
}

fn first(points: [Point]) -> Point {
    return points[0]
}

fn lookup(values: {str: i32}) -> i32 {
    return values["answer"]
}

fn compose(value: (i32, str)) -> i32 {
    return value.0
}

fn describe(status: Status) -> i32 {
    return match status {
        Status.Active => 1,
        Status.Suspended(reason) => 2,
    }
}
"#,
        );
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn checks_generic_signatures_and_inference() {
        let result = check(
            r#"
fn identity<T>(value: T) -> T {
    return value
}

fn inferred() -> i32 {
    return identity(1)
}

fn explicit() -> i32 {
    return identity<i32>(1)
}

fn wrong() -> str {
    return identity(1)
}
"#,
        );
        assert!(result.is_err_and(|codes| codes.contains(&"NS4001".to_owned())));
    }

    #[test]
    fn checks_generic_structs_and_enums() {
        let result = check(
            r#"
struct Box<T> {
    value: T,
}

enum Maybe<T> {
    Just(T),
    Nothing,
}

fn make() -> Box<i32> {
    return Box<i32> { value: 1 }
}

fn read(boxed: Box<i32>) -> i32 {
    return boxed.value
}

fn maybe(value: Maybe<i32>) -> i32 {
    return match value {
        Maybe.Just(inner) => inner,
        Maybe.Nothing => 0,
    }
}
"#,
        );
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn rejects_wrong_generic_arity_and_types() {
        assert_code(
            r#"
struct Box<T> {
    value: T,
}

fn wrong() -> Box<i32, str> {
    return Box<i32> { value: 1 }
}
"#,
            "NS4018",
        );
        assert_code(
            r#"
struct Box<T> {
    value: T,
}

fn wrong() -> Box<str> {
    return Box<str> { value: 1 }
}
"#,
            "NS4001",
        );
    }

    #[test]
    fn enforces_argument_and_return_types() {
        assert_code(
            r#"
fn takes(value: i32) {}

fn caller() {
    let value: u32 = 1
    takes(value)
}
"#,
            "NS4001",
        );
        assert_code(
            r#"
fn returns() -> i32 {
    return "text"
}
"#,
            "NS4001",
        );
    }

    #[test]
    fn requires_matching_argument_counts() {
        assert_code(
            r#"
fn add(a: i32, b: i32) -> i32 {
    return a + b
}

fn call() -> i32 {
    return add(1)
}
"#,
            "NS4004",
        );
    }

    #[test]
    fn argument_count_points_at_the_declaration() {
        let diagnostics = super::check_source(
            r#"
fn add(a: i32, b: i32) -> i32 {
    return a + b
}

fn call() -> i32 {
    return add(1)
}
"#,
            super::CheckOptions::new(super::Mode::Core),
        )
        .expect_err("arity error expected");
        let arity = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "NS4004")
            .expect("NS4004 diagnostic");
        assert_eq!(arity.related.len(), 1);
        assert_eq!(arity.related[0].message, "Function defined here.");
        assert_eq!(arity.related[0].file, "");
        assert!(
            arity.related[0].span.start < arity.span.expect("call span").start,
            "declaration must precede the call"
        );
    }

    #[test]
    fn rejects_unknown_identifiers_and_types() {
        assert_code(
            r#"
fn use() -> i32 {
    return missing
}
"#,
            "NS4002",
        );
        assert_code(
            r#"
fn use(value: Missing) {}
"#,
            "NS4003",
        );
    }

    #[test]
    fn validates_unused_declaration_field_types() {
        assert_code(
            r#"
struct Broken {
    value: Missing,
}
"#,
            "NS4003",
        );
        assert_code(
            r#"
enum Broken {
    Value(Missing),
}
"#,
            "NS4003",
        );
    }

    #[test]
    fn checks_enum_exhaustiveness() {
        assert_code(
            r#"
enum Color {
    Red,
    Green,
    Blue,
}

fn value(color: Color) -> i32 {
    return match color {
        Color.Red => 1,
        Color.Green => 2,
    }
}
"#,
            "NS4010",
        );

        let result = check(
            r#"
enum Color {
    Red,
    Green,
    Blue,
}

fn value(color: Color) -> i32 {
    return match color {
        Color.Red => 1,
        _ => 0,
    }
}
"#,
        );
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn checks_bool_exhaustiveness() {
        let result = check(
            r#"
fn value(flag: bool) -> i32 {
    return match flag {
        true => 1,
        false => 0,
    }
}
"#,
        );
        assert!(result.is_ok(), "{result:?}");

        assert_code(
            r#"
fn value(flag: bool) -> i32 {
    return match flag {
        true => 1,
    }
}
"#,
            "NS4010",
        );
    }

    #[test]
    fn checks_nested_exhaustiveness() {
        assert_code(
            r#"
fn value(input: Option<Option<i32>>) -> i32 {
    return match input {
        Some(Some(number)) => number,
        None => 0,
    }
}
"#,
            "NS4010",
        );

        let result = check(
            r#"
fn value(input: Option<Option<i32>>) -> i32 {
    return match input {
        Some(Some(number)) => number,
        Some(None) => 0,
        None => 0,
    }
}
"#,
        );
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn checks_tuple_exhaustiveness() {
        let result = check(
            r#"
fn value(pair: (bool, bool)) -> i32 {
    return match pair {
        (true, _) => 1,
        (false, true) => 2,
        (false, false) => 3,
    }
}
"#,
        );
        assert!(result.is_ok(), "{result:?}");

        assert_code(
            r#"
fn value(pair: (bool, bool)) -> i32 {
    return match pair {
        (true, _) => 1,
        (_, true) => 2,
    }
}
"#,
            "NS4010",
        );
    }

    #[test]
    fn checks_multi_field_enum_exhaustiveness() {
        let result = check(
            r#"
enum Pair {
    Both(bool, bool),
}

fn value(pair: Pair) -> i32 {
    return match pair {
        Pair.Both(true, _) => 1,
        Pair.Both(false, true) => 2,
        Pair.Both(false, false) => 3,
    }
}
"#,
        );
        assert!(result.is_ok(), "{result:?}");

        assert_code(
            r#"
enum Pair {
    Both(bool, bool),
}

fn value(pair: Pair) -> i32 {
    return match pair {
        Pair.Both(true, _) => 1,
        Pair.Both(_, true) => 2,
    }
}
"#,
            "NS4010",
        );
    }

    #[test]
    fn handles_effects_with_matches() {
        let result = check(
            r#"
fn parse(input: str) -> i32 ! ParseIntError {
    return match input.to_int() {
        Ok(value) => value,
        Err(error) => 0,
    }
}
"#,
        );
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn rejects_propagation_without_an_error() {
        assert_code(
            r#"
fn bad() -> i32 {
    return (1 + 2)!
}
"#,
            "NS4013",
        );
    }

    #[test]
    fn checks_failure_construction_and_declarations() {
        let result = check(
            r#"
struct InvalidAge {
    message: str,
}

fn parse(age: i32) -> i32 ! InvalidAge {
    if age < 0 {
        return !InvalidAge { message: "negative" }
    }
    return age
}
"#,
        );
        assert!(result.is_ok(), "{result:?}");

        assert_code(
            r#"
struct InvalidAge {
    message: str,
}

fn parse(age: i32) -> i32 {
    return !InvalidAge { message: "negative" }
}
"#,
            "NS4012",
        );
    }

    #[test]
    fn checks_mutability() {
        assert_code(
            r#"
fn bad() {
    let value = 1
    value = 2
}
"#,
            "NS4009",
        );

        let result = check(
            r#"
fn good() {
    let mut value = 1
    value = 2
}
"#,
        );
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn requires_returns_on_all_paths() {
        assert_code(
            r#"
fn missing(flag: bool) -> i32 {
    if flag {
        return 1
    }
}
"#,
            "NS4007",
        );

        let result = check(
            r#"
fn complete(flag: bool) -> i32 {
    if flag {
        return 1
    } else {
        return 2
    }
}
"#,
        );
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn checks_mode_requirements_with_the_selected_mode() {
        let source = r#"
@requires(system)
module sample.main

pub fn main() {}
"#;
        assert_code(source, "NS5001");
        let result = check_with_mode(source, Mode::System);
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn checks_capability_tags() {
        assert_code(
            r#"
@tags(fs)
module sample.main

pub fn main() {}
"#,
            "NS5003",
        );

        let result = check_with_mode(
            r#"
@tags(fs)
module sample.main

pub fn main() {}
"#,
            Mode::System,
        );
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn checks_function_scope_modes() {
        let source = r#"
fn entry() {
    {@mode:system;
        let value = 1
    }
}
"#;
        assert_code(source, "NS5002");
        let result = check_with_mode(source, Mode::System);
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn rejects_manual_memory_outside_system_mode() {
        assert_code(
            r#"
fn main() {
    {@memory:manual;
        let value = 1
    }
}
"#,
            "NS5004",
        );

        let result = check_with_mode(
            r#"
fn main() {
    {@memory:manual;
        let value = 1
    }
}
"#,
            Mode::System,
        )
        .expect("system mode accepts manual memory");
        assert_eq!(result.manual_memory_scopes.len(), 1);
    }

    #[test]
    fn rejects_manual_alloc_outside_a_manual_scope() {
        // Spec 8.4: "Memory.alloc in a GC-managed scope is a compile-time
        // error" -> NS6001, reported only when the mode itself is fine.
        let outside = r#"
pub fn main() -> i32 {
    let buffer = Memory.alloc<u8>(16)
    Memory.free(buffer)
    return 0
}
"#;
        let codes = check_with_mode(outside, Mode::System)
            .expect_err("alloc outside any @memory:manual block must be rejected");
        assert!(
            codes.contains(&"NS6001".to_owned()),
            "expected NS6001, got {codes:?}"
        );

        // The block has to *contain* the call: an allocation made after the
        // manual block closes is still in a GC-managed scope.
        let after_block = r#"
pub fn main() -> i32 {
    {@memory:manual;
        let inside = Memory.alloc<u8>(4)
        Memory.free(inside)
    }
    let outside = Memory.alloc<u8>(4)
    Memory.free(outside)
    return 0
}
"#;
        let codes = check_with_mode(after_block, Mode::System)
            .expect_err("alloc after the manual block closes must be rejected");
        assert!(
            codes.contains(&"NS6001".to_owned()),
            "expected NS6001, got {codes:?}"
        );

        // Same code inside the block is accepted.
        let inside = r#"
pub fn main() -> i32 {
    {@memory:manual;
        let buffer = Memory.alloc<u8>(16)
        Memory.free(buffer)
    }
    return 0
}
"#;
        check_with_mode(inside, Mode::System).expect("alloc inside @memory:manual is allowed");

        // The item-level form is recorded by the mode pre-pass, which runs
        // before any body is checked (run(): check_mode_annotations before
        // check_functions). If that ordering ever reverses, this fails.
        let item_level = r#"
{@memory:manual;
    pub fn make() -> Ptr<u8> {
        let buffer = Memory.alloc<u8>(8)
        return buffer
    }
}
"#;
        check_with_mode(item_level, Mode::System)
            .expect("an item-level manual block accepts alloc in its functions");

        // When the mode is what fails, NS5004 is the reported condition and
        // NS6001 stays silent: one program, one violated condition.
        let codes =
            check_with_mode(outside, Mode::Core).expect_err("core mode rejects manual memory");
        assert!(
            codes.contains(&"NS5004".to_owned()) && !codes.contains(&"NS6001".to_owned()),
            "expected only NS5004, got {codes:?}"
        );
    }

    #[test]
    fn rejects_managed_element_types_in_manual_alloc() {
        // GC-001 proposal §11 (condition 2): the probe showed str, [u8],
        // Channel<T>, and structs with str fields all flowing through
        // Memory.alloc before this rule existed. NS6002 closes every form at
        // both allocation sites; the predicate is pointer-free (the inverse of
        // the GC shape analysis), so even a scalar-payload Map is rejected -
        // the map buffer itself is managed.
        let cases: &[(&str, &str)] = &[
            ("str", "Memory.alloc<str>(4)"),
            ("dynamic array", "Memory.alloc<[u8]>(4)"),
            ("map", "Memory.alloc<{i32: i32}>(2)"),
            ("channel", "Memory.alloc<Channel<i32>>(2)"),
            ("task", "Memory.alloc<Task<i32>>(2)"),
            ("option of str", "Memory.alloc<Option<str>>(2)"),
            ("tuple with str", "Memory.alloc<(i32, str)>(2)"),
        ];
        for (label, alloc) in cases {
            let source = format!(
                "pub fn main() -> i32 {{\n    {{@memory:manual;\n        let p = {alloc}\n        \
                 Memory.free(p)\n    }}\n    return 0\n}}\n"
            );
            let codes = check_with_mode(&source, Mode::System)
                .expect_err(&format!("{label}: managed element must be rejected"));
            assert!(
                codes.contains(&"NS6002".to_owned()),
                "{label}: expected NS6002, got {codes:?}"
            );
        }

        // Composite forms: struct with a managed field, a managed enum
        // payload, and a generic instantiation that substitutes to str.
        let composite = r#"
struct Rec { name: str, size: i32 }
enum Tagged { Name(str), Count(i32) }
struct Wrap<T> { value: T }
pub fn main() -> i32 {
    {@memory:manual;
        let a = Memory.alloc<Rec>(2)
        Memory.free(a)
        let b = Memory.alloc<Tagged>(2)
        Memory.free(b)
        let c = Memory.alloc<Wrap<str>>(2)
        Memory.free(c)
    }
    return 0
}
"#;
        let codes = check_with_mode(composite, Mode::System)
            .expect_err("managed composites must be rejected");
        let ns6002 = codes.iter().filter(|code| *code == "NS6002").count();
        assert_eq!(
            ns6002, 3,
            "expected three NS6002 diagnostics, got {codes:?}"
        );

        // The region site needs system mode but no @memory:manual block -
        // the type rule is independent of the scope rule.
        let region = r#"
pub fn main() -> i32 {
    let r = Region.new(64)
    let p = r.alloc<str>(4)
    r.free()
    return 0
}
"#;
        let codes = check_with_mode(region, Mode::System)
            .expect_err("region.alloc of str must be rejected");
        assert!(
            codes.contains(&"NS6002".to_owned()),
            "expected NS6002 for region.alloc<str>, got {codes:?}"
        );
        assert!(
            !codes.contains(&"NS6001".to_owned()),
            "region.alloc carries no scope rule, got {codes:?}"
        );

        // The element type may also arrive by inference from the binding's
        // annotation instead of explicit type arguments - it still reaches
        // NS6002, because the check runs once T is bound in the return type.
        let inferred = r#"
pub fn main() -> i32 {
    {@memory:manual;
        let p: Ptr<str> = Memory.alloc(4)
        Memory.free(p)
    }
    return 0
}
"#;
        let codes = check_with_mode(inferred, Mode::System)
            .expect_err("inferred str element must be rejected");
        assert!(
            codes.contains(&"NS6002".to_owned()),
            "expected NS6002 for inferred Ptr<str>, got {codes:?}"
        );
    }

    #[test]
    fn accepts_pointer_free_element_types_in_manual_alloc() {
        // The other half of the rule: everything pointer-free stays legal,
        // including Ptr<U> (a raw address is one machine word, whatever it
        // points at), structs of scalars/pointers, scalar-payload enums and
        // options, generic instantiations that substitute to scalars, tuples
        // of scalars, and fixed arrays of scalars.
        let source = r#"
struct Pair { a: i32, b: f64 }
struct Addrs { p: Ptr<u8>, n: i32 }
enum ScalarTag { A(i32), B(f64) }
struct Wrap<T> { value: T }
pub fn main() -> i32 {
    {@memory:manual;
        let a = Memory.alloc<i32>(4)
        Memory.free(a)
        let b = Memory.alloc<f64>(4)
        Memory.free(b)
        let c = Memory.alloc<Ptr<u8>>(4)
        Memory.free(c)
        let d = Memory.alloc<Pair>(2)
        Memory.free(d)
        let e = Memory.alloc<Addrs>(2)
        Memory.free(e)
        let f = Memory.alloc<ScalarTag>(2)
        Memory.free(f)
        let g = Memory.alloc<Wrap<i32>>(2)
        Memory.free(g)
        let h = Memory.alloc<Option<i32>>(2)
        Memory.free(h)
        let i = Memory.alloc<(i32, i32)>(2)
        Memory.free(i)
        let j = Memory.alloc<[i32; 4]>(2)
        Memory.free(j)
    }
    let r = Region.new(64)
    let k = r.alloc<i32>(4)
    r.free()
    return 0
}
"#;
        check_with_mode(source, Mode::System)
            .expect("pointer-free element types are allowed in manual memory");
    }

    #[test]
    fn ns6002_reports_only_after_mode_and_scope_hold() {
        // Same one-program-one-condition ordering as NS5004/NS6001: NS6002
        // speaks only once the earlier gates are satisfied.
        let source = r#"
pub fn main() -> i32 {
    {@memory:manual;
        let p = Memory.alloc<str>(4)
        Memory.free(p)
    }
    return 0
}
"#;
        let codes =
            check_with_mode(source, Mode::Core).expect_err("core mode rejects manual memory");
        assert!(
            codes.contains(&"NS5004".to_owned())
                && !codes.contains(&"NS6001".to_owned())
                && !codes.contains(&"NS6002".to_owned()),
            "expected only NS5004, got {codes:?}"
        );

        let outside_scope = r#"
pub fn main() -> i32 {
    let p = Memory.alloc<str>(4)
    Memory.free(p)
    return 0
}
"#;
        let codes = check_with_mode(outside_scope, Mode::System)
            .expect_err("system mode without a manual scope rejects alloc");
        assert!(
            codes.contains(&"NS6001".to_owned()) && !codes.contains(&"NS6002".to_owned()),
            "expected only NS6001, got {codes:?}"
        );
    }

    #[test]
    fn region_api_type_checks_in_system_mode() {
        // Spec 8.4's Region surface: one fixed block carved bump-style. The
        // scope rule names only `Memory.alloc`, so Region use needs system
        // mode but no `@memory:manual` block - that boundary is deliberate,
        // not an oversight (see the NS6001 entry in Docs/archive/Agent/todo.md).
        let source = r#"
pub fn main() -> i32 {
    let region = Region.new(64)
    let first = region.alloc<i32>(2)
    first.write(0, 20)
    let second = region.alloc<u8>(4)
    second.write(0, 7)
    region.free()
    return 0
}
"#;
        check_with_mode(source, Mode::System).expect("system mode accepts the Region API");

        // `Region` is a non-generic handle and resolves in type position.
        let annotated = r#"
pub fn main() -> i32 {
    let region: Region = Region.new(16)
    region.free()
    return 0
}
"#;
        check_with_mode(annotated, Mode::System).expect("Region resolves as an annotation");

        // `Region<T>` is rejected: the element type belongs at the allocation
        // site (`region.alloc<T>`), not on the handle.
        let generic = "pub fn f(r: Region<u8>) { r.free() }\n";
        check_with_mode(generic, Mode::System)
            .expect_err("Region<u8> should be rejected as a type-argument error");
    }

    #[test]
    fn rejects_region_api_outside_system_mode() {
        // `Region.new` is manual memory, so a core-mode program is rejected
        // with NS5004 before the backend ever sees it.
        let new_source =
            "pub fn main() -> i32 { let r = Region.new(16)\n    r.free()\n    return 0 }\n";
        let codes = check_with_mode(new_source, Mode::Core)
            .expect_err("core mode should reject Region.new");
        assert!(
            codes.contains(&"NS5004".to_owned()),
            "expected NS5004 for Region.new in core mode, got {codes:?}"
        );

        // The gate is on region *usage*, not only construction: a core-mode
        // function that merely receives a Region and allocates from it (the
        // same shape as the pointer-access gate) is rejected too.
        let carve = "pub fn carve(r: Region, count: usize) -> Ptr<u8> {\n    return r.alloc<u8>(count)\n}\n";
        let codes = check_with_mode(carve, Mode::Core)
            .expect_err("core mode should reject region allocation");
        assert!(
            codes.contains(&"NS5004".to_owned()),
            "expected NS5004 for region.alloc in core mode, got {codes:?}"
        );

        let release = "pub fn release(r: Region) { r.free() }\n";
        let codes =
            check_with_mode(release, Mode::Core).expect_err("core mode should reject region.free");
        assert!(
            codes.contains(&"NS5004".to_owned()),
            "expected NS5004 for region.free in core mode, got {codes:?}"
        );
    }

    #[test]
    fn infers_spawn_task_result_types() {
        let source = r#"
fn main() {
    let task = spawn || {
        21
    }
    let value: i32 = task.join()
}
"#;
        assert!(check(source).is_ok());

        assert_code(
            r#"
fn main() {
    let task = spawn || {
        21
    }
    let value: str = task.join()
}
"#,
            "NS4001",
        );
    }

    #[test]
    fn accepts_channel_send_receive_and_close() {
        let source = r#"
fn produce(channel: Channel<str>) -> () ! ChannelError {
    channel.send("hello")!
}

fn consume(channel: Channel<str>) -> () {
    match channel.receive() {
        Ok(message) => println(message),
        Err(error) => println("closed"),
    }
    channel.close()
}

fn main() -> i32 {
    let channel = Channel.create<str>()
    match produce(channel) {
        Ok(value) => {},
        Err(error) => {},
    }
    consume(channel)
    return 0
}
"#;
        assert!(check(source).is_ok(), "{source}");
    }

    #[test]
    fn rejects_unknown_channel_members() {
        assert_code(
            r#"
fn main() -> i32 {
    let channel = Channel.create<str>()
    channel.peek()
    return 0
}
"#,
            "NS4015",
        );
    }

    #[test]
    fn rejects_task_handles_in_channel_elements() {
        // A task record names a thread handle and its owner's heap; copying it
        // into a message would let two owners join the same thread.
        for element in [
            "Task<i32>",
            "Option<Task<i32>>",
            "[Task<i32>]",
            "Channel<Task<i32>>",
        ] {
            let source = format!(
                "pub fn main() -> i32 {{\n    let channel = Channel.create<{element}>()\n    return 0\n}}\n"
            );
            assert_code(&source, "NS4019");
        }

        assert_code(
            r#"
struct Holder { task: Task<i32> }
enum MaybeTask { SomeTask(Task<i32>), None }
pub fn main() -> i32 {
    let a = Channel.create<Holder>()
    let b = Channel.create<MaybeTask>()
    return 0
}
"#,
            "NS4019",
        );

        // A channel of plain values is still accepted.
        assert!(check("pub fn main() -> i32 {\n    let channel = Channel.create<bytes>()\n    return 0\n}\n").is_ok());
    }

    #[test]
    fn rejects_undeclared_channel_errors() {
        assert_code(
            r#"
fn produce(channel: Channel<str>) -> () {
    channel.send("hello")!
}
"#,
            "NS4012",
        );
    }

    #[test]
    fn checks_string_byte_operations() {
        let source = r#"
fn describe(value: str) -> u8 {
    let count = value.char_count()
    let raw = value.bytes()
    let characters = value.chars()
    let first = match value.byte_at(0) {
        Some(byte) => byte,
        None => 0,
    }
    let letter = match value.char_at(0) {
        Some(character) => character,
        None => "",
    }
    let text = match value.slice(0, 1) {
        Ok(part) => part,
        Err(error) => "",
    }
    return first
}
"#;
        assert!(check(source).is_ok(), "{source}");

        assert_code(
            r#"
fn bad(value: str) -> str {
    return value.char_count()
}
"#,
            "NS4001",
        );
    }

    #[test]
    fn rejects_unknown_string_operations() {
        assert_code(
            r#"
fn bad(value: str) -> str {
    return value.klingon()
}
"#,
            "NS4015",
        );
    }

    #[test]
    fn rejects_unhandled_slice_errors() {
        assert_code(
            r#"
fn bad(value: str) -> str {
    let text = value.slice(0, 1)
    return text
}
"#,
            "NS4011",
        );
    }

    #[test]
    fn checks_bytes_buffers_and_indexed_writes() {
        let source = r#"
fn fill() -> u8 {
    let mut buffer = bytes.new(4)
    buffer[0] = 255
    buffer[1] = 7
    let index: usize = 2
    buffer[index] = 9
    let size = buffer.len()
    return buffer[0]
}
"#;
        assert!(check(source).is_ok(), "{source}");

        assert_code(
            r#"
fn bad() {
    let mut buffer = bytes.new(2)
    buffer[0] = true
}
"#,
            "NS4001",
        );

        assert_code(
            r#"
fn bad() {
    let mut buffer = bytes.new(2)
    buffer["x"] = 1
}
"#,
            "NS4001",
        );
    }

    #[test]
    fn rejects_unknown_bytes_members() {
        assert_code(
            r#"
fn bad() -> i32 {
    let buffer = bytes.new(2)
    return buffer.swap()
}
"#,
            "NS4015",
        );
    }

    #[test]
    fn hybrid_ns6002_flags_generic_forwarding() {
        let source = r#"
fn alloc_wrap<T>(n: usize) -> i32 {
    {@memory:manual;
        let p = Memory.alloc<T>(n)
        Memory.free(p)
    }
    return 0
}

pub fn main() -> i32 {
    return alloc_wrap<str>(4)
}
"#;
        let codes = check_with_mode(source, Mode::System)
            .expect_err("generic forwarding of managed type must be rejected");
        assert!(
            codes.contains(&"NS6002".to_owned()),
            "expected NS6002 at instantiation site, got {codes:?}"
        );
    }

    #[test]
    fn hybrid_ns6002_flags_two_hop_forwarding() {
        let source = r#"
fn inner<T>(n: usize) -> i32 {
    {@memory:manual;
        let p = Memory.alloc<T>(n)
        Memory.free(p)
    }
    return 0
}

fn outer<T>(n: usize) -> i32 {
    return inner<T>(n)
}

pub fn main() -> i32 {
    return outer<str>(4)
}
"#;
        let codes = check_with_mode(source, Mode::System)
            .expect_err("two-hop generic forwarding of managed type must be rejected");
        assert!(
            codes.contains(&"NS6002".to_owned()),
            "expected NS6002 at outermost instantiation, got {codes:?}"
        );
    }

    #[test]
    fn hybrid_ns6002_check_callee_explicit_type_arg() {
        let source = r#"
fn g<T>(x: T) -> T { return x }

fn f<U>(x: U) -> U {
    return g<U>(x)
}

pub fn main() -> i32 {
    let v = f<i32>(4)
    return v
}
"#;
        check_with_mode(source, Mode::System).expect("explicit type arg forwarding must typecheck");
    }
    #[test]
    fn hybrid_ns6002_accepts_pointer_free_instantiation() {
        let source = r#"
fn alloc_wrap<T>(n: usize) -> i32 {
    {@memory:manual;
        let p = Memory.alloc<T>(n)
        Memory.free(p)
    }
    return 0
}

pub fn main() -> i32 {
    return alloc_wrap<i32>(4)
}
"#;
        check_with_mode(source, Mode::System).expect("pointer-free instantiation must be accepted");
    }

    #[test]
    fn native_order_rejects_two_calls_in_binary_operands() {
        let source = r#"
fn side(x: i32) -> i32 { return x }
pub fn main() -> i32 { return side(1) + side(2) }
"#;
        let codes = check_with_mode(source, Mode::System)
            .expect_err("two calls in binary operands must be rejected");
        assert!(
            codes.contains(&"NS5007".to_owned()),
            "expected NS5007, got {codes:?}"
        );
    }

    #[test]
    fn native_order_accepts_one_call_plus_literal() {
        let source = r#"
fn side(x: i32) -> i32 { return x }
pub fn main() -> i32 { return side(1) + 2 }
"#;
        check_with_mode(source, Mode::System).expect("one call is order-independent");
    }

    #[test]
    fn native_order_rejects_two_calls_in_array_literal() {
        let source = r#"
fn side(x: i32) -> i32 { return x }
pub fn main() -> i32 {
    let xs = [side(1), side(2)]
    return xs.len()
}
"#;
        let codes = check_with_mode(source, Mode::System)
            .expect_err("two calls in an array literal must be rejected");
        assert!(codes.contains(&"NS5007".to_owned()), "got {codes:?}");
    }

    #[test]
    fn native_order_rejects_two_calls_in_struct_fields() {
        let source = r#"
struct Pair { a: i32, b: i32 }
fn side(x: i32) -> i32 { return x }
pub fn main() -> i32 {
    let p = Pair { a: side(1), b: side(2) }
    return p.a + p.b
}
"#;
        let codes = check_with_mode(source, Mode::System)
            .expect_err("two calls in struct fields must be rejected");
        assert!(codes.contains(&"NS5007".to_owned()), "got {codes:?}");
    }

    #[test]
    fn native_order_rejects_two_calls_in_map_entries() {
        let source = r#"
fn side(x: i32) -> i32 { return x }
pub fn main() -> i32 {
    let m: {str: i32} = {"a": side(1), "b": side(2)}
    return 0
}
"#;
        let codes = check_with_mode(source, Mode::System)
            .expect_err("two calls in map entries must be rejected");
        assert!(codes.contains(&"NS5007".to_owned()), "got {codes:?}");
    }

    #[test]
    fn native_order_accepts_call_arguments() {
        let source = r#"
fn side(x: i32) -> i32 { return x }
fn add(a: i32, b: i32) -> i32 { return a + b }
pub fn main() -> i32 { return add(side(1), side(2)) }
"#;
        check_with_mode(source, Mode::System).expect("call arguments are ordered");
    }

    #[test]
    fn native_order_is_system_mode_only() {
        let source = r#"
fn side(x: i32) -> i32 { return x }
pub fn main() -> i32 { return side(1) + side(2) }
"#;
        check_with_mode(source, Mode::Core).expect("core mode keeps target-native order");
    }

    #[test]
    fn native_order_reports_once_for_nested_operators() {
        let source = r#"
fn side(x: i32) -> i32 { return x }
pub fn main() -> i32 { return (side(1) + side(2)) + side(3) }
"#;
        let codes = check_with_mode(source, Mode::System).expect_err("nested operands rejected");
        assert_eq!(
            codes
                .iter()
                .filter(|code| code.as_str() == "NS5007")
                .count(),
            1,
            "expected exactly one NS5007, got {codes:?}"
        );
    }

    #[test]
    fn export_as_rejects_non_generic_and_non_exported_functions() {
        let non_generic = r#"
@export_as("ns_demo_f", T = i32)
pub fn f() -> i32 { return 0 }
"#;
        let codes = check_with_mode(non_generic, Mode::System)
            .expect_err("non-generic @export_as must be rejected");
        assert!(codes.contains(&"NS7010".to_owned()), "got {codes:?}");

        let non_exported = r#"
@export_as("ns_demo_f", T = i32)
fn f<T>(x: T) -> T { return x }
"#;
        let codes = check_with_mode(non_exported, Mode::System)
            .expect_err("non-exported @export_as must be rejected");
        assert!(codes.contains(&"NS7010".to_owned()), "got {codes:?}");
    }

    #[test]
    fn export_as_rejects_bad_bindings() {
        let missing = r#"
@export_as("ns_demo_f")
pub fn f<T>(x: T) -> T { return x }
"#;
        let codes =
            check_with_mode(missing, Mode::System).expect_err("missing binding must be rejected");
        assert!(codes.contains(&"NS7011".to_owned()), "got {codes:?}");

        let unknown = r#"
@export_as("ns_demo_f", U = i32)
pub fn f<T>(x: T) -> T { return x }
"#;
        let codes =
            check_with_mode(unknown, Mode::System).expect_err("unknown binding must be rejected");
        assert!(codes.contains(&"NS7011".to_owned()), "got {codes:?}");
    }

    #[test]
    fn export_as_rejects_non_boundary_safe_bindings() {
        let source = r#"
@export_as("ns_demo_f", T = {str: i32})
pub fn f<T>(x: T) -> T { return x }
"#;
        let codes = check_with_mode(source, Mode::System)
            .expect_err("pointer binding must be rejected as not boundary-safe");
        assert!(codes.contains(&"NS7012".to_owned()), "got {codes:?}");
    }

    #[test]
    fn export_as_accepts_valid_instantiations_and_rejects_duplicates() {
        let valid = r#"
@export_as("ns_demo_id_i32", T = i32)
@export_as("ns_demo_id_str", T = str)
pub fn identity<T>(value: T) -> T { return value }
"#;
        check_with_mode(valid, Mode::System).expect("valid instantiations must typecheck");

        let duplicate = r#"
@export_as("ns_demo_id_i32", T = i32)
@export_as("ns_demo_id_i32", T = str)
pub fn identity<T>(value: T) -> T { return value }
"#;
        let codes = check_with_mode(duplicate, Mode::System)
            .expect_err("duplicate symbol must be rejected");
        assert!(codes.contains(&"NS7013".to_owned()), "got {codes:?}");

        let invalid_symbol = r#"
@export_as("1bad", T = i32)
pub fn identity<T>(value: T) -> T { return value }
"#;
        let codes = check_with_mode(invalid_symbol, Mode::System)
            .expect_err("invalid C identifier must be rejected");
        assert!(codes.contains(&"NS7013".to_owned()), "got {codes:?}");
    }

    #[test]
    fn export_as_rejects_cross_declaration_symbol_reuse() {
        let source = r#"
@export_as("ns_demo_shared", T = i32)
pub fn first<T>(value: T) -> T { return value }

@export_as("ns_demo_shared", T = str)
pub fn second<T>(value: T) -> T { return value }
"#;
        let codes = check_with_mode(source, Mode::System)
            .expect_err("a symbol shared by two declarations must be rejected");
        assert!(codes.contains(&"NS7013".to_owned()), "got {codes:?}");
    }
    #[test]
    fn spawn_capture_channel_accepted() {
        let source = r#"
fn f() -> i32 {
    let ch = Channel.create<i32>()
    spawn || {
        ch
        return 0
    }
    return 0
}
"#;
        check(source).expect("channel capture must be accepted");
    }

    #[test]
    fn spawn_capture_scalar_accepted() {
        let source = r#"
fn f() -> i32 {
    let x = 42
    spawn || {
        x
        return 0
    }
    return 0
}
"#;
        check(source).expect("scalar capture must be accepted");
    }

    #[test]
    fn spawn_capture_struct_without_task_accepted() {
        let source = r#"
struct Holder { value: i32 }
fn f() -> i32 {
        let h = Holder { value: 1 }
    spawn || {
        h.value
        return 0
    }
    return 0
}
"#;
        check(source).expect("struct-without-task capture must be accepted");
    }

    #[test]
    fn spawn_return_struct_without_task_accepted() {
        let source = r#"
struct Holder { value: i32 }
fn f() -> i32 {
    spawn || {
        Holder { value: 1 }
    }
    return 0
}
"#;
        check(source).expect("struct-without-task result must be accepted");
    }

    #[test]
    fn spawn_capture_task_rejected() {
        let source = r#"
fn f() -> i32 {
    let t = spawn || { return 0 }
    spawn || {
        t
        return 0
    }
    return 0
}
"#;
        let codes = check(source).expect_err("task capture must be rejected");
        assert!(codes.contains(&"NS4019".to_owned()), "got {codes:?}");
    }

    #[test]
    fn spawn_capture_composite_task_rejected() {
        let source = r#"
struct Holder { task: Task<i32> }
fn f() -> i32 {
        let h = Holder { task: spawn || { return 0 } }
    spawn || {
        h
        return 0
    }
    return 0
}
"#;
        let codes = check(source).expect_err("composite task capture must be rejected");
        assert!(codes.contains(&"NS4019".to_owned()), "got {codes:?}");
    }

    #[test]
    fn spawn_result_task_rejected() {
        let source = r#"
fn f() -> i32 {
    spawn || {
        spawn || { return 0 }
    }
    return 0
}
"#;
        let codes = check(source).expect_err("task result must be rejected");
        assert!(codes.contains(&"NS4019".to_owned()), "got {codes:?}");
    }

    #[test]
    fn spawn_result_composite_task_rejected() {
        let source = r#"
struct Holder { task: Task<i32> }
fn f() -> i32 {
    spawn || {
        Holder { task: spawn || { return 0 } }
    }
    return 0
}
"#;
        let codes = check(source).expect_err("composite task result must be rejected");
        assert!(codes.contains(&"NS4019".to_owned()), "got {codes:?}");
    }

    #[test]
    fn spawn_shadowed_non_task_binding_does_not_hide_outer_task_capture() {
        let source = r#"
fn f() -> i32 {
    let task = spawn || { 0 }
    spawn || {
        {
            let task = 1
            task
        }
        task.join()
    }
    return 0
}
"#;
        let codes = check(source)
            .expect_err("an outer task referenced after an inner shadowing block must be rejected");
        assert!(codes.contains(&"NS4019".to_owned()), "got {codes:?}");
    }

    #[test]
    fn spawn_shadowed_non_task_capture_is_accepted() {
        let source = r#"
fn f() -> i32 {
    let outer = spawn || { 0 }
    spawn || {
        let outer = 1
        outer
    }
    return outer.join()
}
"#;
        // The inner spawn's body binds its own `outer` (i32), so it does not capture the outer Task handle. The outer handle is joined after both spawns and must remain legal.
        check(source).expect("a shadowed non-task capture must be accepted");
    }
}
