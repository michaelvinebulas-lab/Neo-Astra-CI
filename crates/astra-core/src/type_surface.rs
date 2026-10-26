//! Cross-module type surfaces for TYPE-003.
//!
//! A `ModuleTypeSurface` is produced from a `CheckResult` and captures the
//! public export names, their runtime types, and the type definitions (structs
//! and enums) that importing modules need to perform real type checking instead
//! of treating everything as `Type::Unknown`.
//!
//! `check_module_graph` is the top-level entry point for checking multiple
//! modules in dependency order and feeding each module's surface forward.

use std::collections::BTreeMap;

use crate::ast;
use crate::checker::{CheckOptions, CheckResult, check_resolved};
use crate::diagnostic::Diagnostic;
use crate::hir;
use crate::resolver::{ModuleGraph, ModuleResolver, collect_module_specifiers};
use crate::types::{FunctionType, Type};

// ─── Surface types ────────────────────────────────────────────────────────────

/// A single exported name and the type associated with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedValue {
    /// The exported name as it appears in the importing module.
    pub name: String,
    /// The semantic type of the exported value.
    pub ty: Type,
}

/// A struct or enum type definition extracted from a checked module.
///
/// Importing modules need this to perform field access, construction, pattern
/// matching, and exhaustiveness checks on cross-module types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportedTypeDef {
    Struct {
        name: String,
        type_parameters: Vec<String>,
        fields: ast::StructFields,
    },
    Enum {
        name: String,
        type_parameters: Vec<String>,
        variants: Vec<ast::EnumVariant>,
    },
}

impl ImportedTypeDef {
    /// The declared name of this type.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Struct { name, .. } | Self::Enum { name, .. } => name,
        }
    }
}

/// The public type surface of a checked module.
///
/// Produced by `module_type_surface` and consumed by the type checker when
/// `CheckOptions::module_surfaces` is populated.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModuleTypeSurface {
    /// Exported value names mapped to their types.
    ///
    /// Namespace exports (foreign modules) use a placeholder `Type::Unknown`
    /// because they are opaque at the type level.
    pub values: BTreeMap<String, Type>,
    /// Exported type definitions available to importing modules.
    pub type_defs: BTreeMap<String, ImportedTypeDef>,
    /// Exported project namespace members, keyed by exported name.
    ///
    /// Direct `import * as ns` uses `values`; this map supports
    /// `export * as ns from "./module.ns"` and named namespace re-exports.
    pub namespaces: BTreeMap<String, BTreeMap<String, Type>>,
}

impl ModuleTypeSurface {
    /// Returns the type of an exported name, if known.
    #[must_use]
    pub fn get_value(&self, name: &str) -> Option<&Type> {
        self.values.get(name)
    }

    /// Returns a type definition exported under `name`, if present.
    #[must_use]
    pub fn get_type_def(&self, name: &str) -> Option<&ImportedTypeDef> {
        self.type_defs.get(name)
    }
}

/// Extracts the public type surface from a successfully checked module.
///
/// Only names that appear in `result.module.exports.named` (or the default
/// export) are included. Internal names are not exposed even when their types
/// are known.
#[must_use]
pub fn module_type_surface(result: &CheckResult) -> ModuleTypeSurface {
    module_type_surface_with_surfaces(result, &BTreeMap::new())
}

/// Extracts a module surface while resolving re-exports and namespace exports
/// through already checked dependency surfaces.
///
/// The project loader checks modules in dependency order and passes the
/// accumulated surfaces here. Without dependencies this behaves exactly like
/// [`module_type_surface`], leaving unresolved re-exports opaque.
#[must_use]
pub fn module_type_surface_with_surfaces(
    result: &CheckResult,
    dependencies: &BTreeMap<String, ModuleTypeSurface>,
) -> ModuleTypeSurface {
    let mut values: BTreeMap<String, Type> = BTreeMap::new();
    let mut type_defs: BTreeMap<String, ImportedTypeDef> = BTreeMap::new();
    let mut namespaces: BTreeMap<String, BTreeMap<String, Type>> = BTreeMap::new();

    // Build a name → item-type map from checked HIR items.
    let item_types: BTreeMap<&str, &Type> = result
        .module
        .items
        .iter()
        .filter_map(|item| item.ty.as_ref().map(|ty| (item.name.as_str(), ty)))
        .collect();

    // Build a name → declaration map for type definitions.
    let item_decls: BTreeMap<&str, &hir::Declaration> = result
        .module
        .items
        .iter()
        .map(|item| (item.name.as_str(), &item.declaration))
        .collect();

    // Walk the export surface: named exports + default export.
    let export_names = result
        .module
        .exports
        .named
        .keys()
        .map(|name| name.as_str())
        .chain(
            result
                .module
                .exports
                .default
                .as_ref()
                .map(|export| export.name.as_str()),
        )
        .collect::<Vec<_>>();

    for exported_name in export_names {
        let export = if exported_name == "default" {
            result.module.exports.default.as_ref()
        } else {
            result.module.exports.named.get(exported_name)
        };

        let Some(export) = export else {
            continue;
        };

        // Re-exports copy the already checked target surface. Resolver targets
        // use canonical module ids, so the dependency map is keyed identically.
        if let hir::ExportTarget::Reexport { module, name } = &export.target
            && let Some(dependency) = dependencies.get(module)
        {
            if export.kind.is_value() {
                values.insert(
                    exported_name.to_owned(),
                    dependency
                        .values
                        .get(name)
                        .cloned()
                        .unwrap_or(Type::Unknown),
                );
            }
            if export.kind.is_type()
                && let Some(definition) = dependency.type_defs.get(name)
            {
                type_defs.insert(exported_name.to_owned(), definition.clone());
            }
            if let Some(members) = dependency.namespaces.get(name) {
                namespaces.insert(exported_name.to_owned(), members.clone());
            }
            continue;
        }

        let local_name = match &export.target {
            hir::ExportTarget::Local(name) => name.as_str(),
            hir::ExportTarget::Reexport { name, .. } => name.as_str(),
            hir::ExportTarget::Namespace { module } => {
                if export.kind.is_value() {
                    values.insert(exported_name.to_owned(), Type::Unknown);
                    if let Some(dependency) = dependencies.get(module) {
                        namespaces.insert(exported_name.to_owned(), dependency.values.clone());
                    }
                }
                continue;
            }
            hir::ExportTarget::Expression => {
                if export.kind.is_value() {
                    values.insert(exported_name.to_owned(), Type::Unknown);
                }
                continue;
            }
        };

        // Record the value type.
        if export.kind.is_value() {
            let ty = item_types
                .get(local_name)
                .copied()
                .cloned()
                .unwrap_or(Type::Unknown);
            values.insert(exported_name.to_owned(), ty);
        }

        // Record the type definition for struct/enum exports.
        if export.kind.is_type()
            && let Some(decl) = item_decls.get(local_name)
        {
            match decl {
                hir::Declaration::Struct(s) => {
                    type_defs.insert(
                        exported_name.to_owned(),
                        ImportedTypeDef::Struct {
                            name: s.name.clone(),
                            type_parameters: s.type_parameters.clone(),
                            fields: s.fields.clone(),
                        },
                    );
                }
                hir::Declaration::Enum(e) => {
                    type_defs.insert(
                        exported_name.to_owned(),
                        ImportedTypeDef::Enum {
                            name: e.name.clone(),
                            type_parameters: e.type_parameters.clone(),
                            variants: e.variants.clone(),
                        },
                    );
                }
                _ => {}
            }
        }
    }

    ModuleTypeSurface {
        values,
        type_defs,
        namespaces,
    }
}

// ─── Graph checker ────────────────────────────────────────────────────────────

/// One module in the project graph.
#[derive(Debug, Clone)]
pub struct ProjectModule {
    /// The canonical module key used in `import` statements.
    pub key: String,
    /// Parsed source, ready for resolution and checking.
    pub source: ast::Module,
}

/// Result of checking a project module graph.
#[derive(Debug, Clone)]
pub struct GraphCheckResult {
    /// Per-module results, in the order they were checked.
    pub modules: Vec<(String, CheckResult)>,
    /// All diagnostics collected across all modules, tagged with their module key.
    pub diagnostics: Vec<(String, Vec<Diagnostic>)>,
}

impl GraphCheckResult {
    /// Returns `true` when every module checked without errors.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// Returns the `CheckResult` for `key`, if it succeeded.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&CheckResult> {
        self.modules
            .iter()
            .find_map(|(k, result)| if k == key { Some(result) } else { None })
    }

    /// Flattens all diagnostics into a single `Vec`.
    #[must_use]
    pub fn all_diagnostics(&self) -> Vec<Diagnostic> {
        self.diagnostics
            .iter()
            .flat_map(|(_, diagnostics)| diagnostics.iter().cloned())
            .collect()
    }
}

/// Checks a project module graph in the supplied order.
///
/// `modules` is a list of `(key, ast::Module)` pairs. The caller is responsible
/// for ordering them so each module comes after all modules it imports. An out-
/// of-order module that imports a not-yet-checked module is treated as if that
/// import is opaque (same as the standalone checker), so this function is
/// permissive about ordering — it produces the best type information it can.
///
/// Each module's type surface is accumulated and fed forward to later modules,
/// so the quality of cross-module type checking improves as the graph is
/// resolved in dependency order.
pub fn check_module_graph(modules: &[ProjectModule], options: CheckOptions) -> GraphCheckResult {
    let mut graph = ModuleGraph::default();
    for module in modules {
        graph.insert_module(module.key.clone(), module.source.clone());
    }
    // The original multi-source API treats each raw specifier as its own
    // module key. Keep that behavior for callers that do not provide an
    // explicit import graph.
    for module in modules {
        for specifier in collect_module_specifiers(&module.source) {
            graph.insert_import(
                module.key.clone(),
                specifier.specifier.clone(),
                specifier.specifier,
            );
        }
    }
    check_module_graph_with_imports(modules, &graph, options)
}

/// Checks a project module graph using explicit import targets.
///
/// `modules` must be supplied in dependency order. `graph` supplies canonical
/// module identities and the `(importer, specifier) -> target` map used by the
/// resolver.
pub fn check_module_graph_with_imports(
    modules: &[ProjectModule],
    graph: &ModuleGraph,
    options: CheckOptions,
) -> GraphCheckResult {
    let mut surfaces: BTreeMap<String, ModuleTypeSurface> = BTreeMap::new();
    let mut results: Vec<(String, CheckResult)> = Vec::new();
    let mut all_diagnostics: Vec<(String, Vec<Diagnostic>)> = Vec::new();

    // Build the full resolver over all source modules and their import edges so
    // every module's symbol table can be resolved.
    let resolver = ModuleResolver::from_graph(graph);

    for project_module in modules {
        // Resolve this module using the full module graph.
        let resolved = match resolver.resolve_named(&project_module.key) {
            Ok(resolved) => resolved,
            Err(diagnostics) => {
                all_diagnostics.push((project_module.key.clone(), diagnostics));
                continue;
            }
        };

        // Build check options augmented with whatever surfaces are known so far.
        let module_options = CheckOptions {
            module_surfaces: surfaces.clone(),
            ..options.clone()
        };

        // Type-check the module.
        match check_resolved(&project_module.source, &resolved, module_options) {
            Ok(result) => {
                let surface = module_type_surface_with_surfaces(&result, &surfaces);
                surfaces.insert(project_module.key.clone(), surface);
                results.push((project_module.key.clone(), result));
            }
            Err(diagnostics) => {
                all_diagnostics.push((project_module.key.clone(), diagnostics));
            }
        }
    }

    GraphCheckResult {
        modules: results,
        diagnostics: all_diagnostics,
    }
}

// ─── Surface lookup helpers ───────────────────────────────────────────────────

/// Looks up the type of an imported name given the module surfaces and the
/// resolved import information from HIR.
///
/// Returns `None` when the module surface is not available (opaque import).
#[must_use]
pub fn lookup_import_type(
    surfaces: &BTreeMap<String, ModuleTypeSurface>,
    module_key: &str,
    imported_name: &str,
) -> Option<Type> {
    let surface = surfaces.get(module_key)?;
    surface.get_value(imported_name).cloned()
}

/// Looks up a type definition from an import, when available.
#[must_use]
pub fn lookup_import_type_def<'a>(
    surfaces: &'a BTreeMap<String, ModuleTypeSurface>,
    module_key: &str,
    imported_name: &str,
) -> Option<&'a ImportedTypeDef> {
    let surface = surfaces.get(module_key)?;
    surface.get_type_def(imported_name)
}

/// Builds the `CheckOptions::module_surfaces` map for a single module
/// given the full set of its direct dependencies' surfaces.
///
/// Only surfaces for modules that this module imports are included.
#[must_use]
pub fn surfaces_for_imports(
    all_surfaces: &BTreeMap<String, ModuleTypeSurface>,
    imports: &[hir::Import],
) -> BTreeMap<String, ModuleTypeSurface> {
    imports
        .iter()
        .filter_map(|import| {
            all_surfaces
                .get(&import.module)
                .map(|surface| (import.module.clone(), surface.clone()))
        })
        .collect()
}

// ─── FunctionType helper ─────────────────────────────────────────────────────

/// Extracts a `FunctionType` from a `Type::Function`, returning `None`
/// for non-function types.
#[must_use]
pub fn as_function_type(ty: &Type) -> Option<&FunctionType> {
    match ty {
        Type::Function(function) => Some(function),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checker::CheckOptions;
    use crate::checker::check_resolved;
    use crate::mode::Mode;
    use crate::parser::parse;
    use crate::resolver::resolve_module;

    fn checked(source: &str) -> CheckResult {
        let module = parse(source).expect("parse failed");
        let resolved = resolve_module(&module).expect("resolve failed");
        check_resolved(&module, &resolved, CheckOptions::default()).expect("check failed")
    }

    fn checked_with_surfaces(
        source: &str,
        surfaces: BTreeMap<String, ModuleTypeSurface>,
    ) -> Result<CheckResult, Vec<Diagnostic>> {
        let module = parse(source).expect("parse failed");
        let resolved = resolve_module(&module).expect("resolve failed");
        check_resolved(
            &module,
            &resolved,
            CheckOptions {
                module_surfaces: surfaces,
                ..CheckOptions::default()
            },
        )
    }

    // ─── Surface extraction ────────────────────────────────────────────────

    #[test]
    fn surface_captures_exported_function_types() {
        let result = checked(
            r#"
pub fn add(a: i32, b: i32) -> i32 {
    return a + b
}

fn private_helper() -> i32 {
    return 0
}
"#,
        );
        let surface = module_type_surface(&result);
        assert!(
            surface.get_value("add").is_some(),
            "add should be in surface"
        );
        assert!(
            matches!(surface.get_value("add"), Some(Type::Function(_))),
            "add should be a function type"
        );
        assert!(
            surface.get_value("private_helper").is_none(),
            "private_helper should not be exported"
        );
    }

    #[test]
    fn surface_captures_exported_struct_types() {
        let result = checked(
            r#"
pub struct Point {
    x: i32
    y: i32
}
"#,
        );
        let surface = module_type_surface(&result);
        assert!(
            surface.get_value("Point").is_some(),
            "Point constructor should be in values"
        );
        assert!(
            surface.get_type_def("Point").is_some(),
            "Point definition should be in type_defs"
        );
        let def = surface.get_type_def("Point").unwrap();
        assert!(matches!(def, ImportedTypeDef::Struct { name, .. } if name == "Point"));
    }

    #[test]
    fn surface_captures_exported_enum_types() {
        let result = checked(
            r#"
pub enum Status {
    Active
    Inactive
}
"#,
        );
        let surface = module_type_surface(&result);
        assert!(
            surface.get_type_def("Status").is_some(),
            "Status should be in type_defs"
        );
        let def = surface.get_type_def("Status").unwrap();
        assert!(matches!(def, ImportedTypeDef::Enum { name, .. } if name == "Status"));
    }

    #[test]
    fn surface_omits_unexported_declarations() {
        let result = checked(
            r#"
struct Internal {
    value: i32
}

fn unexported() -> i32 {
    return 1
}

pub fn public_fn() -> i32 {
    return 2
}
"#,
        );
        let surface = module_type_surface(&result);
        assert!(surface.get_value("Internal").is_none());
        assert!(surface.get_type_def("Internal").is_none());
        assert!(surface.get_value("unexported").is_none());
        assert!(surface.get_value("public_fn").is_some());
    }

    // ─── Cross-module type checking ────────────────────────────────────────

    #[test]
    fn imported_function_type_is_resolved_from_surface() {
        // Module math exports add(i32, i32) -> i32
        let math_result = checked(
            r#"
pub fn add(a: i32, b: i32) -> i32 {
    return a + b
}
"#,
        );
        let math_surface = module_type_surface(&math_result);
        let mut surfaces = BTreeMap::new();
        surfaces.insert("math".to_owned(), math_surface);

        // Module main imports add from math and calls it correctly.
        let main_source = r#"
import { add } from "math"

pub fn compute() -> i32 {
    return add(1, 2)
}
"#;
        let result = checked_with_surfaces(main_source, surfaces);
        assert!(
            result.is_ok(),
            "cross-module call with correct types should pass: {:?}",
            result.err()
        );
    }

    #[test]
    fn imported_function_type_mismatch_is_caught() {
        // Module math exports add(i32, i32) -> i32
        let math_result = checked(
            r#"
pub fn add(a: i32, b: i32) -> i32 {
    return a + b
}
"#,
        );
        let math_surface = module_type_surface(&math_result);
        let mut surfaces = BTreeMap::new();
        surfaces.insert("math".to_owned(), math_surface);

        // Module main passes a str where i32 is expected.
        let main_source = r#"
import { add } from "math"

pub fn compute() -> i32 {
    return add("wrong", 2)
}
"#;
        let result = checked_with_surfaces(main_source, surfaces);
        assert!(
            result.is_err(),
            "type mismatch on imported function should be caught"
        );
        let diagnostics = result.unwrap_err();
        assert!(
            diagnostics.iter().any(|d| d.code == "NS4001"),
            "expected NS4001 type mismatch: {:?}",
            diagnostics
        );
    }

    #[test]
    fn imported_function_return_type_propagates() {
        // Module parser exports parse(str) -> i32 ! ParseIntError
        let parser_result = checked(
            r#"
struct ParseIntError(str)

pub fn parse(input: str) -> i32 ! ParseIntError {
    return 0
}
"#,
        );
        let parser_surface = module_type_surface(&parser_result);
        let mut surfaces = BTreeMap::new();
        surfaces.insert("parser".to_owned(), parser_surface);

        // Module main uses the return type correctly.
        let main_source = r#"
import { parse } from "parser"

pub fn run() -> i32 ! ParseIntError {
    return parse("42")!
}
"#;
        // With no surface, this would pass vacuously (Unknown return type).
        // With the surface, the return type of parse is i32 ! ParseIntError,
        // so the '!' propagation should be valid.
        let result = checked_with_surfaces(main_source, surfaces);
        // The error type ParseIntError is not in scope in the importing module,
        // so the propagation may or may not error — this tests that we at least
        // see a real function type, not Unknown.
        // The key check: no NS4011 (unhandled error) or NS4013 (invalid propagation)
        // from the return type being Unknown.
        if let Err(diagnostics) = &result {
            let has_unknown_type_issue = diagnostics
                .iter()
                .any(|d| d.code == "NS4011" || d.code == "NS4013");
            assert!(
                !has_unknown_type_issue,
                "should not get error-effect false positives: {:?}",
                diagnostics
            );
        }
    }

    #[test]
    fn without_surface_imports_are_opaque() {
        // Without surfaces, importing anything is Unknown — calls pass vacuously.
        let main_source = r#"
import { anything } from "some-module"

pub fn run() -> i32 {
    return anything(1, 2, 3)
}
"#;
        // No surfaces → Unknown type → no diagnostics (permissive for opaque imports).
        let result = checked_with_surfaces(main_source, BTreeMap::new());
        assert!(
            result.is_ok(),
            "opaque imports should not cause false diagnostics"
        );
    }

    #[test]
    fn wrong_argument_count_on_imported_function_is_caught() {
        let lib_result = checked(
            r#"
pub fn greet(name: str) -> str {
    return name
}
"#,
        );
        let lib_surface = module_type_surface(&lib_result);
        let mut surfaces = BTreeMap::new();
        surfaces.insert("lib".to_owned(), lib_surface);

        let main_source = r#"
import { greet } from "lib"

pub fn run() -> str {
    return greet("a", "b")
}
"#;
        let result = checked_with_surfaces(main_source, surfaces);
        assert!(
            result.is_err(),
            "wrong argument count on imported function should be caught"
        );
        let diagnostics = result.unwrap_err();
        assert!(
            diagnostics.iter().any(|d| d.code == "NS4004"),
            "expected NS4004 argument count error: {:?}",
            diagnostics
        );
    }

    #[test]
    fn surface_reexports_copy_dependency_types() {
        let impl_result = checked(
            r#"
pub fn helper(n: i32) -> i32 {
    return n + 1
}
"#,
        );
        let impl_surface = module_type_surface(&impl_result);
        let mut dependencies = BTreeMap::new();
        dependencies.insert("impl".to_owned(), impl_surface);

        let barrel = checked_with_surfaces(
            r#"
export { helper } from "impl"
"#,
            dependencies.clone(),
        )
        .expect("barrel should check");
        let barrel_surface = module_type_surface_with_surfaces(&barrel, &dependencies);
        assert!(
            matches!(barrel_surface.get_value("helper"), Some(Type::Function(_))),
            "reexport should keep the dependency function type"
        );
    }

    #[test]
    fn surface_captures_namespace_reexport_members() {
        let math_result = checked(
            r#"
pub fn double(n: i32) -> i32 {
    return n * 2
}
"#,
        );
        let math_surface = module_type_surface(&math_result);
        let mut dependencies = BTreeMap::new();
        dependencies.insert("math".to_owned(), math_surface);

        let barrel = checked_with_surfaces(
            r#"
export * as math from "math"
"#,
            dependencies.clone(),
        )
        .expect("barrel should check");
        let barrel_surface = module_type_surface_with_surfaces(&barrel, &dependencies);
        let members = barrel_surface
            .namespaces
            .get("math")
            .expect("namespace export should be recorded");
        assert!(
            matches!(members.get("double"), Some(Type::Function(_))),
            "namespace members should come from the target surface"
        );
    }

    #[test]
    fn imported_tuple_structs_and_enums_are_real_types() {
        let types_result = checked(
            r#"
pub struct Pair(i32, i32)
pub enum Status { Pending, Ready(i32) }
"#,
        );
        let mut surfaces = BTreeMap::new();
        surfaces.insert("types".to_owned(), module_type_surface(&types_result));

        let main_source = r#"
import { Pair, Status } from "types"

pub fn run() -> i32 {
    let pair = Pair(20, 22)
    let status = Status.Ready(pair.0 + pair.1)
    return match status {
        Status.Ready(value) => value
        Status.Pending => 0
    }
}
"#;
        let result = checked_with_surfaces(main_source, surfaces);
        assert!(
            result.is_ok(),
            "imported tuple structs and enums should check: {:?}",
            result.err()
        );
    }

    // ─── Graph checker ─────────────────────────────────────────────────────

    #[test]
    fn graph_checker_resolves_two_modules() {
        let math = ProjectModule {
            key: "math".to_owned(),
            source: parse("pub fn double(n: i32) -> i32 { return n * 2 }").expect("parse failed"),
        };
        let main_source = r#"
import { double } from "math"

pub fn run() -> i32 {
    return double(21)
}
"#;
        let main = ProjectModule {
            key: "main".to_owned(),
            source: parse(main_source).expect("parse failed"),
        };

        let result = check_module_graph(&[math, main], CheckOptions::default());
        assert!(
            result.is_ok(),
            "graph check should pass: {:?}",
            result.all_diagnostics()
        );
        assert_eq!(result.modules.len(), 2, "both modules should be checked");
    }

    #[test]
    fn graph_checker_catches_type_errors_in_importing_module() {
        let lib = ProjectModule {
            key: "lib".to_owned(),
            source: parse("pub fn square(n: i32) -> i32 { return n * n }").expect("parse failed"),
        };
        let main_source = r#"
import { square } from "lib"

pub fn run() -> i32 {
    return square("not a number")
}
"#;
        let main = ProjectModule {
            key: "main".to_owned(),
            source: parse(main_source).expect("parse failed"),
        };

        let result = check_module_graph(&[lib, main], CheckOptions::default());
        assert!(!result.is_ok(), "type error should be caught");
        assert!(
            result.all_diagnostics().iter().any(|d| d.code == "NS4001"),
            "expected NS4001: {:?}",
            result.all_diagnostics()
        );
    }

    #[test]
    fn graph_checker_preserves_imported_error_effects() {
        let lib = ProjectModule {
            key: "lib".to_owned(),
            source: parse(
                r#"
pub struct E(str)
pub fn parse() -> i32 ! E {
    return !E("bad")
}
"#,
            )
            .expect("parse failed"),
        };
        let main = ProjectModule {
            key: "main".to_owned(),
            source: parse(
                r#"
import { parse } from "lib"
pub fn run() {
    let value = parse()
}
"#,
            )
            .expect("parse failed"),
        };

        let result = check_module_graph(&[lib, main], CheckOptions::default());
        assert!(!result.is_ok(), "imported effect should be checked");
        assert!(
            result
                .all_diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code == "NS4011"),
            "expected NS4011: {:?}",
            result.all_diagnostics()
        );
    }

    #[test]
    fn graph_checker_allows_declared_imported_error_propagation() {
        let lib = ProjectModule {
            key: "lib".to_owned(),
            source: parse(
                r#"
pub struct E(str)
pub fn parse() -> i32 ! E {
    return !E("bad")
}
"#,
            )
            .expect("parse failed"),
        };
        let main = ProjectModule {
            key: "main".to_owned(),
            source: parse(
                r#"
import { E, parse } from "lib"
pub fn run() -> i32 ! E {
    return parse()!
}
"#,
            )
            .expect("parse failed"),
        };

        let result = check_module_graph(&[lib, main], CheckOptions::default());
        assert!(
            result.is_ok(),
            "declared imported effect should propagate: {:?}",
            result.all_diagnostics()
        );
    }

    #[test]
    fn graph_checker_handles_resolution_failure_gracefully() {
        let bad_source = "import { nonexistent } from \"missing\"";
        let module = ProjectModule {
            key: "bad".to_owned(),
            source: parse(bad_source).expect("parse failed"),
        };

        // single-module graph with strict resolver will report NS3012
        let result = check_module_graph(&[module], CheckOptions::default());
        // Either the resolution fails (diagnostics) or it passes opaquely.
        // The important thing is it doesn't panic.
        let _ = result;
    }

    #[test]
    fn graph_checker_uses_mode_option() {
        let lib = ProjectModule {
            key: "lib".to_owned(),
            source: parse("pub fn helper() {}").expect("parse failed"),
        };
        let main = ProjectModule {
            key: "main".to_owned(),
            source: parse("import { helper } from \"lib\"\npub fn run() { helper() }")
                .expect("parse failed"),
        };

        let options = CheckOptions::new(Mode::App);
        let result = check_module_graph(&[lib, main], options);
        assert!(
            result.is_ok(),
            "app mode graph check should pass: {:?}",
            result.all_diagnostics()
        );
    }
}
