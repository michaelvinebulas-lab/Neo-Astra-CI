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

    /// Returns the struct shape for a nominal name, if any.
    pub fn struct_of(&self, name: &str) -> Option<&StructShape> {
        if let Some(DeclShape::Struct(s)) = self.decls.get(name) {
            Some(s)
        } else {
            None
        }
    }

    /// Returns the enum shape for a nominal name, if any.
    pub fn enum_of(&self, name: &str) -> Option<&EnumShape> {
        if let Some(DeclShape::Enum(e)) = self.decls.get(name) {
            Some(e)
        } else {
            None
        }
    }

    /// Returns the shape for a nominal name, if any.
    pub fn shape(&self, name: &str) -> Option<&DeclShape> {
        self.decls.get(name)
    }
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
            return_type: Box::new(return_type.as_ref().map(|t| to_type(t)).unwrap_or(Type::Unit)),
            errors: error_set.iter().map(path_to_type).collect(),
        }),
        TypeKind::Unit => Type::Unit,
    }
}

/// Converts a dotted type path to its checked semantic type.
pub fn path_to_type(path: &ast::TypePath) -> Type {
    let name = path.segments.last().map(String::as_str).unwrap_or("");
    if path.arguments.is_empty() {
        if let Some(primitive) = primitive_type(name) {
            return primitive;
        }
    }
    match name {
        "Option" if path.arguments.len() == 1 => {
            Type::Option(Box::new(to_type(&path.arguments[0])))
        }
        "Result" if path.arguments.len() == 2 => {
            Type::Result(Box::new(to_type(&path.arguments[0])), Box::new(to_type(&path.arguments[1])))
        }
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
}

impl TyCtxt {
    /// Creates a fresh context over a collected [`TypeEnv`].
    pub fn new(env: TypeEnv) -> Self {
        Self {
            env,
            seen: BTreeSet::new(),
            typedefs: Vec::new(),
            rejects: Vec::new(),
        }
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
                if args.is_empty() {
                    sanitize(name)
                } else {
                    format!(
                        "{}_{}",
                        sanitize(name),
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
            Type::Bytes => {
                self.rejects.push(Reject {
                    code: "NS8014",
                    message: "bytes literals are not supported by the C backend first slice"
                        .into(),
                });
                "NS_Str".into()
            }
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
                self.ensure(&name, format!(
                    "typedef struct {{ bool some; {value} value; }} {name};"
                ));
                name
            }
            Type::Result(ok, err) => {
                let value = self.c_type(ok);
                let error = self.c_type(err);
                let name = format!("NS_Result_{}_{}", self.ident(ok), self.ident(err));
                self.ensure(&name, format!(
                    "typedef struct {{ bool ok; {value} value; {error} error; }} {name};"
                ));
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
            Type::Map(_, _) => {
                self.rejects.push(Reject {
                    code: "NS8013",
                    message: "maps are not supported by the C backend first slice".into(),
                });
                "void*".into()
            }
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
        let cname = format!("NS_{}", sanitize(name));
        // Clone the declared shape so we release the immutable borrow of
        // `self.env` before `self.c_type` mutably borrows `self`.
        let owned = match self.env.shape(name) {
            None => {
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
                let tag_name = format!("NS_{}_tag", sanitize(name));
                let mut tags = String::new();
                for (index, variant) in variants.iter().enumerate() {
                    tags.push_str(&format!(
                        "NS_{}_TAG_{} = {index}, ",
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
                        members.push_str(&format!("struct {{ {inner}}} {name}; ", name = variant.name));
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
    pub fn at(&self, span: Span) -> Option<&Type> {
        self.by_span.get(&(span.start, span.end))
    }
}
