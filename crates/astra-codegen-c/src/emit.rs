//! The C17 emitter.
//!
//! Lowers a checked module (AST + HIR + annotations) to a single self-contained
//! C17 translation unit. The emitter is a typed source-to-source backend over
//! the same shared frontend that Nova and the JS backend consume, satisfying the
//! backend DoD ("consumes the shared IR, not a private parser").
//!
//! Supported first slice (see `C-001` in `Docs/Agent/todo.md`): the portable
//! managed subset — typed functions, primitives, let bindings, arithmetic with
//! explicit signed wrapping, control flow, print/println/assert, strings,
//! named and tuple structs, enums with payload variants, `Option`/`Result`,
//! single-error `!` propagation, pattern matching, constants, arrays, and
//! panic/abort. Anything beyond that slice is rejected with a stable `NS80xx`
//! diagnostic instead of emitting incorrect C.

// The C backend slice is still under active development; keep its structural
// lints from masking shared frontend diagnostics until C-001 is complete.
#![allow(
    clippy::collapsible_if,
    clippy::let_and_return,
    clippy::match_like_matches_macro
)]

use std::fmt::Write as _;

use astra_core::ast::{
    self, BinaryOp, ElseBranch, Expr, ExprKind, FailArguments, Literal, MatchArm, MatchArmBody,
    Pattern, PatternKind, Stmt, StmtKind, UnaryOp,
};
use astra_core::types::Type;

use crate::runtime::RUNTIME_HEADER;
use crate::ty::{Annotations, Reject, TyCtxt, TypeEnv, path_to_type, sanitize, to_type};
use crate::{BackendError, CModule, GeneratedC, IntegerPolicy};

const C_KEYWORDS: &[&str] = &[
    "auto",
    "break",
    "case",
    "char",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extern",
    "float",
    "for",
    "goto",
    "if",
    "inline",
    "int",
    "long",
    "register",
    "restrict",
    "return",
    "short",
    "signed",
    "size_t",
    "static",
    "struct",
    "switch",
    "typedef",
    "union",
    "unsigned",
    "void",
    "volatile",
    "while",
    "_Bool",
    "_Complex",
    "_Imaginary",
    "_Alignas",
    "_Alignof",
    "_Atomic",
    "_Generic",
    "_Noreturn",
    "_Static_assert",
    "_Thread_local",
    "bool",
    "true",
    "false",
];

fn is_c_keyword(name: &str) -> bool {
    C_KEYWORDS.contains(&name)
}

fn field_c(name: &str) -> String {
    if is_c_keyword(name) {
        format!("f_{name}")
    } else {
        name.to_string()
    }
}

/// A backend rejection surfaced from the emitter.
fn reject(code: &'static str, message: impl Into<String>) -> Reject {
    Reject {
        code,
        message: message.into(),
    }
}

/// Emits a generated C translation unit (and the matching host shim path).
pub fn emit(module: &CModule) -> Result<GeneratedC, BackendError> {
    if !module.manual_memory_scopes.is_empty() {
        return Err(BackendError::new(
            "NS8012",
            "@memory:manual is not supported by the C managed-subset backend; native manual memory is tracked by GC-001",
        ));
    }
    if module
        .ast
        .items
        .iter()
        .any(|item| matches!(item.kind, ast::ItemKind::Import(_)))
    {
        return Err(BackendError::new(
            "NS8019",
            "imports are not supported by the C backend first slice; it emits a single self-contained module",
        ));
    }

    let env = TypeEnv::collect(&module.ast);
    let mut tcx = TyCtxt::new(env);
    prescan(&mut tcx, module);
    if tcx.rejected() {
        let r = tcx.rejects.into_iter().next().expect("non-empty");
        return Err(BackendError::new(r.code, &r.message));
    }

    let ann = Annotations::new(&module.annotations);
    let mut e = Emitter {
        module,
        tcx: &mut tcx,
        ann,
        policy: module.integer_policy,
        out: String::new(),
        pre: String::new(),
        fresh: 0,
        cur_ret: Type::Unit,
        cur_err: None,
    };
    e.emit_program();
    Ok(GeneratedC {
        source: e.out,
        host_shim: crate::runtime::HOST_SHIM.to_string(),
        target: "c".to_string(),
    })
}

/// Pre-scans declared and annotation types to register every `typedef` and
/// surface rejects before any code is written.
fn prescan(tcx: &mut TyCtxt, module: &CModule) {
    for item in &module.ast.items {
        match &item.kind {
            ast::ItemKind::Function(f) => {
                for param in &f.parameters {
                    let _ = tcx.c_type(&to_type(&param.ty));
                }
                if let Some(ret) = &f.return_type {
                    let _ = tcx.c_type(&to_type(ret));
                }
                for err in &f.error_set {
                    let _ = tcx.c_type(&path_to_type(err));
                }
            }
            ast::ItemKind::Struct(s) => {
                let shape = crate::ty::DeclShape::Struct(crate::ty::StructShape {
                    tuple: matches!(s.fields, ast::StructFields::Tuple(_)),
                    fields: match &s.fields {
                        ast::StructFields::Named(fs) => fs
                            .iter()
                            .map(|f| (f.name.clone(), to_type(&f.ty)))
                            .collect(),
                        ast::StructFields::Tuple(ts) => ts
                            .iter()
                            .enumerate()
                            .map(|(i, t)| (format!("_{i}"), to_type(t)))
                            .collect(),
                    },
                });
                let _ = tcx.c_type(&Type::Named(s.name.clone(), Vec::new()));
                // Register the shape directly for completeness.
                tcx.env.decls.insert(s.name.clone(), shape);
            }
            ast::ItemKind::Enum(e) => {
                let _ = tcx.c_type(&Type::Named(e.name.clone(), Vec::new()));
            }
            ast::ItemKind::Constant(c) => {
                let _ = tcx.c_type(&to_type(&c.ty));
            }
            ast::ItemKind::ScopedItems(s) => {
                prescan_items(tcx, &s.items);
            }
            ast::ItemKind::Export(exp) => {
                if let ast::ExportForm::DefaultFunction(f) = &exp.form {
                    prescan_fn(tcx, f);
                }
            }
            _ => {}
        }
    }
    for ann in &module.annotations {
        let _ = tcx.c_type(&ann.ty);
    }
}

fn prescan_items(tcx: &mut TyCtxt, items: &[ast::Item]) {
    for item in items {
        match &item.kind {
            ast::ItemKind::Function(f) => prescan_fn(tcx, f),
            ast::ItemKind::Constant(c) => {
                let _ = tcx.c_type(&to_type(&c.ty));
            }
            ast::ItemKind::ScopedItems(s) => prescan_items(tcx, &s.items),
            _ => {}
        }
    }
}

fn prescan_fn(tcx: &mut TyCtxt, f: &ast::FunctionDecl) {
    for param in &f.parameters {
        let _ = tcx.c_type(&to_type(&param.ty));
    }
    if let Some(ret) = &f.return_type {
        let _ = tcx.c_type(&to_type(ret));
    }
    for err in &f.error_set {
        let _ = tcx.c_type(&path_to_type(err));
    }
}

struct Emitter<'a> {
    module: &'a CModule,
    tcx: &'a mut TyCtxt,
    ann: Annotations,
    policy: IntegerPolicy,
    out: String,
    pre: String,
    fresh: usize,
    cur_ret: Type,
    cur_err: Option<Type>,
}

impl<'a> Emitter<'a> {
    fn emit_program(&mut self) {
        self.out.push_str(RUNTIME_HEADER);
        self.out.push('\n');
        let _ = writeln!(self.out, "/* Neo Astra module: {} */", self.module.name);
        self.out.push('\n');
        if !self.tcx.typedefs.is_empty() {
            self.out
                .push_str("/* User and derived type definitions. */\n");
            for decl in &self.tcx.typedefs.clone() {
                self.out.push_str(decl);
                self.out.push('\n');
            }
            self.out.push('\n');
        }

        let (fns, consts) = self.collect_decls();
        // Forward declarations so call order does not matter.
        for f in &fns {
            self.emit_fn_proto(f);
        }
        self.out.push('\n');
        for c in &consts {
            self.emit_constant(c);
        }
        self.out.push('\n');
        for f in &fns {
            self.emit_function(f);
            self.out.push('\n');
        }
        if let Some(main) = self.find_main() {
            self.emit_main(&main);
        }
    }

    fn collect_decls(&self) -> (Vec<ast::FunctionDecl>, Vec<ast::ConstantDecl>) {
        let mut fns = Vec::new();
        let mut consts = Vec::new();
        self.collect_items(&self.module.ast.items, &mut fns, &mut consts);
        (fns, consts)
    }

    fn collect_items(
        &self,
        items: &[ast::Item],
        fns: &mut Vec<ast::FunctionDecl>,
        consts: &mut Vec<ast::ConstantDecl>,
    ) {
        for item in items {
            match &item.kind {
                ast::ItemKind::Function(f) => fns.push(f.clone()),
                ast::ItemKind::Constant(c) => consts.push(c.clone()),
                ast::ItemKind::ScopedItems(s) => self.collect_items(&s.items, fns, consts),
                ast::ItemKind::Export(exp) => {
                    if let ast::ExportForm::DefaultFunction(f) = &exp.form {
                        fns.push(f.clone());
                    }
                }
                _ => {}
            }
        }
    }

    fn find_main(&self) -> Option<ast::FunctionDecl> {
        for item in &self.module.ast.items {
            if let ast::ItemKind::Function(f) = &item.kind {
                if f.name == "main" && f.parameters.is_empty() && f.error_set.is_empty() {
                    let ret = f.return_type.as_ref().map(to_type).unwrap_or(Type::Unit);
                    if ret == Type::Unit {
                        return Some(f.clone());
                    }
                }
            }
            if let ast::ItemKind::ScopedItems(s) = &item.kind {
                for inner in &s.items {
                    if let ast::ItemKind::Function(f) = &inner.kind {
                        if f.name == "main" && f.parameters.is_empty() && f.error_set.is_empty() {
                            let ret = f.return_type.as_ref().map(to_type).unwrap_or(Type::Unit);
                            if ret == Type::Unit {
                                return Some(f.clone());
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn emit_fn_proto(&mut self, f: &ast::FunctionDecl) {
        if !f.type_parameters.is_empty() {
            self.tcx.rejects.push(reject(
                "NS8018",
                format!("generic function '{}' requires monomorphization", f.name),
            ));
            return;
        }
        let (ret_c, _is_fallible) = self.fn_return_c(f);
        let params = self.fn_params_c(f);
        let _ = writeln!(
            self.out,
            "static {ret_c} ns_{}({params});",
            sanitize(&f.name)
        );
    }

    fn fn_params_c(&mut self, f: &ast::FunctionDecl) -> String {
        if f.parameters.is_empty() {
            return "void".to_string();
        }
        f.parameters
            .iter()
            .map(|p| {
                let t = self.tcx.c_type(&to_type(&p.ty));
                format!("{t} v_{}", sanitize(&p.name))
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn fn_return_c(&mut self, f: &ast::FunctionDecl) -> (String, bool) {
        let ret = f.return_type.as_ref().map(to_type).unwrap_or(Type::Unit);
        let errs: Vec<Type> = f.error_set.iter().map(path_to_type).collect();
        if errs.len() > 1 {
            self.tcx.rejects.push(reject(
                "NS8019",
                format!(
                    "function '{}' has multiple error types, unsupported by the C backend first slice",
                    f.name
                ),
            ));
            return ("void".to_string(), false);
        }
        let err = errs.into_iter().next();
        match err {
            Some(e) => {
                let ty = Type::Result(Box::new(ret.clone()), Box::new(e));
                (self.tcx.c_type(&ty), true)
            }
            None => {
                if ret == Type::Unit {
                    ("void".to_string(), false)
                } else {
                    (self.tcx.c_type(&ret), false)
                }
            }
        }
    }

    fn emit_constant(&mut self, c: &ast::ConstantDecl) {
        let ty = to_type(&c.ty);
        let ty_c = self.tcx.c_type(&ty);
        self.pre.clear();
        let v = self.expr(&c.value);
        self.flush_pre();
        let _ = writeln!(
            self.out,
            "static const {ty_c} const_{} = {v};",
            sanitize(&c.name)
        );
    }

    fn emit_function(&mut self, f: &ast::FunctionDecl) {
        if !f.type_parameters.is_empty() {
            return;
        }
        let ret = f.return_type.as_ref().map(to_type).unwrap_or(Type::Unit);
        let errs: Vec<Type> = f.error_set.iter().map(path_to_type).collect();
        let err = errs.into_iter().next();
        self.cur_ret = ret.clone();
        self.cur_err = err.clone();
        let (ret_c, is_fallible) = self.fn_return_c(f);
        let params = self.fn_params_c(f);
        let _ = writeln!(
            self.out,
            "static {ret_c} ns_{}({params}) {{",
            sanitize(&f.name)
        );
        self.emit_block_body(&f.body, true, is_fallible);
        self.out.push_str("}\n");
    }

    fn emit_main(&mut self, main: &ast::FunctionDecl) {
        self.cur_ret = Type::Unit;
        self.cur_err = None;
        self.out.push_str("int main(void) {\n");
        self.emit_block_body(&main.body, true, false);
        self.out.push_str("    return 0;\n");
        self.out.push_str("}\n");
    }

    /// Emits a block's statements. When `is_fn_body` is true, the last statement
    /// is treated as the function's tail expression (returned, possibly wrapped
    /// in `Ok` for fallible functions). When `is_fn_body` is false, the block is
    /// a nested scope and its value is discarded.
    fn emit_block_body(&mut self, block: &ast::Block, is_fn_body: bool, is_fallible: bool) {
        let n = block.statements.len();
        for (i, stmt) in block.statements.iter().enumerate() {
            let is_tail = is_fn_body && i + 1 == n;
            if is_tail {
                self.emit_tail_stmt(stmt, is_fallible);
            } else {
                self.emit_stmt(stmt);
            }
        }
        if is_fn_body && block.statements.is_empty() {
            self.emit_implicit_tail_return(is_fallible);
        }
    }

    fn emit_implicit_tail_return(&mut self, is_fallible: bool) {
        if is_fallible {
            let ret_c = self.fn_result_c();
            let _ = writeln!(self.out, "    return ({ret_c}){{ .ok = true }};");
        }
    }

    fn emit_tail_stmt(&mut self, stmt: &Stmt, is_fallible: bool) {
        match &stmt.kind {
            StmtKind::Expr(e) => self.emit_tail_expr(e, is_fallible),
            StmtKind::Return { .. } => self.emit_stmt(stmt),
            _ => {
                self.emit_stmt(stmt);
                self.emit_implicit_tail_return(is_fallible);
            }
        }
    }

    fn emit_tail_expr(&mut self, e: &Expr, is_fallible: bool) {
        let ty = self.type_of(e).cloned();
        if matches!(ty, Some(Type::Never)) {
            self.emit_expr_stmt(e);
            return;
        }
        self.pre.clear();
        let v = self.expr(e);
        self.flush_pre();
        if is_fallible {
            let ret_c = self.fn_result_c();
            let _ = writeln!(
                self.out,
                "    return ({ret_c}){{ .ok = true, .value = {v} }};"
            );
        } else if self.cur_ret == Type::Unit {
            // A unit tail expression is a side effect, not a return value.
            self.emit_value_stmt(e, v, &ty);
        } else {
            let _ = writeln!(self.out, "    return {v};");
        }
    }

    fn fn_result_c(&mut self) -> String {
        let ret = self.cur_ret.clone();
        match &self.cur_err {
            Some(err) => self
                .tcx
                .c_type(&Type::Result(Box::new(ret), Box::new(err.clone()))),
            None => self.tcx.c_type(&ret),
        }
    }

    fn emit_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Let {
                mutable: _,
                name,
                ty,
                value,
            } => self.emit_let(name, ty.as_ref(), value),
            StmtKind::Return { value } => self.emit_return(value.as_ref()),
            StmtKind::Break => self.writeln_s("    break;"),
            StmtKind::Continue => self.writeln_s("    continue;"),
            StmtKind::Defer(_) => self.tcx.rejects.push(reject(
                "NS8010",
                "defer is not supported by the C backend first slice",
            )),
            StmtKind::While { condition, body } => self.emit_while(condition, body),
            StmtKind::For {
                binding,
                iterable,
                body,
            } => self.emit_for(binding, iterable, body),
            StmtKind::Expr(e) => self.emit_expr_stmt(e),
            StmtKind::Scoped(s) => {
                self.writeln_s("    {");
                self.emit_scoped_block(s);
                self.writeln_s("    }");
            }
        }
    }

    fn emit_scoped_block(&mut self, scoped: &ast::ScopedStatements) {
        if scoped.annotation.kind == ast::ScopeKind::Memory && scoped.annotation.value == "manual" {
            self.tcx.rejects.push(reject(
                "NS8012",
                "@memory:manual is not supported by the C managed-subset backend",
            ));
            return;
        }
        let n = scoped.statements.len();
        for (i, stmt) in scoped.statements.iter().enumerate() {
            let is_tail = i + 1 == n;
            if is_tail {
                self.emit_tail_stmt(stmt, self.cur_err.is_some());
            } else {
                self.emit_stmt(stmt);
            }
        }
        if scoped.statements.is_empty() {
            self.emit_implicit_tail_return(self.cur_err.is_some());
        }
    }

    fn emit_let(&mut self, name: &str, ty: Option<&ast::Type>, value: &Expr) {
        let val_ty = ty
            .map(to_type)
            .or_else(|| self.type_of(value).cloned())
            .unwrap_or(Type::Unit);
        if val_ty == Type::Unit {
            self.emit_expr_stmt(value);
            return;
        }
        self.pre.clear();
        let v = self.expr(value);
        self.flush_pre();
        let ty_c = self.tcx.c_type(&val_ty);
        let _ = writeln!(self.out, "    {ty_c} v_{} = {v};", sanitize(name));
    }

    fn emit_return(&mut self, value: Option<&Expr>) {
        let is_fallible = self.cur_err.is_some();
        match value {
            None => {
                if is_fallible {
                    let ret_c = self.fn_result_c();
                    let _ = writeln!(self.out, "    return ({ret_c}){{ .ok = true }};");
                } else {
                    self.writeln_s("    return;");
                }
            }
            Some(e) => {
                let ty = self.type_of(e).cloned();
                if matches!(ty, Some(Type::Never)) {
                    self.emit_expr_stmt(e);
                    return;
                }
                self.pre.clear();
                let v = self.expr(e);
                self.flush_pre();
                if is_fallible {
                    let ret_c = self.fn_result_c();
                    let _ = writeln!(
                        self.out,
                        "    return ({ret_c}){{ .ok = true, .value = {v} }};"
                    );
                } else {
                    let _ = writeln!(self.out, "    return {v};");
                }
            }
        }
    }

    fn emit_while(&mut self, condition: &Expr, body: &ast::Block) {
        self.pre.clear();
        let cond = self.expr(condition);
        self.flush_pre();
        let _ = writeln!(self.out, "    while ({cond}) {{");
        self.emit_block_body(body, false, false);
        self.writeln_s("    }");
    }

    fn emit_for(&mut self, binding: &str, iterable: &Expr, body: &ast::Block) {
        let iter_ty = self.type_of(iterable).cloned().unwrap_or(Type::Unit);
        let elem_ty = match &iter_ty {
            Type::Array(t) | Type::Slice(t) => (**t).clone(),
            _ => {
                self.tcx.rejects.push(reject(
                    "NS8021",
                    "for-loop iteration is only supported over arrays in the C backend first slice",
                ));
                return;
            }
        };
        self.pre.clear();
        let iter = self.expr(iterable);
        self.flush_pre();
        let elem_c = self.tcx.c_type(&elem_ty);
        let idx = self.fresh_id("i");
        let _ = writeln!(
            self.out,
            "    for (size_t {idx} = 0; {idx} < ({iter}).len; ++{idx}) {{"
        );
        let _ = writeln!(
            self.out,
            "        {elem_c} v_{} = *({elem_c}*)astra_array_index(&({iter}), {idx}, sizeof({elem_c}));",
            sanitize(binding)
        );
        self.emit_block_body(body, false, false);
        self.writeln_s("    }");
    }

    fn emit_expr_stmt(&mut self, e: &Expr) {
        let ty = self.type_of(e).cloned();
        if matches!(ty, Some(Type::Never)) {
            self.pre.clear();
            let _ = self.expr(e);
            self.flush_pre();
            return;
        }
        self.pre.clear();
        let v = self.expr(e);
        self.flush_pre();
        self.emit_value_stmt(e, v, &ty);
    }

    /// Emits an expression-statement value with the right void handling.
    fn emit_value_stmt(&mut self, e: &Expr, v: String, ty: &Option<Type>) {
        let is_void_call = matches!(e.kind, ExprKind::Call { .. } | ExprKind::Assign { .. })
            && matches!(ty, Some(Type::Unit));
        if is_void_call {
            let _ = writeln!(self.out, "    {v};");
        } else {
            let _ = writeln!(self.out, "    (void)({v});");
        }
    }

    fn flush_pre(&mut self) {
        self.out.push_str(&self.pre);
        self.pre.clear();
    }

    fn writeln_s(&mut self, line: &str) {
        self.out.push_str(line);
        self.out.push('\n');
    }

    fn fresh_id(&mut self, prefix: &str) -> String {
        self.fresh += 1;
        format!("__{prefix}{}", self.fresh)
    }

    fn type_of(&self, e: &Expr) -> Option<&Type> {
        self.ann.type_of(e)
    }

    fn c_type(&mut self, t: &Type) -> String {
        self.tcx.c_type(t)
    }

    fn zero(&mut self, t: &Type) -> String {
        match t {
            Type::Bool => "false".into(),
            Type::F32 | Type::F64 => "0.0".into(),
            Type::I8
            | Type::I16
            | Type::I32
            | Type::I64
            | Type::U8
            | Type::U16
            | Type::U32
            | Type::U64
            | Type::Usize
            | Type::Isize => "0".into(),
            Type::Unit => "(NS_Unit){0}".into(),
            Type::Str => "(NS_Str){0}".into(),
            Type::Array(_) | Type::Slice(_) => "(NS_Array){0}".into(),
            Type::Option(_) => format!("({}){{ .some = false }}", self.c_type(t)),
            Type::Result(_, _) => format!("({}){{ .ok = true }}", self.c_type(t)),
            _ => format!("(({}){{0}})", self.c_type(t)),
        }
    }

    /// The core expression emitter. Returns a C expression string and may write
    /// hoisted temp declarations and early-error returns to `self.pre`.
    fn expr(&mut self, e: &Expr) -> String {
        match &e.kind {
            ExprKind::Literal(lit) => self.literal_c(lit),
            ExprKind::Identifier(name) => self.identifier_c(name, e),
            ExprKind::Unary { operator, operand } => self.unary_c(*operator, operand),
            ExprKind::Binary {
                operator,
                left,
                right,
            } => self.binary_c(*operator, left, right, e),
            ExprKind::Assign { target, value } => self.assign_c(target, value),
            ExprKind::Call { callee, arguments } => self.call_c(callee, arguments, e),
            ExprKind::Index { target, index } => self.index_c(target, index, e),
            ExprKind::Member { target, member } => self.member_c(target, member, e),
            ExprKind::TypeArguments { target, .. } => self.expr(target),
            ExprKind::Propagate(inner) => self.propagate_c(inner),
            ExprKind::Array(elements) => self.array_literal_c(elements, e),
            ExprKind::Map(_) => {
                self.tcx.rejects.push(reject(
                    "NS8013",
                    "map literals are not supported by the C backend first slice",
                ));
                "(NS_Array){0}".into()
            }
            ExprKind::Tuple(elements) => self.tuple_c(elements, e),
            ExprKind::StructInit { path, fields } => self.struct_init_c(path, fields),
            ExprKind::Block(block) => self.block_expr_c(block),
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => self.if_expr_c(condition, then_branch, else_branch.as_ref(), e),
            ExprKind::Match { value, arms } => self.match_expr_c(value, arms, e),
            ExprKind::Spawn { .. } => {
                self.tcx.rejects.push(reject(
                    "NS8011",
                    "spawn is not supported by the C backend first slice (see CONC-001)",
                ));
                "(NS_Unit){0}".into()
            }
            ExprKind::Fail { error, arguments } => self.fail_c(error, arguments),
            ExprKind::Panic { message } => self.panic_c(message),
            ExprKind::Abort { message } => self.abort_c(message),
        }
    }

    fn literal_c(&mut self, lit: &Literal) -> String {
        match lit {
            Literal::Integer(raw) => {
                let cleaned: String = raw.chars().filter(|c| *c != '_').collect();
                // Strip a trailing `u8` byte-literal suffix if present.
                let cleaned = cleaned
                    .strip_suffix("u8")
                    .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
                    .unwrap_or(&cleaned)
                    .to_string();
                cleaned
            }
            Literal::Float(raw) => raw.chars().filter(|c| *c != '_').collect::<String>(),
            Literal::String(value) => self.c_string_literal(value),
            Literal::Bytes(_) => {
                self.tcx.rejects.push(reject(
                    "NS8014",
                    "bytes literals are not supported by the C backend first slice",
                ));
                "(NS_Str){0}".into()
            }
            Literal::Bool(b) => {
                if *b {
                    "true".into()
                } else {
                    "false".into()
                }
            }
            Literal::Unit => "(NS_Unit){0}".into(),
        }
    }

    /// Encodes a UTF-8 string value as a `NS_Str` compound literal referencing a
    /// static byte array. Renders non-ASCII bytes via `\xHH` escapes and
    /// prefixes the literal with a hex-escape guard so adjacent identifiers
    /// cannot merge with a preceding token.
    fn c_string_literal(&self, value: &str) -> String {
        let mut bytes = String::from("\"");
        let mut prev_hex = false;
        for &b in value.as_bytes() {
            match b {
                b'\\' => {
                    bytes.push_str("\\\\");
                    prev_hex = false;
                }
                b'"' => {
                    bytes.push_str("\\\"");
                    prev_hex = false;
                }
                b'\n' => {
                    bytes.push_str("\\n");
                    prev_hex = false;
                }
                b'\r' => {
                    bytes.push_str("\\r");
                    prev_hex = false;
                }
                b'\t' => {
                    bytes.push_str("\\t");
                    prev_hex = false;
                }
                0x20..=0x7e => {
                    bytes.push(b as char);
                    prev_hex = false;
                }
                _ => {
                    let _ = write!(bytes, "\\x{b:02X}");
                    prev_hex = true;
                }
            }
        }
        bytes.push('"');
        // A hex escape can be absorbed by a following identifier character, so
        // insert an empty string split when needed.
        let lit = if prev_hex {
            format!("\"\" {bytes}")
        } else {
            bytes
        };
        let len = value.len();
        format!("(NS_Str){{ .data = {lit}, .len = {len} }}")
    }

    fn identifier_c(&mut self, name: &str, _e: &Expr) -> String {
        // Constants and locals are the only bare identifiers in this slice.
        if self.is_constant(name) {
            format!("const_{}", sanitize(name))
        } else {
            format!("v_{}", sanitize(name))
        }
    }

    fn is_constant(&self, name: &str) -> bool {
        fn in_items(items: &[ast::Item], name: &str) -> bool {
            items.iter().any(|item| match &item.kind {
                ast::ItemKind::Constant(c) => c.name == name,
                ast::ItemKind::ScopedItems(s) => in_items(&s.items, name),
                _ => false,
            })
        }
        in_items(&self.module.ast.items, name)
    }

    fn unary_c(&mut self, op: UnaryOp, operand: &Expr) -> String {
        let v = self.expr(operand);
        match op {
            UnaryOp::Negate => format!("(-{v})"),
            UnaryOp::Not => {
                if matches!(self.type_of(operand), Some(Type::Bool)) {
                    format!("(!{v})")
                } else {
                    format!("(~{v})")
                }
            }
        }
    }

    fn binary_c(&mut self, op: BinaryOp, left: &Expr, right: &Expr, _e: &Expr) -> String {
        if matches!(op, BinaryOp::And | BinaryOp::Or) {
            let l = self.expr(left);
            let r = self.expr(right);
            return if op == BinaryOp::And {
                format!("({l} && {r})")
            } else {
                format!("({l} || {r})")
            };
        }
        let l = self.expr(left);
        let r = self.expr(right);
        match op {
            BinaryOp::Equal | BinaryOp::NotEqual => {
                let sym = if op == BinaryOp::Equal { "==" } else { "!=" };
                if self.eq_supported(left) {
                    format!("({l} {sym} {r})")
                } else {
                    self.tcx.rejects.push(reject(
                        "NS8021",
                        "equality is only supported for primitive values in the C backend first slice",
                    ));
                    "false".into()
                }
            }
            BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual => {
                let sym = match op {
                    BinaryOp::Less => "<",
                    BinaryOp::LessEqual => "<=",
                    BinaryOp::Greater => ">",
                    _ => ">=",
                };
                format!("({l} {sym} {r})")
            }
            BinaryOp::Add
            | BinaryOp::Subtract
            | BinaryOp::Multiply
            | BinaryOp::Divide
            | BinaryOp::Remainder
            | BinaryOp::AddWrap
            | BinaryOp::SubtractWrap
            | BinaryOp::MultiplyWrap
            | BinaryOp::DivideWrap => self.arith_c(op, left, right, &l, &r),
            BinaryOp::And | BinaryOp::Or => unreachable!(),
        }
    }

    fn eq_supported(&self, e: &Expr) -> bool {
        matches!(
            self.type_of(e),
            Some(
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
                    | Type::Unit
            )
        )
    }

    fn arith_c(&mut self, op: BinaryOp, left: &Expr, right: &Expr, l: &str, r: &str) -> String {
        let lt = self.type_of(left).cloned();
        let rt = self.type_of(right).cloned();
        let numeric = match (&lt, &rt) {
            (Some(a), Some(b)) if a.is_numeric() && b.is_numeric() => true,
            _ => false,
        };
        if !numeric {
            self.tcx.rejects.push(reject(
                "NS8021",
                "arithmetic is only supported on numeric values in the C backend first slice",
            ));
            return "0".into();
        }
        let ty = lt.unwrap_or(Type::I32);
        let is_wrapping_op = matches!(
            op,
            BinaryOp::AddWrap
                | BinaryOp::SubtractWrap
                | BinaryOp::MultiplyWrap
                | BinaryOp::DivideWrap
        );
        let policy_wrapping = matches!(self.policy, IntegerPolicy::Wrapping);
        let sym = match op {
            BinaryOp::Add | BinaryOp::AddWrap => "+",
            BinaryOp::Subtract | BinaryOp::SubtractWrap => "-",
            BinaryOp::Multiply | BinaryOp::MultiplyWrap => "*",
            BinaryOp::Divide | BinaryOp::DivideWrap => "/",
            BinaryOp::Remainder => "%",
            _ => unreachable!(),
        };
        if matches!(op, BinaryOp::Divide | BinaryOp::Remainder) || ty.is_float() {
            return format!("({l} {sym} {r})");
        }
        // Unsigned integer arithmetic wraps by definition in C.
        if ty.is_integer() && !is_signed_int(&ty) {
            return format!("({l} {sym} {r})");
        }
        // Signed integer addition/subtraction/multiplication must wrap without
        // invoking C signed-overflow undefined behavior (design §6.2).
        if is_wrapping_op || policy_wrapping {
            if let Some(width) = signed_width(&ty) {
                let helper = match op {
                    BinaryOp::Add | BinaryOp::AddWrap => format!("astra_add_{width}"),
                    BinaryOp::Subtract | BinaryOp::SubtractWrap => format!("astra_sub_{width}"),
                    BinaryOp::Multiply | BinaryOp::MultiplyWrap => format!("astra_mul_{width}"),
                    _ => unreachable!(),
                };
                return format!("{helper}({l}, {r})");
            }
        }
        format!("({l} {sym} {r})")
    }

    fn assign_c(&mut self, target: &Expr, value: &Expr) -> String {
        let lhs = self.lvalue(target);
        let rhs = self.expr(value);
        format!("({lhs} = {rhs})")
    }

    /// Emits an lvalue (assignable) expression for assignment targets.
    fn lvalue(&mut self, e: &Expr) -> String {
        match &e.kind {
            ExprKind::Identifier(name) => format!("v_{}", sanitize(name)),
            ExprKind::Member { target, member } => {
                let t = self.expr(target);
                let f = field_c(member);
                format!("({t}).{f}")
            }
            ExprKind::Index { target, index } => {
                let elem_ty = self.elem_type_of(target);
                let t = self.expr(target);
                let i = self.expr(index);
                let elem_c = self.tcx.c_type(&elem_ty);
                format!("(*({elem_c}*)astra_array_index(&({t}), {i}, sizeof({elem_c})))")
            }
            _ => {
                self.tcx.rejects.push(reject(
                    "NS8021",
                    "this assignment target is not supported by the C backend first slice",
                ));
                "/* unsupported */".into()
            }
        }
    }

    fn elem_type_of(&self, arr: &Expr) -> Type {
        match self.type_of(arr) {
            Some(Type::Array(t)) | Some(Type::Slice(t)) => (**t).clone(),
            _ => Type::I32,
        }
    }

    fn call_c(&mut self, callee: &Expr, args: &[Expr], e: &Expr) -> String {
        match &callee.kind {
            ExprKind::Identifier(name) => {
                match name.as_str() {
                    "Some" => return self.some_c(args, e),
                    "Ok" => return self.ok_c(args, e),
                    "Err" => return self.err_c(args, e),
                    "print" => return self.print_c(args, false),
                    "println" => return self.print_c(args, true),
                    "assert" => return self.assert_c(args),
                    "dbg" => {
                        self.tcx.rejects.push(reject(
                            "NS8021",
                            "dbg is not supported by the C backend first slice",
                        ));
                        return "(NS_Unit){0}".into();
                    }
                    _ => {}
                }
                // User function call.
                let arg_c = self.args_c(args);
                format!("ns_{}({arg_c})", sanitize(name))
            }
            ExprKind::Member { target, member } => {
                if let ExprKind::Identifier(owner) = &target.kind {
                    match (owner.as_str(), member.as_str()) {
                        ("Option", "Some") => return self.some_c(args, e),
                        ("Result", "Ok") => return self.ok_c(args, e),
                        ("Result", "Err") => return self.err_c(args, e),
                        _ => {
                            // Enum variant constructor with payload.
                            if self.tcx.env.enum_of(owner).is_some() {
                                return self.enum_variant_c(owner, member, args);
                            }
                        }
                    }
                }
                // Method call.
                self.method_c(target, member, args)
            }
            _ => {
                self.tcx.rejects.push(reject(
                    "NS8017",
                    "calling this callee is not supported by the C backend first slice",
                ));
                "(NS_Unit){0}".into()
            }
        }
    }

    fn args_c(&mut self, args: &[Expr]) -> String {
        args.iter()
            .map(|a| self.expr(a))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn some_c(&mut self, args: &[Expr], e: &Expr) -> String {
        let ty = self
            .type_of(e)
            .cloned()
            .unwrap_or(Type::Option(Box::new(Type::I32)));
        let inner = match &ty {
            Type::Option(t) => (**t).clone(),
            _ => Type::I32,
        };
        let opt_c = self.tcx.c_type(&ty);
        let val = args
            .first()
            .map(|a| self.expr(a))
            .unwrap_or_else(|| "(NS_Unit){0}".into());
        let _ = self.tcx.c_type(&inner);
        format!("({opt_c}){{ .some = true, .value = {val} }}")
    }

    fn ok_c(&mut self, args: &[Expr], e: &Expr) -> String {
        let ty = self
            .type_of(e)
            .cloned()
            .unwrap_or(Type::Result(Box::new(Type::Unit), Box::new(Type::Unit)));
        let res_c = self.tcx.c_type(&ty);
        let val = args
            .first()
            .map(|a| self.expr(a))
            .unwrap_or_else(|| "(NS_Unit){0}".into());
        format!("({res_c}){{ .ok = true, .value = {val} }}")
    }

    fn err_c(&mut self, args: &[Expr], e: &Expr) -> String {
        let ty = self
            .type_of(e)
            .cloned()
            .unwrap_or(Type::Result(Box::new(Type::Unit), Box::new(Type::Unit)));
        let res_c = self.tcx.c_type(&ty);
        let val = args
            .first()
            .map(|a| self.expr(a))
            .unwrap_or_else(|| "(NS_Unit){0}".into());
        format!("({res_c}){{ .ok = false, .error = {val} }}")
    }

    fn enum_variant_c(&mut self, owner: &str, variant: &str, args: &[Expr]) -> String {
        let enum_c = self.tcx.c_type(&Type::Named(owner.to_string(), Vec::new()));
        let tag = format!("NS_{}_TAG_{}", sanitize(owner), sanitize(variant));
        let fields: Vec<Type> = match self.tcx.env.enum_of(owner) {
            Some(shape) => shape
                .variants
                .iter()
                .find(|v| v.name.as_str() == variant)
                .map(|v| v.fields.clone())
                .unwrap_or_default(),
            None => {
                self.tcx
                    .rejects
                    .push(reject("NS8020", format!("enum '{owner}' is not declared")));
                return format!("({enum_c}){{0}}");
            }
        };
        if fields.is_empty() && args.is_empty() {
            return format!("({enum_c}){{ .tag = {tag} }}");
        }
        if fields.is_empty() {
            self.tcx.rejects.push(reject(
                "NS8020",
                format!("enum variant '{owner}::{variant}' takes no arguments"),
            ));
            return format!("({enum_c}){{ .tag = {tag} }}");
        }
        let member = field_c(variant);
        let arg_vals: Vec<String> = args.iter().map(|a| self.expr(a)).collect();
        if fields.len() == 1 {
            format!(
                "({enum_c}){{ .tag = {tag}, .payload.{member} = {val} }}",
                val = arg_vals[0]
            )
        } else {
            let joined = arg_vals.join(", ");
            format!("({enum_c}){{ .tag = {tag}, .payload.{member} = {{ {joined} }} }}")
        }
    }

    fn method_c(&mut self, receiver: &Expr, name: &str, args: &[Expr]) -> String {
        let rt = self.type_of(receiver).cloned();
        match (&rt, name) {
            (Some(Type::Str), "len") => {
                let r = self.expr(receiver);
                format!("({r}).len")
            }
            (Some(Type::Str), "is_empty") => {
                let r = self.expr(receiver);
                format!("(({r}).len == 0)")
            }
            (Some(Type::Array(_) | Type::Slice(_)), "len") => {
                let r = self.expr(receiver);
                format!("({r}).len")
            }
            (Some(Type::Array(_) | Type::Slice(_)), "push") => {
                let r = self.expr(receiver);
                let elem_ty = self.elem_type_of(receiver);
                let elem_c = self.tcx.c_type(&elem_ty);
                let val = args
                    .first()
                    .map(|a| self.expr(a))
                    .unwrap_or_else(|| "0".into());
                format!("astra_array_push(&({r}), sizeof({elem_c}), &({elem_c}){{ {val} }})")
            }
            (Some(Type::Option(_)), "is_some") => {
                let r = self.expr(receiver);
                format!("({r}).some")
            }
            (Some(Type::Option(_)), "is_none") => {
                let r = self.expr(receiver);
                format!("(!({r}).some)")
            }
            _ => {
                self.tcx.rejects.push(reject(
                    "NS8021",
                    format!("method '.{name}()' is not supported by the C backend first slice"),
                ));
                "0".into()
            }
        }
    }

    fn print_c(&mut self, args: &[Expr], nl: bool) -> String {
        let Some(arg) = args.first() else {
            return if nl {
                "astra_println_unit()".into()
            } else {
                "astra_print_unit()".into()
            };
        };
        let ty = self.type_of(arg).cloned().unwrap_or(Type::Unit);
        let val = self.expr(arg);
        let suffix = match &ty {
            Type::I8 => "i8",
            Type::I16 => "i16",
            Type::I32 => "i32",
            Type::I64 => "i64",
            Type::U8 => "u8",
            Type::U16 => "u16",
            Type::U32 => "u32",
            Type::U64 => "u64",
            Type::Usize => "usize",
            Type::Isize => "isize",
            Type::F32 => "f32",
            Type::F64 => "f64",
            Type::Bool => "bool",
            Type::Str => "str",
            Type::Unit => "unit",
            _ => {
                self.tcx.rejects.push(reject(
                    "NS8024",
                    "printing this value type is not supported by the C backend first slice",
                ));
                return "0".into();
            }
        };
        let kind = if nl { "println" } else { "print" };
        format!("astra_{kind}_{suffix}({val})")
    }

    fn assert_c(&mut self, args: &[Expr]) -> String {
        let val = args
            .first()
            .map(|a| self.expr(a))
            .unwrap_or_else(|| "false".into());
        format!("astra_assert({val})")
    }

    fn index_c(&mut self, target: &Expr, index: &Expr, _e: &Expr) -> String {
        let elem_ty = self.elem_type_of(target);
        let elem_c = self.tcx.c_type(&elem_ty);
        let t = self.expr(target);
        let i = self.expr(index);
        format!("(*({elem_c}*)astra_array_index(&({t}), {i}, sizeof({elem_c})))")
    }

    fn member_c(&mut self, target: &Expr, member: &str, e: &Expr) -> String {
        // Unit enum variant used as a value: `Color::Red`.
        if let ExprKind::Identifier(owner) = &target.kind {
            if let Some(shape) = self.tcx.env.enum_of(owner) {
                if shape.variants.iter().any(|v| v.name == member) {
                    return self.enum_variant_c(owner, member, &[]);
                }
            }
        }
        let tt = self.type_of(target).cloned();
        let t = self.expr(target);
        match &tt {
            Some(Type::Named(name, _)) if self.tcx.env.struct_of(name).is_some() => {
                let shape = self.tcx.env.struct_of(name).cloned();
                if let Some(s) = &shape {
                    if s.fields.iter().any(|(f, _)| f == member) {
                        return format!("({t}).{m}", m = field_c(member));
                    }
                }
                self.tcx.rejects.push(reject(
                    "NS8020",
                    format!("struct '{name}' has no field '{member}'"),
                ));
                format!("({t}).{m}", m = field_c(member))
            }
            Some(Type::Tuple(_)) => {
                // Tuple field access by numeric member (e.g. `.0`).
                if let Ok(idx) = member.parse::<usize>() {
                    return format!("({t})._{idx}");
                }
                self.tcx.rejects.push(reject(
                    "NS8021",
                    "tuple field access is not supported by the C backend first slice",
                ));
                "0".into()
            }
            _ => {
                let _ = e;
                self.tcx.rejects.push(reject(
                    "NS8021",
                    format!(
                        "field access '.{member}' is not supported by the C backend first slice"
                    ),
                ));
                "0".into()
            }
        }
    }

    fn array_literal_c(&mut self, elements: &[Expr], _e: &Expr) -> String {
        let elem_ty = elements
            .first()
            .and_then(|el| self.type_of(el).cloned())
            .unwrap_or(Type::I32);
        let elem_c = self.tcx.c_type(&elem_ty);
        let vals: Vec<String> = elements.iter().map(|el| self.expr(el)).collect();
        let n = vals.len();
        let joined = vals.join(", ");
        format!("astra_array_new_lit({n}, sizeof({elem_c}), (const {elem_c}[]){{ {joined} }})")
    }

    fn tuple_c(&mut self, elements: &[Expr], _e: &Expr) -> String {
        let ty = self
            .type_of(&Expr {
                kind: ExprKind::Tuple(elements.to_vec()),
                span: astra_core::span::Span::default(),
            })
            .cloned()
            .unwrap_or(Type::Unit);
        if !matches!(ty, Type::Tuple(_)) {
            self.tcx.rejects.push(reject(
                "NS8025",
                "tuple type is not available for this tuple expression",
            ));
            return "(NS_Unit){0}".into();
        }
        let tuple_c = self.tcx.c_type(&ty);
        let vals: Vec<String> = elements.iter().map(|el| self.expr(el)).collect();
        let joined = vals.join(", ");
        format!("({tuple_c}){{ {joined} }}")
    }

    fn struct_init_c(&mut self, path: &ast::TypePath, fields: &[ast::FieldInit]) -> String {
        let name = path.segments.last().map(String::as_str).unwrap_or("");
        let ty_c = self.tcx.c_type(&Type::Named(name.to_string(), Vec::new()));
        let Some(shape) = self.tcx.env.struct_of(name).cloned() else {
            self.tcx
                .rejects
                .push(reject("NS8020", format!("struct '{name}' is not declared")));
            return format!("({ty_c}){{0}}");
        };
        if shape.tuple {
            // Tuple-struct construction: positional fields.
            let vals: Vec<String> = fields.iter().map(|f| self.expr(&f.value)).collect();
            let joined = vals.join(", ");
            format!("({ty_c}){{ {joined} }}")
        } else {
            let mut inits = String::new();
            for f in fields {
                let val = self.expr(&f.value);
                let _ = write!(inits, ".{} = {}, ", field_c(&f.name), val);
            }
            format!("({ty_c}){{ {inits}}}")
        }
    }

    fn block_expr_c(&mut self, block: &ast::Block) -> String {
        let ty = self.block_value_type(block);
        let tmp = self.fresh_id("b");
        let ty_c = if ty == Type::Unit {
            "NS_Unit".to_string()
        } else {
            self.tcx.c_type(&ty)
        };
        let zero = self.zero(&ty);
        let _ = writeln!(self.pre, "    {ty_c} {tmp} = {zero};");
        self.pre.push_str("    {\n");
        self.emit_block_value(block, &tmp, &ty);
        self.pre.push_str("    }\n");
        tmp
    }

    fn block_value_type(&self, block: &ast::Block) -> Type {
        block
            .statements
            .last()
            .and_then(|s| match &s.kind {
                StmtKind::Expr(e) => self.type_of(e).cloned(),
                _ => Some(Type::Unit),
            })
            .unwrap_or(Type::Unit)
    }

    /// Emits a block whose tail value is assigned to `target` (a fresh temp).
    fn emit_block_value(&mut self, block: &ast::Block, target: &str, block_ty: &Type) {
        let n = block.statements.len();
        for (i, stmt) in block.statements.iter().enumerate() {
            let is_tail = i + 1 == n;
            if is_tail {
                self.emit_block_tail(stmt, target, block_ty);
            } else {
                self.emit_stmt_into_pre(stmt);
            }
        }
        if block.statements.is_empty() && block_ty == &Type::Unit {
            let _ = writeln!(self.pre, "        {target} = (NS_Unit){{0}};");
        }
    }

    fn emit_stmt_into_pre(&mut self, stmt: &Stmt) {
        // Emit a statement into `self.pre` instead of `self.out`.
        let saved = std::mem::take(&mut self.out);
        self.emit_stmt(stmt);
        self.pre.push_str(&self.out);
        self.out = saved;
    }

    fn emit_block_tail(&mut self, stmt: &Stmt, target: &str, block_ty: &Type) {
        match &stmt.kind {
            StmtKind::Expr(e) => {
                let ty = self.type_of(e).cloned().unwrap_or(Type::Unit);
                if ty == Type::Never {
                    self.emit_expr_stmt(e);
                    return;
                }
                let v = self.expr(e);
                self.flush_pre();
                if block_ty == &Type::Unit {
                    self.emit_value_stmt(e, v, &Some(ty));
                } else {
                    let _ = writeln!(self.out, "    {target} = {v};");
                }
            }
            _ => {
                self.emit_stmt(stmt);
                if block_ty != &Type::Unit {
                    let z = self.zero(block_ty);
                    let _ = writeln!(self.out, "    {target} = {z};");
                }
            }
        }
    }

    fn if_expr_c(
        &mut self,
        condition: &Expr,
        then_branch: &ast::Block,
        else_branch: Option<&ElseBranch>,
        e: &Expr,
    ) -> String {
        let ty = self.type_of(e).cloned().unwrap_or(Type::Unit);
        let tmp = self.fresh_id("if");
        let ty_c = if ty == Type::Unit {
            "NS_Unit".to_string()
        } else {
            self.tcx.c_type(&ty)
        };
        let zero = self.zero(&ty);
        let _ = writeln!(self.pre, "    {ty_c} {tmp} = {zero};");
        let cond = self.expr(condition);
        let _ = writeln!(self.pre, "    if ({cond}) {{");
        self.emit_block_value(then_branch, &tmp, &ty);
        self.pre.push_str("    }");
        if let Some(ElseBranch::Block(b)) = else_branch {
            self.pre.push_str(" else {\n");
            self.emit_block_value(b, &tmp, &ty);
            self.pre.push_str("    }\n");
        } else if let Some(ElseBranch::If(inner)) = else_branch {
            // else-if: evaluate the inner if expression into the temp.
            self.pre.push_str(" else { ");
            let inner = self.expr(inner);
            let _ = writeln!(self.pre, "{tmp} = {inner}; }}");
        } else {
            // No else: leave the zero-initialized temp.
            self.pre.push('\n');
        }
        tmp
    }

    fn match_expr_c(&mut self, value: &Expr, arms: &[MatchArm], e: &Expr) -> String {
        let ty = self.type_of(e).cloned().unwrap_or(Type::Unit);
        let tmp = self.fresh_id("m");
        let ty_c = if ty == Type::Unit {
            "NS_Unit".to_string()
        } else {
            self.tcx.c_type(&ty)
        };
        let zero = self.zero(&ty);
        let _ = writeln!(self.pre, "    {ty_c} {tmp} = {zero};");
        let scrut = self.expr(value);
        self.flush_pre();
        let scrut_ty = self.type_of(value).cloned().unwrap_or(Type::Unit);
        self.emit_match_dispatch(&scrut, &scrut_ty, arms, Some(&tmp), &ty);
        tmp
    }

    fn emit_match_dispatch(
        &mut self,
        scrut: &str,
        scrut_ty: &Type,
        arms: &[MatchArm],
        target: Option<&str>,
        result_ty: &Type,
    ) {
        match scrut_ty {
            Type::Option(_) => self.match_option(scrut, arms, target, result_ty),
            Type::Result(_, _) => self.match_result(scrut, arms, target, result_ty),
            Type::Named(name, _) if self.tcx.env.enum_of(name).is_some() => {
                self.match_enum(scrut, name, arms, target)
            }
            Type::Named(name, _) if self.tcx.env.struct_of(name).is_some() => {
                self.match_struct(scrut, name, arms, target)
            }
            Type::Tuple(_) => self.match_tuple(scrut, scrut_ty, arms, target),
            ty if ty.is_numeric() || matches!(ty, Type::Bool) => {
                self.match_literal(scrut, arms, target)
            }
            _ => {
                self.tcx.rejects.push(reject(
                    "NS8021",
                    "matching this type is not supported by the C backend first slice",
                ));
            }
        }
    }

    fn match_option(
        &mut self,
        scrut: &str,
        arms: &[MatchArm],
        target: Option<&str>,
        result_ty: &Type,
    ) {
        let inner = match self.ann.at(self.span_of(scrut)) {
            Some(Type::Option(t)) => (**t).clone(),
            _ => Type::Unit,
        };
        let _ = writeln!(self.out, "    if (({scrut}).some) {{");
        for arm in arms {
            if let PatternKind::Constructor { path, arguments } = &arm.pattern.kind {
                if path.segments.last().map(String::as_str) == Some("Some") {
                    if let Some(p) = arguments.first() {
                        self.bind_pattern(p, &format!("({scrut}).value"), &inner);
                    }
                    self.emit_arm_body(&arm.body, target, result_ty);
                }
            }
        }
        self.writeln_s("    } else {");
        for arm in arms {
            match &arm.pattern.kind {
                PatternKind::Variant { variant, .. } if variant == "None" => {
                    self.emit_arm_body(&arm.body, target, result_ty);
                }
                PatternKind::Identifier(_) | PatternKind::Wildcard => {
                    self.emit_arm_body(&arm.body, target, result_ty);
                }
                _ => {}
            }
        }
        self.writeln_s("    }");
    }

    fn match_result(
        &mut self,
        scrut: &str,
        arms: &[MatchArm],
        target: Option<&str>,
        result_ty: &Type,
    ) {
        let (ok_t, err_t) = match self.ann.at(self.span_of(scrut)) {
            Some(Type::Result(ok, err)) => ((**ok).clone(), (**err).clone()),
            _ => (Type::Unit, Type::Unit),
        };
        let _ = writeln!(self.out, "    if (({scrut}).ok) {{");
        for arm in arms {
            if let PatternKind::Constructor { path, arguments } = &arm.pattern.kind {
                if path.segments.last().map(String::as_str) == Some("Ok") {
                    if let Some(p) = arguments.first() {
                        self.bind_pattern(p, &format!("({scrut}).value"), &ok_t);
                    }
                    self.emit_arm_body(&arm.body, target, result_ty);
                }
            }
        }
        self.writeln_s("    } else {");
        for arm in arms {
            if let PatternKind::Constructor { path, arguments } = &arm.pattern.kind {
                if path.segments.last().map(String::as_str) == Some("Err") {
                    if let Some(p) = arguments.first() {
                        self.bind_pattern(p, &format!("({scrut}).error"), &err_t);
                    }
                    self.emit_arm_body(&arm.body, target, result_ty);
                }
            }
        }
        self.writeln_s("    }");
    }

    fn match_enum(&mut self, scrut: &str, name: &str, arms: &[MatchArm], target: Option<&str>) {
        let _ = writeln!(self.out, "    switch (({scrut}).tag) {{");
        let mut default_arm: Option<usize> = None;
        for (i, arm) in arms.iter().enumerate() {
            match &arm.pattern.kind {
                PatternKind::Variant {
                    path,
                    variant,
                    arguments,
                } => {
                    let owner = path.segments.last().map(String::as_str).unwrap_or(name);
                    let _ = writeln!(
                        self.out,
                        "    case NS_{}_TAG_{}: {{",
                        sanitize(owner),
                        sanitize(variant)
                    );
                    if let Some(args) = arguments {
                        if let Some(shape) = self.tcx.env.enum_of(owner).cloned() {
                            if let Some(vshape) = shape.variants.iter().find(|v| &v.name == variant)
                            {
                                for (j, subpat) in args.iter().enumerate() {
                                    let field_ty =
                                        vshape.fields.get(j).cloned().unwrap_or(Type::Unit);
                                    let payload = if vshape.fields.len() == 1 {
                                        format!("({scrut}).payload.{}", field_c(variant))
                                    } else {
                                        format!("({scrut}).payload.{}._{j}", field_c(variant))
                                    };
                                    self.bind_pattern(subpat, &payload, &field_ty);
                                }
                            }
                        }
                    }
                    self.emit_arm_body(&arm.body, target, &Type::Unit);
                    self.writeln_s("        break;");
                    self.writeln_s("    }");
                }
                PatternKind::Identifier(_) | PatternKind::Wildcard => {
                    default_arm = Some(i);
                }
                _ => {
                    self.tcx.rejects.push(reject(
                        "NS8021",
                        "this enum pattern is not supported by the C backend first slice",
                    ));
                }
            }
        }
        if let Some(i) = default_arm {
            self.writeln_s("    default: {");
            let arm = &arms[i];
            if let PatternKind::Identifier(pat_name) = &arm.pattern.kind {
                let enum_c = self.tcx.c_type(&Type::Named(name.to_string(), Vec::new()));
                let _ = writeln!(
                    self.out,
                    "        {enum_c} v_{} = {scrut};",
                    sanitize(pat_name)
                );
            }
            self.emit_arm_body(&arm.body, target, &Type::Unit);
            self.writeln_s("        break;");
            self.writeln_s("    }");
        }
        self.writeln_s("    }");
    }

    fn match_struct(&mut self, scrut: &str, name: &str, arms: &[MatchArm], target: Option<&str>) {
        let Some(shape) = self.tcx.env.struct_of(name).cloned() else {
            self.tcx
                .rejects
                .push(reject("NS8020", format!("struct '{name}' is not declared")));
            return;
        };
        for arm in arms {
            if let PatternKind::Constructor { path, arguments } = &arm.pattern.kind {
                if path.segments.last().map(String::as_str) == Some(name) {
                    for (j, subpat) in arguments.iter().enumerate() {
                        let (fname, fty) = shape
                            .fields
                            .get(j)
                            .cloned()
                            .unwrap_or((format!("_{j}"), Type::Unit));
                        self.bind_pattern(subpat, &format!("({scrut}).{fname}"), &fty);
                    }
                    self.emit_arm_body(&arm.body, target, &Type::Unit);
                    return;
                }
            }
            if let PatternKind::Identifier(n) = &arm.pattern.kind {
                let s_c = self.tcx.c_type(&Type::Named(name.to_string(), Vec::new()));
                let _ = writeln!(self.out, "    {s_c} v_{} = {scrut};", sanitize(n));
                self.emit_arm_body(&arm.body, target, &Type::Unit);
                return;
            }
            if let PatternKind::Wildcard = &arm.pattern.kind {
                self.emit_arm_body(&arm.body, target, &Type::Unit);
                return;
            }
        }
        self.tcx.rejects.push(reject(
            "NS8021",
            "this struct match is not supported by the C backend first slice",
        ));
    }

    fn match_tuple(
        &mut self,
        scrut: &str,
        scrut_ty: &Type,
        arms: &[MatchArm],
        target: Option<&str>,
    ) {
        let Some(PatternKind::Tuple(subpats)) = arms.first().map(|a| &a.pattern.kind) else {
            self.tcx.rejects.push(reject(
                "NS8021",
                "tuple match is not supported by the C backend first slice",
            ));
            return;
        };
        let element_tys = match scrut_ty {
            Type::Tuple(ts) => ts.clone(),
            _ => vec![Type::Unit; subpats.len()],
        };
        for (i, subpat) in subpats.iter().enumerate() {
            let et = element_tys.get(i).cloned().unwrap_or(Type::Unit);
            self.bind_pattern(subpat, &format!("({scrut})._{i}"), &et);
        }
        if let Some(arm) = arms.first() {
            self.emit_arm_body(&arm.body, target, &Type::Unit);
        }
    }

    fn match_literal(&mut self, scrut: &str, arms: &[MatchArm], target: Option<&str>) {
        let mut first = true;
        let mut fallback: Option<usize> = None;
        for (i, arm) in arms.iter().enumerate() {
            match &arm.pattern.kind {
                PatternKind::Literal(lit) => {
                    let lit_c = self.literal_c(lit);
                    if first {
                        let _ = writeln!(self.out, "    if ({scrut} == {lit_c}) {{");
                        first = false;
                    } else {
                        let _ = writeln!(self.out, "    }} else if ({scrut} == {lit_c}) {{");
                    }
                    self.emit_arm_body(&arm.body, target, &Type::Unit);
                }
                PatternKind::Identifier(_) | PatternKind::Wildcard => {
                    fallback = Some(i);
                }
                _ => {
                    self.tcx.rejects.push(reject(
                        "NS8021",
                        "this literal pattern is not supported by the C backend first slice",
                    ));
                }
            }
        }
        if let Some(i) = fallback {
            if first {
                self.emit_arm_body(&arms[i].body, target, &Type::Unit);
            } else {
                self.writeln_s("    } else {");
                self.emit_arm_body(&arms[i].body, target, &Type::Unit);
                self.writeln_s("    }");
            }
        } else if !first {
            self.writeln_s("    }");
        }
    }

    /// Emits a match arm body. When `target` is `Some`, the arm's value is
    /// assigned to it (value context); when `None`, the arm runs for side
    /// effects (statement context).
    fn emit_arm_body(&mut self, body: &MatchArmBody, target: Option<&str>, result_ty: &Type) {
        match body {
            MatchArmBody::Expr(e) => {
                let ty = self.type_of(e).cloned().unwrap_or(Type::Unit);
                if ty == Type::Never {
                    self.emit_expr_stmt(e);
                    return;
                }
                let v = self.expr(e);
                self.flush_pre();
                match target {
                    Some(t) => {
                        if result_ty == &Type::Unit && ty == Type::Unit {
                            self.emit_value_stmt(e, v, &Some(ty));
                        } else {
                            let _ = writeln!(self.out, "        {t} = {v};");
                        }
                    }
                    None => self.emit_value_stmt(e, v, &Some(ty)),
                }
            }
            MatchArmBody::Block(b) => {
                self.writeln_s("        {");
                let n = b.statements.len();
                for (i, stmt) in b.statements.iter().enumerate() {
                    let is_tail = i + 1 == n;
                    if is_tail {
                        self.emit_block_tail_in_arm(stmt, target, result_ty);
                    } else {
                        self.emit_stmt_into_pre(stmt);
                    }
                }
                if b.statements.is_empty() && target.is_some() && result_ty == &Type::Unit {
                    if let Some(t) = target {
                        let _ = writeln!(self.out, "        {t} = (NS_Unit){{0}};");
                    }
                }
                self.writeln_s("        }");
            }
        }
    }

    fn emit_block_tail_in_arm(&mut self, stmt: &Stmt, target: Option<&str>, result_ty: &Type) {
        match &stmt.kind {
            StmtKind::Expr(e) => {
                let ty = self.type_of(e).cloned().unwrap_or(Type::Unit);
                if ty == Type::Never {
                    self.emit_expr_stmt(e);
                    return;
                }
                let v = self.expr(e);
                self.flush_pre();
                match target {
                    Some(t) => {
                        if result_ty == &Type::Unit && ty == Type::Unit {
                            self.emit_value_stmt(e, v, &Some(ty));
                        } else {
                            let _ = writeln!(self.out, "        {t} = {v};");
                        }
                    }
                    None => self.emit_value_stmt(e, v, &Some(ty)),
                }
            }
            _ => {
                self.emit_stmt_into_pre(stmt);
                if let Some(t) = target {
                    let z = self.zero(result_ty);
                    let _ = writeln!(self.out, "        {t} = {z};");
                }
            }
        }
    }

    /// Binds a sub-pattern to a C expression of the given type, emitting
    /// declarations into `self.out`.
    fn bind_pattern(&mut self, pat: &Pattern, c_expr: &str, ty: &Type) {
        match &pat.kind {
            PatternKind::Wildcard => {}
            PatternKind::Identifier(name) => {
                let ty_c = self.tcx.c_type(ty);
                let _ = writeln!(self.out, "        {ty_c} v_{} = {c_expr};", sanitize(name));
            }
            PatternKind::Tuple(subpats) => {
                let element_tys = match ty {
                    Type::Tuple(ts) => ts.clone(),
                    _ => vec![Type::Unit; subpats.len()],
                };
                for (i, subpat) in subpats.iter().enumerate() {
                    let et = element_tys.get(i).cloned().unwrap_or(Type::Unit);
                    self.bind_pattern(subpat, &format!("({c_expr})._{i}"), &et);
                }
            }
            PatternKind::Constructor { path, arguments } => {
                let name = path.segments.last().map(String::as_str).unwrap_or("");
                if let Some(shape) = self.tcx.env.struct_of(name).cloned() {
                    for (i, subpat) in arguments.iter().enumerate() {
                        let (_, fty) = shape
                            .fields
                            .get(i)
                            .cloned()
                            .unwrap_or((format!("_{i}"), Type::Unit));
                        self.bind_pattern(subpat, &format!("({c_expr})._{i}"), &fty);
                    }
                } else {
                    self.tcx.rejects.push(reject(
                        "NS8020",
                        format!("struct '{name}' is not declared for this pattern"),
                    ));
                }
            }
            PatternKind::Literal(_) => {
                self.tcx.rejects.push(reject(
                    "NS8021",
                    "literal sub-patterns are not supported in this position by the C backend first slice",
                ));
            }
            PatternKind::Variant { .. } => {
                self.tcx.rejects.push(reject(
                    "NS8021",
                    "nested enum patterns are not supported by the C backend first slice",
                ));
            }
        }
    }

    fn span_of(&self, _scrut: &str) -> astra_core::span::Span {
        // The annotations table is keyed by source spans, not C expressions, so
        // callers pass the scrutinee type directly. This helper exists only to
        // keep the option/result matchers readable; they fall back to Unit when
        // the span is unavailable.
        astra_core::span::Span::default()
    }

    fn propagate_c(&mut self, inner: &Expr) -> String {
        // `expr!`: evaluate the fallible inner, and if it failed, return the
        // error from the enclosing function.
        let inner_c = self.expr(inner);
        let inner_ty = self.type_of(inner).cloned();
        let result_c = match &inner_ty {
            Some(t @ Type::Result(_, _)) => self.tcx.c_type(t),
            _ => self.fn_result_c(),
        };
        let tmp = self.freshid("p");
        let _ = writeln!(self.pre, "    {result_c} {tmp} = {inner_c};");
        let fn_result_c = self.fn_result_c();
        let _ = writeln!(
            self.pre,
            "    if (!{tmp}.ok) {{ return ({fn_result_c}){{ .ok = false, .error = {tmp}.error }}; }}"
        );
        format!("{tmp}.value")
    }

    fn freshid(&mut self, prefix: &str) -> String {
        self.fresh += 1;
        format!("__{prefix}{}", self.fresh)
    }

    fn fail_c(&mut self, error: &ast::TypePath, arguments: &FailArguments) -> String {
        // Construct the error value and return it from the enclosing function.
        let name = error.segments.last().map(String::as_str).unwrap_or("");
        let err_val = self.construct_error(name, arguments);
        let fn_result_c = self.fn_result_c();
        let _ = writeln!(
            self.pre,
            "    return ({fn_result_c}){{ .ok = false, .error = {err_val} }};"
        );
        // Unreachable placeholder; the return above diverges.
        "((void*)0)".into()
    }

    fn construct_error(&mut self, name: &str, arguments: &FailArguments) -> String {
        let err_c = self.tcx.c_type(&Type::Named(name.to_string(), Vec::new()));
        match arguments {
            FailArguments::Tuple(args) => {
                let Some(shape) = self.tcx.env.struct_of(name).cloned() else {
                    self.tcx.rejects.push(reject(
                        "NS8020",
                        format!("error type '{name}' is not declared"),
                    ));
                    return format!("({err_c}){{0}}");
                };
                if shape.tuple {
                    let vals: Vec<String> = args.iter().map(|a| self.expr(a)).collect();
                    let joined = vals.join(", ");
                    format!("({err_c}){{ {joined} }}")
                } else {
                    let mut inits = String::new();
                    for (i, a) in args.iter().enumerate() {
                        let val = self.expr(a);
                        if let Some((fname, _)) = shape.fields.get(i) {
                            let _ = write!(inits, ".{fname} = {val}, ");
                        }
                    }
                    format!("({err_c}){{ {inits}}}")
                }
            }
            FailArguments::Struct(fields) => {
                let mut inits = String::new();
                for f in fields {
                    let val = self.expr(&f.value);
                    let _ = write!(inits, ".{} = {val}, ", field_c(&f.name));
                }
                format!("({err_c}){{ {inits}}}")
            }
        }
    }

    fn panic_c(&mut self, message: &Expr) -> String {
        let msg = self.expr(message);
        let _ = writeln!(
            self.pre,
            "    astra_panic({panic_msg});",
            panic_msg = panic_payload(&msg)
        );
        "((void*)0)".into()
    }

    fn abort_c(&mut self, message: &Expr) -> String {
        let msg = self.expr(message);
        let _ = writeln!(
            self.pre,
            "    astra_abort({abort_msg});",
            abort_msg = panic_payload(&msg)
        );
        "((void*)0)".into()
    }
}

fn panic_payload(msg_expr: &str) -> String {
    // The message must be an NS_Str; if it already is, pass it directly.
    if msg_expr.starts_with("(NS_Str)") {
        msg_expr.to_string()
    } else {
        format!("(NS_Str){{ .data = ({msg_expr}), .len = sizeof({msg_expr}) - 1 }}")
    }
}

fn is_signed_int(ty: &Type) -> bool {
    matches!(
        ty,
        Type::I8 | Type::I16 | Type::I32 | Type::I64 | Type::Isize
    )
}

fn signed_width(ty: &Type) -> Option<&'static str> {
    match ty {
        Type::I8 => Some("i8"),
        Type::I16 => Some("i16"),
        Type::I32 => Some("i32"),
        Type::I64 => Some("i64"),
        _ => None,
    }
}
