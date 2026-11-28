//! Project module loading, resolution, checking, and runtime linking.
//!
//! Module identity is the canonical project-relative path (for example
//! `src/main.ns`), not the raw import specifier. The loader discovers local
//! modules from imports and re-exports, rejects cycles before resolution, and
//! feeds each dependency's type surface into the modules that import it.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use astra_core::ast;
use astra_core::checker::{CheckOptions, CheckResult, check_resolved};
use astra_core::diagnostic::Diagnostic;
use astra_core::hir;
use astra_core::mode::Mode;
use astra_core::parser::parse;
use astra_core::resolver::{ModuleGraph, ModuleResolver, collect_module_specifiers};
use astra_core::span::Span;
use astra_core::type_surface::{ModuleTypeSurface, module_type_surface_with_surfaces};

use crate::error::NovaError;
use crate::host::{Host, StdHost};
use crate::interpreter::{Interpreter, NovaOptions};
use crate::value::Value;
use astra_capabilities::{infer_module_mode_named, infer_project};

use Diagnostic as CoreDiagnostic;

/// A canonical project-relative module identity.
///
/// Identities always use `/`, are relative to the project root, and end in
/// `.ns`. They never contain `.` or `..` components.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModuleId(String);

impl ModuleId {
    /// Parses and validates a canonical module identity.
    pub fn parse(value: impl Into<String>) -> Result<Self, ProjectError> {
        let value = value.into();
        let id = Self(value.clone());
        if let Some(message) = id.validation_error() {
            return Err(ProjectError::InvalidModule { id: value, message });
        }
        Ok(id)
    }

    /// Returns the canonical identity.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the module directory, or an empty string at the project root.
    #[must_use]
    pub fn parent(&self) -> &str {
        self.0.rsplit_once('/').map_or("", |(parent, _)| parent)
    }

    /// Returns the file name without the `.ns` extension.
    #[must_use]
    pub fn stem(&self) -> &str {
        let name = self.0.rsplit('/').next().unwrap_or(&self.0);
        name.strip_suffix(".ns").unwrap_or(name)
    }

    fn validation_error(&self) -> Option<String> {
        let value = &self.0;
        if value.is_empty() {
            return Some("module identity cannot be empty".to_owned());
        }
        if value.contains('\\') {
            return Some("module identity must use forward slashes".to_owned());
        }
        if value.starts_with('/') || value.contains(':') {
            return Some("module identity must be project-relative".to_owned());
        }
        if !value.ends_with(".ns") {
            return Some("module identity must end in '.ns'".to_owned());
        }
        for component in value.split('/') {
            if component.is_empty() || component == "." || component == ".." {
                return Some("module identity contains an invalid path component".to_owned());
            }
        }
        None
    }
}

impl fmt::Display for ModuleId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A diagnostic attached to one project module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectDiagnostic {
    /// Canonical module identity.
    pub module: ModuleId,
    /// Filesystem path shown to the user.
    pub path: PathBuf,
    /// Original frontend/loader diagnostic.
    pub diagnostic: Diagnostic,
}

/// A linked project compiled and checked in memory.
#[derive(Debug, Clone)]
pub struct Project {
    /// Canonical project root.
    pub root: PathBuf,
    /// Entry module.
    pub entry: ModuleId,
    /// Mode selected for checking and execution.
    pub mode: Mode,
    /// Every module, in canonical identity order.
    pub modules: BTreeMap<ModuleId, CompiledModule>,
}

/// One parsed and checked module.
#[derive(Debug, Clone)]
pub struct CompiledModule {
    pub id: ModuleId,
    pub path: PathBuf,
    pub text: Rc<str>,
    pub ast: ast::Module,
    pub resolved: hir::Module,
    pub checked: CheckResult,
}

/// Options for loading a project from disk.
#[derive(Debug, Clone)]
pub struct ProjectOptions {
    pub root: PathBuf,
    pub entry: PathBuf,
    pub mode: Mode,
    pub infer_mode: bool,
    pub allow_root_escape: bool,
}

/// A project loading, checking, or runtime failure.
#[derive(Debug, Clone)]
pub enum ProjectError {
    /// Loader, resolver, or type-checker diagnostics.
    Diagnostics(Vec<ProjectDiagnostic>),
    /// A runtime or unsupported-module failure tied to one module.
    Runtime {
        module: ModuleId,
        path: PathBuf,
        error: Box<NovaError>,
    },
    /// A caller supplied an invalid `ModuleId`.
    InvalidModule { id: String, message: String },
}

impl ProjectError {
    /// Returns all frontend diagnostics when this is a diagnostic failure.
    #[must_use]
    pub fn diagnostics(&self) -> &[ProjectDiagnostic] {
        match self {
            Self::Diagnostics(diagnostics) => diagnostics,
            Self::Runtime { .. } | Self::InvalidModule { .. } => &[],
        }
    }

    /// Returns the stable code for non-diagnostic failures.
    #[must_use]
    pub fn code(&self) -> Option<&'static str> {
        match self {
            Self::Diagnostics(_) => None,
            Self::Runtime { error, .. } => Some(error.code()),
            Self::InvalidModule { .. } => Some("NS3012"),
        }
    }
}

impl fmt::Display for ProjectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Diagnostics(diagnostics) => {
                formatter.write_str("the project did not pass the frontend.")?;
                for diagnostic in diagnostics {
                    write!(
                        formatter,
                        "\n{}: [{}{}]: {}",
                        diagnostic.path.display(),
                        diagnostic.diagnostic.severity.as_str(),
                        diagnostic.diagnostic.code,
                        diagnostic.diagnostic.message
                    )?;
                }
                Ok(())
            }
            Self::Runtime {
                module,
                path,
                error,
            } => write!(formatter, "{} ({module}): {error}", path.display()),
            Self::InvalidModule { id, message } => {
                write!(formatter, "invalid module id '{id}': {message}")
            }
        }
    }
}

impl std::error::Error for ProjectError {}

/// Loads and checks a project from the filesystem.
pub fn load_project(options: ProjectOptions) -> Result<Project, ProjectError> {
    let root = fs::canonicalize(&options.root).map_err(|error| {
        io_error(
            None,
            &options.root,
            "NS9008",
            format!("Unable to access the project root: {error}"),
        )
    })?;
    let entry_path = fs::canonicalize(&options.entry).map_err(|error| {
        io_error(
            None,
            &options.entry,
            "NS9008",
            format!("Unable to access the entry module: {error}"),
        )
    })?;
    let Ok(relative) = entry_path.strip_prefix(&root) else {
        if !options.allow_root_escape {
            return Err(io_error(
                None,
                &options.entry,
                "NS9008",
                "The entry module is outside the project root.".to_owned(),
            ));
        }
        return Err(ProjectError::InvalidModule {
            id: entry_path.display().to_string(),
            message: "entry files outside the root cannot be given a canonical module id"
                .to_owned(),
        });
    };
    let entry = module_id_from_path(relative).map_err(|message| ProjectError::InvalidModule {
        id: relative.display().to_string(),
        message,
    })?;

    let mut sources = SourceSet::from_disk(root.clone());
    sources.ensure(&entry, None)?;
    assemble(entry, options.mode, options.infer_mode, sources)
}

/// Builds a project from an in-memory source map.
///
/// This bypasses the filesystem while using the same graph, resolver, and
/// checker path as [`load_project`]. It is the deterministic test constructor.
pub fn project_from_sources(
    entry: ModuleId,
    sources: BTreeMap<ModuleId, String>,
    options: NovaOptions,
) -> Result<Project, ProjectError> {
    let mut source_set = SourceSet::from_memory(PathBuf::new());
    for (id, text) in sources {
        if let Some(message) = id.validation_error() {
            return Err(ProjectError::InvalidModule {
                id: id.to_string(),
                message,
            });
        }
        if let Some(previous) = source_set
            .case_ids
            .insert(id.as_str().to_lowercase(), id.clone())
            && previous != id
        {
            return Err(ProjectError::Diagnostics(vec![ProjectDiagnostic {
                module: previous.clone(),
                path: PathBuf::from(previous.as_str()),
                diagnostic: CoreDiagnostic::error(
                    "NS3012",
                    format!(
                        "Module ids '{}' and '{}' collide case-insensitively.",
                        previous.as_str(),
                        id.as_str()
                    ),
                ),
            }]));
        }
        source_set
            .files
            .insert(id.clone(), (PathBuf::from(id.as_str()), text));
    }
    assemble(entry, options.mode, options.infer_mode, source_set)
}

/// Builds a linked interpreter for a checked project.
pub fn build_project_interpreter(
    project: Project,
    options: NovaOptions,
    host: Box<dyn Host>,
) -> Result<Interpreter, ProjectError> {
    let entry = project.entry.clone();
    for (id, module) in &project.modules {
        if let Some(span) = module.checked.manual_memory_scopes.first() {
            return Err(ProjectError::Runtime {
                module: id.clone(),
                path: module.path.clone(),
                error: Box::new(NovaError::ManualMemoryUnsupported { span: *span }),
            });
        }
    }
    let entry_path = project
        .modules
        .get(&entry)
        .map_or_else(PathBuf::new, |module| module.path.clone());
    let checked = project
        .modules
        .into_iter()
        .map(|(id, module)| (id.to_string(), module.checked))
        .collect();
    Interpreter::from_modules(entry.to_string(), checked, options, host).map_err(|error| {
        ProjectError::Runtime {
            module: entry,
            path: entry_path,
            error: Box::new(error),
        }
    })
}

/// Loads and executes a project from disk.
pub fn run_project(entry: impl AsRef<Path>, options: NovaOptions) -> Result<Value, ProjectError> {
    let entry = entry.as_ref();
    let root = project_root(entry);
    let project = load_project(ProjectOptions {
        root,
        entry: entry.to_path_buf(),
        mode: options.mode,
        infer_mode: options.infer_mode,
        allow_root_escape: false,
    })?;
    let entry_id = project.entry.clone();
    let module_paths = project
        .modules
        .iter()
        .map(|(id, module)| (id.to_string(), module.path.clone()))
        .collect::<BTreeMap<_, _>>();
    let fallback_path = module_paths
        .get(entry_id.as_str())
        .cloned()
        .unwrap_or_else(|| entry.to_path_buf());
    let mut interpreter = build_project_interpreter(project, options, Box::new(StdHost))?;
    match interpreter.run() {
        Ok(value) => Ok(value),
        Err(error) => {
            let module = interpreter
                .fault_module()
                .and_then(|id| ModuleId::parse(id).ok())
                .unwrap_or_else(|| module_for_error(&interpreter, &entry_id));
            let path = module_paths
                .get(module.as_str())
                .cloned()
                .unwrap_or(fallback_path);
            Err(ProjectError::Runtime {
                module,
                path,
                error: Box::new(error),
            })
        }
    }
}

fn module_for_error(interpreter: &Interpreter, fallback: &ModuleId) -> ModuleId {
    if let Ok(module) = ModuleId::parse(interpreter.active_module()) {
        return module;
    }
    fallback.clone()
}

/// Chooses the project root for a direct file invocation.
///
/// A project is normally the current directory, which allows imports to walk up
/// from a nested entry file (`nova run src/main.ns`). If the entry is outside the
/// current directory, its parent is the best available root.
fn project_root(entry: &Path) -> PathBuf {
    let canonical = fs::canonicalize(entry).unwrap_or_else(|_| entry.to_path_buf());
    if let Ok(current) = env::current_dir() {
        let current = fs::canonicalize(&current).unwrap_or(current);
        if canonical.starts_with(&current) {
            return current;
        }
    }
    entry
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

// ─── Source discovery ────────────────────────────────────────────────────────

#[derive(Debug)]
struct SourceSet {
    root: PathBuf,
    from_disk: bool,
    files: BTreeMap<ModuleId, (PathBuf, String)>,
    case_ids: BTreeMap<String, ModuleId>,
}

impl SourceSet {
    fn from_disk(root: PathBuf) -> Self {
        Self {
            root,
            from_disk: true,
            files: BTreeMap::new(),
            case_ids: BTreeMap::new(),
        }
    }

    fn from_memory(root: PathBuf) -> Self {
        Self {
            root,
            from_disk: false,
            files: BTreeMap::new(),
            case_ids: BTreeMap::new(),
        }
    }

    fn ensure(
        &mut self,
        id: &ModuleId,
        import: Option<(ModuleId, PathBuf, Span)>,
    ) -> Result<(), ProjectError> {
        if self.files.contains_key(id) {
            return Ok(());
        }
        if !self.from_disk {
            return Err(missing_module(import, id));
        }

        let path = self.root.join(id.as_str());
        let bytes = fs::read(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                match import.clone() {
                    Some(_) => missing_module(import.clone(), id),
                    None => io_error(
                        None,
                        &path,
                        "NS9008",
                        format!("Unable to read the entry module '{}': {error}", id),
                    ),
                }
            } else {
                io_error(
                    import.clone().map(|(module, path, _)| (module, path)),
                    &path,
                    "NS9008",
                    format!("Unable to read module '{}': {error}", id),
                )
            }
        })?;
        let canonical = fs::canonicalize(&path).map_err(|error| {
            io_error(
                import.clone().map(|(module, path, _)| (module, path)),
                &path,
                "NS9008",
                format!("Unable to canonicalize module '{}': {error}", id),
            )
        })?;
        if !canonical.starts_with(&self.root) {
            return Err(io_error(
                import.map(|(module, path, _)| (module, path)),
                &path,
                "NS3012",
                format!("Module '{}' escapes the project root.", id),
            ));
        }

        let relative = canonical
            .strip_prefix(&self.root)
            .expect("containment was checked");
        let actual = module_id_from_path(relative).map_err(|message| {
            io_error(
                None,
                &path,
                "NS3012",
                format!("Module '{}' has an invalid canonical id: {message}.", id),
            )
        })?;
        if actual != *id {
            return Err(io_error(
                None,
                &path,
                "NS3012",
                format!(
                    "Module import '{}' resolves to '{}'. Import paths must match the canonical file path, including case.",
                    id.as_str(),
                    actual.as_str()
                ),
            ));
        }
        if let Some(existing) = self.case_ids.get(&id.as_str().to_lowercase())
            && existing != id
        {
            return Err(io_error(
                None,
                &path,
                "NS3012",
                format!(
                    "Module ids '{}' and '{}' collide case-insensitively.",
                    existing.as_str(),
                    id.as_str()
                ),
            ));
        }
        self.case_ids.insert(id.as_str().to_lowercase(), id.clone());

        let text = String::from_utf8(bytes).map_err(|_| {
            io_error(
                import.map(|(module, path, _)| (module, path)),
                &path,
                "NS9007",
                format!("Module '{}' is not valid UTF-8.", id),
            )
        })?;
        self.files.insert(id.clone(), (path, text));
        Ok(())
    }

    fn text(&self, id: &ModuleId) -> &str {
        &self
            .files
            .get(id)
            .expect("source must be ensured before discovery")
            .1
    }

    fn path(&self, id: &ModuleId) -> PathBuf {
        self.files
            .get(id)
            .map_or_else(|| PathBuf::from(id.as_str()), |(path, _)| path.clone())
    }
}

fn assemble(
    entry: ModuleId,
    mode: Mode,
    infer_mode: bool,
    mut sources: SourceSet,
) -> Result<Project, ProjectError> {
    let mut parsed: BTreeMap<ModuleId, ast::Module> = BTreeMap::new();
    let mut graph = ModuleGraph::default();
    let mut visiting = Vec::new();
    let mut done = BTreeSet::new();
    let mut order = Vec::new();
    let mut diagnostics = Vec::new();

    discover(
        &entry,
        &mut sources,
        &mut parsed,
        &mut graph,
        &mut visiting,
        &mut done,
        &mut order,
        &mut diagnostics,
    )?;

    if !diagnostics.is_empty() {
        return Err(diagnostic_error(diagnostics));
    }

    let mode = if infer_mode {
        let mut inferences = Vec::new();
        for (id, module) in &parsed {
            let inference = infer_module_mode_named(module, id.as_str());
            if !inference.is_ok() {
                diagnostics.extend(inference.diagnostics.iter().cloned().map(|diagnostic| {
                    ProjectDiagnostic {
                        module: id.clone(),
                        path: sources.path(id),
                        diagnostic,
                    }
                }));
            }
            inferences.push(inference);
        }
        let joined = infer_project(&inferences);
        let resolved = joined.resolved_mode();
        if !joined.is_ok() {
            diagnostics.extend(joined.diagnostics.into_iter().map(|diagnostic| {
                ProjectDiagnostic {
                    module: entry.clone(),
                    path: sources.path(&entry),
                    diagnostic,
                }
            }));
        }
        if !diagnostics.is_empty() {
            return Err(diagnostic_error(diagnostics));
        }
        resolved
    } else {
        mode
    };

    let resolver = ModuleResolver::from_graph(&graph);
    let mut surfaces: BTreeMap<String, ModuleTypeSurface> = BTreeMap::new();
    let mut checked: BTreeMap<ModuleId, CheckResult> = BTreeMap::new();

    for id in &order {
        let module = parsed.get(id).expect("order only names parsed modules");
        let resolved = match resolver.resolve_root(id.as_str()) {
            Ok(resolved) => resolved,
            Err(errors) => {
                diagnostics.extend(errors.into_iter().map(|diagnostic| ProjectDiagnostic {
                    module: id.clone(),
                    path: sources.path(id),
                    diagnostic,
                }));
                continue;
            }
        };
        let options = CheckOptions::with_surfaces(mode, surfaces.clone());
        match check_resolved(module, &resolved, options) {
            Ok(result) => {
                let surface = module_type_surface_with_surfaces(&result, &surfaces);
                surfaces.insert(id.to_string(), surface);
                checked.insert(id.clone(), result);
            }
            Err(errors) => {
                diagnostics.extend(errors.into_iter().map(|diagnostic| ProjectDiagnostic {
                    module: id.clone(),
                    path: sources.path(id),
                    diagnostic,
                }));
            }
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostic_error(diagnostics));
    }

    let mut modules = BTreeMap::new();
    for (id, module) in parsed {
        let resolved = resolver.resolve_root(id.as_str()).map_err(|errors| {
            ProjectError::Diagnostics(
                errors
                    .into_iter()
                    .map(|diagnostic| ProjectDiagnostic {
                        module: id.clone(),
                        path: sources.path(&id),
                        diagnostic,
                    })
                    .collect(),
            )
        })?;
        let checked = checked.remove(&id).ok_or_else(|| ProjectError::Runtime {
            module: id.clone(),
            path: sources.path(&id),
            error: Box::new(NovaError::Internal(format!(
                "module '{id}' was not checked before project assembly"
            ))),
        })?;
        let (path, text) = sources
            .files
            .remove(&id)
            .expect("parsed module has source text");
        modules.insert(
            id.clone(),
            CompiledModule {
                id,
                path,
                text: Rc::from(text),
                ast: module,
                resolved,
                checked,
            },
        );
    }

    Ok(Project {
        root: sources.root,
        entry,
        mode,
        modules,
    })
}

#[allow(clippy::too_many_arguments)]
fn discover(
    id: &ModuleId,
    sources: &mut SourceSet,
    parsed: &mut BTreeMap<ModuleId, ast::Module>,
    graph: &mut ModuleGraph,
    visiting: &mut Vec<ModuleId>,
    done: &mut BTreeSet<ModuleId>,
    order: &mut Vec<ModuleId>,
    diagnostics: &mut Vec<ProjectDiagnostic>,
) -> Result<(), ProjectError> {
    if done.contains(id) {
        return Ok(());
    }
    if let Some(start) = visiting.iter().position(|active| active == id) {
        let cycle = visiting[start..]
            .iter()
            .chain(std::iter::once(id))
            .map(ModuleId::as_str)
            .collect::<Vec<_>>()
            .join(" -> ");
        let path = sources.path(id);
        diagnostics.push(ProjectDiagnostic {
            module: id.clone(),
            path,
            diagnostic: CoreDiagnostic::error("NS3013", format!("Cyclic module import: {cycle}."))
                .with_hint("Break the direct or transitive import cycle."),
        });
        return Ok(());
    }

    sources.ensure(id, None)?;
    let text = sources.text(id).to_owned();
    let module = match parse(&text) {
        Ok(module) => module,
        Err(errors) => {
            diagnostics.extend(errors.into_iter().map(|diagnostic| ProjectDiagnostic {
                module: id.clone(),
                path: sources.path(id),
                diagnostic,
            }));
            done.insert(id.clone());
            return Ok(());
        }
    };
    graph.insert_module(id.as_str(), module.clone());
    parsed.insert(id.clone(), module.clone());
    visiting.push(id.clone());

    let mut local_targets = BTreeMap::new();
    for specifier in collect_module_specifiers(&module) {
        match resolve_specifier(id, &specifier.specifier) {
            Ok(Some(target)) => {
                local_targets.insert(target, (specifier.specifier, specifier.span));
            }
            Ok(None) => {
                return Err(ProjectError::Runtime {
                    module: id.clone(),
                    path: sources.path(id),
                    error: Box::new(NovaError::Unsupported {
                        message: format!(
                            "External module '{}' has no Nova adapter yet. Local project modules and relative '.ns' paths are supported.",
                            specifier.specifier
                        ),
                        span: Some(specifier.span),
                    }),
                });
            }
            Err(diagnostic) => {
                diagnostics.push(ProjectDiagnostic {
                    module: id.clone(),
                    path: sources.path(id),
                    diagnostic,
                });
            }
        }
    }

    for (target, (specifier, span)) in local_targets {
        if visiting.contains(&target) {
            let path = sources.path(id);
            diagnostics.push(ProjectDiagnostic {
                module: id.clone(),
                path,
                diagnostic: CoreDiagnostic::error(
                    "NS3013",
                    format!(
                        "Cyclic module import: '{}' imports '{}' while it is still loading.",
                        id.as_str(),
                        specifier
                    ),
                )
                .with_span(span)
                .with_hint("Break the direct or transitive import cycle."),
            });
            continue;
        }
        graph.insert_import(id.as_str(), specifier, target.as_str());
        sources.ensure(&target, Some((id.clone(), sources.path(id), span)))?;
        discover(
            &target,
            sources,
            parsed,
            graph,
            visiting,
            done,
            order,
            diagnostics,
        )?;
    }

    visiting.pop();
    done.insert(id.clone());
    order.push(id.clone());
    Ok(())
}

// `Diagnostic` carries the optional `related` label list, which puts it at the
// clippy large-Err threshold. The payload is still small in practice.
#[allow(clippy::result_large_err)]
fn resolve_specifier(importer: &ModuleId, raw: &str) -> Result<Option<ModuleId>, Diagnostic> {
    if raw.starts_with("std:") {
        return Ok(None);
    }
    if raw.starts_with('/') || raw.contains(':') || raw.contains('\\') {
        return Err(Diagnostic::error(
            "NS3012",
            format!("Absolute module specifier '{raw}' is not allowed."),
        )
        .with_hint("Use a relative '.ns' path or a dotted project path."));
    }

    if raw.starts_with("./") || raw.starts_with("../") {
        let base = importer.parent();
        let joined = if base.is_empty() {
            raw.to_owned()
        } else {
            format!("{base}/{raw}")
        };
        return normalize_path(&joined).map(Some).map_err(|message| {
            Diagnostic::error(
                "NS3012",
                format!("Invalid module specifier '{raw}': {message}."),
            )
            .with_hint("Local modules must stay inside the project root.")
        });
    }

    if raw.contains('/') || raw.starts_with('@') || !raw.contains('.') {
        return Ok(None);
    }

    if raw.ends_with(".ns") {
        return Ok(None);
    }

    if raw
        .split('.')
        .all(|segment| !segment.is_empty() && is_identifier(segment))
    {
        let path = format!("{}.ns", raw.replace('.', "/"));
        return normalize_path(&path).map(Some).map_err(|message| {
            Diagnostic::error(
                "NS3012",
                format!("Invalid module specifier '{raw}': {message}."),
            )
        });
    }

    Ok(None)
}

fn normalize_path(path: &str) -> Result<ModuleId, String> {
    let mut components: Vec<&str> = Vec::new();
    for component in path.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                if components.pop().is_none() {
                    return Err("the path escapes the project root".to_owned());
                }
            }
            value => components.push(value),
        }
    }
    let joined = components.join("/");
    if joined.is_empty() {
        return Err("the path does not name a module".to_owned());
    }
    if !joined.ends_with(".ns") {
        return Err("local module paths must end in '.ns'".to_owned());
    }
    ModuleId::parse(joined.clone()).map_err(|error| error.to_string())
}

fn is_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    (first == '_' || first.is_alphabetic())
        && characters.all(|character| character == '_' || character.is_alphanumeric())
}

fn module_id_from_path(path: &Path) -> Result<ModuleId, String> {
    let mut parts = Vec::new();
    for component in path.components() {
        let std::path::Component::Normal(part) = component else {
            return Err("module path contains a non-normal component".to_owned());
        };
        let Some(part) = part.to_str() else {
            return Err("module path is not valid UTF-8".to_owned());
        };
        parts.push(part.to_owned());
    }
    let joined = parts.join("/");
    ModuleId::parse(joined).map_err(|error| error.to_string())
}

fn missing_module(import: Option<(ModuleId, PathBuf, Span)>, id: &ModuleId) -> ProjectError {
    let (module, path, span) =
        import.unwrap_or_else(|| (id.clone(), PathBuf::from(id.as_str()), Span::default()));
    ProjectError::Diagnostics(vec![ProjectDiagnostic {
        module,
        path,
        diagnostic: Diagnostic::error(
            "NS3012",
            format!("Imported module '{}' could not be resolved.", id.as_str()),
        )
        .with_span(span)
        .with_hint("Create the module file or fix the import specifier."),
    }])
}

fn diagnostic_error(mut diagnostics: Vec<ProjectDiagnostic>) -> ProjectError {
    diagnostics.sort_by(|left, right| {
        (
            left.module.as_str(),
            left.diagnostic.span.map_or(0, |span| span.start),
            left.diagnostic.code.as_str(),
            left.diagnostic.message.as_str(),
        )
            .cmp(&(
                right.module.as_str(),
                right.diagnostic.span.map_or(0, |span| span.start),
                right.diagnostic.code.as_str(),
                right.diagnostic.message.as_str(),
            ))
    });
    diagnostics.dedup();
    ProjectError::Diagnostics(diagnostics)
}

fn io_error(
    module: Option<(ModuleId, PathBuf)>,
    path: &Path,
    code: &'static str,
    message: String,
) -> ProjectError {
    let (module, diagnostic_path) =
        module.unwrap_or_else(|| (ModuleId("<entry>".to_owned()), path.to_path_buf()));
    ProjectError::Diagnostics(vec![ProjectDiagnostic {
        module,
        path: diagnostic_path,
        diagnostic: Diagnostic::error(code, message),
    }])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::BufferHost;
    use crate::value::Value;
    use astra_core::types::Type;

    fn sources(entries: &[(&str, &str)]) -> BTreeMap<ModuleId, String> {
        entries
            .iter()
            .map(|(id, source)| {
                (
                    ModuleId::parse(*id).expect("valid test module id"),
                    (*source).to_owned(),
                )
            })
            .collect()
    }

    fn run(entry: &str, entries: &[(&str, &str)]) -> Result<Value, ProjectError> {
        let project = project_from_sources(
            ModuleId::parse(entry).expect("valid entry"),
            sources(entries),
            NovaOptions::default(),
        )?;
        let mut interpreter = build_project_interpreter(
            project,
            NovaOptions::default(),
            Box::new(BufferHost::new()),
        )?;
        interpreter.run().map_err(|error| {
            let module = interpreter
                .fault_module()
                .and_then(|id| ModuleId::parse(id).ok())
                .unwrap_or_else(|| ModuleId::parse(entry).expect("valid entry"));
            ProjectError::Runtime {
                module: module.clone(),
                path: PathBuf::from(module.as_str()),
                error: Box::new(error),
            }
        })
    }

    #[test]
    fn links_imported_function_and_constant() {
        let result = run(
            "main.ns",
            &[
                (
                    "main.ns",
                    "import { add } from \"./math.ns\"\nfn main() -> i32 { return add(2) }",
                ),
                (
                    "math.ns",
                    "pub const BASE: i32 = 40\npub fn add(n: i32) -> i32 { return BASE + n }",
                ),
            ],
        )
        .expect("project should run");
        assert_eq!(result, Value::Integer(crate::Integer::new(42, Type::I32)));
    }

    #[test]
    fn links_nested_relative_imports_and_namespaces() {
        let result = run(
            "app/main.ns",
            &[
                (
                    "app/main.ns",
                    "import * as math from \"../shared/math.ns\"\nfn main() -> i32 { return math.double(21) }",
                ),
                (
                    "shared/math.ns",
                    "pub fn double(n: i32) -> i32 { return n * 2 }",
                ),
            ],
        )
        .expect("project should run");
        assert_eq!(result, Value::Integer(crate::Integer::new(42, Type::I32)));
    }

    #[test]
    fn same_function_names_do_not_collide_across_modules() {
        let result = run(
            "main.ns",
            &[
                (
                    "main.ns",
                    "import { value as one } from \"./a.ns\"\nimport { value as two } from \"./b.ns\"\nfn main() -> i32 { return one() + two() }",
                ),
                ("a.ns", "pub fn value() -> i32 { return 1 }"),
                ("b.ns", "pub fn value() -> i32 { return 2 }"),
            ],
        )
        .expect("project should run");
        assert_eq!(result, Value::Integer(crate::Integer::new(3, Type::I32)));
    }

    #[test]
    fn links_imported_named_struct() {
        let result = run(
            "main.ns",
            &[
                (
                    "main.ns",
                    "import { Point } from \"./point.ns\"\nfn main() -> i32 { let p = Point { x: 19, y: 23 }\n return p.x + p.y }",
                ),
                (
                    "point.ns",
                    "pub struct Point { x: i32, y: i32 }",
                ),
            ],
        )
        .expect("project should run");
        assert_eq!(result, Value::Integer(crate::Integer::new(42, Type::I32)));
    }

    #[test]
    fn reports_module_cycle() {
        let error = run(
            "a.ns",
            &[
                (
                    "a.ns",
                    "import { b } from \"./b.ns\"\npub fn a() -> i32 { return b() }",
                ),
                (
                    "b.ns",
                    "import { a } from \"./a.ns\"\npub fn b() -> i32 { return a() }",
                ),
            ],
        )
        .expect_err("cycle should fail");
        assert!(
            error
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.diagnostic.code == "NS3013"),
            "expected NS3013: {error:?}"
        );
    }

    #[test]
    fn reports_missing_module() {
        let error = run(
            "main.ns",
            &[(
                "main.ns",
                "import { nope } from \"./missing.ns\"\nfn main() {}",
            )],
        )
        .expect_err("missing module should fail");
        assert!(
            error
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.diagnostic.code == "NS3012"),
            "expected NS3012: {error:?}"
        );
    }

    #[test]
    fn imported_type_error_is_reported_against_the_importing_module() {
        let error = run(
            "main.ns",
            &[
                (
                    "main.ns",
                    "import { double } from \"./math.ns\"\nfn main() -> i32 { return double(\"no\") }",
                ),
                (
                    "math.ns",
                    "pub fn double(n: i32) -> i32 { return n * 2 }",
                ),
            ],
        )
        .expect_err("type mismatch should fail");
        assert!(
            error.diagnostics().iter().any(|diagnostic| {
                diagnostic.module.as_str() == "main.ns" && diagnostic.diagnostic.code == "NS4001"
            }),
            "expected NS4001 in main.ns: {error:?}"
        );
    }

    #[test]
    fn links_transitive_reexports_and_deep_dependencies() {
        let result = run(
            "main.ns",
            &[
                (
                    "main.ns",
                    "import { helper } from \"./barrel.ns\"\nfn main() -> i32 { return helper(40) }",
                ),
                (
                    "barrel.ns",
                    "export { helper } from \"./impl.ns\"",
                ),
                (
                    "impl.ns",
                    "import { bump } from \"./leaf.ns\"\npub fn helper(n: i32) -> i32 { return bump(n) }",
                ),
                ("leaf.ns", "pub fn bump(n: i32) -> i32 { return n + 2 }"),
            ],
        )
        .expect("project should run");
        assert_eq!(result, Value::Integer(crate::Integer::new(42, Type::I32)));
    }

    #[test]
    fn links_default_function_exports() {
        let result = run(
            "main.ns",
            &[
                (
                    "main.ns",
                    "import make from \"./factory.ns\"\nfn main() -> i32 { return make(40) }",
                ),
                (
                    "factory.ns",
                    "fn build(n: i32) -> i32 { return n + 2 }\nexport default build",
                ),
            ],
        )
        .expect("project should run");
        assert_eq!(result, Value::Integer(crate::Integer::new(42, Type::I32)));
    }

    #[test]
    fn links_namespace_reexports() {
        let result = run(
            "main.ns",
            &[
                (
                    "main.ns",
                    "import { math } from \"./barrel.ns\"\nfn main() -> i32 { return math.double(21) }",
                ),
                (
                    "barrel.ns",
                    "export * as math from \"./math.ns\"",
                ),
                (
                    "math.ns",
                    "pub fn double(n: i32) -> i32 { return n * 2 }",
                ),
            ],
        )
        .expect("project should run");
        assert_eq!(result, Value::Integer(crate::Integer::new(42, Type::I32)));
    }

    #[test]
    fn links_imported_tuple_structs_and_enums() {
        let result = run(
            "main.ns",
            &[
                (
                    "main.ns",
                    r#"import { Pair, Status } from "./types.ns"
fn main() -> i32 {
    let pair = Pair(20, 22)
    let status = Status.Ready(pair.0 + pair.1)
    return match status {
        Status.Ready(value) => value
        Status.Pending => 0
    }
}"#,
                ),
                (
                    "types.ns",
                    "pub struct Pair(i32, i32)\npub enum Status { Pending, Ready(i32) }",
                ),
            ],
        )
        .expect("project should run");
        assert_eq!(result, Value::Integer(crate::Integer::new(42, Type::I32)));
    }

    #[test]
    fn links_dotted_project_paths() {
        let result = run(
            "main.ns",
            &[
                (
                    "main.ns",
                    "import { value } from \"app.util\"\nfn main() -> i32 { return value() }",
                ),
                ("app/util.ns", "pub fn value() -> i32 { return 42 }"),
            ],
        )
        .expect("project should run");
        assert_eq!(result, Value::Integer(crate::Integer::new(42, Type::I32)));
    }

    #[test]
    fn rejects_external_modules_at_load_time() {
        let error = run(
            "main.ns",
            &[("main.ns", "import { map } from \"lodash\"\nfn main() {}")],
        )
        .expect_err("external module should fail");
        let ProjectError::Runtime { error, .. } = error else {
            panic!("expected an external-module runtime error");
        };
        assert_eq!(error.code(), "NV0003");
    }

    #[test]
    fn rejects_relative_paths_that_escape_the_project_root() {
        let error = run(
            "main.ns",
            &[(
                "main.ns",
                "import { x } from \"../outside.ns\"\nfn main() {}",
            )],
        )
        .expect_err("escaping path should fail");
        assert!(
            error
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.diagnostic.code == "NS3012"),
            "expected NS3012: {error:?}"
        );
    }

    #[test]
    fn rejects_case_insensitive_module_id_collisions() {
        let error = run(
            "main.ns",
            &[
                ("main.ns", "fn main() {}"),
                ("Util.ns", "pub fn upper() -> i32 { return 1 }"),
                ("util.ns", "pub fn lower() -> i32 { return 2 }"),
            ],
        )
        .expect_err("case collision should fail");
        assert!(
            error
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.diagnostic.code == "NS3012"),
            "expected NS3012: {error:?}"
        );
    }

    #[test]
    fn panic_frames_name_the_defining_module() {
        let error = run(
            "main.ns",
            &[
                (
                    "main.ns",
                    "import { explode } from \"./boom.ns\"\nfn main() { explode() }",
                ),
                ("boom.ns", "pub fn explode() { panic(\"boom\") }"),
            ],
        )
        .expect_err("panic should fail");
        let ProjectError::Runtime { error, .. } = error else {
            panic!("expected a runtime panic");
        };
        let NovaError::Panic(panic) = *error else {
            panic!("expected a panic");
        };
        let frame = panic.stack.first().expect("panic stack");
        assert_eq!(frame.function, "explode");
        assert_eq!(frame.module.as_deref(), Some("boom.ns"));
    }

    #[test]
    fn project_diagnostics_are_sorted_by_module_id() {
        let error = run(
            "main.ns",
            &[
                (
                    "main.ns",
                    "import { z } from \"./z.ns\"\nimport { a } from \"./a.ns\"\nfn main() {}",
                ),
                ("z.ns", "pub fn z() -> i32 { return \"z\" }"),
                ("a.ns", "pub fn a() -> i32 { return \"a\" }"),
            ],
        )
        .expect_err("both modules should fail type checking");
        let modules = error
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.module.as_str())
            .collect::<Vec<_>>();
        assert_eq!(modules, vec!["a.ns", "z.ns"]);
    }

    #[test]
    fn rejects_absolute_import_paths() {
        let error = run(
            "main.ns",
            &[(
                "main.ns",
                "import { value } from \"/absolute.ns\"\nfn main() {}",
            )],
        )
        .expect_err("absolute import should fail");
        assert!(
            error
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.diagnostic.code == "NS3012"),
            "expected NS3012: {error:?}"
        );
    }

    #[test]
    fn links_star_reexports() {
        let result = run(
            "main.ns",
            &[
                (
                    "main.ns",
                    "import { add } from \"./barrel.ns\"\nfn main() -> i32 { return add(40) }",
                ),
                ("barrel.ns", "export * from \"./math.ns\""),
                ("math.ns", "pub fn add(n: i32) -> i32 { return n + 2 }"),
            ],
        )
        .expect("project should run");
        assert_eq!(result, Value::Integer(crate::Integer::new(42, Type::I32)));
    }

    #[test]
    fn side_effect_imports_load_without_initialization() {
        let result = run(
            "main.ns",
            &[
                (
                    "main.ns",
                    "import \"./helper.ns\"\nfn main() -> i32 { return 42 }",
                ),
                ("helper.ns", "pub fn unused() -> i32 { return 0 }"),
            ],
        )
        .expect("project should run");
        assert_eq!(result, Value::Integer(crate::Integer::new(42, Type::I32)));
    }

    #[test]
    fn links_constants_that_depend_on_other_modules() {
        let result = run(
            "main.ns",
            &[
                (
                    "main.ns",
                    "import { ANSWER } from \"./math.ns\"\nfn main() -> i32 { return ANSWER }",
                ),
                (
                    "math.ns",
                    "import { BASE } from \"./base.ns\"\npub const ANSWER: i32 = BASE + 2",
                ),
                ("base.ns", "pub const BASE: i32 = 40"),
            ],
        )
        .expect("project should run");
        assert_eq!(result, Value::Integer(crate::Integer::new(42, Type::I32)));
    }

    #[test]
    fn default_expressions_are_rejected_explicitly() {
        let error = run(
            "main.ns",
            &[
                (
                    "main.ns",
                    "import value from \"./lib.ns\"\nfn main() -> i32 { return value }",
                ),
                ("lib.ns", "export default 42"),
            ],
        )
        .expect_err("anonymous default expressions are unsupported in V1");
        let ProjectError::Runtime { error, .. } = error else {
            panic!("expected a runtime unsupported error");
        };
        assert_eq!(error.code(), "NV0003");
    }

    #[test]
    fn panic_frames_cover_every_cross_module_call() {
        let error = run(
            "main.ns",
            &[
                (
                    "main.ns",
                    "import { middle } from \"./middle.ns\"\nfn main() { middle() }",
                ),
                (
                    "middle.ns",
                    "import { explode } from \"./boom.ns\"\npub fn middle() { explode() }",
                ),
                ("boom.ns", "pub fn explode() { panic(\"boom\") }"),
            ],
        )
        .expect_err("panic should fail");
        let ProjectError::Runtime { error, .. } = error else {
            panic!("expected a runtime panic");
        };
        let NovaError::Panic(panic) = *error else {
            panic!("expected a panic");
        };
        let frames = panic
            .stack
            .iter()
            .map(|frame| (frame.module.as_deref(), frame.function.as_str()))
            .collect::<Vec<_>>();
        assert!(frames.contains(&(Some("boom.ns"), "explode")));
        assert!(frames.contains(&(Some("middle.ns"), "middle")));
        assert!(frames.contains(&(Some("main.ns"), "main")));
    }

    #[test]
    fn runtime_faults_are_attributed_to_the_defining_module() {
        let error = run(
            "main.ns",
            &[
                (
                    "main.ns",
                    "import { divide } from \"./boom.ns\"\nfn main() -> i32 { return divide(0) }",
                ),
                ("boom.ns", "pub fn divide(n: i32) -> i32 { return 10 / n }"),
            ],
        )
        .expect_err("division by zero should fail");
        let ProjectError::Runtime { module, path, .. } = error else {
            panic!("expected a runtime panic");
        };
        assert_eq!(module.as_str(), "boom.ns");
        assert_eq!(path, PathBuf::from("boom.ns"));
    }

    #[test]
    fn manual_memory_in_an_imported_module_is_rejected_at_preflight() {
        let options = NovaOptions {
            mode: astra_core::mode::Mode::System,
            infer_mode: false,
            ..NovaOptions::default()
        };
        let project = project_from_sources(
            ModuleId::parse("main.ns").unwrap(),
            sources(&[
                (
                    "main.ns",
                    "import { value } from \"./manual.ns\"\nfn main() -> i32 { return value() }",
                ),
                (
                    "manual.ns",
                    r#"pub fn value() -> i32 {
    {@memory:manual;
        let temporary = 1
    }
    return 0
}"#,
                ),
            ]),
            options,
        )
        .expect("project should check");
        let error = match build_project_interpreter(project, options, Box::new(BufferHost::new())) {
            Ok(_) => panic!("manual memory should be rejected before execution"),
            Err(error) => error,
        };
        let ProjectError::Runtime { module, error, .. } = error else {
            panic!("expected a runtime preflight error");
        };
        assert_eq!(module.as_str(), "manual.ns");
        assert!(matches!(*error, NovaError::ManualMemoryUnsupported { .. }));
    }
}
