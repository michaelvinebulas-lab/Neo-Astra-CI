//! Versioned compiler protocol types.
//!
//! The native binary and Wasm entrypoint share this JSON-RPC 2.0 boundary. The
//! single-source frontend parses and resolves source until the type checker
//! and code generator land.

use std::collections::BTreeMap;

use astra_capabilities::{Mode, ModeInference, infer_module_mode_named, infer_project};
use astra_core::checker::{CheckOptions, TypeAnnotation, check_resolved};
use astra_core::diagnostic::Diagnostic;
use astra_core::formatter::format_source;
use astra_core::ir::Program;
use astra_core::parser::parse;
use astra_core::resolver::resolve_module;
use astra_core::type_surface::{ProjectModule, check_module_graph};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

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
    /// Explicit project mode. When omitted, the project graph is inferred.
    #[serde(default)]
    pub mode: Option<Mode>,
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
    let mut path_map: BTreeMap<String, String> = BTreeMap::new();
    let mut early_failures: Vec<ModuleCheckResult> = Vec::new();

    for unit in &request.sources {
        path_map.insert(unit.key.clone(), unit.path.clone());
        match parse(&unit.text) {
            Ok(module) => {
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

    let graph = check_module_graph(&project_modules, CheckOptions::new(selected_mode));

    let ok = graph.is_ok();

    // Collect per-module results in request order.
    let mut module_results: BTreeMap<String, ModuleCheckResult> = BTreeMap::new();
    for (key, _result) in &graph.modules {
        module_results.insert(
            key.clone(),
            ModuleCheckResult {
                key: key.clone(),
                path: path_map.get(key).cloned().unwrap_or_else(|| key.clone()),
                diagnostics: Vec::new(),
                ok: true,
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

// ─── Single-source frontend ───────────────────────────────────────────────────

#[derive(Debug, Default, Clone, Copy)]
pub struct FrontendCompiler;

impl FrontendCompiler {
    /// Checks and attaches typed IR for one backend target.
    #[must_use]
    pub fn compile(&self, request: &CompileRequest) -> CompileResponse {
        if !matches!(request.target.as_str(), "js" | "ts") {
            return CompileResponse::failure(vec![
                Diagnostic::error(
                    "NS8001",
                    format!(
                        "Target '{}' is not supported by the typed source backend.",
                        request.target
                    ),
                )
                .with_hint("Use target 'js' or 'ts'."),
            ])
            .with_metadata(json!({ "target": request.target }));
        }

        self.run(request, true)
    }

    fn run(&self, request: &CompileRequest, typed: bool) -> CompileResponse {
        match parse(&request.source.text) {
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
                        match check_resolved(&module, &resolved, CheckOptions::new(selected_mode)) {
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
                                    response = response.with_ir(ModuleIr {
                                        key: request.source.path.clone(),
                                        path: request.source.path.clone(),
                                        ast: module,
                                        hir: checked.module,
                                        annotations: checked.annotations,
                                        manual_memory_scopes: checked.manual_memory_scopes,
                                        integer_policy: request.integer_policy,
                                    });
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
                "commands": ["initialize", "check", "check_project", "compile", "ast", "format", "explain", "shutdown"],
                "features": ["resolver", "type-checker", "project-checker", "typed-ir"],
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
        "format" => match parse_compile_request(request.params) {
            Ok(compile_request) => {
                let response = match format_source(&compile_request.source.text) {
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
        "ast" | "explain" => JsonRpcResponse::error(
            request.id,
            -32000,
            format!("{} is not implemented in the scaffold.", request.method),
        ),
        _ => JsonRpcResponse::error(request.id, -32601, "Method not found."),
    };

    serialize_response(response)
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
}
