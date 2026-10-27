//! Type mapping from Neo Astra types to C17.
//!
//! [`TypeEnv`] records the shape of every declared struct and enum so the
//! emitter can lower field access, struct initializers, and enum pattern
//! matching. [`TyCtxt`] maps a [`Type`] to its C type name, registering each
//! generated `typedef` exactly once in dependency order (inner types before
//! the types that embed them).

use std::collections::{BTreeMap, BTreeSet};

use astra_core::ast::{self, EnumDecl, StructDecl, StructFields, TypeKind};
use astra_core::span::Span;
use astra_core::types::Type;

/// Built-in nominal error payloads declared by the runtime header rather than
/// by any module. Their C names carry no module stem, so every translation
/// unit — and every module — shares one type (`NS_SliceError`).
const BUILTIN_NOMINALS: &[&str] = &[
    "SliceError",
    "ParseIntError",
    "OverflowError",
    "ChannelError",
];

/// Resolves a built-in nominal name (as written in source) to its shared C
/// identifier fragment, if it is one.
fn builtin_nominal(name: &str) -> Option<&'static str> {
    BUILTIN_NOMINALS
        .iter()
        .copied()
        .find(|builtin| *builtin == name)
}

/// A declared nominal type's shape.
#[derive(Debug, Clone)]
pub enum DeclShape {
    Struct(StructShape),
    Enum(EnumShape),
}

/// A struct's fields in declaration order.
///
/// Tuple structs use synthesized field names `_0`, `_1`, ...
#[derive(Debug, Clone)]
pub struct StructShape {
    pub tuple: bool,
    pub fields: Vec<(String, Type)>,
}

/// An enum's variants in declaration order.
#[derive(Debug, Clone)]
pub struct EnumShape {
    pub variants: Vec<EnumVariantShape>,
}

/// One enum variant and its payload field types (empty for a unit variant).
#[derive(Debug, Clone)]
pub struct EnumVariantShape {
    pub name: String,
    pub fields: Vec<Type>,
}

/// An owned snapshot of a declared shape, used to release the immutable borrow
/// of [`TypeEnv`] before the type mapper mutates itself.
enum OwnedDecl {
    Struct(Vec<(String, Type)>),
    Enum(Vec<EnumVariantShape>),
}

/// Declared nominal types keyed by their dotted source name.
#[derive(Debug, Clone, Default)]
pub struct TypeEnv {
    pub decls: BTreeMap<String, DeclShape>,
}

impl TypeEnv {
    /// Collects struct and enum shapes from a parsed module.
    pub fn collect(module: &ast::Module) -> Self {
        let mut decls = BTreeMap::new();
        for item in &module.items {
            match &item.kind {
                ast::ItemKind::Struct(s) => {
                    decls.insert(s.name.clone(), DeclShape::Struct(struct_shape(s)));
                }
                ast::ItemKind::Enum(e) => {
                    decls.insert(e.name.clone(), DeclShape::Enum(enum_shape(e)));
                }
                _ => {}
            }
        }
        Self { decls }
    }

    /// Returns the shape for a nominal name, if any.
    pub fn shape(&self, name: &str) -> Option<&DeclShape> {
        self.decls.get(name)
    }
}

/// A nominal type declared in a dependency module.
///
/// [`TyCtxt`] keys these by the name as written in the *using* module — the
/// imported name, a rename, or a namespace-dotted path — so signatures,
/// constructors, and patterns lower to the dependency's qualified typedef
/// (`NS_<dep stem>_<Type>`) instead of being rejected with `NS8020`.
#[derive(Debug, Clone)]
pub struct ForeignDecl {
    /// The dependency module's C stem; it is that module's type prefix.
    pub stem: String,
    /// The type's own name inside the declaring module (differs from the
    /// written name for rename imports).
    pub local: String,
    pub shape: DeclShape,
}

/// How a nominal type name resolves: which module declares it, its C prefix,
/// and its shape.
#[derive(Debug, Clone)]
pub struct Nominal {
    /// Prefix including the trailing `_`; empty for this module in standalone
    /// builds, `<stem>_` in linked builds.
    pub prefix: String,
    /// The type's own name in the declaring module.
    pub source: String,
    pub shape: DeclShape,
    /// True when the declaring module is a dependency. Its translation unit
    /// (and header) own the typedef, so the user must never re-emit it.
    pub foreign: bool,
}

fn struct_shape(decl: &StructDecl) -> StructShape {
    match &decl.fields {
        StructFields::Named(fields) => StructShape {
            tuple: false,
            fields: fields
                .iter()
                .map(|field| (field.name.clone(), to_type(&field.ty)))
                .collect(),
        },
        StructFields::Tuple(types) => StructShape {
            tuple: true,
            fields: types
                .iter()
                .enumerate()
                .map(|(index, ty)| (format!("_{index}"), to_type(ty)))
                .collect(),
        },
    }
}

fn enum_shape(decl: &EnumDecl) -> EnumShape {
    EnumShape {
        variants: decl
            .variants
            .iter()
            .map(|variant| EnumVariantShape {
                name: variant.name.clone(),
                fields: variant
                    .fields
                    .iter()
                    .flatten()
                    .map(|field| to_type(&field.ty))
                    .collect(),
            })
            .collect(),
    }
}

/// Converts a source type expression to its checked semantic type.
///
/// The type checker expands aliases before types reach HIR annotations, but
/// function signatures in the AST still carry [`ast::Type`]. This mirrors the
/// checker's resolution for the portable subset the C backend supports.
pub fn to_type(ast_type: &ast::Type) -> Type {
    match &ast_type.kind {
        TypeKind::Path(path) => path_to_type(path),
        TypeKind::Array(inner) => Type::Array(Box::new(to_type(inner))),
        TypeKind::FixedArray(inner, length) => {
            Type::FixedArray(Box::new(to_type(inner)), length.parse().unwrap_or(0))
        }
        TypeKind::Map(key, value) => Type::Map(Box::new(to_type(key)), Box::new(to_type(value))),
        TypeKind::Tuple(elements) => Type::Tuple(elements.iter().map(to_type).collect()),
        TypeKind::Function {
            parameters,
            return_type,
            error_set,
        } => Type::Function(astra_core::types::FunctionType {
            type_parameters: Vec::new(),
            parameters: parameters.iter().map(to_type).collect(),
            return_type: Box::new(
                return_type
                    .as_ref()
                    .map(|t| to_type(t))
                    .unwrap_or(Type::Unit),
            ),
            errors: error_set.iter().map(path_to_type).collect(),
        }),
        TypeKind::Unit => Type::Unit,
    }
}

/// Converts a dotted type path to its checked semantic type.
pub fn path_to_type(path: &ast::TypePath) -> Type {
    let name = path.segments.last().map(String::as_str).unwrap_or("");
    if path.arguments.is_empty()
        && let Some(primitive) = primitive_type(name)
    {
        return primitive;
    }
    match name {
        "Option" if path.arguments.len() == 1 => {
            Type::Option(Box::new(to_type(&path.arguments[0])))
        }
        "Result" if path.arguments.len() == 2 => Type::Result(
            Box::new(to_type(&path.arguments[0])),
            Box::new(to_type(&path.arguments[1])),
        ),
        _ => Type::Named(
            path.segments.join("."),
            path.arguments.iter().map(to_type).collect(),
        ),
    }
}

fn primitive_type(name: &str) -> Option<Type> {
    Some(match name {
        "i8" => Type::I8,
        "i16" => Type::I16,
        "i32" => Type::I32,
        "i64" => Type::I64,
        "u8" => Type::U8,
        "u16" => Type::U16,
        "u32" => Type::U32,
        "u64" => Type::U64,
        "usize" => Type::Usize,
        "isize" => Type::Isize,
        "f32" => Type::F32,
        "f64" => Type::F64,
        "bool" => Type::Bool,
        "str" => Type::Str,
        "bytes" => Type::Bytes,
        _ => return None,
    })
}

/// Sanitizes a source name into a C identifier fragment.
pub fn sanitize(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut last_under = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_under = false;
        } else if !last_under {
            out.push('_');
            last_under = true;
        }
    }
    if out.is_empty() {
        out.push('_');
    }
    out
}

/// A backend rejection with a stable diagnostic code and message.
#[derive(Debug, Clone)]
pub struct Reject {
    pub code: &'static str,
    pub message: String,
}

/// Type-mapping context that registers generated `typedef`s once, in order.
pub struct TyCtxt {
    pub env: TypeEnv,
    seen: BTreeSet<String>,
    pub typedefs: Vec<String>,
    pub rejects: Vec<Reject>,
    /// Prefix (with trailing `_`) prepended to every nominal type's C name.
    ///
    /// Empty in standalone (single-module) builds so the emitted names are
    /// unchanged. In multi-module linked builds it is the module stem, so two
    /// modules that each declare `struct Point` emit distinct typedefs
    /// (`NS_lib_Point` vs `NS_main_Point`) and their headers cannot collide.
    type_prefix: String,
    /// Nominal types declared by dependency modules, keyed by the name as
    /// written in this module (imported name, rename, or `alias.Type` path).
    foreign: BTreeMap<String, ForeignDecl>,
    /// Active type-parameter substitution while emitting a generic
    /// instantiation (`T` → `i32`, …). Empty outside specialization bodies.
    subst: BTreeMap<String, Type>,
}

impl TyCtxt {
    /// Creates a fresh context over a collected [`TypeEnv`].
    pub fn new(env: TypeEnv) -> Self {
        Self {
            env,
            seen: BTreeSet::new(),
            typedefs: Vec::new(),
            rejects: Vec::new(),
            type_prefix: String::new(),
            foreign: BTreeMap::new(),
            subst: BTreeMap::new(),
        }
    }

    /// Installs the type-parameter substitution for one generic instantiation.
    pub fn set_subst(&mut self, subst: BTreeMap<String, Type>) {
        self.subst = subst;
    }

    /// Clears the active type-parameter substitution.
    pub fn clear_subst(&mut self) {
        self.subst.clear();
    }

    /// Returns `ty` with the active type-parameter substitutions applied.
    pub fn substitute(&self, ty: &Type) -> Type {
        if self.subst.is_empty() {
            return ty.clone();
        }
        match ty {
            Type::Generic(name) => match self.subst.get(name) {
                Some(concrete) => concrete.clone(),
                None => ty.clone(),
            },
            Type::Named(name, arguments) if arguments.is_empty() => match self.subst.get(name) {
                Some(concrete) => concrete.clone(),
                None => ty.clone(),
            },
            Type::Option(inner) => Type::Option(Box::new(self.substitute(inner))),
            Type::Array(inner) => Type::Array(Box::new(self.substitute(inner))),
            Type::Slice(inner) => Type::Slice(Box::new(self.substitute(inner))),
            Type::FixedArray(inner, length) => {
                Type::FixedArray(Box::new(self.substitute(inner)), *length)
            }
            Type::Result(ok, error) => Type::Result(
                Box::new(self.substitute(ok)),
                Box::new(self.substitute(error)),
            ),
            Type::Map(key, value) => Type::Map(
                Box::new(self.substitute(key)),
                Box::new(self.substitute(value)),
            ),
            Type::Tuple(elements) => Type::Tuple(
                elements
                    .iter()
                    .map(|element| self.substitute(element))
                    .collect(),
            ),
            Type::Named(name, arguments) => Type::Named(
                name.clone(),
                arguments
                    .iter()
                    .map(|argument| self.substitute(argument))
                    .collect(),
            ),
            _ => ty.clone(),
        }
    }

    /// Registers a type declared by a dependency module under the name as
    /// written in this module. Must be called before [`TyCtxt`] maps any type.
    pub fn add_foreign(&mut self, name: String, decl: ForeignDecl) {
        self.foreign.insert(name, decl);
    }

    /// Resolves a nominal type name — local, imported, renamed, or
    /// namespace-dotted — to its declaring module's C prefix and shape.
    pub fn nominal(&self, name: &str) -> Option<Nominal> {
        if let Some(shape) = self.env.shape(name).cloned() {
            return Some(Nominal {
                prefix: self.type_prefix.clone(),
                source: name.to_string(),
                shape,
                foreign: false,
            });
        }
        let decl = self.foreign.get(name)?;
        Some(Nominal {
            prefix: if decl.stem.is_empty() {
                String::new()
            } else {
                format!("{}_", decl.stem)
            },
            source: decl.local.clone(),
            shape: decl.shape.clone(),
            foreign: true,
        })
    }

    /// Resolves a struct name to its shape, local or foreign.
    pub fn struct_shape(&self, name: &str) -> Option<StructShape> {
        match self.nominal(name)?.shape {
            DeclShape::Struct(shape) => Some(shape),
            DeclShape::Enum(_) => None,
        }
    }

    /// Resolves an enum name to its shape, local or foreign.
    pub fn enum_shape(&self, name: &str) -> Option<EnumShape> {
        match self.nominal(name)?.shape {
            DeclShape::Enum(shape) => Some(shape),
            DeclShape::Struct(_) => None,
        }
    }

    /// Builds the C tag constant for an enum variant
    /// (`NS_<prefix><Owner>_TAG_<Variant>`), local or foreign.
    pub fn enum_tag(&self, owner: &str, variant: &str) -> Option<String> {
        let nominal = self.nominal(owner)?;
        if !matches!(nominal.shape, DeclShape::Enum(_)) {
            return None;
        }
        Some(format!(
            "NS_{}{}_TAG_{}",
            nominal.prefix,
            sanitize(&nominal.source),
            sanitize(variant)
        ))
    }

    /// Sets the module stem used to qualify nominal type C names in linked
    /// (multi-module) builds. Must be called before any type is mapped.
    pub fn set_type_prefix(&mut self, stem: &str) {
        self.type_prefix = if stem.is_empty() {
            String::new()
        } else {
            format!("{stem}_")
        };
    }

    /// Returns true if any type was rejected.
    pub fn rejected(&self) -> bool {
        !self.rejects.is_empty()
    }

    fn ensure(&mut self, name: &str, decl: String) {
        if self.seen.insert(name.to_string()) {
            self.typedefs.push(decl);
        }
    }

    /// A stable identifier for a type, used to build nested typedef names. Does
    /// not register any declaration.
    pub fn ident(&self, ty: &Type) -> String {
        if let Type::Generic(name) = ty
            && let Some(concrete) = self.subst.get(name)
        {
            return self.ident(concrete);
        }
        if let Type::Named(name, arguments) = ty
            && arguments.is_empty()
            && let Some(concrete) = self.subst.get(name)
        {
            return self.ident(concrete);
        }
        match ty {
            Type::I8 => "i8".into(),
            Type::I16 => "i16".into(),
            Type::I32 => "i32".into(),
            Type::I64 => "i64".into(),
            Type::U8 => "u8".into(),
            Type::U16 => "u16".into(),
            Type::U32 => "u32".into(),
            Type::U64 => "u64".into(),
            Type::Usize => "usize".into(),
            Type::Isize => "isize".into(),
            Type::F32 => "f32".into(),
            Type::F64 => "f64".into(),
            Type::Bool => "bool".into(),
            Type::Str => "Str".into(),
            Type::Bytes => "Bytes".into(),
            Type::Unit => "Unit".into(),
            Type::Never => "Never".into(),
            Type::Error => "Error".into(),
            Type::Unknown => "Unknown".into(),
            Type::Option(inner) => format!("Option_{}", self.ident(inner)),
            Type::Result(ok, err) => format!("Result_{}_{}", self.ident(ok), self.ident(err)),
            Type::Array(inner) => format!("Array_{}", self.ident(inner)),
            Type::Slice(inner) => format!("Slice_{}", self.ident(inner)),
            Type::FixedArray(inner, n) => format!("FixedArray_{}_{}", self.ident(inner), n),
            Type::Map(key, value) => format!("Map_{}_{}", self.ident(key), self.ident(value)),
            Type::Tuple(elements) => {
                format!(
                    "Tuple{}_{}",
                    elements.len(),
                    elements
                        .iter()
                        .map(|t| self.ident(t))
                        .collect::<Vec<_>>()
                        .join("_")
                )
            }
            Type::Function(_) => "Fn".into(),
            Type::Named(name, args) => {
                // Foreign types take the declaring dependency's prefix so
                // container names (`NS_Option_lib_Point`) match the names the
                // dependency itself generates when values cross modules.
                let base = match self.nominal(name) {
                    Some(nominal) => format!("{}{}", nominal.prefix, sanitize(&nominal.source)),
                    // Built-in error payloads are stem-free by design.
                    None => match builtin_nominal(name) {
                        Some(builtin) => builtin.to_string(),
                        None => format!("{}{}", self.type_prefix, sanitize(name)),
                    },
                };
                if args.is_empty() {
                    base
                } else {
                    format!(
                        "{}_{}",
                        base,
                        args.iter()
                            .map(|t| self.ident(t))
                            .collect::<Vec<_>>()
                            .join("_")
                    )
                }
            }
            Type::Generic(name) => sanitize(name),
        }
    }

    /// Maps a type to its C type name, registering any `typedef` it needs. May
    /// push a [`Reject`] for types the first slice does not support.
    pub fn c_type(&mut self, ty: &Type) -> String {
        if let Type::Generic(name) = ty
            && let Some(concrete) = self.subst.get(name).cloned()
        {
            return self.c_type(&concrete);
        }
        if let Type::Named(name, arguments) = ty
            && arguments.is_empty()
            && let Some(concrete) = self.subst.get(name).cloned()
            && concrete != *ty
        {
            return self.c_type(&concrete);
        }
        match ty {
            Type::I8 => "int8_t".into(),
            Type::I16 => "int16_t".into(),
            Type::I32 => "int32_t".into(),
            Type::I64 => "int64_t".into(),
            Type::U8 => "uint8_t".into(),
            Type::U16 => "uint16_t".into(),
            Type::U32 => "uint32_t".into(),
            Type::U64 => "uint64_t".into(),
            Type::Usize => "size_t".into(),
            Type::Isize => "intptr_t".into(),
            Type::F32 => "float".into(),
            Type::F64 => "double".into(),
            Type::Bool => "bool".into(),
            Type::Str => "NS_Str".into(),
            Type::Bytes => "NS_Bytes".into(),
            Type::Unit => "NS_Unit".into(),
            Type::Never => {
                self.rejects.push(Reject {
                    code: "NS8099",
                    message: "Never appears as a stored type, which the C backend cannot emit"
                        .into(),
                });
                "void".into()
            }
            Type::Option(inner) => {
                let value = self.c_type(inner);
                let name = format!("NS_Option_{}", self.ident(inner));
                self.ensure(
                    &name,
                    format!("typedef struct {{ bool some; {value} value; }} {name};"),
                );
                name
            }
            Type::Result(ok, err) => {
                let value = self.c_type(ok);
                let error = self.c_type(err);
                let name = format!("NS_Result_{}_{}", self.ident(ok), self.ident(err));
                self.ensure(
                    &name,
                    format!("typedef struct {{ bool ok; {value} value; {error} error; }} {name};"),
                );
                name
            }
            Type::Array(_) | Type::Slice(_) => "NS_Array".into(),
            Type::FixedArray(_, _) => {
                self.rejects.push(Reject {
                    code: "NS8022",
                    message: "fixed-size arrays are not supported by the C backend first slice"
                        .into(),
                });
                "NS_Array".into()
            }
            Type::Map(_, _) => "NS_Map".into(),
            Type::Tuple(elements) => {
                let name = format!("NS_{}", self.ident(ty));
                let mut fields = String::new();
                for (index, element) in elements.iter().enumerate() {
                    let field = self.c_type(element);
                    fields.push_str(&format!("{field} _{index}; "));
                }
                self.ensure(&name, format!("typedef struct {{ {fields}}} {name};"));
                name
            }
            Type::Function(_) => {
                self.rejects.push(Reject {
                    code: "NS8017",
                    message: "function values are not supported by the C backend first slice"
                        .into(),
                });
                "void*".into()
            }
            Type::Named(name, _) if name == "Channel" => "NS_Channel*".into(),
            Type::Named(name, args) if name == "Task" => {
                let inner = args.first().cloned().unwrap_or(Type::Unit);
                let value = self.c_type(&inner);
                let name = format!("NS_Task_{}", self.ident(&inner));
                self.ensure(
                    &name,
                    format!("typedef struct {{ {value} value; bool canceled; }} {name};"),
                );
                name
            }
            Type::Named(name, args) => {
                if !args.is_empty() {
                    self.rejects.push(Reject {
                        code: "NS8018",
                        message: format!(
                            "generic type '{name}' requires monomorphization, unsupported by the C backend first slice"
                        ),
                    });
                    return format!("NS_{}", sanitize(name));
                }
                self.user_typedef(name)
            }
            Type::Generic(name) => {
                self.rejects.push(Reject {
                    code: "NS8018",
                    message: format!(
                        "generic parameter '{name}' requires monomorphization, unsupported by the C backend first slice"
                    ),
                });
                sanitize(name)
            }
            Type::Error | Type::Unknown => {
                self.rejects.push(Reject {
                    code: "NS8099",
                    message: "an unresolved or recovery type reached the C backend".into(),
                });
                "void*".into()
            }
        }
    }

    fn user_typedef(&mut self, name: &str) -> String {
        let cname = format!("NS_{}{}", self.type_prefix, sanitize(name));
        // A dependency module declares this type: its translation unit and
        // header own the typedef (this module already includes that header),
        // so reference the qualified name without re-emitting it here.
        if let Some(nominal) = self.nominal(name)
            && nominal.foreign
        {
            return format!("NS_{}{}", nominal.prefix, sanitize(&nominal.source));
        }
        // Clone the declared shape so we release the immutable borrow of
        // `self.env` before `self.c_type` mutably borrows `self`.
        let owned = match self.env.shape(name) {
            None => {
                // Built-in error payloads (`SliceError`) are declared once in
                // the runtime header, not in any module.
                if let Some(builtin) = builtin_nominal(name) {
                    return format!("NS_{builtin}");
                }
                self.rejects.push(Reject {
                    code: "NS8020",
                    message: format!("type '{name}' is not declared in this module"),
                });
                return cname;
            }
            Some(DeclShape::Struct(s)) => OwnedDecl::Struct(s.fields.clone()),
            Some(DeclShape::Enum(e)) => OwnedDecl::Enum(e.variants.clone()),
        };
        match owned {
            OwnedDecl::Struct(fields) => {
                let mut fields_c = String::new();
                for (field_name, field_type) in &fields {
                    let field_c = self.c_type(field_type);
                    fields_c.push_str(&format!("{field_c} {field_name}; "));
                }
                self.ensure(&cname, format!("typedef struct {{ {fields_c}}} {cname};"));
            }
            OwnedDecl::Enum(variants) => {
                let tag_name = format!("NS_{}{}_tag", self.type_prefix, sanitize(name));
                let mut tags = String::new();
                for (index, variant) in variants.iter().enumerate() {
                    tags.push_str(&format!(
                        "NS_{}{}_TAG_{} = {index}, ",
                        self.type_prefix,
                        sanitize(name),
                        variant.name
                    ));
                }
                self.ensure(&tag_name, format!("typedef enum {{ {tags}}} {tag_name};"));
                let mut members = String::new();
                for variant in &variants {
                    if variant.fields.is_empty() {
                        members.push_str(&format!("char _{name}; ", name = variant.name));
                    } else if variant.fields.len() == 1 {
                        let field_c = self.c_type(&variant.fields[0]);
                        members.push_str(&format!("{field_c} {name}; ", name = variant.name));
                    } else {
                        let mut inner = String::new();
                        for (index, field_type) in variant.fields.iter().enumerate() {
                            let field_c = self.c_type(field_type);
                            inner.push_str(&format!("{field_c} _{index}; "));
                        }
                        members.push_str(&format!(
                            "struct {{ {inner}}} {name}; ",
                            name = variant.name
                        ));
                    }
                }
                self.ensure(
                    &cname,
                    format!(
                        "typedef struct {{ {tag_name} tag; union {{ {members}}} payload; }} {cname};"
                    ),
                );
            }
        }
        cname
    }
}

/// A span-keyed lookup table of checked expression types.
#[derive(Debug, Clone, Default)]
pub struct Annotations {
    by_span: BTreeMap<(usize, usize), Type>,
}

impl Annotations {
    /// Builds the index from checker annotations in source order.
    pub fn new(annotations: &[astra_core::checker::TypeAnnotation]) -> Self {
        let mut by_span = BTreeMap::new();
        for ann in annotations {
            by_span.insert((ann.span.start, ann.span.end), ann.ty.clone());
        }
        Self { by_span }
    }

    /// Returns the checked type of an expression, if recorded.
    pub fn type_of(&self, expr: &ast::Expr) -> Option<&Type> {
        self.by_span.get(&(expr.span.start, expr.span.end))
    }

    /// Returns the checked type at a span.
    #[allow(dead_code)]
    pub fn at(&self, span: Span) -> Option<&Type> {
        self.by_span.get(&(span.start, span.end))
    }
}
