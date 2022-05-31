use std::collections::{HashMap, HashSet};
use std::string::String;

use syn::*;

use crate::ir::IrType::*;
use crate::ir::*;

use crate::markers;

use crate::source_graph::{Enum, Struct};

use crate::parser::{extract_comments, extract_metadata, type_to_string};

pub struct TypeParser<'a> {
    src_structs: HashMap<String, &'a Struct>,
    src_enums: HashMap<String, &'a Enum>,

    parsing_or_parsed_struct_names: HashSet<String>,
    struct_pool: IrStructPool,

    parsed_enums: HashSet<String>,
    enum_pool: IrEnumPool,
}

impl<'a> TypeParser<'a> {
    pub fn new(
        src_structs: HashMap<String, &'a Struct>,
        src_enums: HashMap<String, &'a Enum>,
    ) -> Self {
        TypeParser {
            src_structs,
            src_enums,
            struct_pool: HashMap::new(),
            enum_pool: HashMap::new(),
            parsing_or_parsed_struct_names: HashSet::new(),
            parsed_enums: HashSet::new(),
        }
    }

    pub fn consume(self) -> (IrStructPool, IrEnumPool) {
        (self.struct_pool, self.enum_pool)
    }
}

/// Generic intermediate representation of a type that can appear inside a function signature.
#[derive(Debug)]
pub enum SupportedInnerType {
    /// Path types with up to 1 generic type argument on the final segment. All segments before
    /// the last segment are ignored. The generic type argument must also be a valid
    /// `SupportedInnerType`.
    Path(SupportedPathType),
    /// Array type
    Array(Box<Self>, usize),
    /// The unit type `()`.
    Unit,
}

impl std::fmt::Display for SupportedInnerType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Path(p) => write!(f, "{}", p),
            Self::Array(u, len) => write!(f, "[{}; {}]", u, len),
            Self::Unit => write!(f, "()"),
        }
    }
}

/// Represents a named type, with an optional path and up to 1 generic type argument.
#[derive(Debug)]
pub struct SupportedPathType {
    pub ident: syn::Ident,
    pub generic: Option<Box<SupportedInnerType>>,
}

impl std::fmt::Display for SupportedPathType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let ident = self.ident.to_string();
        if let Some(generic) = &self.generic {
            write!(f, "{}<{}>", ident, generic)
        } else {
            write!(f, "{}", ident)
        }
    }
}

impl SupportedInnerType {
    /// Given a `syn::Type`, returns a simplified representation of the type if it's supported,
    /// or `None` otherwise.
    pub fn try_from_syn_type(ty: &syn::Type) -> Option<Self> {
        match ty {
            syn::Type::Path(syn::TypePath { path, .. }) => {
                let last_segment = path.segments.last().unwrap().clone();
                match last_segment.arguments {
                    syn::PathArguments::None => Some(SupportedInnerType::Path(SupportedPathType {
                        ident: last_segment.ident,
                        generic: None,
                    })),
                    syn::PathArguments::AngleBracketed(a) => {
                        let generic = match a.args.into_iter().next() {
                            Some(syn::GenericArgument::Type(t)) => {
                                Some(Box::new(SupportedInnerType::try_from_syn_type(&t)?))
                            }
                            _ => None,
                        };

                        Some(SupportedInnerType::Path(SupportedPathType {
                            ident: last_segment.ident,
                            generic,
                        }))
                    }
                    _ => None,
                }
            }
            syn::Type::Array(syn::TypeArray { elem, len, .. }) => {
                let len: usize = match len {
                    syn::Expr::Lit(lit) => match &lit.lit {
                        syn::Lit::Int(x) => x.base10_parse().unwrap(),
                        _ => panic!("Cannot parse array length"),
                    },
                    _ => panic!("Cannot parse array length"),
                };
                Some(SupportedInnerType::Array(
                    Box::new(SupportedInnerType::try_from_syn_type(elem)?),
                    len,
                ))
            }
            syn::Type::Tuple(syn::TypeTuple { elems, .. }) if elems.is_empty() => {
                Some(SupportedInnerType::Unit)
            }
            _ => None,
        }
    }
}

impl<'a> TypeParser<'a> {
    pub fn parse_type(&mut self, ty: &syn::Type) -> IrType {
        let supported_type = SupportedInnerType::try_from_syn_type(ty)
            .unwrap_or_else(|| panic!("Unsupported type `{}`", type_to_string(ty)));

        self.convert_to_ir_type(supported_type)
            .unwrap_or_else(|| panic!("parse_type failed for ty={}", type_to_string(ty)))
    }

    /// Converts an inner type into an `IrType` if possible.
    pub fn convert_to_ir_type(&mut self, ty: SupportedInnerType) -> Option<IrType> {
        match ty {
            SupportedInnerType::Path(p) => self.convert_path_to_ir_type(p),
            SupportedInnerType::Array(p, len) => self.convert_array_to_ir_type(*p, len),
            SupportedInnerType::Unit => Some(IrType::Primitive(IrTypePrimitive::Unit)),
        }
    }

    /// Converts an array type into an `IrType` if possible.
    pub fn convert_array_to_ir_type(
        &mut self,
        generic: SupportedInnerType,
        _len: usize,
    ) -> Option<IrType> {
        self.convert_to_ir_type(generic).map(|inner| match inner {
            Primitive(primitive) => PrimitiveList(IrTypePrimitiveList { primitive }),
            others => GeneralList(IrTypeGeneralList {
                inner: Box::new(others),
            }),
        })
    }

    /// Converts a path type into an `IrType` if possible.
    pub fn convert_path_to_ir_type(&mut self, p: SupportedPathType) -> Option<IrType> {
        let p_as_str = format!("{}", &p);
        let ident_string = &p.ident.to_string();
        if let Some(generic) = p.generic {
            match ident_string.as_str() {
                "SyncReturn" => {
                    // Special-case SyncReturn<Vec<u8>>. SyncReturn for any other type is not
                    // supported.
                    match *generic {
                        SupportedInnerType::Path(SupportedPathType {
                            ident,
                            generic: Some(generic),
                        }) if ident == "Vec" => match *generic {
                            SupportedInnerType::Path(SupportedPathType {
                                ident,
                                generic: None,
                            }) if ident == "u8" => {
                                Some(IrType::Delegate(IrTypeDelegate::SyncReturnVecU8))
                            }
                            _ => None,
                        },
                        _ => None,
                    }
                }
                "Vec" => {
                    // Special-case Vec<String> as StringList
                    if matches!(*generic, SupportedInnerType::Path(SupportedPathType { ref ident, .. }) if ident == "String")
                    {
                        Some(IrType::Delegate(IrTypeDelegate::StringList))
                    } else {
                        self.convert_to_ir_type(*generic).map(|inner| match inner {
                            Primitive(primitive) => {
                                PrimitiveList(IrTypePrimitiveList { primitive })
                            }
                            others => GeneralList(IrTypeGeneralList {
                                inner: Box::new(others),
                            }),
                        })
                    }
                }
                "ZeroCopyBuffer" => {
                    let inner = self.convert_to_ir_type(*generic);
                    if let Some(IrType::PrimitiveList(IrTypePrimitiveList { primitive })) = inner {
                        Some(IrType::Delegate(
                            IrTypeDelegate::ZeroCopyBufferVecPrimitive(primitive),
                        ))
                    } else {
                        None
                    }
                }
                "Box" => self.convert_to_ir_type(*generic).map(|inner| {
                    Boxed(IrTypeBoxed {
                        exist_in_real_api: true,
                        inner: Box::new(inner),
                    })
                }),
                "Option" => {
                    // Disallow nested Option
                    if matches!(*generic, SupportedInnerType::Path(SupportedPathType { ref ident, .. }) if ident == "Option")
                    {
                        panic!(
                            "Nested optionals without indirection are not supported. (Option<Option<{}>>)",
                            p_as_str
                        );
                    }
                    self.convert_to_ir_type(*generic).map(|inner| match inner {
                        Primitive(prim) => IrType::Optional(IrTypeOptional::new_prim(prim)),
                        st @ StructRef(_) => {
                            IrType::Optional(IrTypeOptional::new_ptr(Boxed(IrTypeBoxed {
                                inner: Box::new(st),
                                exist_in_real_api: false,
                            })))
                        }
                        other => IrType::Optional(IrTypeOptional::new_ptr(other)),
                    })
                }
                _ => None,
            }
        } else {
            IrTypePrimitive::try_from_rust_str(ident_string)
                .map(Primitive)
                .or_else(|| {
                    if ident_string == "String" {
                        Some(IrType::Delegate(IrTypeDelegate::String))
                    } else if self.src_structs.contains_key(ident_string) {
                        if !self.parsing_or_parsed_struct_names.contains(ident_string) {
                            self.parsing_or_parsed_struct_names
                                .insert(ident_string.to_owned());
                            let api_struct = self.parse_struct_core(&p.ident);
                            self.struct_pool.insert(ident_string.to_owned(), api_struct);
                        }

                        Some(StructRef(IrTypeStructRef {
                            name: ident_string.to_owned(),
                            freezed: self
                                .struct_pool
                                .get(ident_string)
                                .map(IrStruct::using_freezed)
                                .unwrap_or(false),
                        }))
                    } else if self.src_enums.contains_key(ident_string) {
                        if self.parsed_enums.insert(ident_string.to_owned()) {
                            let enu = self.parse_enum_core(&p.ident);
                            self.enum_pool.insert(ident_string.to_owned(), enu);
                        }

                        Some(EnumRef(IrTypeEnumRef {
                            name: ident_string.to_owned(),
                            is_struct: self
                                .enum_pool
                                .get(ident_string)
                                .map(IrEnum::is_struct)
                                .unwrap_or(true),
                        }))
                    } else {
                        None
                    }
                })
        }
    }
}

impl<'a> TypeParser<'a> {
    fn parse_enum_core(&mut self, ident: &syn::Ident) -> IrEnum {
        let src_enum = self.src_enums[&ident.to_string()];
        let name = src_enum.ident.to_string();
        let wrapper_name = if src_enum.mirror {
            Some(format!("mirror_{}", name))
        } else {
            None
        };
        let path = src_enum.path.clone();
        let comments = extract_comments(&src_enum.src.attrs);
        let variants = src_enum
            .src
            .variants
            .iter()
            .map(|variant| IrVariant {
                name: IrIdent::new(variant.ident.to_string()),
                comments: extract_comments(&variant.attrs),
                kind: match variant.fields.iter().next() {
                    None => IrVariantKind::Value,
                    Some(Field {
                        attrs,
                        ident: field_ident,
                        ..
                    }) => {
                        let variant_ident = variant.ident.to_string();
                        IrVariantKind::Struct(IrStruct {
                            name: variant_ident,
                            wrapper_name: None,
                            path: None,
                            is_fields_named: field_ident.is_some(),
                            dart_metadata: extract_metadata(attrs),
                            comments: extract_comments(attrs),
                            fields: variant
                                .fields
                                .iter()
                                .enumerate()
                                .map(|(idx, field)| IrField {
                                    name: IrIdent::new(
                                        field
                                            .ident
                                            .as_ref()
                                            .map(ToString::to_string)
                                            .unwrap_or_else(|| format!("field{}", idx)),
                                    ),
                                    ty: self.parse_type(&field.ty),
                                    is_final: true,
                                    comments: extract_comments(&field.attrs),
                                })
                                .collect(),
                        })
                    }
                },
            })
            .collect();
        IrEnum::new(name, wrapper_name, path, comments, variants)
    }

    fn parse_struct_core(&mut self, ident: &syn::Ident) -> IrStruct {
        let src_struct = self.src_structs[&ident.to_string()];
        let mut fields = Vec::new();

        let (is_fields_named, struct_fields) = match &src_struct.src.fields {
            Fields::Named(FieldsNamed { named, .. }) => (true, named),
            Fields::Unnamed(FieldsUnnamed { unnamed, .. }) => (false, unnamed),
            _ => panic!("unsupported type: {:?}", src_struct.src.fields),
        };

        for (idx, field) in struct_fields.iter().enumerate() {
            let field_name = field
                .ident
                .as_ref()
                .map_or(format!("field{}", idx), ToString::to_string);
            let field_type = self.parse_type(&field.ty);
            fields.push(IrField {
                name: IrIdent::new(field_name),
                ty: field_type,
                is_final: !markers::has_non_final(&field.attrs),
                comments: extract_comments(&field.attrs),
            });
        }

        let name = src_struct.ident.to_string();
        let wrapper_name = if src_struct.mirror {
            Some(format!("mirror_{}", name))
        } else {
            None
        };
        let path = Some(src_struct.path.clone());
        let metadata = extract_metadata(&src_struct.src.attrs);
        let comments = extract_comments(&src_struct.src.attrs);
        IrStruct {
            name,
            wrapper_name,
            path,
            fields,
            is_fields_named,
            dart_metadata: metadata,
            comments,
        }
    }
}
