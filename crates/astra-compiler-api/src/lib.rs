//! Versioned compiler protocol types.
//!
//! The native binary and Wasm entrypoint share this JSON-RPC 2.0 boundary. The
//! single-source frontend parses and resolves source until the type checker
//! and code generator land.

use std::collections::BTreeMap;

use astra_capabilities::{Mode, ModeInference, infer_module_mode_named, infer_project};
#[allow(unused_imports)]
use astra_codegen_c::{CModule, IntegerPolicy as CIntegerPolicy};
use astra_core::canonical::check_canonical as lint_canonical;
use astra_core::checker::{CheckOptions, TypeAnnotation, check_resolved};
use astra_core::diagnostic::Diagnostic;
use astra_core::edition::Edition;
use astra_core::formatter::{FormatOptions, format_source_with_options};
use astra_core::ir::Program;
use astra_core::migrate::migrate_source as rewrite_source;
use astra_core::parser::{ParseOptions, parse_with_options};
use astra_core::resolver::resolve_module;
use astra_core::resolver::{ModuleGraph, collect_module_specifiers};
use astra_core::type_surface::{ProjectModule, check_module_graph_with_imports};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Runs the deny-by-default canonical syntax lint for one source unit.
///
/// This is a check-only pass: it reports deprecated spellings and never edits
/// source. Migration is the only rewrite path.
#[must_use]
pub fn check_canonical(source: &str, edition: Edition) -> Vec<Diagnostic> {
    lint_canonical(source, edition)
}

/// Migrates one source unit to edition `0.2` without touching manifests.
pub use astra_core::migrate::{MigrateOptions, Migration};

/// Runs the source-to-source migrator over one source unit.
pub fn migrate_source(source: &str, options: MigrateOptions) -> Result<Migration, Vec<Diagnostic>> {
    rewrite_source(source, options)
}

/// Compiler API version. This is separate from the language version.
pub const API_VERSION: &str = "1.0";

/// Integer form of the API major version used by raw Wasm entrypoints.
pub const API_VERSION_MAJOR: u32 = 1;

/// A source unit passed over the compiler boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceUnit {
    pub path: String,
    pub text: String,
}

/// Integer arithmetic policy selected by the manifest.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IntegerPolicy {
    /// Fixed-width operations wrap modulo 2^N.
    #[default]
    Wrapping,
    /// Overflow traps at runtime.
    SafeChecked,
    /// Arbitrary-precision behavior where the target supports it.
    Arbitrary,
}

/// A typed module ready for a source-to-source backend.
///
/// The AST carries declaration bodies, HIR carries resolved imports and
/// exports, and `annotations` maps expression byte spans to checked types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleIr {
    /// Canonical module identity used by imports.
    pub key: String,
    /// Display path used by diagnostics.
    pub path: String,
    /// Parsed source declaration tree.
    pub ast: astra_core::ast::Module,
    /// Resolved HIR with imports, exports, and symbol tables.
    pub hir: astra_core::hir::Module,
    /// Checked expression types, in source order.
    pub annotations: Vec<TypeAnnotation>,
    /// Spans of `@memory:manual` scopes, which the JavaScript backend rejects.
    pub manual_memory_scopes: Vec<astra_core::span::Span>,
    /// Selected integer arithmetic policy.
    pub integer_policy: IntegerPolicy,
}

/// Parameters for the `check` and `compile` commands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompileRequest {
    pub api_version: String,
    pub source: SourceUnit,
    pub target: String,
    /// Language edition. Defaults to `0.1` for old callers.
    #[serde(default)]
    pub edition: Edition,
    /// Explicit mode. When omitted, the compiler infers the least-privileged
    /// mode from the source graph.
    #[serde(default)]
    pub mode: Option<Mode>,
    /// Integer policy from the manifest. Defaults to wrapping.
    #[serde(default)]
    pub integer_policy: IntegerPolicy,
}

/// A compiler result returned inside a JSON-RPC response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompileResponse {
    pub api_version: String,
    pub program: Option<Program>,
    /// Typed module payload for backend code generation. Present for `compile`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ir: Option<ModuleIr>,
    pub diagnostics: Vec<Diagnostic>,
    pub metadata: Value,
    pub artifacts: Vec<Value>,
}

impl CompileResponse {
    /// Creates a successful response.
    #[must_use]
    pub fn success(program: Program) -> Self {
        Self {
            api_version: API_VERSION.to_owned(),
            program: Some(program),
            ir: None,
            diagnostics: Vec::new(),
            metadata: json!({}),
            artifacts: Vec::new(),
        }
    }

    /// Extends a successful response with typed backend input.
    #[must_use]
    pub fn with_ir(mut self, ir: ModuleIr) -> Self {
        self.ir = Some(ir);
        self
    }

    /// Attaches structured metadata to a successful response.
    #[must_use]
    pub fn with_metadata(mut self, metadata: Value) -> Self {
        self.metadata = metadata;
        self
    }

    /// Attaches a backend artifact to a successful response.
    #[must_use]
    pub fn with_artifact(mut self, artifact: Value) -> Self {
        self.artifacts.push(artifact);
        self
    }

    /// Creates a failed response.
    #[must_use]
    pub fn failure(diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            api_version: API_VERSION.to_owned(),
            program: None,
            ir: None,
            diagnostics,
            metadata: json!({}),
            artifacts: Vec::new(),
        }
    }
}

/// A canonical-format result returned inside a JSON-RPC response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatResponse {
    pub api_version: String,
    pub source: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
}

impl FormatResponse {
    /// Creates a successful format response.
    #[must_use]
    pub fn success(source: String) -> Self {
        Self {
            api_version: API_VERSION.to_owned(),
            source: Some(source),
            diagnostics: Vec::new(),
        }
    }

    /// Creates a failed format response.
    #[must_use]
    pub fn failure(diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            api_version: API_VERSION.to_owned(),
            source: None,
            diagnostics,
        }
    }
}

/// A parsed surface-AST result returned inside a JSON-RPC response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AstResponse {
    pub api_version: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ast: Option<astra_core::ast::Module>,
    pub diagnostics: Vec<Diagnostic>,
}

impl AstResponse {
    /// Creates a successful AST response.
    #[must_use]
    pub fn success(ast: astra_core::ast::Module) -> Self {
        Self {
            api_version: API_VERSION.to_owned(),
            ok: true,
            ast: Some(ast),
            diagnostics: Vec::new(),
        }
    }

    /// Creates a failed AST response.
    #[must_use]
    pub fn failure(diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            api_version: API_VERSION.to_owned(),
            ok: false,
            ast: None,
            diagnostics,
        }
    }
}

/// A mode-inference explanation returned inside a JSON-RPC response.
///
/// `explanation` is the human-readable rendering of the structured `inference`
/// payload, so both machine and display consumers read the same evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplainResponse {
    pub api_version: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inference: Option<ModeInference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
}

impl ExplainResponse {
    /// Creates a successful explanation response.
    #[must_use]
    pub fn success(inference: ModeInference) -> Self {
        let explanation = inference.explain();
        Self {
            api_version: API_VERSION.to_owned(),
            ok: true,
            inference: Some(inference),
            explanation: Some(explanation),
            diagnostics: Vec::new(),
        }
    }

    /// Creates a failed explanation response.
    #[must_use]
    pub fn failure(diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            api_version: API_VERSION.to_owned(),
            ok: false,
            inference: None,
            explanation: None,
            diagnostics,
        }
    }
}

/// Derives a stem name from a source path for output file naming
/// (e.g. `"src/app/main.ns"` → `"main"`).
fn module_stem(path: &str) -> &str {
    let base = path.rsplit(&['/', '\\'][..]).next().unwrap_or(path);
    base.strip_suffix(".ns").unwrap_or(base)
}

/// Boundary implemented by the Rust core.
pub trait CompilerApi {
    /// Checks and lowers one request.
    fn check(&self, request: &CompileRequest) -> CompileResponse;
}

// ─── Project (multi-source) check ────────────────────────────────────────────

/// Parameters for the `check_project` command.
///
/// `sources` is a list in dependency order: each module must appear before any
/// module that imports it. The root entry-point module appears last.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCheckRequest {
    pub api_version: String,
    /// Ordered module sources, keyed by their canonical import specifier.
    pub sources: Vec<ProjectSourceUnit>,
    pub target: String,
    /// Language edition. Defaults to `0.1` for old callers.
    #[serde(default)]
    pub edition: Edition,
    /// Explicit project mode. When omitted, the project graph is inferred.
    #[serde(default)]
    pub mode: Option<Mode>,
    /// Integer policy carried into each module's typed IR.
    #[serde(default)]
    pub integer_policy: IntegerPolicy,
}

/// One source unit in a project check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSourceUnit {
    /// Canonical module key as used in `import` statements.
    pub key: String,
    /// File path shown in diagnostics.
    pub path: String,
    /// Source text.
    pub text: String,
}

/// Per-module result inside a `ProjectCheckResponse`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleCheckResult {
    pub key: String,
    pub path: String,
    pub diagnostics: Vec<Diagnostic>,
    pub ok: bool,
    /// Typed module payload for successful project checks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ir: Option<ModuleIr>,
}

/// Response for the `check_project` command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCheckResponse {
    pub api_version: String,
    pub ok: bool,
    pub modules: Vec<ModuleCheckResult>,
    pub program: Option<Program>,
    pub metadata: Value,
}

/// Checks a project source list through `check_module_graph` and returns a
/// structured response.
pub fn check_project(request: &ProjectCheckRequest) -> ProjectCheckResponse {
    if request.api_version != API_VERSION {
        return ProjectCheckResponse {
            api_version: API_VERSION.to_owned(),
            ok: false,
            modules: Vec::new(),
            program: None,
            metadata: json!({ "error": "unsupported apiVersion" }),
        };
    }

    // Parse all source units up front so we can fail fast on lex/parse errors.
    let mut project_modules: Vec<ProjectModule> = Vec::new();
    let mut ast_map: BTreeMap<String, astra_core::ast::Module> = BTreeMap::new();
    let mut path_map: BTreeMap<String, String> = BTreeMap::new();
    let mut early_failures: Vec<ModuleCheckResult> = Vec::new();

    for unit in &request.sources {
        path_map.insert(unit.key.clone(), unit.path.clone());
        match parse_with_options(
            &unit.text,
            ParseOptions {
                edition: request.edition,
            },
        ) {
            Ok(module) => {
                ast_map.insert(unit.key.clone(), module.clone());
                project_modules.push(ProjectModule {
                    key: unit.key.clone(),
                    source: module,
                });
            }
            Err(diagnostics) => {
                early_failures.push(ModuleCheckResult {
                    key: unit.key.clone(),
                    path: unit.path.clone(),
                    diagnostics,
                    ok: false,
                    ir: None,
                });
            }
        }
    }

    if !early_failures.is_empty() {
        return ProjectCheckResponse {
            api_version: API_VERSION.to_owned(),
            ok: false,
            modules: early_failures,
            program: None,
            metadata: json!({
                "moduleOrder": request.sources.iter().map(|s| &s.key).collect::<Vec<_>>(),
                "target": request.target,
                "mode": request.mode.unwrap_or(Mode::Core),
                "modeSource": if request.mode.is_some() { "explicit" } else { "inferred" },
            }),
        };
    }

    let inferences: Vec<ModeInference> = project_modules
        .iter()
        .map(|module| infer_module_mode_named(&module.source, &module.key))
        .collect();
    let project_inference = infer_project(&inferences);
    let mode_source = if request.mode.is_some() {
        "explicit"
    } else {
        "inferred"
    };
    let selected_mode = request
        .mode
        .unwrap_or_else(|| project_inference.resolved_mode());
    let inferred_modes: BTreeMap<String, Mode> = inferences
        .iter()
        .map(|inference| {
            (
                inference
                    .module
                    .clone()
                    .unwrap_or_else(|| "<module>".to_owned()),
                inference.resolved_mode(),
            )
        })
        .collect();
    let metadata = json!({
        "moduleOrder": request.sources.iter().map(|s| &s.key).collect::<Vec<_>>(),
        "target": request.target,
        "mode": selected_mode,
        "modeSource": mode_source,
        "requiredMode": project_inference.required_mode,
        "inferredModes": inferred_modes,
        "modeReasons": project_inference.reasons,
        "modeExplanation": project_inference.explain(),
    });

    if request.mode.is_none() && !project_inference.is_ok() {
        let mut module_results: Vec<ModuleCheckResult> = request
            .sources
            .iter()
            .map(|unit| {
                let inference = inferences
                    .iter()
                    .find(|inference| inference.module.as_deref() == Some(unit.key.as_str()));
                ModuleCheckResult {
                    key: unit.key.clone(),
                    path: unit.path.clone(),
                    diagnostics: inference
                        .map(|inference| inference.diagnostics.clone())
                        .unwrap_or_default(),
                    ok: inference.is_none_or(ModeInference::is_ok),
                    ir: None,
                }
            })
            .collect();
        let mut project_diagnostics = project_inference.diagnostics.clone();
        if let Some(last) = module_results.last_mut() {
            for diagnostic in project_diagnostics.drain(..) {
                if !last.diagnostics.contains(&diagnostic) {
                    last.diagnostics.push(diagnostic);
                }
            }
            last.ok = false;
        }
        return ProjectCheckResponse {
            api_version: API_VERSION.to_owned(),
            ok: false,
            modules: module_results,
            program: None,
            metadata,
        };
    }

    // Build the canonical module graph. Keys are project-relative paths, so
    // resolve relative specifiers against the importer before checking. A
    // specifier that already matches a source key is treated as an identity
    // import for compatibility with the original multi-source API.
    let mut module_graph = ModuleGraph::default();
    for module in &project_modules {
        module_graph.insert_module(module.key.clone(), module.source.clone());
    }
    for module in &project_modules {
        for specifier in collect_module_specifiers(&module.source) {
            let target = resolve_project_target(
                &module.key,
                &specifier.specifier,
                &module_graph,
                &BTreeMap::new(),
            );
            module_graph.insert_import(module.key.clone(), specifier.specifier, target);
        }
    }

    let ordered_keys = dependency_order(&project_modules, &module_graph);
    let ordered_modules: Vec<ProjectModule> = ordered_keys
        .iter()
        .filter_map(|key| {
            project_modules
                .iter()
                .find(|module| module.key == *key)
                .cloned()
        })
        .collect();
    let graph = check_module_graph_with_imports(
        &ordered_modules,
        &module_graph,
        CheckOptions::new(selected_mode).with_edition(request.edition),
    );

    let ok = graph.is_ok();

    // Collect per-module results in request order and attach typed IR to every
    // successful module.
    let mut module_results: BTreeMap<String, ModuleCheckResult> = BTreeMap::new();
    for (key, result) in &graph.modules {
        let ir = ast_map.get(key).map(|ast| ModuleIr {
            key: key.clone(),
            path: path_map.get(key).cloned().unwrap_or_else(|| key.clone()),
            ast: ast.clone(),
            hir: result.module.clone(),
            annotations: result.annotations.clone(),
            manual_memory_scopes: result.manual_memory_scopes.clone(),
            integer_policy: request.integer_policy,
        });
        module_results.insert(
            key.clone(),
            ModuleCheckResult {
                key: key.clone(),
                path: path_map.get(key).cloned().unwrap_or_else(|| key.clone()),
                diagnostics: Vec::new(),
                ok: true,
                ir,
            },
        );
    }
    for (key, diagnostics) in &graph.diagnostics {
        module_results.insert(
            key.clone(),
            ModuleCheckResult {
                key: key.clone(),
                path: path_map.get(key).cloned().unwrap_or_else(|| key.clone()),
                diagnostics: diagnostics.clone(),
                ok: false,
                ir: None,
            },
        );
    }

    // Emit in the original request order.
    let modules: Vec<ModuleCheckResult> = request
        .sources
        .iter()
        .filter_map(|source| module_results.remove(&source.key))
        .collect();

    let program = ok.then(|| {
        let last_key = project_modules
            .last()
            .map(|m| m.key.clone())
            .unwrap_or_default();
        Program { name: last_key }
    });

    let module_order: Vec<&str> = request.sources.iter().map(|s| s.key.as_str()).collect();

    ProjectCheckResponse {
        api_version: API_VERSION.to_owned(),
        ok,
        modules,
        program,
        metadata: json!({
            "moduleOrder": module_order,
            "target": request.target,
            "mode": selected_mode,
            "modeSource": mode_source,
            "requiredMode": project_inference.required_mode,
            "inferredModes": inferred_modes,
            "modeReasons": project_inference.reasons,
            "modeExplanation": project_inference.explain(),
        }),
    }
}

// ─── Project (multi-source) compile ─────────────────────────────────────────

/// Parameters for the `compile_project` command (C multi-module linking).
///
/// `sources` must be in dependency order: each module before the modules that
/// import it. The last entry is treated as the root (entry-point) module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCompileRequest {
    pub api_version: String,
    /// Ordered module sources, keyed by their canonical import specifier.
    pub sources: Vec<ProjectSourceUnit>,
    pub target: String,
    /// Language edition. Defaults to `0.1` for old callers.
    #[serde(default)]
    pub edition: Edition,
    /// Explicit project mode. When omitted, the compiler infers.
    #[serde(default)]
    pub mode: Option<Mode>,
    /// Integer arithmetic policy. Defaults to wrapping.
    #[serde(default)]
    pub integer_policy: IntegerPolicy,
    /// Emit a library artifact: suppress the `int main(void)` wrapper while
    /// keeping `pub` functions external. Defaults to an executable.
    #[serde(default)]
    pub library: bool,
    /// Package import map: source specifier → canonical module key. Used to
    /// resolve bare package specifiers (e.g. `"mathlib"`) to a module whose
    /// key lives under a package namespace.
    #[serde(default)]
    pub import_map: BTreeMap<String, String>,
}

/// Per-module artifact in a `ProjectCompileResponse`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleCompileArtifact {
    pub key: String,
    pub path: String,
    /// Emitted C source filename.
    pub c_filename: String,
    /// Emitted C header filename.
    pub h_filename: String,
    pub c_source: String,
    pub h_source: String,
    /// Script translation units (`@script:c` / `@script:cpp`) from this module.
    pub script_sources: Vec<(String, String)>,
}

/// Response for the `compile_project` command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCompileResponse {
    pub api_version: String,
    pub ok: bool,
    pub modules: Vec<ModuleCompileArtifact>,
    /// Single shared host shim (`astra_host.c`).
    pub host_shim: String,
    pub diagnostics: Vec<Diagnostic>,
    pub metadata: Value,
}

/// Checks and compiles a multi-module project to C, returning one translation
/// unit + one header per source module.
///
/// Internally runs `check_module_graph` then `astra_codegen_c::generate_project`.
pub fn compile_project(request: &ProjectCompileRequest) -> ProjectCompileResponse {
    if request.api_version != API_VERSION {
        return ProjectCompileResponse {
            api_version: API_VERSION.to_owned(),
            ok: false,
            modules: Vec::new(),
            host_shim: String::new(),
            diagnostics: vec![Diagnostic::error(
                "NS9009",
                format!(
                    "Unsupported apiVersion '{}'; expected '{}'.",
                    request.api_version, API_VERSION
                ),
            )],
            metadata: json!({ "error": "unsupported apiVersion" }),
        };
    }

    if request.sources.is_empty() {
        return ProjectCompileResponse {
            api_version: API_VERSION.to_owned(),
            ok: false,
            modules: Vec::new(),
            host_shim: String::new(),
            diagnostics: vec![Diagnostic::error(
                "NS9010",
                "compile_project requires at least one source unit",
            )],
            metadata: json!({}),
        };
    }

    // Parse all sources.
    let mut project_modules: Vec<astra_core::type_surface::ProjectModule> = Vec::new();
    let mut path_map: std::collections::BTreeMap<String, String> =
        std::collections::BTreeMap::new();
    let mut parse_errors: Vec<Diagnostic> = Vec::new();

    for unit in &request.sources {
        path_map.insert(unit.key.clone(), unit.path.clone());
        match parse_with_options(
            &unit.text,
            ParseOptions {
                edition: request.edition,
            },
        ) {
            Ok(module) => {
                project_modules.push(astra_core::type_surface::ProjectModule {
                    key: unit.key.clone(),
                    source: module,
                });
            }
            Err(diagnostics) => {
                parse_errors.extend(diagnostics);
            }
        }
    }

    if !parse_errors.is_empty() {
        return ProjectCompileResponse {
            api_version: API_VERSION.to_owned(),
            ok: false,
            modules: Vec::new(),
            host_shim: String::new(),
            diagnostics: parse_errors,
            metadata: json!({}),
        };
    }

    // Infer mode.
    use astra_capabilities::{ModeInference, infer_module_mode_named, infer_project};
    let inferences: Vec<ModeInference> = project_modules
        .iter()
        .map(|m| infer_module_mode_named(&m.source, &m.key))
        .collect();
    let project_inference = infer_project(&inferences);
    let selected_mode = request
        .mode
        .unwrap_or_else(|| project_inference.resolved_mode());

    if request.mode.is_none() && !project_inference.is_ok() {
        return ProjectCompileResponse {
            api_version: API_VERSION.to_owned(),
            ok: false,
            modules: Vec::new(),
            host_shim: String::new(),
            diagnostics: project_inference.diagnostics.clone(),
            metadata: json!({}),
        };
    }

    // Type-check the graph. Keys are project-relative paths, so resolve
    // relative specifiers against the importer before checking (mirrors
    // `check_project`); `check_module_graph` alone would treat `./lib` as its
    // own module key and fail to resolve it.
    let mut module_graph = ModuleGraph::default();
    for module in &project_modules {
        module_graph.insert_module(module.key.clone(), module.source.clone());
    }
    for module in &project_modules {
        for specifier in collect_module_specifiers(&module.source) {
            let target = resolve_project_target(
                &module.key,
                &specifier.specifier,
                &module_graph,
                &request.import_map,
            );
            module_graph.insert_import(module.key.clone(), specifier.specifier, target);
        }
    }
    let ordered_keys = dependency_order(&project_modules, &module_graph);
    let ordered_modules: Vec<astra_core::type_surface::ProjectModule> = ordered_keys
        .iter()
        .filter_map(|key| {
            project_modules
                .iter()
                .find(|module| module.key == *key)
                .cloned()
        })
        .collect();
    let graph = astra_core::type_surface::check_module_graph_with_imports(
        &ordered_modules,
        &module_graph,
        astra_core::checker::CheckOptions::new(selected_mode).with_edition(request.edition),
    );

    if !graph.is_ok() {
        return ProjectCompileResponse {
            api_version: API_VERSION.to_owned(),
            ok: false,
            modules: Vec::new(),
            host_shim: String::new(),
            diagnostics: graph.all_diagnostics(),
            metadata: json!({}),
        };
    }

    // Build the C project entries.
    let c_ip = match request.integer_policy {
        IntegerPolicy::Wrapping => CIntegerPolicy::Wrapping,
        IntegerPolicy::SafeChecked => CIntegerPolicy::SafeChecked,
        IntegerPolicy::Arbitrary => CIntegerPolicy::Arbitrary,
    };

    // For each source module in request order, build a ProjectEntry.
    // dep_keys = the direct imports of this module (from HIR).
    let entries: Vec<astra_codegen_c::ProjectEntry> = project_modules
        .iter()
        .filter_map(|pm| {
            let checked = graph.get(&pm.key)?;
            let dep_keys: Vec<String> = checked
                .module
                .imports
                .iter()
                .map(|imp| imp.module.clone())
                .collect();
            Some(astra_codegen_c::ProjectEntry {
                module: CModule {
                    name: checked.module.name.clone(),
                    ast: pm.source.clone(),
                    hir: checked.module.clone(),
                    annotations: checked.annotations.clone(),
                    manual_memory_scopes: checked.manual_memory_scopes.clone(),
                    integer_policy: c_ip,
                },
                key: pm.key.clone(),
                dep_keys,
            })
        })
        .collect();

    match astra_codegen_c::generate_project_with_kind(
        &entries,
        if request.library {
            astra_codegen_c::EmitKind::Library
        } else {
            astra_codegen_c::EmitKind::Executable
        },
    ) {
        Ok(generated) => {
            let mut modules = Vec::new();
            let mut host_shim = String::new();
            for (entry, module_out) in entries.iter().zip(generated.iter()) {
                // Strip a trailing `.ns` before replacing separators so the
                // artifact filename matches the stem used for symbol
                // qualification and `#include` directives in the emitter.
                let stem = entry
                    .key
                    .strip_suffix(".ns")
                    .unwrap_or(&entry.key)
                    .replace(['/', '.', '-'], "_");
                let c_filename = format!("{stem}.c");
                let h_filename = format!("{stem}.h");
                modules.push(ModuleCompileArtifact {
                    key: entry.key.clone(),
                    path: path_map
                        .get(&entry.key)
                        .cloned()
                        .unwrap_or_else(|| entry.key.clone()),
                    c_filename,
                    h_filename,
                    c_source: module_out.source.clone(),
                    h_source: module_out.header.clone().unwrap_or_default(),
                    script_sources: module_out.script_sources.clone(),
                });
                if host_shim.is_empty() {
                    host_shim = module_out.host_shim.clone();
                }
            }
            ProjectCompileResponse {
                api_version: API_VERSION.to_owned(),
                ok: true,
                modules,
                host_shim,
                diagnostics: Vec::new(),
                metadata: json!({
                    "moduleCount": entries.len(),
                    "mode": selected_mode,
                    "target": request.target,
                    "integerPolicy": request.integer_policy,
                }),
            }
        }
        Err(backend_error) => ProjectCompileResponse {
            api_version: API_VERSION.to_owned(),
            ok: false,
            modules: Vec::new(),
            host_shim: String::new(),
            diagnostics: vec![Diagnostic::error(
                backend_error.code.clone(),
                backend_error.message.clone(),
            )],
            metadata: json!({}),
        },
    }
}

// ─── Single-source frontend ───────────────────────────────────────────────────

#[derive(Debug, Default, Clone, Copy)]
pub struct FrontendCompiler;

impl FrontendCompiler {
    /// Checks and attaches typed IR for one backend target.
    #[must_use]
    pub fn compile(&self, request: &CompileRequest) -> CompileResponse {
        if !matches!(request.target.as_str(), "js" | "ts" | "c") {
            return CompileResponse::failure(vec![
                Diagnostic::error(
                    "NS8001",
                    format!(
                        "Target '{}' is not supported by the typed source backend.",
                        request.target
                    ),
                )
                .with_hint("Use target 'js', 'ts', or 'c'."),
            ])
            .with_metadata(json!({ "target": request.target }));
        }

        self.run(request, true)
    }

    fn run(&self, request: &CompileRequest, typed: bool) -> CompileResponse {
        match parse_with_options(
            &request.source.text,
            ParseOptions {
                edition: request.edition,
            },
        ) {
            Ok(module) => {
                let inference = infer_module_mode_named(&module, &request.source.path);
                let mode_source = if request.mode.is_some() {
                    "explicit"
                } else {
                    "inferred"
                };
                let selected_mode = request.mode.unwrap_or_else(|| inference.resolved_mode());
                if request.mode.is_none() && !inference.is_ok() {
                    return CompileResponse::failure(inference.diagnostics.clone()).with_metadata(
                        json!({
                            "mode": selected_mode,
                            "modeSource": mode_source,
                            "requiredMode": inference.required_mode,
                            "modeReasons": inference.reasons,
                            "modeExplanation": inference.explain(),
                        }),
                    );
                }

                match resolve_module(&module) {
                    Ok(resolved) => {
                        match check_resolved(
                            &module,
                            &resolved,
                            CheckOptions::new(selected_mode).with_edition(request.edition),
                        ) {
                            Ok(checked) => {
                                let name = checked.module.name.clone();
                                let metadata = json!({
                                    "mode": selected_mode,
                                    "modeSource": mode_source,
                                    "requiredMode": inference.required_mode,
                                    "modeReasons": inference.reasons,
                                    "modeExplanation": inference.explain(),
                                    "target": request.target,
                                    "integerPolicy": request.integer_policy,
                                });
                                let mut response = CompileResponse::success(Program { name })
                                    .with_metadata(metadata);
                                if typed {
                                    let ir = ModuleIr {
                                        key: request.source.path.clone(),
                                        path: request.source.path.clone(),
                                        ast: module.clone(),
                                        hir: checked.module.clone(),
                                        annotations: checked.annotations.clone(),
                                        manual_memory_scopes: checked.manual_memory_scopes.clone(),
                                        integer_policy: request.integer_policy,
                                    };
                                    // C backend: run the emitter and attach the
                                    // generated source as artifacts.
                                    if request.target == "c" {
                                        let c_module = CModule {
                                            name: checked.module.name.clone(),
                                            ast: module,
                                            hir: checked.module,
                                            annotations: checked.annotations,
                                            manual_memory_scopes: checked.manual_memory_scopes,
                                            integer_policy: match request.integer_policy {
                                                IntegerPolicy::Wrapping => CIntegerPolicy::Wrapping,
                                                IntegerPolicy::SafeChecked => {
                                                    CIntegerPolicy::SafeChecked
                                                }
                                                IntegerPolicy::Arbitrary => {
                                                    CIntegerPolicy::Arbitrary
                                                }
                                            },
                                        };
                                        match astra_codegen_c::generate(&c_module) {
                                            Ok(generated) => {
                                                response = response
                                                    .with_ir(ir)
                                                    .with_artifact(json!({
                                                        "type": "c-source",
                                                        "name": format!("{}.c", module_stem(&request.source.path)),
                                                        "filename": format!("{}.c", module_stem(&request.source.path)),
                                                        "content": generated.source,
                                                    }))
                                                    .with_artifact(json!({
                                                        "type": "c-host-shim",
                                                        "name": "astra_host.c",
                                                        "filename": "astra_host.c",
                                                        "content": generated.host_shim,
                                                    }));
                                                for (filename, content) in generated.script_sources
                                                {
                                                    response = response.with_artifact(json!({
                                                        "type": "c-script",
                                                        "name": filename,
                                                        "filename": filename,
                                                        "content": content,
                                                    }));
                                                }
                                            }
                                            Err(backend_error) => {
                                                return CompileResponse::failure(vec![
                                                    Diagnostic::error(
                                                        backend_error.code.clone(),
                                                        backend_error.message.clone(),
                                                    ),
                                                ]);
                                            }
                                        }
                                    } else {
                                        response = response.with_ir(ir);
                                    }
                                }
                                response
                            }
                            Err(diagnostics) => CompileResponse::failure(diagnostics),
                        }
                    }
                    Err(diagnostics) => CompileResponse::failure(diagnostics),
                }
            }
            Err(diagnostics) => CompileResponse::failure(diagnostics),
        }
    }
}

impl CompilerApi for FrontendCompiler {
    fn check(&self, request: &CompileRequest) -> CompileResponse {
        self.run(request, false)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: Option<Value>, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
            }),
        }
    }
}

/// Processes one JSON-RPC request and returns one JSON-RPC response.
#[must_use]
pub fn process_json(input: &str) -> String {
    let request = match serde_json::from_str::<JsonRpcRequest>(input) {
        Ok(request) => request,
        Err(error) => {
            return serialize_response(JsonRpcResponse::error(
                None,
                -32700,
                format!("Parse error: {error}"),
            ));
        }
    };

    if request.jsonrpc != "2.0" {
        return serialize_response(JsonRpcResponse::error(
            request.id,
            -32600,
            "Invalid Request: jsonrpc must be \"2.0\".",
        ));
    }

    let response = match request.method.as_str() {
        "initialize" => JsonRpcResponse::success(
            request.id,
            json!({
                "apiVersion": API_VERSION,
                "coreVersion": env!("CARGO_PKG_VERSION"),
                "commands": ["initialize", "check", "check_project", "compile", "compile_project", "ast", "format", "explain", "shutdown"],
                "features": ["resolver", "type-checker", "project-checker", "typed-ir", "project-compile-c"],
                "targets": ["js", "ts", "native", "wasm-web", "wasm-wasi"],
            }),
        ),
        "check" => match parse_compile_request(request.params) {
            Ok(compile_request) => JsonRpcResponse::success(
                request.id,
                serde_json::to_value(FrontendCompiler.check(&compile_request))
                    .unwrap_or(Value::Null),
            ),
            Err(message) => JsonRpcResponse::error(request.id, -32602, message),
        },
        "compile" => match parse_compile_request(request.params) {
            Ok(compile_request) => JsonRpcResponse::success(
                request.id,
                serde_json::to_value(FrontendCompiler.compile(&compile_request))
                    .unwrap_or(Value::Null),
            ),
            Err(message) => JsonRpcResponse::error(request.id, -32602, message),
        },
        "check_project" => match parse_project_check_request(request.params) {
            Ok(project_request) => JsonRpcResponse::success(
                request.id,
                serde_json::to_value(check_project(&project_request)).unwrap_or(Value::Null),
            ),
            Err(message) => JsonRpcResponse::error(request.id, -32602, message),
        },
        "compile_project" => match parse_project_compile_request(request.params) {
            Ok(project_request) => JsonRpcResponse::success(
                request.id,
                serde_json::to_value(compile_project(&project_request)).unwrap_or(Value::Null),
            ),
            Err(message) => JsonRpcResponse::error(request.id, -32602, message),
        },
        "format" => match parse_compile_request(request.params) {
            Ok(compile_request) => {
                let response = match format_source_with_options(
                    &compile_request.source.text,
                    FormatOptions {
                        edition: compile_request.edition,
                    },
                ) {
                    Ok(source) => FormatResponse::success(source),
                    Err(diagnostics) => FormatResponse::failure(diagnostics),
                };
                JsonRpcResponse::success(
                    request.id,
                    serde_json::to_value(response).unwrap_or(Value::Null),
                )
            }
            Err(message) => JsonRpcResponse::error(request.id, -32602, message),
        },
        "shutdown" => JsonRpcResponse::success(request.id, Value::Null),
        "ast" => match parse_compile_request(request.params) {
            Ok(compile_request) => JsonRpcResponse::success(
                request.id,
                serde_json::to_value(ast_for_request(&compile_request)).unwrap_or(Value::Null),
            ),
            Err(message) => JsonRpcResponse::error(request.id, -32602, message),
        },
        "explain" => match parse_compile_request(request.params) {
            Ok(compile_request) => JsonRpcResponse::success(
                request.id,
                serde_json::to_value(explain_for_request(&compile_request)).unwrap_or(Value::Null),
            ),
            Err(message) => JsonRpcResponse::error(request.id, -32602, message),
        },
        _ => JsonRpcResponse::error(request.id, -32601, "Method not found."),
    };

    serialize_response(response)
}

/// Parses one source unit into the surface AST. Parse errors are reported as a
/// failed response, mirroring `check` and `format`.
fn ast_for_request(request: &CompileRequest) -> AstResponse {
    match parse_with_options(
        &request.source.text,
        ParseOptions {
            edition: request.edition,
        },
    ) {
        Ok(module) => AstResponse::success(module),
        Err(diagnostics) => AstResponse::failure(diagnostics),
    }
}

/// Infers the least-privileged mode for one source unit and returns both the
/// structured evidence and its human-readable rendering.
fn explain_for_request(request: &CompileRequest) -> ExplainResponse {
    match parse_with_options(
        &request.source.text,
        ParseOptions {
            edition: request.edition,
        },
    ) {
        Ok(module) => ExplainResponse::success(infer_module_mode_named(
            &module,
            request.source.path.clone(),
        )),
        Err(diagnostics) => ExplainResponse::failure(diagnostics),
    }
}

fn parse_compile_request(params: Option<Value>) -> Result<CompileRequest, String> {
    let params = params.ok_or_else(|| "Missing params.".to_owned())?;
    let request: CompileRequest =
        serde_json::from_value(params).map_err(|error| format!("Invalid params: {error}"))?;

    if request.api_version != API_VERSION {
        return Err(format!(
            "Unsupported apiVersion '{}'; expected '{}'.",
            request.api_version, API_VERSION
        ));
    }

    Ok(request)
}

fn parse_project_check_request(params: Option<Value>) -> Result<ProjectCheckRequest, String> {
    let params = params.ok_or_else(|| "Missing params.".to_owned())?;
    let request: ProjectCheckRequest =
        serde_json::from_value(params).map_err(|error| format!("Invalid params: {error}"))?;

    if request.api_version != API_VERSION {
        return Err(format!(
            "Unsupported apiVersion '{}'; expected '{}'.",
            request.api_version, API_VERSION
        ));
    }

    if request.sources.is_empty() {
        return Err("check_project requires at least one source unit.".to_owned());
    }

    Ok(request)
}

fn parse_project_compile_request(params: Option<Value>) -> Result<ProjectCompileRequest, String> {
    let params = params.ok_or_else(|| "Missing params.".to_owned())?;
    let request: ProjectCompileRequest =
        serde_json::from_value(params).map_err(|error| format!("Invalid params: {error}"))?;

    if request.api_version != API_VERSION {
        return Err(format!(
            "Unsupported apiVersion '{}'; expected '{}'.",
            request.api_version, API_VERSION
        ));
    }

    if request.sources.is_empty() {
        return Err("compile_project requires at least one source unit.".to_owned());
    }

    Ok(request)
}

/// Resolves one import specifier to a canonical project-relative module path.
fn resolve_project_target(
    importer: &str,
    specifier: &str,
    graph: &ModuleGraph,
    import_map: &BTreeMap<String, String>,
) -> String {
    if let Some(target) = import_map.get(specifier) {
        return target.clone();
    }
    if graph.modules.contains_key(specifier) {
        return specifier.to_owned();
    }

    if let Some(target) = resolve_relative_target(importer, specifier) {
        return target;
    }

    // A dotted path such as `app.util` maps to `app/util.ns` when no source key
    // matched it exactly.
    if !specifier.contains('/') && specifier.contains('.') && !specifier.ends_with(".ns") {
        return format!("{}.ns", specifier.replace('.', "/"));
    }

    specifier.to_owned()
}

fn resolve_relative_target(importer: &str, specifier: &str) -> Option<String> {
    if !specifier.starts_with("./") && !specifier.starts_with("../") {
        return None;
    }

    let mut parts: Vec<&str> = importer.split('/').collect();
    parts.pop();
    for component in specifier.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }

    let mut target = parts.join("/");
    if target.is_empty() {
        return None;
    }
    if !target.ends_with(".ns") {
        target.push_str(".ns");
    }
    Some(target)
}

/// Orders modules so every dependency appears before its importers.
fn dependency_order(modules: &[ProjectModule], graph: &ModuleGraph) -> Vec<String> {
    fn visit(
        key: &str,
        graph: &ModuleGraph,
        order: &mut Vec<String>,
        visiting: &mut std::collections::BTreeSet<String>,
        visited: &mut std::collections::BTreeSet<String>,
    ) {
        if visited.contains(key) || !visiting.insert(key.to_owned()) {
            return;
        }
        for ((importer, _specifier), target) in &graph.imports {
            if importer == key {
                visit(target, graph, order, visiting, visited);
            }
        }
        visiting.remove(key);
        visited.insert(key.to_owned());
        order.push(key.to_owned());
    }

    let mut order = Vec::new();
    let mut visiting = std::collections::BTreeSet::new();
    let mut visited = std::collections::BTreeSet::new();
    for module in modules {
        visit(&module.key, graph, &mut order, &mut visiting, &mut visited);
    }
    order
}

fn serialize_response(response: JsonRpcResponse) -> String {
    serde_json::to_string(&response).unwrap_or_else(|_| {
        r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32603,"message":"Internal error."}}"#
            .to_owned()
    })
}

#[cfg(test)]
mod tests {
    use astra_core::diagnostic::Diagnostic;
    use astra_core::ir::Program;

    use super::{API_VERSION, CompileResponse, process_json};

    #[test]
    fn responses_report_the_api_version() {
        let response = CompileResponse::success(Program::default());
        assert_eq!(response.api_version, API_VERSION);
        assert!(response.diagnostics.is_empty());
    }

    #[test]
    fn failures_carry_diagnostics() {
        let diagnostic = Diagnostic::error("NS0001", "invalid token");
        let response = CompileResponse::failure(vec![diagnostic]);
        assert!(response.program.is_none());
        assert_eq!(response.diagnostics.len(), 1);
    }

    #[test]
    fn compile_project_uses_ns9009_for_an_unsupported_api_version() {
        let request = super::ProjectCompileRequest {
            api_version: "99.0".to_owned(),
            sources: Vec::new(),
            target: "c".to_owned(),
            edition: astra_core::edition::Edition::V0_1,
            mode: None,
            integer_policy: super::IntegerPolicy::Wrapping,
            library: false,
            import_map: Default::default(),
        };
        let response = super::compile_project(&request);
        assert!(!response.ok);
        assert!(
            response.diagnostics.iter().any(|d| d.code == "NS9009"),
            "{:?}",
            response.diagnostics
        );
    }

    #[test]
    fn compile_project_uses_ns9010_for_an_empty_source_list() {
        let request = super::ProjectCompileRequest {
            api_version: API_VERSION.to_owned(),
            sources: Vec::new(),
            target: "c".to_owned(),
            edition: astra_core::edition::Edition::V0_1,
            mode: None,
            integer_policy: super::IntegerPolicy::Wrapping,
            library: false,
            import_map: Default::default(),
        };
        let response = super::compile_project(&request);
        assert!(!response.ok);
        assert!(
            response.diagnostics.iter().any(|d| d.code == "NS9010"),
            "{:?}",
            response.diagnostics
        );
    }

    #[test]
    fn initialize_reports_the_api_version() {
        let output = process_json(r#"{"jsonrpc":"2.0","id":"req-001","method":"initialize"}"#);
        assert!(output.contains(r#""apiVersion":"1.0""#));
        assert!(output.contains(r#""id":"req-001""#));
    }

    #[test]
    fn check_returns_a_resolved_program() {
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":1,"method":"check","params":{"apiVersion":"1.0","source":{"path":"src/main.ns","text":"module app.main\npub fn main() {}\n"},"target":"js","mode":"core"}}"#,
        );
        assert!(output.contains(r#""apiVersion":"1.0""#));
        assert!(output.contains(r#""name":"app.main""#));
        assert!(!output.contains("NS9999"));
        assert!(output.contains(r#""diagnostics":[]"#));
    }

    #[test]
    fn check_reports_resolution_diagnostics() {
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":1,"method":"check","params":{"apiVersion":"1.0","source":{"path":"src/main.ns","text":"private fn secret() {}\nexport { secret }\n"},"target":"js","mode":"core"}}"#,
        );
        assert!(output.contains("NS3001"));
        assert!(!output.contains("NS9999"));
    }

    #[test]
    fn check_reports_type_diagnostics() {
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":1,"method":"check","params":{"apiVersion":"1.0","source":{"path":"src/main.ns","text":"fn wrong() -> i32 { return \"no\" }\n"},"target":"js","mode":"core"}}"#,
        );
        assert!(output.contains("NS4001"));
        assert!(output.contains(r#""program":null"#));
    }

    #[test]
    fn check_reports_mode_diagnostics() {
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":1,"method":"check","params":{"apiVersion":"1.0","source":{"path":"src/main.ns","text":"@requires(system)\nmodule app.main\npub fn main() {}\n"},"target":"js","mode":"core"}}"#,
        );
        assert!(output.contains("NS5001"));
        assert!(output.contains(r#""program":null"#));
    }

    #[test]
    fn compile_returns_typed_ir() {
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":1,"method":"compile","params":{"apiVersion":"1.0","source":{"path":"src/main.ns","text":"module app.main\npub fn add(a: i32, b: i32) -> i32 { return a + b }\n"},"target":"js","mode":"core"}}"#,
        );
        assert!(output.contains(r#""apiVersion":"1.0""#), "{output}");
        assert!(output.contains(r#""ir":{"#), "{output}");
        assert!(output.contains(r#""key":"src/main.ns""#), "{output}");
        assert!(output.contains(r#""integerPolicy":"wrapping""#), "{output}");
        assert!(output.contains(r#""annotations":[{"#), "{output}");
        assert!(!output.contains("NS9999"), "{output}");
    }

    #[test]
    fn compile_rejects_an_unsupported_backend_target() {
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":1,"method":"compile","params":{"apiVersion":"1.0","source":{"path":"src/main.ns","text":"fn main() {}\n"},"target":"native","mode":"core"}}"#,
        );
        assert!(output.contains("NS8001"), "{output}");
        assert!(!output.contains(r#""ir""#), "{output}");
    }

    #[test]
    fn check_omits_typed_ir() {
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":1,"method":"check","params":{"apiVersion":"1.0","source":{"path":"src/main.ns","text":"fn main() {}\n"},"target":"js","mode":"core"}}"#,
        );
        assert!(!output.contains(r#""ir""#), "{output}");
    }

    #[test]
    fn malformed_requests_use_json_rpc_errors() {
        let output = process_json("not json");
        assert!(output.contains("-32700"));
    }

    #[test]
    fn lexical_errors_are_reported_before_parsing() {
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":1,"method":"check","params":{"apiVersion":"1.0","source":{"path":"src/main.ns","text":"/* invalid */"},"target":"js","mode":"core"}}"#,
        );
        assert!(output.contains("NS1001"));
        assert!(!output.contains("NS9999"));
    }

    #[test]
    fn format_returns_canonical_source() {
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":"fmt","method":"format","params":{"apiVersion":"1.0","source":{"path":"src/main.ns","text":"// doc\npublic function f() {\n    // body\n    var mut x = 1 // value\n}\n"},"target":"js","mode":"core"}}"#,
        );
        assert!(output.contains(r#""id":"fmt""#));
        assert!(output.contains("pub fn f()"));
        assert!(output.contains("let mut x = 1"));
        assert!(output.contains("// doc"));
        assert!(output.contains("// body"));
        assert!(output.contains("// value"));
        assert!(!output.contains("public function"));
    }

    #[test]
    fn format_accepts_explicit_edition_and_defaults_to_v0_1() {
        let base = r#"{"jsonrpc":"2.0","id":"fmt","method":"format","params":{"apiVersion":"1.0","source":{"path":"src/main.ns","text":"pub fn f() { var x = 1 }\n"},"target":"js""#;
        let default_output = process_json(&format!("{base}}}}}"));
        let explicit_output = process_json(&format!("{base},\"edition\":\"0.2\"}}}}"));
        assert_eq!(default_output, explicit_output);
        assert!(default_output.contains("pub fn f()"));
    }

    #[test]
    fn parse_errors_are_reported_before_resolution() {
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":1,"method":"check","params":{"apiVersion":"1.0","source":{"path":"src/main.ns","text":"module app.main\nfn main( {\n"},"target":"js","mode":"core"}}"#,
        );
        assert!(output.contains("NS2001"));
        assert!(!output.contains("NS9999"));
    }

    #[test]
    fn initialize_reports_check_project_command() {
        let output = process_json(r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#);
        assert!(
            output.contains("check_project"),
            "initialize should list check_project"
        );
        assert!(
            output.contains("project-checker"),
            "initialize should list project-checker feature"
        );
    }

    #[test]
    fn check_infers_a_mode_when_one_is_omitted() {
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":1,"method":"check","params":{"apiVersion":"1.0","source":{"path":"src/main.ns","text":"@tags(fetch)\nmodule app.main\npub fn main() {}\n"},"target":"js"}}"#,
        );
        assert!(output.contains(r#""mode":"web""#), "{output}");
        assert!(output.contains(r#""modeSource":"inferred""#), "{output}");
        assert!(output.contains("fetch"), "{output}");
    }

    #[test]
    fn explicit_mode_remains_authoritative() {
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":1,"method":"check","params":{"apiVersion":"1.0","source":{"path":"src/main.ns","text":"@tags(fetch)\nmodule app.main\npub fn main() {}\n"},"target":"js","mode":"web"}}"#,
        );
        assert!(output.contains(r#""mode":"web""#), "{output}");
        assert!(output.contains(r#""modeSource":"explicit""#), "{output}");
    }

    #[test]
    fn check_project_infers_the_least_privileged_mode() {
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":2,"method":"check_project","params":{"apiVersion":"1.0","sources":[{"key":"core","path":"src/core.ns","text":"pub fn helper() {}\n"},{"key":"web","path":"src/web.ns","text":"@tags(fetch)\npub fn load() {}\n"}],"target":"js"}}"#,
        );
        assert!(output.contains(r#""ok":true"#), "{output}");
        assert!(output.contains(r#""mode":"web""#), "{output}");
        assert!(output.contains(r#""modeSource":"inferred""#), "{output}");
        assert!(output.contains(r#""core":"core""#), "{output}");
        assert!(output.contains(r#""web":"web""#), "{output}");
    }

    #[test]
    fn check_project_rejects_web_and_system_requirements() {
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":2,"method":"check_project","params":{"apiVersion":"1.0","sources":[{"key":"web","path":"src/web.ns","text":"@tags(fetch)\npub fn load() {}\n"},{"key":"native","path":"src/native.ns","text":"@tags(fs)\npub fn read() {}\n"}],"target":"js"}}"#,
        );
        assert!(output.contains(r#""ok":false"#), "{output}");
        assert!(output.contains("NS5006"), "{output}");
    }

    #[test]
    fn check_project_infers_the_minimum_compatible_mode() {
        // With no explicit --mode, MODE-001 inference selects the least mode
        // that satisfies all declared requirements. `@tags(fetch)` requires
        // web, so the inferred mode should be web and the project should pass.
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":2,"method":"check_project","params":{"apiVersion":"1.0","sources":[{"key":"core","path":"src/core.ns","text":"@requires(core)\npub fn helper() {}\n"},{"key":"web","path":"src/web.ns","text":"@tags(fetch)\npub fn load() {}\n"}],"target":"js"}}"#,
        );
        assert!(
            output.contains(r#""ok":true"#),
            "inferred-mode check should pass when the least mode satisfies requirements: {output}"
        );
        // The metadata should record that the mode was inferred and what it landed on.
        assert!(
            output.contains(r#""modeSource":"inferred""#),
            "expected modeSource inferred: {output}"
        );
        assert!(
            output.contains(r#""requiredMode":"web""#),
            "expected requiredMode web: {output}"
        );
    }

    #[test]
    fn check_project_rejects_an_explicit_mode_that_is_too_narrow() {
        // When the caller pins --mode core but a module requires web via a
        // capability tag, the project must fail with NS5003 (capability
        // requirement not satisfied by the selected mode).
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":2,"method":"check_project","params":{"apiVersion":"1.0","sources":[{"key":"core","path":"src/core.ns","text":"@requires(core)\npub fn helper() {}\n"},{"key":"web","path":"src/web.ns","text":"@tags(fetch)\npub fn load() {}\n"}],"target":"js","mode":"core"}}"#,
        );
        assert!(output.contains(r#""ok":false"#), "{output}");
        assert!(
            output.contains("NS5003") || output.contains("NS5001"),
            "expected NS5001 or NS5003 when an explicit core mode is too narrow for a web requirement: {output}"
        );
    }

    #[test]
    fn check_project_returns_ok_for_valid_two_module_project() {
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":2,"method":"check_project","params":{"apiVersion":"1.0","sources":[{"key":"lib","path":"src/lib.ns","text":"pub fn add(a: i32, b: i32) -> i32 { return a + b }\n"},{"key":"main","path":"src/main.ns","text":"import { add } from \"lib\"\npub fn main() -> i32 { return add(1, 2) }\n"}],"target":"js","mode":"core"}}"#,
        );
        assert!(
            output.contains(r#""ok":true"#),
            "two-module valid project should pass: {output}"
        );
        assert!(output.contains(r#""apiVersion":"1.0""#));
        assert!(output.contains(r#""key":"lib""#));
        assert!(output.contains(r#""key":"main""#));
    }

    #[test]
    fn check_project_resolves_relative_imports_and_returns_ir() {
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":2,"method":"check_project","params":{"apiVersion":"1.0","sources":[{"key":"src/main.ns","path":"src/main.ns","text":"import { add } from \"./lib\"\npub fn main() -> i32 { return add(1, 2) }\n"},{"key":"src/lib.ns","path":"src/lib.ns","text":"pub fn add(a: i32, b: i32) -> i32 { return a + b }\n"}],"target":"js","mode":"core","integerPolicy":"safe-checked"}}"#,
        );
        assert!(output.contains(r#""ok":true"#), "{output}");
        assert!(output.contains(r#""key":"src/lib.ns""#), "{output}");
        assert!(output.contains(r#""key":"src/main.ns""#), "{output}");
        assert!(output.contains(r#""ir":{"#), "{output}");
        assert!(
            output.contains(r#""integerPolicy":"safe-checked""#),
            "{output}"
        );
    }

    #[test]
    fn check_project_catches_cross_module_type_error() {
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":3,"method":"check_project","params":{"apiVersion":"1.0","sources":[{"key":"lib","path":"src/lib.ns","text":"pub fn greet(name: str) -> str { return name }\n"},{"key":"main","path":"src/main.ns","text":"import { greet } from \"lib\"\npub fn run() -> str { return greet(42) }\n"}],"target":"js","mode":"core"}}"#,
        );
        assert!(
            output.contains(r#""ok":false"#),
            "cross-module type error should fail: {output}"
        );
        assert!(
            output.contains("NS4001"),
            "expected NS4001 type mismatch: {output}"
        );
    }

    #[test]
    fn check_project_catches_wrong_argument_count_across_modules() {
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":4,"method":"check_project","params":{"apiVersion":"1.0","sources":[{"key":"math","path":"src/math.ns","text":"pub fn double(n: i32) -> i32 { return n * 2 }\n"},{"key":"main","path":"src/main.ns","text":"import { double } from \"math\"\npub fn run() -> i32 { return double(1, 2, 3) }\n"}],"target":"js","mode":"core"}}"#,
        );
        assert!(
            output.contains(r#""ok":false"#),
            "wrong argument count should fail: {output}"
        );
        assert!(output.contains("NS4004"), "expected NS4004: {output}");
    }

    #[test]
    fn check_project_reports_module_results_in_request_order() {
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":5,"method":"check_project","params":{"apiVersion":"1.0","sources":[{"key":"a","path":"a.ns","text":"pub fn f() {}\n"},{"key":"b","path":"b.ns","text":"pub fn g() {}\n"},{"key":"c","path":"c.ns","text":"pub fn h() {}\n"}],"target":"js","mode":"core"}}"#,
        );
        assert!(output.contains(r#""ok":true"#));
        assert!(output.contains(r#""key":"a""#));
        assert!(output.contains(r#""key":"b""#));
        assert!(output.contains(r#""key":"c""#));
    }

    #[test]
    fn check_project_rejects_empty_sources() {
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":6,"method":"check_project","params":{"apiVersion":"1.0","sources":[],"target":"js","mode":"core"}}"#,
        );
        assert!(
            output.contains("-32602"),
            "empty sources should be a params error: {output}"
        );
    }

    #[test]
    fn check_project_reports_parse_errors_per_module() {
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":7,"method":"check_project","params":{"apiVersion":"1.0","sources":[{"key":"broken","path":"broken.ns","text":"fn main( {\n"},{"key":"fine","path":"fine.ns","text":"pub fn ok() {}\n"}],"target":"js","mode":"core"}}"#,
        );
        assert!(
            output.contains(r#""ok":false"#),
            "parse error should fail project: {output}"
        );
        assert!(output.contains("NS2001"), "expected NS2001: {output}");
    }

    #[test]
    fn compile_project_emits_one_translation_unit_per_module() {
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":10,"method":"compile_project","params":{"apiVersion":"1.0","sources":[{"key":"lib","path":"src/lib.ns","text":"pub fn add(a: i32, b: i32) -> i32 { return a + b }\n"},{"key":"app/main","path":"src/main.ns","text":"import { add } from \"lib\"\npub fn main() -> i32 { return add(1, 2) }\n"}],"target":"c","mode":"core"}}"#,
        );
        assert!(output.contains(r#""ok":true"#), "{output}");
        // One module artifact per source.
        assert!(output.contains(r#""cFilename":"lib.c""#), "{output}");
        assert!(output.contains(r#""cFilename":"app_main.c""#), "{output}");
        // The dependency's symbol is module-qualified and the caller includes
        // its header.
        assert!(output.contains("ns_lib_add"), "{output}");
        // JSON escapes the quotes in the emitted `#include "lib.h"` line.
        assert!(output.contains(r#"#include \"lib.h\""#), "{output}");
    }

    #[test]
    fn compile_project_resolves_package_import_map() {
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":13,"method":"compile_project","params":{"apiVersion":"1.0","sources":[{"key":"deps/mathlib/src/lib.ns","path":"packages/mathlib/src/lib.ns","text":"pub fn add(a: i32, b: i32) -> i32 { return a + b }\n"},{"key":"src/main.ns","path":"src/main.ns","text":"import { add } from \"mathlib\"\npub fn main() -> i32 { return add(1, 2) }\n"}],"importMap":{"mathlib":"deps/mathlib/src/lib.ns"},"target":"c","mode":"core"}}"#,
        );
        assert!(output.contains(r#""ok":true"#), "{output}");
        assert!(
            output.contains("ns_deps_mathlib_src_lib_add"),
            "package symbol missing: {output}"
        );
        assert!(
            output.contains("deps_mathlib_src_lib.h"),
            "package header include missing: {output}"
        );
    }

    #[test]
    fn compile_project_reports_cross_module_diagnostics() {
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":11,"method":"compile_project","params":{"apiVersion":"1.0","sources":[{"key":"lib","path":"src/lib.ns","text":"pub const MAX: i32 = 10\n"},{"key":"app/main","path":"src/main.ns","text":"import { MAX } from \"lib\"\npub fn main() -> i32 { return MAX }\n"}],"target":"c","mode":"core"}}"#,
        );
        assert!(output.contains(r#""ok":false"#), "{output}");
        assert!(output.contains("NS8019"), "expected NS8019: {output}");
    }

    #[test]
    fn compile_project_rejects_empty_sources() {
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":12,"method":"compile_project","params":{"apiVersion":"1.0","sources":[],"target":"c","mode":"core"}}"#,
        );
        assert!(
            output.contains("-32602"),
            "empty sources should be a JSON-RPC invalid-params error: {output}"
        );
        assert!(output.contains("at least one source unit"), "{output}");
    }

    #[test]
    fn ast_returns_the_surface_tree() {
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":21,"method":"ast","params":{"apiVersion":"1.0","source":{"path":"src/main.ns","text":"module app.main\npub fn add(a: i32, b: i32) -> i32 { return a + b }\n"},"target":"js","mode":"core"}}"#,
        );
        assert!(output.contains(r#""ok":true"#), "{output}");
        assert!(output.contains(r#""apiVersion":"1.0""#), "{output}");
        assert!(output.contains(r#""ast""#), "{output}");
        // The surface tree carries declaration names, not just a blank module.
        assert!(output.contains(r#""add""#), "{output}");
    }

    #[test]
    fn ast_reports_parse_errors_without_failing_the_rpc_call() {
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":22,"method":"ast","params":{"apiVersion":"1.0","source":{"path":"src/broken.ns","text":"fn main( {\n"},"target":"js","mode":"core"}}"#,
        );
        assert!(output.contains(r#""ok":false"#), "{output}");
        assert!(output.contains("NS2001"), "expected NS2001: {output}");
        assert!(
            !output.contains(r#""ast""#),
            "failed ast must omit the tree: {output}"
        );
    }

    #[test]
    fn explain_returns_mode_evidence_and_rendering() {
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":23,"method":"explain","params":{"apiVersion":"1.0","source":{"path":"src/main.ns","text":"@tags(fetch)\nmodule app.main\npub fn load() {}\n"},"target":"js"}}"#,
        );
        assert!(output.contains(r#""ok":true"#), "{output}");
        assert!(output.contains(r#""requiredMode":"web""#), "{output}");
        assert!(output.contains(r#""reasons""#), "{output}");
        assert!(output.contains(r#""explanation""#), "{output}");
        assert!(
            output.contains("fetch"),
            "capability evidence missing: {output}"
        );
    }

    #[test]
    fn explain_reports_parse_errors() {
        let output = process_json(
            r#"{"jsonrpc":"2.0","id":24,"method":"explain","params":{"apiVersion":"1.0","source":{"path":"src/broken.ns","text":"fn main( {\n"},"target":"js"}}"#,
        );
        assert!(output.contains(r#""ok":false"#), "{output}");
        assert!(output.contains("NS2001"), "expected NS2001: {output}");
    }
}
