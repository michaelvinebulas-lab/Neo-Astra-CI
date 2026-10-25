//! Mode and capability inference.
//!
//! The mode vocabulary lives in `astra-core` because it is part of the language
//! contract. This crate builds the static capability graph and selects the
//! least-privileged built-in mode for a module or project.
//!
//! Inference is deliberately explainable: every requirement carries a source
//! span and a reason. Tooling can show the same tree that selected the mode
//! without re-implementing the pass.

use std::collections::{BTreeMap, BTreeSet};

use astra_core::ast;
use astra_core::diagnostic::Diagnostic;
use astra_core::span::Span;
use serde::{Deserialize, Serialize};

pub use astra_core::mode::{Mode, ModeRequirement};

/// No single built-in mode contains every requirement.
pub const NO_SINGLE_MODE: &str = "NS5006";
/// An explicit requirement does not contain a discovered requirement.
pub const MODE_REQUIREMENT: &str = "NS5001";
/// A scoped block tries to widen beyond an explicit enclosing requirement.
pub const MODE_WIDENING: &str = "NS5002";
/// A capability tag requires a mode that is not currently available.
pub const CAPABILITY_REQUIREMENT: &str = "NS5003";

/// Why a mode requirement exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModeReasonKind {
    /// An explicit `@requires(...)` annotation.
    ExplicitRequirement,
    /// A `@tags(...)` capability annotation.
    CapabilityTag,
    /// A capability-bearing built-in such as `println`.
    Builtin,
    /// A foreign module declaration.
    ForeignModule,
    /// An `@memory:manual` scope.
    ManualMemory,
    /// A transitive call to a function with a requirement.
    CallGraph,
    /// A scoped `@mode:` boundary.
    ScopedMode,
    /// A requirement contributed by another module in a project join.
    ProjectJoin,
}

/// One explainable mode requirement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeReason {
    /// Mode required by this reason.
    pub mode: Mode,
    /// Kind of evidence.
    pub kind: ModeReasonKind,
    /// Capability name, when the reason came from a capability tag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capability: Option<String>,
    /// Source span of the evidence.
    pub span: Span,
    /// Human-readable explanation.
    pub detail: String,
    /// Module key in a project inference, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
}

impl ModeReason {
    #[must_use]
    pub fn new(
        mode: Mode,
        kind: ModeReasonKind,
        span: Span,
        detail: impl Into<String>,
        capability: Option<String>,
    ) -> Self {
        Self {
            mode,
            kind,
            capability,
            span,
            detail: detail.into(),
            module: None,
        }
    }

    #[must_use]
    pub fn in_module(mut self, module: impl Into<String>) -> Self {
        self.module = Some(module.into());
        self
    }
}

/// Result of mode inference for one module or a joined project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeInference {
    /// Least-privileged mode implied by direct and transitive requirements.
    pub required_mode: Mode,
    /// Explicit `@requires` mode, which is authoritative when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explicit_mode: Option<Mode>,
    /// Capability names observed during inference.
    pub capabilities: BTreeSet<String>,
    /// Explainable requirements in deterministic discovery order.
    pub reasons: Vec<ModeReason>,
    /// Contradictions, widening errors, and unsatisfiable joins.
    pub diagnostics: Vec<Diagnostic>,
    /// Module key, when this inference describes one module.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
}

impl ModeInference {
    /// Creates an empty inference requiring only `core`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            required_mode: Mode::Core,
            explicit_mode: None,
            capabilities: BTreeSet::new(),
            reasons: Vec::new(),
            diagnostics: Vec::new(),
            module: None,
        }
    }

    /// Creates an inference for a mode with no evidence.
    #[must_use]
    pub fn from_mode(mode: Mode) -> Self {
        Self {
            required_mode: mode,
            explicit_mode: Some(mode),
            ..Self::new()
        }
    }

    /// The mode selected for checking: an explicit requirement wins, otherwise
    /// the inferred least-privileged requirement is used.
    #[must_use]
    pub fn resolved_mode(&self) -> Mode {
        self.explicit_mode.unwrap_or(self.required_mode)
    }

    /// Whether inference found a contradiction.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// Attaches a project module key to this inference and its reasons.
    #[must_use]
    pub fn named(mut self, module: impl Into<String>) -> Self {
        let module = module.into();
        self.module = Some(module.clone());
        for reason in &mut self.reasons {
            if reason.module.is_none() {
                reason.module = Some(module.clone());
            }
        }
        self
    }

    /// Joins another module's inference into this project inference.
    ///
    /// Requirements use containment (`web` contains `app`, `system` contains
    /// `app`), but `web` and `system` are incomparable. A join that needs both
    /// produces `NS5006`.
    pub fn merge(&mut self, other: &ModeInference) {
        match join_modes(self.required_mode, other.required_mode) {
            Some(mode) => self.required_mode = mode,
            None => {
                let span = other
                    .reasons
                    .first()
                    .map(|reason| reason.span)
                    .unwrap_or_default();
                self.diagnostics.push(
                    Diagnostic::error(
                        NO_SINGLE_MODE,
                        format!(
                            "No single mode satisfies '{}' and '{}' requirements.",
                            self.required_mode.as_str(),
                            other.required_mode.as_str()
                        ),
                    )
                    .with_span(span)
                    .with_hint(
                        "Split the project at a mode boundary or remove one of the incompatible capabilities.",
                    ),
                );
            }
        }

        if let Some(other_explicit) = other.explicit_mode {
            match self.explicit_mode {
                Some(explicit) => match join_modes(explicit, other_explicit) {
                    Some(mode) => self.explicit_mode = Some(mode),
                    None => {
                        let span = other
                            .reasons
                            .first()
                            .map(|reason| reason.span)
                            .unwrap_or_default();
                        self.diagnostics.push(
                            Diagnostic::error(
                                NO_SINGLE_MODE,
                                format!(
                                    "No single explicit mode satisfies '{}' and '{}'.",
                                    explicit.as_str(),
                                    other_explicit.as_str()
                                ),
                            )
                            .with_span(span)
                            .with_hint("Use compatible @requires annotations."),
                        );
                    }
                },
                None => self.explicit_mode = Some(other_explicit),
            }
        }

        self.capabilities.extend(other.capabilities.iter().cloned());
        for reason in &other.reasons {
            if !self.reasons.contains(reason) {
                self.reasons.push(reason.clone());
            }
        }
        for diagnostic in &other.diagnostics {
            if !self.diagnostics.contains(diagnostic) {
                self.diagnostics.push(diagnostic.clone());
            }
        }

        if let Some(explicit) = self.explicit_mode
            && !explicit.allows(self.required_mode)
        {
            let span = self
                .reasons
                .iter()
                .find(|reason| reason.mode != Mode::Core && !explicit.allows(reason.mode))
                .map_or_else(Span::default, |reason| reason.span);
            self.diagnostics.push(
                Diagnostic::error(
                    MODE_REQUIREMENT,
                    format!(
                        "Project requires '{}' mode, but an explicit module requirement is '{}'.",
                        self.required_mode.as_str(),
                        explicit.as_str()
                    ),
                )
                .with_span(span)
                .with_hint(format!(
                    "Raise the explicit requirement to '{}' or remove the excess capability usage.",
                    self.required_mode.as_str()
                )),
            );
        }
    }

    /// Renders the deterministic explanation tree used by tooling.
    #[must_use]
    pub fn explain(&self) -> String {
        let source = if self.explicit_mode.is_some() {
            "explicit"
        } else {
            "inferred"
        };
        let mut output = format!(
            "Resolved mode: {} ({source})\n",
            self.resolved_mode().as_str()
        );
        for reason in &self.reasons {
            output.push_str(&format!(
                "  - {} via {} ({}..{})",
                reason.mode.as_str(),
                reason.detail,
                reason.span.start,
                reason.span.end
            ));
            if let Some(capability) = &reason.capability {
                output.push_str(&format!(" [{capability}]"));
            }
            output.push('\n');
        }
        for diagnostic in &self.diagnostics {
            output.push_str(&format!("  ! {diagnostic}\n"));
        }
        output
    }
}

impl Default for ModeInference {
    fn default() -> Self {
        Self::new()
    }
}

/// Joins two mode requirements if one contains the other.
pub fn join_modes(left: Mode, right: Mode) -> Option<Mode> {
    if left.allows(right) {
        Some(left)
    } else if right.allows(left) {
        Some(right)
    } else {
        None
    }
}

/// Infers the least-privileged mode for one parsed module.
#[must_use]
pub fn infer_module_mode(module: &ast::Module) -> ModeInference {
    let mut analyzer = Analyzer::new(None);
    analyzer.analyze(module);
    analyzer.finish()
}

/// Infers a module and attaches a stable project key to every reason.
#[must_use]
pub fn infer_module_mode_named(module: &ast::Module, key: impl Into<String>) -> ModeInference {
    let key = key.into();
    let mut analyzer = Analyzer::new(Some(key));
    analyzer.analyze(module);
    analyzer.finish()
}

/// Joins a list of module inferences into one project inference.
#[must_use]
pub fn infer_project(inferences: &[ModeInference]) -> ModeInference {
    let mut project = ModeInference::new();
    for inference in inferences {
        project.merge(inference);
    }
    project
}

#[derive(Debug, Clone)]
struct FunctionFacts {
    span: Span,
    local_mode: Mode,
    mode: Mode,
    explicit: Option<Mode>,
    calls: BTreeMap<String, Span>,
    reasons: Vec<ModeReason>,
}

#[derive(Debug, Default)]
struct Usage {
    mode: Mode,
    calls: BTreeMap<String, Span>,
    reasons: Vec<ModeReason>,
}

impl Usage {
    fn new() -> Self {
        Self {
            mode: Mode::Core,
            ..Self::default()
        }
    }

    fn require(
        &mut self,
        mode: Mode,
        kind: ModeReasonKind,
        span: Span,
        detail: impl Into<String>,
        capability: Option<String>,
    ) {
        match join_modes(self.mode, mode) {
            Some(joined) => self.mode = joined,
            None => self.mode = mode,
        }
        self.reasons
            .push(ModeReason::new(mode, kind, span, detail, capability));
    }

    fn join(&mut self, other: Usage) {
        match join_modes(self.mode, other.mode) {
            Some(joined) => self.mode = joined,
            None => self.mode = other.mode,
        }
        for (name, span) in other.calls {
            self.calls.entry(name).or_insert(span);
        }
        for reason in other.reasons {
            if !self.reasons.contains(&reason) {
                self.reasons.push(reason);
            }
        }
    }
}

struct Analyzer {
    module: Option<String>,
    functions: BTreeMap<String, FunctionFacts>,
    other_reasons: Vec<ModeReason>,
    other_mode: Mode,
    module_explicit: Option<Mode>,
    diagnostics: Vec<Diagnostic>,
}

impl Analyzer {
    fn new(module: Option<String>) -> Self {
        Self {
            module,
            functions: BTreeMap::new(),
            other_reasons: Vec::new(),
            other_mode: Mode::Core,
            module_explicit: None,
            diagnostics: Vec::new(),
        }
    }

    fn analyze(&mut self, module: &ast::Module) {
        let (explicit, usage) = self.attributes(&module.attributes, None);
        self.module_explicit = explicit;

        self.walk_items(&module.items, explicit);
        self.join_other(usage);

        // Propagate call-graph requirements to a fixed point. Recursion and
        // mutual recursion converge because the mode lattice has four values.
        let limit = self.functions.len().saturating_add(1);
        for _ in 0..limit {
            let mut changed = false;
            let snapshot = self.functions.clone();
            for facts in self.functions.values_mut() {
                for (callee, call_span) in &facts.calls {
                    let Some(callee_facts) = snapshot.get(callee) else {
                        continue;
                    };
                    if callee_facts.mode == Mode::Core {
                        continue;
                    }
                    if let Some(explicit) = facts.explicit
                        && !explicit.allows(callee_facts.mode)
                    {
                        self.diagnostics.push(
                            Diagnostic::error(
                                MODE_REQUIREMENT,
                                format!(
                                    "Call to '{callee}' requires '{}' mode, but the explicit requirement is '{}'.",
                                    callee_facts.mode.as_str(),
                                    explicit.as_str()
                                ),
                            )
                            .with_span(*call_span)
                            .with_hint(format!(
                                "Change '@requires({})' to '@requires({})' or narrow the callee.",
                                explicit.as_str(),
                                callee_facts.mode.as_str()
                            )),
                        );
                        continue;
                    }
                    let joined = match join_modes(facts.mode, callee_facts.mode) {
                        Some(joined) => joined,
                        None => {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    NO_SINGLE_MODE,
                                    format!(
                                        "No single mode satisfies '{}' and the call to '{callee}' requiring '{}'.",
                                        facts.mode.as_str(),
                                        callee_facts.mode.as_str()
                                    ),
                                )
                                .with_span(*call_span)
                                .with_hint(
                                    "Separate web and system work behind a mode boundary.",
                                ),
                            );
                            facts.mode
                        }
                    };
                    if joined != facts.mode {
                        facts.mode = joined;
                        facts.reasons.push(
                            ModeReason::new(
                                callee_facts.mode,
                                ModeReasonKind::CallGraph,
                                *call_span,
                                format!("transitive call to '{callee}'"),
                                None,
                            )
                            .in_module(self.module.clone().unwrap_or_default()),
                        );
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }

        let facts_snapshot: Vec<FunctionFacts> = self.functions.values().cloned().collect();
        for facts in &facts_snapshot {
            self.other_reasons.extend(facts.reasons.clone());
            self.join_mode(facts.mode);
            self.join_mode(facts.local_mode);
            if let Some(explicit) = facts.explicit
                && !explicit.allows(facts.local_mode)
            {
                self.diagnostics.push(
                    Diagnostic::error(
                        MODE_REQUIREMENT,
                        format!(
                            "Function/constant requires '{}' mode, but its explicit requirement is '{}'.",
                            facts.local_mode.as_str(),
                            explicit.as_str()
                        ),
                    )
                    .with_span(facts.span)
                    .with_hint(format!(
                        "Change the explicit requirement to '{}'.",
                        facts.local_mode.as_str()
                    )),
                );
            }
        }

        self.report_conflicting_reasons();
    }

    fn finish(mut self) -> ModeInference {
        let explicit = self.module_explicit;
        if let Some(explicit) = explicit
            && !explicit.allows(self.other_mode)
        {
            self.diagnostics.push(
                Diagnostic::error(
                    MODE_REQUIREMENT,
                    format!(
                        "Module requires '{}' mode, but its explicit requirement is '{}'.",
                        self.other_mode.as_str(),
                        explicit.as_str()
                    ),
                )
                .with_hint(format!(
                    "Change '@requires({})' to '@requires({})' or remove the excess capability usage.",
                    explicit.as_str(),
                    self.other_mode.as_str()
                )),
            );
        }

        let required_mode = explicit.map_or(self.other_mode, |explicit| {
            join_or_keep(self.other_mode, explicit)
        });
        let mut reasons = self.other_reasons;
        if let Some(module) = &self.module {
            for reason in &mut reasons {
                if reason.module.is_none() {
                    reason.module = Some(module.clone());
                }
            }
        }
        ModeInference {
            required_mode,
            explicit_mode: explicit,
            capabilities: reasons
                .iter()
                .filter_map(|reason| reason.capability.clone())
                .collect(),
            reasons,
            diagnostics: dedupe_diagnostics(self.diagnostics),
            module: self.module,
        }
    }

    fn walk_items(&mut self, items: &[ast::Item], outer_explicit: Option<Mode>) {
        for item in items {
            let (item_explicit, mut usage) = self.attributes(&item.attributes, outer_explicit);
            let explicit = item_explicit.or(outer_explicit);

            if let (Some(enclosing), Some(required)) = (outer_explicit, item_explicit)
                && !enclosing.allows(required)
            {
                self.diagnostics.push(
                    Diagnostic::error(
                        MODE_REQUIREMENT,
                        format!(
                            "Item requires '{}' mode, but the enclosing requirement is '{}'.",
                            required.as_str(),
                            enclosing.as_str()
                        ),
                    )
                    .with_span(item.span)
                    .with_hint(format!(
                        "Raise the enclosing requirement to '{}'.",
                        required.as_str()
                    )),
                );
            }

            match &item.kind {
                ast::ItemKind::Function(function) => {
                    usage.join(self.walk_block(&function.body, explicit));
                    let local_mode = usage.mode;
                    let mode = explicit.unwrap_or(local_mode);
                    self.functions.insert(
                        function.name.clone(),
                        FunctionFacts {
                            span: function.span,
                            local_mode,
                            mode,
                            explicit,
                            calls: usage.calls,
                            reasons: usage.reasons,
                        },
                    );
                }
                ast::ItemKind::Constant(constant) => {
                    usage.join(self.walk_expr(&constant.value, explicit));
                    self.functions.insert(
                        constant.name.clone(),
                        FunctionFacts {
                            span: constant.span,
                            local_mode: usage.mode,
                            mode: explicit.unwrap_or(usage.mode),
                            explicit,
                            calls: usage.calls,
                            reasons: usage.reasons,
                        },
                    );
                }
                ast::ItemKind::Import(_) | ast::ItemKind::Export(_) => {
                    self.other_mode = join_or_keep(self.other_mode, usage.mode);
                    self.other_reasons.extend(usage.reasons);
                }
                ast::ItemKind::ForeignModule(foreign) => {
                    usage.require(
                        Mode::System,
                        ModeReasonKind::ForeignModule,
                        foreign.span,
                        format!("foreign module '{}'", foreign.name),
                        Some("ffi".to_owned()),
                    );
                    self.other_mode = join_or_keep(self.other_mode, usage.mode);
                    self.other_reasons.extend(usage.reasons);
                }
                ast::ItemKind::ScopedItems(scoped) => {
                    let (scoped_mode, scope_usage) = self.scope_mode(&scoped.annotation, explicit);
                    usage.join(scope_usage);
                    self.other_mode = join_or_keep(self.other_mode, usage.mode);
                    self.other_reasons.extend(usage.reasons);
                    self.walk_items(&scoped.items, scoped_mode.or(explicit));
                }
                ast::ItemKind::Struct(_) | ast::ItemKind::Enum(_) | ast::ItemKind::TypeAlias(_) => {
                    self.other_mode = join_or_keep(self.other_mode, usage.mode);
                    self.other_reasons.extend(usage.reasons);
                }
            }
        }
    }

    fn attributes(
        &mut self,
        attributes: &[ast::Attribute],
        enclosing: Option<Mode>,
    ) -> (Option<Mode>, Usage) {
        let mut explicit = None;
        let mut usage = Usage::new();
        for attribute in attributes {
            match attribute.name.as_str() {
                "requires" => {
                    for argument in &attribute.arguments {
                        let Some((name, span)) = annotation_name(argument) else {
                            continue;
                        };
                        let Some(mode) = Mode::parse(&name) else {
                            continue;
                        };
                        explicit = match explicit {
                            Some(current) => match join_modes(current, mode) {
                                Some(joined) => Some(joined),
                                None => {
                                    self.diagnostics.push(
                                        Diagnostic::error(
                                            NO_SINGLE_MODE,
                                            format!(
                                                "No single mode satisfies '{}' and '{}' in '@requires'.",
                                                current.as_str(),
                                                mode.as_str()
                                            ),
                                        )
                                        .with_span(span)
                                        .with_hint("Use one compatible mode requirement."),
                                    );
                                    Some(current)
                                }
                            },
                            None => Some(mode),
                        };
                        usage.reasons.push(ModeReason::new(
                            mode,
                            ModeReasonKind::ExplicitRequirement,
                            span,
                            format!("explicit @requires({})", mode.as_str()),
                            None,
                        ));
                    }
                }
                "tags" => {
                    for argument in &attribute.arguments {
                        let Some((tag, span)) = annotation_name(argument) else {
                            continue;
                        };
                        if let Some(mode) = Mode::requirement_for_capability(&tag) {
                            usage.require(
                                mode,
                                ModeReasonKind::CapabilityTag,
                                span,
                                format!("capability tag '{tag}'"),
                                Some(tag),
                            );
                        } else {
                            usage.reasons.push(ModeReason::new(
                                Mode::Core,
                                ModeReasonKind::CapabilityTag,
                                span,
                                format!("user-defined capability tag '{tag}'"),
                                Some(tag),
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
        if let Some(enclosing) = enclosing
            && let Some(explicit) = explicit
            && !enclosing.allows(explicit)
        {
            self.diagnostics.push(
                Diagnostic::error(
                    MODE_REQUIREMENT,
                    format!(
                        "Explicit requirement '{}' exceeds the enclosing '{}' mode.",
                        explicit.as_str(),
                        enclosing.as_str()
                    ),
                )
                .with_span(attributes.first().map_or(Span::default(), |a| a.span))
                .with_hint("A nested explicit requirement cannot widen capabilities."),
            );
        }
        (explicit, usage)
    }

    fn walk_block(&mut self, block: &ast::Block, explicit: Option<Mode>) -> Usage {
        let mut usage = Usage::new();
        for statement in &block.statements {
            usage.join(self.walk_stmt(statement, explicit));
        }
        usage
    }

    fn walk_stmt(&mut self, statement: &ast::Stmt, explicit: Option<Mode>) -> Usage {
        match &statement.kind {
            ast::StmtKind::Let { value, .. } => self.walk_expr(value, explicit),
            ast::StmtKind::Return { value } => value
                .as_ref()
                .map(|value| self.walk_expr(value, explicit))
                .unwrap_or_else(Usage::new),
            ast::StmtKind::Break | ast::StmtKind::Continue => Usage::new(),
            ast::StmtKind::Defer(statement) => self.walk_stmt(statement, explicit),
            ast::StmtKind::While { condition, body } => {
                let mut usage = self.walk_expr(condition, explicit);
                usage.join(self.walk_block(body, explicit));
                usage
            }
            ast::StmtKind::For { iterable, body, .. } => {
                let mut usage = self.walk_expr(iterable, explicit);
                usage.join(self.walk_block(body, explicit));
                usage
            }
            ast::StmtKind::Expr(expression) => self.walk_expr(expression, explicit),
            ast::StmtKind::Scoped(scoped) => {
                let (scoped_mode, mut usage) = self.scope_mode(&scoped.annotation, explicit);
                let next = scoped_mode.or(explicit);
                for statement in &scoped.statements {
                    usage.join(self.walk_stmt(statement, next));
                }
                usage
            }
        }
    }

    fn walk_expr(&mut self, expression: &ast::Expr, explicit: Option<Mode>) -> Usage {
        match &expression.kind {
            ast::ExprKind::Literal(_) | ast::ExprKind::Identifier(_) => Usage::new(),
            ast::ExprKind::Unary { operand, .. } => self.walk_expr(operand, explicit),
            ast::ExprKind::Binary { left, right, .. }
            | ast::ExprKind::Assign {
                target: left,
                value: right,
            } => {
                let mut usage = self.walk_expr(left, explicit);
                usage.join(self.walk_expr(right, explicit));
                usage
            }
            ast::ExprKind::Call { callee, arguments } => {
                let mut usage = Usage::new();
                if let ast::ExprKind::Identifier(name) = &callee.kind {
                    match name.as_str() {
                        "print" | "println" | "dbg" => usage.require(
                            Mode::App,
                            ModeReasonKind::Builtin,
                            expression.span,
                            format!("console built-in '{name}'"),
                            Some("console".to_owned()),
                        ),
                        "Some" | "Ok" | "Err" | "None" | "assert" => {}
                        _ => {
                            usage.calls.insert(name.clone(), expression.span);
                        }
                    }
                }
                usage.join(self.walk_expr(callee, explicit));
                for argument in arguments {
                    usage.join(self.walk_expr(argument, explicit));
                }
                usage
            }
            ast::ExprKind::Index { target, index } => {
                let mut usage = self.walk_expr(target, explicit);
                usage.join(self.walk_expr(index, explicit));
                usage
            }
            ast::ExprKind::Member { target, .. } => self.walk_expr(target, explicit),
            ast::ExprKind::TypeArguments { target, .. } => self.walk_expr(target, explicit),
            ast::ExprKind::Propagate(inner) => self.walk_expr(inner, explicit),
            ast::ExprKind::Array(elements) | ast::ExprKind::Tuple(elements) => {
                let mut usage = Usage::new();
                for element in elements {
                    usage.join(self.walk_expr(element, explicit));
                }
                usage
            }
            ast::ExprKind::Map(entries) => {
                let mut usage = Usage::new();
                for entry in entries {
                    usage.join(self.walk_expr(&entry.key, explicit));
                    usage.join(self.walk_expr(&entry.value, explicit));
                }
                usage
            }
            ast::ExprKind::StructInit { fields, .. } => {
                let mut usage = Usage::new();
                for field in fields {
                    usage.join(self.walk_expr(&field.value, explicit));
                }
                usage
            }
            ast::ExprKind::Block(block) => self.walk_block(block, explicit),
            ast::ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let mut usage = self.walk_expr(condition, explicit);
                usage.join(self.walk_block(then_branch, explicit));
                if let Some(else_branch) = else_branch {
                    match else_branch {
                        ast::ElseBranch::Block(block) => {
                            usage.join(self.walk_block(block, explicit))
                        }
                        ast::ElseBranch::If(expression) => {
                            usage.join(self.walk_expr(expression, explicit));
                        }
                    }
                }
                usage
            }
            ast::ExprKind::Match { value, arms } => {
                let mut usage = self.walk_expr(value, explicit);
                for arm in arms {
                    match &arm.body {
                        ast::MatchArmBody::Expr(expression) => {
                            usage.join(self.walk_expr(expression, explicit));
                        }
                        ast::MatchArmBody::Block(block) => {
                            usage.join(self.walk_block(block, explicit));
                        }
                    }
                }
                usage
            }
            ast::ExprKind::Spawn { body } => self.walk_block(body, explicit),
            ast::ExprKind::Fail { arguments, .. } => match arguments {
                ast::FailArguments::Tuple(arguments) => {
                    let mut usage = Usage::new();
                    for argument in arguments {
                        usage.join(self.walk_expr(argument, explicit));
                    }
                    usage
                }
                ast::FailArguments::Struct(fields) => {
                    let mut usage = Usage::new();
                    for field in fields {
                        usage.join(self.walk_expr(&field.value, explicit));
                    }
                    usage
                }
            },
            ast::ExprKind::Panic { message } | ast::ExprKind::Abort { message } => {
                self.walk_expr(message, explicit)
            }
        }
    }

    fn scope_mode(
        &mut self,
        annotation: &ast::ScopeAnnotation,
        enclosing: Option<Mode>,
    ) -> (Option<Mode>, Usage) {
        let mut usage = Usage::new();
        if annotation.kind == ast::ScopeKind::Memory {
            if annotation.value == "manual" {
                usage.require(
                    Mode::System,
                    ModeReasonKind::ManualMemory,
                    annotation.span,
                    "manual memory scope",
                    Some("memory".to_owned()),
                );
                if let Some(enclosing) = enclosing
                    && !enclosing.allows(Mode::System)
                {
                    self.diagnostics.push(
                        Diagnostic::error(
                            MODE_WIDENING,
                            format!(
                                "Manual memory requires 'system' mode, but the enclosing mode is '{}'.",
                                enclosing.as_str()
                            ),
                        )
                        .with_span(annotation.span)
                        .with_hint("Move manual memory into a system-mode boundary."),
                    );
                }
            }
            return (None, usage);
        }

        let Some(scoped) = Mode::parse(&annotation.value) else {
            return (None, usage);
        };
        if let Some(enclosing) = enclosing
            && !enclosing.allows(scoped)
        {
            self.diagnostics.push(
                Diagnostic::error(
                    MODE_WIDENING,
                    format!(
                        "Mode widening from '{}' to '{}' is not allowed.",
                        enclosing.as_str(),
                        scoped.as_str()
                    ),
                )
                .with_span(annotation.span)
                .with_hint("A scoped block may narrow capabilities, not widen them."),
            );
            return (Some(enclosing), usage);
        }
        usage.require(
            scoped,
            ModeReasonKind::ScopedMode,
            annotation.span,
            format!("scoped @mode:{} block", scoped.as_str()),
            None,
        );
        (Some(scoped), usage)
    }

    fn report_conflicting_reasons(&mut self) {
        let has_web = self
            .other_reasons
            .iter()
            .any(|reason| reason.mode == Mode::Web);
        let has_system = self
            .other_reasons
            .iter()
            .any(|reason| reason.mode == Mode::System);
        if !(has_web && has_system) {
            return;
        }
        let span = self
            .other_reasons
            .iter()
            .find(|reason| reason.mode == Mode::System)
            .map(|reason| reason.span)
            .unwrap_or_default();
        self.diagnostics.push(
            Diagnostic::error(
                NO_SINGLE_MODE,
                "No single built-in mode contains both web and system requirements.",
            )
            .with_span(span)
            .with_hint("Split browser and native/system work behind an explicit mode boundary."),
        );
    }

    fn join_mode(&mut self, mode: Mode) {
        self.other_mode = join_or_keep(self.other_mode, mode);
    }

    fn join_other(&mut self, usage: Usage) {
        self.other_mode = join_or_keep(self.other_mode, usage.mode);
        self.other_reasons.extend(usage.reasons);
    }
}

fn join_or_keep(left: Mode, right: Mode) -> Mode {
    join_modes(left, right).unwrap_or(left)
}

fn annotation_name(expression: &ast::Expr) -> Option<(String, Span)> {
    match &expression.kind {
        ast::ExprKind::Identifier(name) => Some((name.clone(), expression.span)),
        ast::ExprKind::Literal(ast::Literal::String(name)) => Some((name.clone(), expression.span)),
        _ => None,
    }
}

fn dedupe_diagnostics(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    let mut result = Vec::new();
    for diagnostic in diagnostics {
        if !result.contains(&diagnostic) {
            result.push(diagnostic);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{
        ModeInference, ModeReasonKind, NO_SINGLE_MODE, infer_module_mode, infer_project, join_modes,
    };
    use astra_core::mode::Mode;
    use astra_core::parser::parse;

    fn infer(source: &str) -> ModeInference {
        let module = parse(source).expect("source should parse");
        infer_module_mode(&module)
    }

    #[test]
    fn core_program_infers_core() {
        let inference = infer("fn main() {}\n");
        assert_eq!(inference.required_mode, Mode::Core);
        assert_eq!(inference.resolved_mode(), Mode::Core);
        assert!(inference.is_ok());
    }

    #[test]
    fn console_builtin_requires_app() {
        let inference = infer("fn main() { println(\"hello\") }\n");
        assert_eq!(inference.required_mode, Mode::App);
        assert!(
            inference
                .reasons
                .iter()
                .any(|reason| reason.kind == ModeReasonKind::Builtin && reason.mode == Mode::App)
        );
    }

    #[test]
    fn capability_tags_force_web() {
        let inference = infer(
            r#"
@tags(fetch)
module app.client
fn load() {}
"#,
        );
        assert_eq!(inference.required_mode, Mode::Web);
        assert!(inference.capabilities.contains("fetch"));
    }

    #[test]
    fn transitive_calls_force_the_callee_mode() {
        let inference = infer(
            r#"
@tags(fs)
fn read() {}
fn wrapper() { read() }
fn main() { wrapper() }
"#,
        );
        assert_eq!(inference.required_mode, Mode::System);
        assert!(
            inference
                .reasons
                .iter()
                .any(|reason| reason.kind == ModeReasonKind::CallGraph)
        );
    }

    #[test]
    fn constant_calls_join_the_call_graph() {
        let inference = infer(
            r#"
@tags(fetch)
fn load() -> i32 { return 1 }
const VALUE: i32 = load()
fn main() {}
"#,
        );
        assert_eq!(inference.required_mode, Mode::Web);
        assert!(
            inference
                .reasons
                .iter()
                .any(|reason| reason.kind == ModeReasonKind::CallGraph)
        );
    }

    #[test]
    fn recursion_converges() {
        let inference = infer(
            r#"
@tags(fetch)
fn load() {}
fn a() { b() }
fn b() { a() }
fn main() { a(); load() }
"#,
        );
        assert_eq!(inference.required_mode, Mode::Web);
    }

    #[test]
    fn explicit_core_does_not_hide_console_usage() {
        let inference = infer(
            r#"
@requires(core)
module app.main
fn main() { println("hello") }
"#,
        );
        assert_eq!(inference.resolved_mode(), Mode::Core);
        assert!(!inference.is_ok());
    }

    #[test]
    fn scoped_blocks_may_narrow() {
        let inference = infer(
            r#"
@requires(web)
module app.client
fn main() {
    {@mode:core;
        let value = 1
    }
}
"#,
        );
        assert_eq!(inference.resolved_mode(), Mode::Web);
        assert!(inference.is_ok());
    }

    #[test]
    fn scoped_blocks_may_not_widen_an_explicit_requirement() {
        let inference = infer(
            r#"
@requires(core)
module app.main
fn main() {
    {@mode:system;
        let value = 1
    }
}
"#,
        );
        assert!(!inference.is_ok());
    }

    #[test]
    fn web_and_system_are_rejected_as_a_single_mode() {
        let inference = infer(
            r#"
@tags(fetch)
@tags(fs)
module app.main
fn main() {}
"#,
        );
        assert!(
            inference
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == NO_SINGLE_MODE)
        );
    }

    #[test]
    fn project_join_raises_the_least_privileged_mode() {
        let core = infer("fn main() {}\n").named("core");
        let app = infer("fn main() { println(\"hi\") }\n").named("app");
        let project = infer_project(&[core, app]);
        assert_eq!(project.required_mode, Mode::App);
        assert!(project.is_ok());
    }

    #[test]
    fn join_modes_is_containment_not_numeric_order() {
        assert_eq!(join_modes(Mode::App, Mode::Web), Some(Mode::Web));
        assert_eq!(join_modes(Mode::App, Mode::System), Some(Mode::System));
        assert_eq!(join_modes(Mode::Web, Mode::System), None);
    }

    #[test]
    fn explanation_lists_reasons() {
        let inference = infer("@tags(fetch)\nfn load() {}\n");
        let explanation = inference.explain();
        assert!(explanation.contains("Resolved mode: web"));
        assert!(explanation.contains("capability tag 'fetch'"));
    }
}
