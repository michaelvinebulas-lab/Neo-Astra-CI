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

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;

use astra_core::ast::{
    self, BinaryOp, ElseBranch, Expr, ExprKind, FailArguments, Literal, MatchArm, MatchArmBody,
    Pattern, PatternKind, Stmt, StmtKind, UnaryOp,
};
use astra_core::hir::ImportKind;
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

    let mut scripts = Vec::new();
    collect_scripts(&module.ast.items, &mut scripts);
    for s in &scripts {
        if s.language != "c" && s.language != "cpp" {
            return Err(BackendError::new(
                "NS7002",
                format!(
                    "Script language '{}' is not supported by the active target 'c'.",
                    s.language
                ),
            ));
        }
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
        scripts: scripts.clone(),
        linked: false,
        prefix: String::new(),
        imports: HashMap::new(),
        namespaces: HashMap::new(),
        default_imports: HashSet::new(),
        dep_headers: Vec::new(),
        emit_main_entry: true,
        out: String::new(),
        pre: String::new(),
        fresh: 0,
        cur_ret: Type::Unit,
        cur_err: None,
        defers: Vec::new(),
    };
    e.emit_program();
    let script_sources = e.emit_scripts();
    Ok(GeneratedC {
        source: e.out,
        header: None,
        host_shim: crate::runtime::HOST_SHIM.to_string(),
        script_sources,
        target: "c".to_string(),
    })
}

/// Emits a multi-module project: one translation unit + public header per entry.
///
/// Functions are given external linkage (no `static`). Each module's translation
/// unit `#include`s the headers of its direct dependencies. The returned vector
/// is in the same order as the input `entries` slice.
pub fn emit_project(entries: &[crate::ProjectEntry]) -> Result<Vec<GeneratedC>, BackendError> {
    use std::collections::HashMap;
    // Build a map key -> header filename so each module can include its deps.
    let header_map: HashMap<String, String> = entries
        .iter()
        .map(|e| {
            let stem = module_key_stem(&e.key);
            (e.key.clone(), format!("{stem}.h"))
        })
        .collect();

    let mut results = Vec::new();
    let last_key = entries.last().map(|e| e.key.clone());
    // Declared type shapes per module key, used to resolve imported type names
    // to the dependency that declares them (cross-module struct/enum types).
    let envs_by_key: std::collections::BTreeMap<String, TypeEnv> = entries
        .iter()
        .map(|e| (e.key.clone(), TypeEnv::collect(&e.module.ast)))
        .collect();
    for entry in entries {
        if !entry.module.manual_memory_scopes.is_empty() {
            return Err(BackendError::new(
                "NS8012",
                format!(
                    "module '{}': @memory:manual is not supported by the C managed-subset backend",
                    entry.module.name
                ),
            ));
        }

        let mut scripts = Vec::new();
        collect_scripts(&entry.module.ast.items, &mut scripts);
        for s in &scripts {
            if s.language != "c" && s.language != "cpp" {
                return Err(BackendError::new(
                    "NS7002",
                    format!(
                        "module '{}': script language '{}' is not supported by target 'c'",
                        entry.module.name, s.language
                    ),
                ));
            }
        }

        let stem = module_key_stem(&entry.key);
        let env = TypeEnv::collect(&entry.module.ast);
        let mut tcx = TyCtxt::new(env);
        // Qualify nominal type names with the module stem so two modules that
        // each declare `struct Point` emit distinct typedefs and their headers
        // cannot collide when a dependent includes both.
        tcx.set_type_prefix(&stem);

        // Resolve this module's imports into call-site lookup tables. A named
        // or namespace import resolves to the dependency's qualified symbol;
        // a default import cannot be mapped to a C symbol and is rejected at
        // the call site.
        let (imports, namespaces, default_imports) = collect_import_tables(&entry.module.hir);
        // Register imported types before the prescan so signatures naming a
        // dependency's struct or enum lower to that module's qualified typedef
        // instead of being rejected with `NS8020`.
        install_foreign_types(&mut tcx, &imports, &namespaces, &envs_by_key);

        prescan(&mut tcx, &entry.module);
        if tcx.rejected() {
            let r = tcx.rejects.into_iter().next().expect("non-empty");
            return Err(BackendError::new(r.code, &r.message));
        }

        // Collect dep headers: for each key in dep_keys, look up the header filename.
        let dep_headers: Vec<String> = entry
            .dep_keys
            .iter()
            .filter_map(|k| header_map.get(k).cloned())
            .collect();

        let ann = Annotations::new(&entry.module.annotations);
        let mut e = Emitter {
            module: &entry.module,
            tcx: &mut tcx,
            ann,
            policy: entry.module.integer_policy,
            scripts: scripts.clone(),
            linked: true,
            prefix: format!("{stem}_"),
            imports,
            namespaces,
            default_imports,
            dep_headers: dep_headers.clone(),
            // Only the entry (last) module emits the `int main(void)` wrapper.
            emit_main_entry: last_key.as_deref() == Some(entry.key.as_str()),
            out: String::new(),
            pre: String::new(),
            fresh: 0,
            cur_ret: Type::Unit,
            cur_err: None,
            defers: Vec::new(),
        };
        e.emit_program_linked(&dep_headers);
        if e.tcx.rejected() {
            let r = e.tcx.rejects.clone().into_iter().next().expect("non-empty");
            return Err(BackendError::new(r.code, &r.message));
        }
        let script_sources = e.emit_scripts();
        let source = e.out.clone();
        let header = e.emit_header();

        results.push(GeneratedC {
            source,
            header: Some(header),
            host_shim: crate::runtime::HOST_SHIM.to_string(),
            script_sources,
            target: "c".to_string(),
        });
    }
    Ok(results)
}

/// Call-site lookup tables derived from a module's HIR imports.
///
/// - `named`: local binding name → (dep module key, imported name), so a call
///   to a named import resolves to the dependency's qualified symbol.
/// - `namespaces`: namespace alias → dep module key, for `alias.fn(...)` calls.
/// - `default_imports`: local names bound by a default import, which the C
///   backend cannot lower and rejects when referenced.
type ImportTables = (
    HashMap<String, (String, String)>,
    HashMap<String, String>,
    HashSet<String>,
);

/// Builds call-site lookup tables from a module's HIR imports.
fn collect_import_tables(hir: &astra_core::hir::Module) -> ImportTables {
    let mut named = HashMap::new();
    let mut namespaces = HashMap::new();
    let mut default_imports = HashSet::new();
    for import in &hir.imports {
        match &import.kind {
            ImportKind::Named { bindings } => {
                for binding in bindings {
                    named.insert(
                        binding.local.clone(),
                        (import.module.clone(), binding.imported.clone()),
                    );
                }
            }
            ImportKind::Namespace { alias } => {
                namespaces.insert(alias.clone(), import.module.clone());
            }
            ImportKind::Default { local } => {
                default_imports.insert(local.clone());
            }
            ImportKind::DefaultNamed { default, bindings } => {
                default_imports.insert(default.clone());
                for binding in bindings {
                    named.insert(
                        binding.local.clone(),
                        (import.module.clone(), binding.imported.clone()),
                    );
                }
            }
            ImportKind::SideEffect => {}
        }
    }
    (named, namespaces, default_imports)
}

/// Derives a safe C header stem from a module key.
/// `"app/math"` → `"app_math"`, `"lib"` → `"lib"`, `"src/main.ns"` → `"src_main"`.
///
/// A trailing `.ns` source extension is stripped before separators are
/// replaced so CLI file-path keys do not leak a `_ns` suffix into filenames
/// and symbol prefixes. This must stay in step with the filename stem
/// computed in `astra-compiler-api`'s `compile_project`.
fn module_key_stem(key: &str) -> String {
    key.strip_suffix(".ns")
        .unwrap_or(key)
        .replace(['/', '.', '-'], "_")
}

/// Registers every type declared by a dependency as the using module sees it —
/// imported by name, renamed, or namespace-dotted (`alias.Type`) — so
/// cross-module signatures, constructors, and patterns resolve to the
/// dependency's qualified typedef instead of failing with `NS8020`.
///
/// Named imports take precedence over bare names discovered by scanning
/// dependencies, and every phase iterates in sorted order so the result is
/// deterministic.
fn install_foreign_types(
    tcx: &mut TyCtxt,
    imports: &HashMap<String, (String, String)>,
    namespaces: &HashMap<String, String>,
    envs: &BTreeMap<String, TypeEnv>,
) {
    let mut table: BTreeMap<String, crate::ty::ForeignDecl> = BTreeMap::new();
    // 1. Named imports: the written name (and the dependency's own name when
    //    the import renames it) resolve to the declaring dependency.
    let mut named: Vec<_> = imports.iter().collect();
    named.sort();
    for (written, (dep_key, imported)) in named {
        let Some(env) = envs.get(dep_key) else {
            continue;
        };
        let Some(shape) = env.shape(imported).cloned() else {
            continue; // functions and constants have no nominal shape
        };
        let decl = crate::ty::ForeignDecl {
            stem: module_key_stem(dep_key),
            local: imported.clone(),
            shape,
        };
        table.insert(written.clone(), decl.clone());
        table.insert(imported.clone(), decl);
    }
    // 2. Namespace imports: `alias.Type` for every declared type, plus the
    //    bare name when no named import already claimed it.
    let mut namespace_list: Vec<_> = namespaces.iter().collect();
    namespace_list.sort();
    for (alias, dep_key) in namespace_list {
        let Some(env) = envs.get(dep_key) else {
            continue;
        };
        let stem = module_key_stem(dep_key);
        for (name, shape) in &env.decls {
            let decl = crate::ty::ForeignDecl {
                stem: stem.clone(),
                local: name.clone(),
                shape: shape.clone(),
            };
            table.insert(format!("{alias}.{name}"), decl.clone());
            table.entry(name.clone()).or_insert(decl);
        }
    }
    // 3. Bare names checker annotations can carry without an explicit type
    //    import (e.g. `let p = make_point()`): first declaring dependency in
    //    sorted key order wins. Local declarations always win in `nominal`.
    for (dep_key, env) in envs {
        let stem = module_key_stem(dep_key);
        for (name, shape) in &env.decls {
            table.entry(name.clone()).or_insert(crate::ty::ForeignDecl {
                stem: stem.clone(),
                local: name.clone(),
                shape: shape.clone(),
            });
        }
    }
    for (name, decl) in table {
        tcx.add_foreign(name, decl);
    }
}

/// Pre-scans declared and annotation types to register every `typedef` and
/// surface rejects before any code is written.
fn prescan(tcx: &mut TyCtxt, module: &CModule) {
    for item in &module.ast.items {
        match &item.kind {
            ast::ItemKind::Function(f) => {
                prescan_fn(tcx, f);
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
            ast::ItemKind::ScriptBlock(s) => {
                prescan_script(tcx, s);
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
        // Only prescan concrete stored types, not Function types (which appear
        // as annotations on call expressions and are never stored values).
        match &ann.ty {
            Type::Function(_) | Type::Unknown | Type::Error | Type::Never => {}
            _ => {
                let _ = tcx.c_type(&ann.ty);
            }
        }
    }
}

fn prescan_items(tcx: &mut TyCtxt, items: &[ast::Item]) {
    for item in items {
        match &item.kind {
            ast::ItemKind::Function(f) => prescan_fn(tcx, f),
            ast::ItemKind::Constant(c) => {
                let _ = tcx.c_type(&to_type(&c.ty));
            }
            ast::ItemKind::ScriptBlock(s) => prescan_script(tcx, s),
            ast::ItemKind::ScopedItems(s) => prescan_items(tcx, &s.items),
            _ => {}
        }
    }
}

fn prescan_script(tcx: &mut TyCtxt, s: &ast::ScriptBlock) {
    for exp in &s.exports {
        for param in &exp.parameters {
            let _ = tcx.c_type(&to_type(&param.ty));
        }
        if let Some(ret) = &exp.return_type {
            let _ = tcx.c_type(&to_type(ret));
        }
        for err in &exp.error_set {
            let _ = tcx.c_type(&path_to_type(err));
        }
    }
}

fn collect_scripts(items: &[ast::Item], scripts: &mut Vec<ast::ScriptBlock>) {
    for item in items {
        match &item.kind {
            ast::ItemKind::ScriptBlock(s) => scripts.push(s.clone()),
            ast::ItemKind::ScopedItems(s) => collect_scripts(&s.items, scripts),
            _ => {}
        }
    }
}

fn prescan_fn(tcx: &mut TyCtxt, f: &ast::FunctionDecl) {
    for param in &f.parameters {
        let _ = tcx.c_type(&to_type(&param.ty));
    }
    let ret = f.return_type.as_ref().map(to_type).unwrap_or(Type::Unit);
    let errs: Vec<Type> = f.error_set.iter().map(path_to_type).collect();
    // Register the bare return type first.
    if ret != Type::Unit {
        let _ = tcx.c_type(&ret);
    }
    // Register each error type.
    for err in &errs {
        let _ = tcx.c_type(err);
    }
    // Register the composite Result return type so it is declared before use.
    if let Some(err) = errs.into_iter().next() {
        let _ = tcx.c_type(&Type::Result(Box::new(ret), Box::new(err)));
    }
}

struct Emitter<'a> {
    module: &'a CModule,
    tcx: &'a mut TyCtxt,
    ann: Annotations,
    policy: IntegerPolicy,
    scripts: Vec<ast::ScriptBlock>,
    /// When `true`, functions are emitted with external linkage (no `static`)
    /// so they are visible across translation units.
    linked: bool,
    /// Prefix (with trailing `_`) prepended to every function symbol
    /// (`ns_<prefix><name>`). Empty in standalone builds so single-module
    /// output is unchanged; the module stem in linked builds, so two modules
    /// that each declare `helper` emit distinct symbols.
    prefix: String,
    /// Named-import bindings: local name → (dep module key, imported name).
    /// Used to resolve a cross-module call to the dependency's symbol.
    imports: HashMap<String, (String, String)>,
    /// Namespace-import aliases → dep module key, for `alias.fn(...)` calls.
    namespaces: HashMap<String, String>,
    /// Local names bound by a default import (`import foo from "lib"`), which
    /// the C backend cannot resolve to a symbol and rejects at the call site.
    default_imports: HashSet<String>,
    /// Header filenames of direct dependencies. The translation unit includes
    /// them for code, and the public header includes them so exported
    /// signatures that reference imported types (`NS_lib_Point`) resolve for
    /// consumers of the header alone.
    dep_headers: Vec<String>,
    /// When `false`, the module's `main` wrapper is suppressed (linked builds
    /// emit `int main(void)` only for the entry module).
    emit_main_entry: bool,
    out: String,
    pre: String,
    fresh: usize,
    cur_ret: Type,
    cur_err: Option<Type>,
    defers: Vec<Vec<ast::Stmt>>,
}

impl<'a> Emitter<'a> {
    /// The C symbol for a function declared in this module.
    fn fn_symbol(&self, name: &str) -> String {
        format!("ns_{}{}", self.prefix, sanitize(name))
    }
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
        if !self.scripts.is_empty() {
            self.out
                .push_str("/* Inline script foreign prototypes. */\n");
            for s in &self.scripts.clone() {
                let _ = writeln!(self.out, "/* Script: {} ({}) */", s.name, s.language);
                for exp in &s.exports {
                    let (ret_c, params_c) = self.script_export_signature(exp);
                    let _ = writeln!(self.out, "{ret_c} {}({params_c});", sanitize(&exp.name));
                }
            }
            self.out.push('\n');
        }
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

    /// Like `emit_program` but for multi-module (linked) builds:
    /// includes the dep headers, skips the import-reject guard (already lifted
    /// by `emit_project`), and uses external linkage for functions.
    fn emit_program_linked(&mut self, dep_headers: &[String]) {
        self.out.push_str(RUNTIME_HEADER);
        self.out.push('\n');
        let _ = writeln!(self.out, "/* Neo Astra module: {} */", self.module.name);
        self.out.push('\n');
        // Include headers for direct dependencies.
        for h in dep_headers {
            let _ = writeln!(self.out, "#include \"{h}\"");
        }
        if !dep_headers.is_empty() {
            self.out.push('\n');
        }
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
        // Forward declarations (extern linkage in linked mode).
        for f in &fns {
            self.emit_fn_proto(f);
        }
        self.out.push('\n');
        if !self.scripts.is_empty() {
            self.out
                .push_str("/* Inline script foreign prototypes. */\n");
            for s in &self.scripts.clone() {
                let _ = writeln!(self.out, "/* Script: {} ({}) */", s.name, s.language);
                for exp in &s.exports {
                    let (ret_c, params_c) = self.script_export_signature(exp);
                    let _ = writeln!(self.out, "{ret_c} {}({params_c});", sanitize(&exp.name));
                }
            }
            self.out.push('\n');
        }
        for c in &consts {
            self.emit_constant(c);
        }
        self.out.push('\n');
        for f in &fns {
            self.emit_function(f);
            self.out.push('\n');
        }
        if self.emit_main_entry
            && let Some(main) = self.find_main()
        {
            self.emit_main(&main);
        }
    }

    /// Emits a C header (`<module>.h`) containing extern prototypes for every
    /// exported function so that dependent modules can `#include` it.
    fn emit_header(&mut self) -> String {
        let mut h = String::new();
        let guard = format!(
            "NS_MOD_{}_H",
            self.module
                .name
                .replace(['.', '-', '/'], "_")
                .to_uppercase()
        );
        let _ = writeln!(
            h,
            "/* Auto-generated by Neo Astra C backend — do not edit. */"
        );
        let _ = writeln!(h, "#ifndef {guard}");
        let _ = writeln!(h, "#define {guard}");
        h.push('\n');
        h.push_str(RUNTIME_HEADER);
        h.push('\n');
        // Dependency headers: exported signatures may reference imported types
        // (`NS_lib_Point`), so consumers of this header alone still see them.
        for dep in &self.dep_headers {
            let _ = writeln!(h, "#include \"{dep}\"");
        }
        if !self.dep_headers.is_empty() {
            h.push('\n');
        }
        // Typedefs used in exported function signatures.
        for decl in &self.tcx.typedefs.clone() {
            h.push_str(decl);
            h.push('\n');
        }
        h.push('\n');
        // Extern prototypes for exported functions.
        let (fns, _) = self.collect_decls();
        let exported_fns: Vec<_> = fns
            .iter()
            .filter(|f| {
                // A function is exported when `pub`/`export` is used on it.
                self.module
                    .hir
                    .items
                    .iter()
                    .any(|item| item.name == f.name && item.exported)
            })
            .collect();
        if !exported_fns.is_empty() {
            h.push_str("/* Exported function prototypes. */\n");
            for f in exported_fns {
                if f.type_parameters.is_empty() {
                    let (ret_c, _) = self.fn_return_c(f);
                    let params = self.fn_params_c(f);
                    let _ = writeln!(h, "{ret_c} {}({params});", self.fn_symbol(&f.name));
                }
            }
        }
        h.push('\n');
        let _ = writeln!(h, "#endif /* {guard} */");
        h
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
            if let ast::ItemKind::Function(f) = &item.kind
                && f.name == "main"
                && f.parameters.is_empty()
                && f.error_set.is_empty()
            {
                let ret = f.return_type.as_ref().map(to_type).unwrap_or(Type::Unit);
                if ret == Type::Unit || ret == Type::I32 {
                    return Some(f.clone());
                }
            }
            if let ast::ItemKind::ScopedItems(s) = &item.kind {
                for inner in &s.items {
                    if let ast::ItemKind::Function(f) = &inner.kind
                        && f.name == "main"
                        && f.parameters.is_empty()
                        && f.error_set.is_empty()
                    {
                        let ret = f.return_type.as_ref().map(to_type).unwrap_or(Type::Unit);
                        if ret == Type::Unit || ret == Type::I32 {
                            return Some(f.clone());
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
        let sym = self.fn_symbol(&f.name);
        if self.linked {
            // External linkage: forward declaration without `static`.
            let _ = writeln!(self.out, "{ret_c} {sym}({params});");
        } else {
            let _ = writeln!(self.out, "static {ret_c} {sym}({params});");
        }
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

    fn script_export_signature(&mut self, exp: &ast::ScriptExport) -> (String, String) {
        let ret = exp.return_type.as_ref().map(to_type).unwrap_or(Type::Unit);
        let errs: Vec<Type> = exp.error_set.iter().map(path_to_type).collect();
        if errs.len() > 1 {
            self.tcx.rejects.push(reject(
                "NS8019",
                format!(
                    "script export '{}' has multiple error types, unsupported by the C backend",
                    exp.name
                ),
            ));
        }
        let ret_c = if let Some(e) = errs.into_iter().next() {
            let ty = Type::Result(Box::new(ret), Box::new(e));
            self.tcx.c_type(&ty)
        } else if ret == Type::Unit {
            "void".to_string()
        } else {
            self.tcx.c_type(&ret)
        };
        let params_c = if exp.parameters.is_empty() {
            "void".to_string()
        } else {
            exp.parameters
                .iter()
                .map(|p| {
                    let t = self.tcx.c_type(&to_type(&p.ty));
                    format!("{t} {}", sanitize(&p.name))
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        (ret_c, params_c)
    }

    fn emit_scripts(&mut self) -> Vec<(String, String)> {
        let mut script_sources = Vec::new();
        for s in &self.scripts.clone() {
            let ext = if s.language == "cpp" { "cpp" } else { "c" };
            let filename = format!("script_{}.{ext}", sanitize(&s.name));
            let mut content = String::new();
            let _ = writeln!(
                content,
                "/* Neo Astra inline script: {} ({}) */",
                s.name, s.language
            );
            content.push_str("#include <stdint.h>\n#include <stdbool.h>\n#include <stddef.h>\n\n");
            if s.language == "cpp" {
                content.push_str("extern \"C\" {\n");
                for exp in &s.exports {
                    let (ret_c, params_c) = self.script_export_signature(exp);
                    let _ = writeln!(content, "{ret_c} {}({params_c});", sanitize(&exp.name));
                }
                content.push_str("}\n\n");
            }
            content.push_str(&s.raw_source);
            if !content.ends_with('\n') {
                content.push('\n');
            }
            script_sources.push((filename, content));
        }
        script_sources
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
        let sym = self.fn_symbol(&f.name);
        if self.linked {
            let _ = writeln!(self.out, "{ret_c} {sym}({params}) {{");
        } else {
            let _ = writeln!(self.out, "static {ret_c} {sym}({params}) {{");
        }
        self.emit_block_body(&f.body, true, is_fallible);
        self.out.push_str("}\n");
    }

    fn emit_main(&mut self, main: &ast::FunctionDecl) {
        let ret = main.return_type.as_ref().map(to_type).unwrap_or(Type::Unit);
        let sym = self.fn_symbol("main");
        self.out.push_str("int main(void) {\n");
        if ret == Type::I32 {
            let _ = writeln!(self.out, "    return (int){sym}();");
        } else {
            let _ = writeln!(self.out, "    {sym}();");
            self.out.push_str("    return 0;\n");
        }
        self.out.push_str("}\n");
    }

    /// Emits a block's statements. When `is_fn_body` is true, the last statement
    /// is treated as the function's tail expression (returned, possibly wrapped
    /// in `Ok` for fallible functions). When `is_fn_body` is false, the block is
    /// a nested scope and its value is discarded.
    fn emit_block_body(&mut self, block: &ast::Block, is_fn_body: bool, is_fallible: bool) {
        self.defers.push(Vec::new());
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
        } else if !is_fn_body {
            self.emit_scope_defers();
        }
        self.defers.pop();
    }

    fn emit_implicit_tail_return(&mut self, is_fallible: bool) {
        self.emit_all_defers();
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
        // If every reachable branch of this expression ends with an explicit
        // `return`, emit it as a statement (which routes to `emit_stmt`/`emit_expr_stmt`)
        // rather than through the temp-variable path. This avoids the output
        // routing confusion when `if_expr_c` writes to `self.pre` while
        // `emit_stmt_into_pre` also bounces through `self.out`.
        if self.all_branches_return(e) {
            self.emit_expr_stmt(e);
            return;
        }
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
            self.emit_all_defers();
            let _ = writeln!(
                self.out,
                "    return ({ret_c}){{ .ok = true, .value = {v} }};"
            );
        } else if self.cur_ret == Type::Unit {
            self.emit_value_stmt(e, v, &ty);
            self.emit_all_defers();
        } else {
            self.emit_all_defers();
            let _ = writeln!(self.out, "    return {v};");
        }
    }

    fn emit_all_defers(&mut self) {
        let all: Vec<ast::Stmt> = self
            .defers
            .iter()
            .rev()
            .flat_map(|scope| scope.iter().rev().cloned())
            .collect();
        for d in &all {
            self.emit_stmt(d);
        }
    }

    fn emit_scope_defers(&mut self) {
        if let Some(scope) = self.defers.last() {
            let defers: Vec<ast::Stmt> = scope.iter().rev().cloned().collect();
            for d in &defers {
                self.emit_stmt(d);
            }
        }
    }

    fn emit_scope_defers_into_pre(&mut self) {
        if let Some(scope) = self.defers.last() {
            let defers: Vec<ast::Stmt> = scope.iter().rev().cloned().collect();
            for d in &defers {
                self.emit_stmt_into_pre(d);
            }
        }
    }

    /// Returns true when every reachable branch of an expression ends with an
    /// explicit `return` statement (so no temp assignment is needed).
    fn all_branches_return(&self, e: &Expr) -> bool {
        match &e.kind {
            ExprKind::If {
                then_branch,
                else_branch,
                ..
            } => {
                let then_ret = self.block_diverges(then_branch);
                let else_ret = match else_branch {
                    Some(ElseBranch::Block(b)) => self.block_diverges(b),
                    Some(ElseBranch::If(inner)) => self.all_branches_return(inner),
                    None => false,
                };
                then_ret && else_ret
            }
            ExprKind::Block(b) => self.block_diverges(b),
            _ => false,
        }
    }

    /// Returns true if the block always diverges (its last statement is a
    /// `return`, `fail`, `panic`, or `abort`).
    fn block_diverges(&self, block: &ast::Block) -> bool {
        match block.statements.last().map(|s| &s.kind) {
            Some(StmtKind::Return { .. }) => true,
            Some(StmtKind::Expr(e)) => matches!(
                e.kind,
                ExprKind::Fail { .. } | ExprKind::Panic { .. } | ExprKind::Abort { .. }
            ),
            _ => false,
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
            StmtKind::Defer(s) => {
                if let Some(scope) = self.defers.last_mut() {
                    scope.push((**s).clone());
                }
            }
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
        self.defers.push(Vec::new());
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
        self.emit_scope_defers();
        self.defers.pop();
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
                self.emit_all_defers();
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
                self.emit_all_defers();
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
        enum IterKind {
            Array,
            Map,
            Bytes,
        }
        let (elem_ty, kind) = match &iter_ty {
            Type::Array(t) | Type::Slice(t) => ((**t).clone(), IterKind::Array),
            Type::Map(k, _) => ((**k).clone(), IterKind::Map),
            Type::Bytes => (Type::U8, IterKind::Bytes),
            _ => {
                self.tcx.rejects.push(reject(
                    "NS8021",
                    "for-loop iteration is only supported over arrays, maps, and bytes in the C backend first slice",
                ));
                return;
            }
        };
        self.pre.clear();
        let iter = self.expr(iterable);
        self.flush_pre();
        let iter_var = self.fresh_id("it");
        let iter_c_ty = self.tcx.c_type(&iter_ty);
        let _ = writeln!(self.out, "    {iter_c_ty} {iter_var} = {iter};");
        let elem_c = self.tcx.c_type(&elem_ty);
        let idx = self.fresh_id("i");
        let _ = writeln!(
            self.out,
            "    for (size_t {idx} = 0; {idx} < ({iter_var}).len; ++{idx}) {{"
        );
        match kind {
            IterKind::Map => {
                let _ = writeln!(
                    self.out,
                    "        {elem_c} v_{} = *({elem_c}*)astra_map_key_at(&({iter_var}), {idx});",
                    sanitize(binding)
                );
            }
            IterKind::Array => {
                let _ = writeln!(
                    self.out,
                    "        {elem_c} v_{} = *({elem_c}*)astra_array_index(&({iter_var}), {idx}, sizeof({elem_c}));",
                    sanitize(binding)
                );
            }
            IterKind::Bytes => {
                let _ = writeln!(
                    self.out,
                    "        {elem_c} v_{} = astra_bytes_get({iter_var}, {idx});",
                    sanitize(binding)
                );
            }
        }
        self.emit_block_body(body, false, false);
        self.writeln_s("    }");
    }

    fn emit_expr_stmt(&mut self, e: &Expr) {
        if let ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } = &e.kind
        {
            self.emit_if_stmt(condition, then_branch, else_branch.as_ref());
            return;
        }
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

    fn emit_if_stmt(
        &mut self,
        condition: &Expr,
        then_branch: &ast::Block,
        else_branch: Option<&ElseBranch>,
    ) {
        self.pre.clear();
        let cond = self.expr(condition);
        self.flush_pre();
        let _ = writeln!(self.out, "    if ({cond}) {{");
        for stmt in &then_branch.statements {
            self.emit_stmt(stmt);
        }
        self.emit_else_branch(else_branch);
    }

    fn emit_else_branch(&mut self, else_branch: Option<&ElseBranch>) {
        match else_branch {
            Some(ElseBranch::Block(b)) => {
                self.writeln_s("    } else {");
                for stmt in &b.statements {
                    self.emit_stmt(stmt);
                }
                self.writeln_s("    }");
            }
            Some(ElseBranch::If(inner)) => {
                if let ExprKind::If {
                    condition: inner_cond,
                    then_branch: inner_then,
                    else_branch: inner_else,
                } = &inner.kind
                {
                    self.pre.clear();
                    let icond = self.expr(inner_cond);
                    self.flush_pre();
                    let _ = writeln!(self.out, "    }} else if ({icond}) {{");
                    for stmt in &inner_then.statements {
                        self.emit_stmt(stmt);
                    }
                    self.emit_else_branch(inner_else.as_ref());
                } else {
                    self.writeln_s("    } else {");
                    self.emit_expr_stmt(inner);
                    self.writeln_s("    }");
                }
            }
            None => self.writeln_s("    }"),
        }
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
            Type::Bytes => "(NS_Bytes){0}".into(),
            Type::Array(_) | Type::Slice(_) => "(NS_Array){0}".into(),
            Type::Map(_, _) => "(NS_Map){0}".into(),
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
            ExprKind::Map(entries) => self.map_literal_c(entries, e),
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

                cleaned
                    .strip_suffix("u8")
                    .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
                    .unwrap_or(&cleaned)
                    .to_string()
            }
            Literal::Float(raw) => raw.chars().filter(|c| *c != '_').collect::<String>(),
            Literal::String(value) => self.c_string_literal(value),
            Literal::Bytes(bytes) => self.c_bytes_literal(bytes),
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
        // A bare identifier that resolves to an import is a cross-module value
        // reference (imported constant, struct, or enum). The C backend links
        // cross-module *calls* only, so reject it here instead of emitting a
        // symbol that would not resolve at link time.
        if self.imports.contains_key(name) || self.default_imports.contains(name) {
            self.tcx.rejects.push(reject(
                "NS8019",
                format!(
                    "cross-module reference to value '{name}' is not supported by the C backend; only cross-module function calls link"
                ),
            ));
            return "0 /* unresolved cross-module value */".to_string();
        }
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
                let ty = self.type_of(left).cloned();
                if matches!(ty, Some(Type::Str)) {
                    if op == BinaryOp::Equal {
                        format!("astra_str_eq({l}, {r})")
                    } else {
                        format!("(!astra_str_eq({l}, {r}))")
                    }
                } else if matches!(ty, Some(Type::Bytes)) {
                    if op == BinaryOp::Equal {
                        format!("astra_bytes_eq({l}, {r})")
                    } else {
                        format!("(!astra_bytes_eq({l}, {r}))")
                    }
                } else if self.eq_supported(left) {
                    let sym = if op == BinaryOp::Equal { "==" } else { "!=" };
                    format!("({l} {sym} {r})")
                } else {
                    self.tcx.rejects.push(reject(
                        "NS8021",
                        "equality is only supported for primitive, str, and bytes values in the C backend first slice",
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
                let ty = self.type_of(left).cloned();
                if matches!(ty, Some(Type::Str)) {
                    format!("(astra_str_cmp({l}, {r}) {sym} 0)")
                } else if matches!(ty, Some(Type::Bytes)) {
                    format!("(astra_bytes_cmp({l}, {r}) {sym} 0)")
                } else {
                    format!("({l} {sym} {r})")
                }
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
        let numeric = matches!((&lt, &rt), (Some(a), Some(b)) if a.is_numeric() && b.is_numeric());
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
        if (is_wrapping_op || policy_wrapping)
            && let Some(width) = signed_width(&ty)
        {
            let helper = match op {
                BinaryOp::Add | BinaryOp::AddWrap => format!("astra_add_{width}"),
                BinaryOp::Subtract | BinaryOp::SubtractWrap => format!("astra_sub_{width}"),
                BinaryOp::Multiply | BinaryOp::MultiplyWrap => format!("astra_mul_{width}"),
                _ => unreachable!(),
            };
            return format!("{helper}({l}, {r})");
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
                // Tuple struct constructor call: `Pair(1, 2)`.
                if let Some(shape) = self.tcx.struct_shape(name)
                    && shape.tuple
                {
                    let ty_c = self.tcx.c_type(&Type::Named(name.to_string(), Vec::new()));
                    let vals: Vec<String> = args.iter().map(|a| self.expr(a)).collect();
                    let joined = vals.join(", ");
                    return format!("({ty_c}){{ {joined} }}");
                }
                // Call through a named import: resolve to the dependency's
                // qualified symbol so the linker sees one external definition.
                if let Some((dep_key, imported)) = self.imports.get(name).cloned() {
                    let arg_c = self.args_c(args);
                    let dep_stem = module_key_stem(&dep_key);
                    return format!("ns_{dep_stem}_{}({arg_c})", sanitize(&imported));
                }
                // A default import has no C symbol to call into this slice.
                if self.default_imports.contains(name) {
                    self.tcx.rejects.push(reject(
                        "NS8019",
                        format!(
                            "call to default import '{name}' is not supported by the C backend; import the symbol by name"
                        ),
                    ));
                    return "(NS_Unit){0}".into();
                }
                // Local user function call.
                let arg_c = self.args_c(args);
                format!("{}({arg_c})", self.fn_symbol(name))
            }
            ExprKind::Member { target, member } => {
                if let ExprKind::Identifier(owner) = &target.kind {
                    match (owner.as_str(), member.as_str()) {
                        ("Option", "Some") => return self.some_c(args, e),
                        ("Result", "Ok") => return self.ok_c(args, e),
                        ("Result", "Err") => return self.err_c(args, e),
                        _ => {
                            // Namespace-import call: `alias.fn(...)`.
                            if let Some(dep_key) = self.namespaces.get(owner).cloned() {
                                let arg_c = self.args_c(args);
                                let dep_stem = module_key_stem(&dep_key);
                                return format!("ns_{dep_stem}_{}({arg_c})", sanitize(member));
                            }
                            // Enum variant constructor with payload.
                            if self.tcx.enum_shape(owner).is_some() {
                                return self.enum_variant_c(owner, member, args);
                            }
                            // Inline script export call: `<script>.<func>(...)`.
                            if self.scripts.iter().any(|s| s.name == *owner) {
                                let arg_c = self.args_c(args);
                                return format!("{}({arg_c})", sanitize(member));
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
        // The tag constant is declared with the owning module's prefix — the
        // local stem for local enums, the dependency's stem for imported ones.
        let Some(tag) = self.tcx.enum_tag(owner, variant) else {
            self.tcx
                .rejects
                .push(reject("NS8020", format!("enum '{owner}' is not declared")));
            return format!("({enum_c}){{0}}");
        };
        let fields: Vec<Type> = match self.tcx.enum_shape(owner) {
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
            (Some(Type::Str), "slice") => {
                let r = self.expr(receiver);
                let start = args
                    .first()
                    .map(|a| self.expr(a))
                    .unwrap_or_else(|| "0".into());
                let len = args
                    .get(1)
                    .map(|a| self.expr(a))
                    .unwrap_or_else(|| "0".into());
                format!("astra_str_slice({r}, {start}, {len})")
            }
            (Some(Type::Bytes), "len") => {
                let r = self.expr(receiver);
                format!("({r}).len")
            }
            (Some(Type::Bytes), "is_empty") => {
                let r = self.expr(receiver);
                format!("(({r}).len == 0)")
            }
            (Some(Type::Bytes), "slice") => {
                let r = self.expr(receiver);
                let start = args
                    .first()
                    .map(|a| self.expr(a))
                    .unwrap_or_else(|| "0".into());
                let len = args
                    .get(1)
                    .map(|a| self.expr(a))
                    .unwrap_or_else(|| "0".into());
                format!("astra_bytes_slice({r}, {start}, {len})")
            }
            (Some(Type::Map(key_ty, _)), "len") => {
                let _ = key_ty; // only need receiver
                let r = self.expr(receiver);
                format!("astra_map_len(&({r}))")
            }
            (Some(Type::Map(key_ty, val_ty)), "contains") => {
                let kt = (**key_ty).clone();
                let key_c = self.tcx.c_type(&kt);
                let r = self.expr(receiver);
                let k = args
                    .first()
                    .map(|a| self.expr(a))
                    .unwrap_or_else(|| "0".into());
                let _ = val_ty;
                // Hoist key into a temp to avoid &(T){(T){...}} double-wrap
                let ktmp = self.freshid("mk");
                let _ = writeln!(self.pre, "    {key_c} {ktmp} = {k};");
                format!("astra_map_contains(&({r}), &{ktmp})")
            }
            (Some(Type::Map(key_ty, val_ty)), "get") => {
                let kt = (**key_ty).clone();
                let vt = (**val_ty).clone();
                let key_c = self.tcx.c_type(&kt);
                let val_c = self.tcx.c_type(&vt);
                let r = self.expr(receiver);
                let k = args
                    .first()
                    .map(|a| self.expr(a))
                    .unwrap_or_else(|| "0".into());
                // Hoist key into a temp to avoid &(T){(T){...}} double-wrap
                let ktmp = self.freshid("mk");
                let _ = writeln!(self.pre, "    {key_c} {ktmp} = {k};");
                // Returns Option<V>: wrap the raw pointer result in NS_Option_V
                let opt_c = self.tcx.c_type(&Type::Option(Box::new(vt.clone())));
                let tmp = self.freshid("mg");
                let _ = writeln!(
                    self.pre,
                    "    void* {tmp}_ptr = astra_map_get(&({r}), &{ktmp});"
                );
                let _ = writeln!(
                    self.pre,
                    "    {opt_c} {tmp} = {tmp}_ptr ? ({opt_c}){{ .some = true, .value = *({val_c}*){tmp}_ptr }} : ({opt_c}){{ .some = false }};"
                );
                tmp
            }
            (Some(Type::Map(key_ty, val_ty)), "insert") => {
                let kt = (**key_ty).clone();
                let vt = (**val_ty).clone();
                let key_c = self.tcx.c_type(&kt);
                let val_c = self.tcx.c_type(&vt);
                let r = self.expr(receiver);
                let k = args
                    .first()
                    .map(|a| self.expr(a))
                    .unwrap_or_else(|| "0".into());
                let v = args
                    .get(1)
                    .map(|a| self.expr(a))
                    .unwrap_or_else(|| "0".into());
                let mtmp = self.freshid("mr");
                let _ = writeln!(self.pre, "    NS_Map* {mtmp} = &({r});");
                let ktmp = self.freshid("mk");
                let vtmp = self.freshid("mv");
                let _ = writeln!(self.pre, "    {key_c} {ktmp} = {k};");
                let _ = writeln!(self.pre, "    {val_c} {vtmp} = {v};");
                let opt_c = self.tcx.c_type(&Type::Option(Box::new(vt.clone())));
                let tmp = self.freshid("mi");
                let _ = writeln!(
                    self.pre,
                    "    void* {tmp}_prev = astra_map_get({mtmp}, &{ktmp});"
                );
                let _ = writeln!(
                    self.pre,
                    "    {opt_c} {tmp} = {tmp}_prev ? ({opt_c}){{ .some = true, .value = *({val_c}*){tmp}_prev }} : ({opt_c}){{ .some = false }};"
                );
                let _ = writeln!(self.pre, "    astra_map_set({mtmp}, &{ktmp}, &{vtmp});");
                tmp
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
            Type::Bytes => "bytes",
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
        let tt = self.type_of(target).cloned();
        let t = self.expr(target);
        let i = self.expr(index);
        if matches!(tt, Some(Type::Bytes)) {
            return format!("astra_bytes_get({t}, {i})");
        }
        let elem_ty = self.elem_type_of(target);
        let elem_c = self.tcx.c_type(&elem_ty);
        format!("(*({elem_c}*)astra_array_index(&({t}), {i}, sizeof({elem_c})))")
    }

    fn member_c(&mut self, target: &Expr, member: &str, e: &Expr) -> String {
        // Unit enum variant used as a value: `Color::Red`.
        if let ExprKind::Identifier(owner) = &target.kind
            && let Some(shape) = self.tcx.enum_shape(owner)
            && shape.variants.iter().any(|v| v.name == member)
        {
            return self.enum_variant_c(owner, member, &[]);
        }
        let tt = self.type_of(target).cloned();
        let t = self.expr(target);
        match &tt {
            Some(Type::Named(name, _)) if self.tcx.struct_shape(name).is_some() => {
                let shape = self.tcx.struct_shape(name);
                if let Some(s) = &shape
                    && s.fields.iter().any(|(f, _)| f == member)
                {
                    return format!("({t}).{m}", m = field_c(member));
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

    /// Emits a map literal `{ k: v, ... }` into `self.pre` and returns the
    /// local variable name holding the `NS_Map`.
    fn map_literal_c(&mut self, entries: &[ast::MapEntry], e: &Expr) -> String {
        let ty = self
            .type_of(e)
            .cloned()
            .unwrap_or(Type::Map(Box::new(Type::Str), Box::new(Type::I32)));
        let (key_ty, val_ty) = match &ty {
            Type::Map(k, v) => ((**k).clone(), (**v).clone()),
            _ => (Type::Str, Type::I32),
        };
        let key_c = self.tcx.c_type(&key_ty);
        let val_c = self.tcx.c_type(&val_ty);
        let key_is_str = key_ty == Type::Str;
        let tmp = self.freshid("map");
        let _ = writeln!(
            self.pre,
            "    NS_Map {tmp} = astra_map_new(sizeof({key_c}), sizeof({val_c}), {key_is_str});"
        );
        for entry in entries {
            let k = self.expr(&entry.key);
            let v = self.expr(&entry.value);
            let ktmp = self.freshid("mk");
            let vtmp = self.freshid("mv");
            let _ = writeln!(self.pre, "    {key_c} {ktmp} = {k};");
            let _ = writeln!(self.pre, "    {val_c} {vtmp} = {v};");
            let _ = writeln!(self.pre, "    astra_map_set(&{tmp}, &{ktmp}, &{vtmp});");
        }
        tmp
    }

    /// Emits a bytes literal `b"..."` as a static `uint8_t` array and wraps it
    /// in an `NS_Bytes` compound literal.
    fn c_bytes_literal(&mut self, bytes: &[u8]) -> String {
        let len = bytes.len();
        if len == 0 {
            return "(NS_Bytes){ .data = (const uint8_t*)\"\", .len = 0 }".into();
        }
        let hex: String = bytes.iter().map(|b| format!("0x{b:02X},")).collect();
        format!("(NS_Bytes){{ .data = (const uint8_t[]){{ {hex} }}, .len = {len} }}")
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
        // The full path: a namespace-qualified literal (`lib.Point { .. }`)
        // resolves through the foreign type table, a bare path locally.
        let name = path.segments.join(".");
        let ty_c = self.tcx.c_type(&Type::Named(name.clone(), Vec::new()));
        let Some(shape) = self.tcx.struct_shape(&name) else {
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
        self.defers.push(Vec::new());
        let n = block.statements.len();
        for (i, stmt) in block.statements.iter().enumerate() {
            let is_tail = i + 1 == n;
            if is_tail {
                // If the tail is a return, emit it directly rather than
                // assigning to the temp (which would generate dead code).
                if matches!(&stmt.kind, StmtKind::Return { .. }) {
                    self.emit_stmt_into_pre(stmt);
                } else {
                    self.emit_block_tail_into_pre(stmt, target, block_ty);
                }
            } else {
                self.emit_stmt_into_pre(stmt);
            }
        }
        if block.statements.is_empty() && block_ty == &Type::Unit {
            let _ = writeln!(self.pre, "        {target} = (NS_Unit){{0}};");
        }
        self.emit_scope_defers_into_pre();
        self.defers.pop();
    }

    fn emit_stmt_into_pre(&mut self, stmt: &Stmt) {
        // Emit a statement into `self.pre` instead of `self.out`.
        let saved_out = std::mem::take(&mut self.out);
        let saved_pre = std::mem::take(&mut self.pre);
        self.emit_stmt(stmt);
        let mut result = saved_pre;
        result.push_str(&self.out);
        result.push_str(&self.pre);
        self.pre = result;
        self.out = saved_out;
    }

    fn emit_block_tail_into_pre(&mut self, stmt: &Stmt, target: &str, block_ty: &Type) {
        let saved_out = std::mem::take(&mut self.out);
        let saved_pre = std::mem::take(&mut self.pre);
        self.emit_block_tail(stmt, target, block_ty);
        let mut result = saved_pre;
        result.push_str(&self.out);
        result.push_str(&self.pre);
        self.pre = result;
        self.out = saved_out;
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
        let scrut_ty = self.type_of(value).cloned().unwrap_or(Type::Unit);
        let scrut = self.expr(value);
        self.flush_pre();
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
            Type::Option(_) => self.match_option(scrut, scrut_ty, arms, target, result_ty),
            Type::Result(_, _) => self.match_result(scrut, scrut_ty, arms, target, result_ty),
            Type::Named(name, _) if self.tcx.enum_shape(name).is_some() => {
                self.match_enum(scrut, name, arms, target)
            }
            Type::Named(name, _) if self.tcx.struct_shape(name).is_some() => {
                self.match_struct(scrut, name, arms, target)
            }
            Type::Tuple(_) => self.match_tuple(scrut, scrut_ty, arms, target),
            ty if ty.is_numeric() || matches!(ty, Type::Bool | Type::Str | Type::Bytes) => {
                self.match_literal(scrut, scrut_ty, arms, target)
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
        scrut_ty: &Type,
        arms: &[MatchArm],
        target: Option<&str>,
        result_ty: &Type,
    ) {
        let inner = match scrut_ty {
            Type::Option(t) => (**t).clone(),
            _ => Type::Unit,
        };
        let _ = writeln!(self.out, "    if (({scrut}).some) {{");
        for arm in arms {
            if let PatternKind::Constructor { path, arguments } = &arm.pattern.kind
                && path.segments.last().map(String::as_str) == Some("Some")
            {
                if let Some(p) = arguments.first() {
                    self.bind_pattern(p, &format!("({scrut}).value"), &inner);
                }
                self.emit_arm_body(&arm.body, target, result_ty);
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
        scrut_ty: &Type,
        arms: &[MatchArm],
        target: Option<&str>,
        result_ty: &Type,
    ) {
        let (ok_t, err_t) = match scrut_ty {
            Type::Result(ok, err) => ((**ok).clone(), (**err).clone()),
            _ => (Type::Unit, Type::Unit),
        };
        let _ = writeln!(self.out, "    if (({scrut}).ok) {{");
        for arm in arms {
            if let PatternKind::Constructor { path, arguments } = &arm.pattern.kind
                && path.segments.last().map(String::as_str) == Some("Ok")
            {
                if let Some(p) = arguments.first() {
                    self.bind_pattern(p, &format!("({scrut}).value"), &ok_t);
                }
                self.emit_arm_body(&arm.body, target, result_ty);
            }
        }
        self.writeln_s("    } else {");
        for arm in arms {
            if let PatternKind::Constructor { path, arguments } = &arm.pattern.kind
                && path.segments.last().map(String::as_str) == Some("Err")
            {
                if let Some(p) = arguments.first() {
                    self.bind_pattern(p, &format!("({scrut}).error"), &err_t);
                }
                self.emit_arm_body(&arm.body, target, result_ty);
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
                    // Full owner path: `lib.Status` resolves through the
                    // foreign type table, `Status` locally (or through the
                    // stem prefix recorded with it).
                    let owner = if path.segments.is_empty() {
                        name.to_string()
                    } else {
                        path.segments.join(".")
                    };
                    let Some(tag) = self.tcx.enum_tag(&owner, variant) else {
                        self.tcx
                            .rejects
                            .push(reject("NS8020", format!("enum '{owner}' is not declared")));
                        continue;
                    };
                    let _ = writeln!(self.out, "    case {tag}: {{");
                    if let Some(args) = arguments
                        && let Some(shape) = self.tcx.enum_shape(&owner)
                        && let Some(vshape) = shape.variants.iter().find(|v| &v.name == variant)
                    {
                        for (j, subpat) in args.iter().enumerate() {
                            let field_ty = vshape.fields.get(j).cloned().unwrap_or(Type::Unit);
                            let payload = if vshape.fields.len() == 1 {
                                format!("({scrut}).payload.{}", field_c(variant))
                            } else {
                                format!("({scrut}).payload.{}._{j}", field_c(variant))
                            };
                            self.bind_pattern(subpat, &payload, &field_ty);
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
        let Some(shape) = self.tcx.struct_shape(name) else {
            self.tcx
                .rejects
                .push(reject("NS8020", format!("struct '{name}' is not declared")));
            return;
        };
        for arm in arms {
            if let PatternKind::Constructor { path, arguments } = &arm.pattern.kind
                && (path.segments.join(".") == name
                    || path.segments.last().map(String::as_str) == Some(name))
            {
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

    fn match_literal(
        &mut self,
        scrut: &str,
        scrut_ty: &Type,
        arms: &[MatchArm],
        target: Option<&str>,
    ) {
        let mut first = true;
        let mut fallback: Option<usize> = None;
        for (i, arm) in arms.iter().enumerate() {
            match &arm.pattern.kind {
                PatternKind::Literal(lit) => {
                    let lit_c = self.literal_c(lit);
                    let cond = match scrut_ty {
                        Type::Str => format!("astra_str_eq({scrut}, {lit_c})"),
                        Type::Bytes => format!("astra_bytes_eq({scrut}, {lit_c})"),
                        _ => format!("{scrut} == {lit_c}"),
                    };
                    if first {
                        let _ = writeln!(self.out, "    if ({cond}) {{");
                        first = false;
                    } else {
                        let _ = writeln!(self.out, "    }} else if ({cond}) {{");
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
            let arm = &arms[i];
            if first {
                self.bind_pattern(&arm.pattern, scrut, scrut_ty);
                self.emit_arm_body(&arm.body, target, &Type::Unit);
            } else {
                self.writeln_s("    } else {");
                self.bind_pattern(&arm.pattern, scrut, scrut_ty);
                self.emit_arm_body(&arm.body, target, &Type::Unit);
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
                if b.statements.is_empty()
                    && target.is_some()
                    && result_ty == &Type::Unit
                    && let Some(t) = target
                {
                    let _ = writeln!(self.out, "        {t} = (NS_Unit){{0}};");
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
                let name = path.segments.join(".");
                if let Some(shape) = self.tcx.struct_shape(&name) {
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
        // Construct the error value and return it immediately. `fail` has type
        // `Never`, so no value is ever consumed from this expression. Write the
        // return directly to `self.out` so it is emitted in the right place
        // regardless of whether we are inside a `pre` accumulation.
        // Full path so an imported error type (`lib.MyError`) resolves to the
        // declaring dependency instead of being rejected.
        let name = error.segments.join(".");
        let err_val = self.construct_error(&name, arguments);
        let fn_result_c = self.fn_result_c();
        let _ = writeln!(
            self.out,
            "    return ({fn_result_c}){{ .ok = false, .error = {err_val} }};"
        );
        "((void*)0)".into()
    }

    fn construct_error(&mut self, name: &str, arguments: &FailArguments) -> String {
        let err_c = self.tcx.c_type(&Type::Named(name.to_string(), Vec::new()));
        match arguments {
            FailArguments::Tuple(args) => {
                let Some(shape) = self.tcx.struct_shape(name) else {
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
        let payload = panic_payload(&msg);
        let _ = writeln!(self.out, "    astra_panic({payload});");
        "((void*)0)".into()
    }

    fn abort_c(&mut self, message: &Expr) -> String {
        let msg = self.expr(message);
        let payload = panic_payload(&msg);
        let _ = writeln!(self.out, "    astra_abort({payload});");
        "((void*)0)".into()
    }
}

fn panic_payload(msg_expr: &str) -> String {
    // The message is an NS_Str; extract the .data pointer for the char* parameter.
    // If it's already a string literal compound literal, pull out the data directly.
    if msg_expr.starts_with("(NS_Str)") {
        format!("({msg_expr}).data")
    } else {
        msg_expr.to_string()
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
