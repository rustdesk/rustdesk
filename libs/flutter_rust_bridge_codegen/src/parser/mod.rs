mod ty;

use std::string::String;

use log::debug;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::*;

use crate::ir::*;

use crate::generator::rust::HANDLER_NAME;
use crate::parser::ty::TypeParser;
use crate::source_graph::Crate;

const STREAM_SINK_IDENT: &str = "StreamSink";
const RESULT_IDENT: &str = "Result";

pub fn parse(source_rust_content: &str, file: File, manifest_path: &str) -> IrFile {
    let crate_map = Crate::new(manifest_path);

    let src_fns = extract_fns_from_file(&file);
    let src_structs = crate_map.root_module.collect_structs_to_vec();
    let src_enums = crate_map.root_module.collect_enums_to_vec();

    let parser = Parser::new(TypeParser::new(src_structs, src_enums));
    parser.parse(source_rust_content, src_fns)
}

struct Parser<'a> {
    type_parser: TypeParser<'a>,
}

impl<'a> Parser<'a> {
    pub fn new(type_parser: TypeParser<'a>) -> Self {
        Parser { type_parser }
    }
}

impl<'a> Parser<'a> {
    fn parse(mut self, source_rust_content: &str, src_fns: Vec<&ItemFn>) -> IrFile {
        let funcs = src_fns.iter().map(|f| self.parse_function(f)).collect();

        let has_executor = source_rust_content.contains(HANDLER_NAME);

        let (struct_pool, enum_pool) = self.type_parser.consume();

        IrFile {
            funcs,
            struct_pool,
            enum_pool,
            has_executor,
        }
    }

    /// Attempts to parse the type from the return part of a function signature. There is a special
    /// case for top-level `Result` types.
    pub fn try_parse_fn_output_type(&mut self, ty: &syn::Type) -> Option<IrFuncOutput> {
        let inner = ty::SupportedInnerType::try_from_syn_type(ty)?;

        match inner {
            ty::SupportedInnerType::Path(ty::SupportedPathType {
                ident,
                generic: Some(generic),
            }) if ident == RESULT_IDENT => Some(IrFuncOutput::ResultType(
                self.type_parser.convert_to_ir_type(*generic)?,
            )),
            _ => Some(IrFuncOutput::Type(
                self.type_parser.convert_to_ir_type(inner)?,
            )),
        }
    }

    /// Attempts to parse the type from an argument of a function signature. There is a special
    /// case for top-level `StreamSink` types.
    pub fn try_parse_fn_arg_type(&mut self, ty: &syn::Type) -> Option<IrFuncArg> {
        match ty {
            syn::Type::Path(syn::TypePath { path, .. }) => {
                let last_segment = path.segments.last().unwrap();
                if last_segment.ident == STREAM_SINK_IDENT {
                    match &last_segment.arguments {
                        syn::PathArguments::AngleBracketed(
                            syn::AngleBracketedGenericArguments { args, .. },
                        ) if args.len() == 1 => {
                            // Unwrap is safe here because args.len() == 1
                            match args.last().unwrap() {
                                syn::GenericArgument::Type(t) => {
                                    Some(IrFuncArg::StreamSinkType(self.type_parser.parse_type(t)))
                                }
                                _ => None,
                            }
                        }
                        _ => None,
                    }
                } else {
                    Some(IrFuncArg::Type(self.type_parser.parse_type(ty)))
                }
            }
            _ => None,
        }
    }

    fn parse_function(&mut self, func: &ItemFn) -> IrFunc {
        debug!("parse_function function name: {:?}", func.sig.ident);

        let sig = &func.sig;
        let func_name = sig.ident.to_string();

        let mut inputs = Vec::new();
        let mut output = None;
        let mut mode = None;
        let mut fallible = true;

        for sig_input in &sig.inputs {
            if let FnArg::Typed(ref pat_type) = sig_input {
                let name = if let Pat::Ident(ref pat_ident) = *pat_type.pat {
                    format!("{}", pat_ident.ident)
                } else {
                    panic!("unexpected pat_type={:?}", pat_type)
                };

                match self.try_parse_fn_arg_type(&pat_type.ty).unwrap_or_else(|| {
                    panic!(
                        "Failed to parse function argument type `{}`",
                        type_to_string(&pat_type.ty)
                    )
                }) {
                    IrFuncArg::StreamSinkType(ty) => {
                        output = Some(ty);
                        mode = Some(IrFuncMode::Stream);
                    }
                    IrFuncArg::Type(ty) => {
                        inputs.push(IrField {
                            name: IrIdent::new(name),
                            ty,
                            is_final: true,
                            comments: extract_comments(&pat_type.attrs),
                        });
                    }
                }
            } else {
                panic!("unexpected sig_input={:?}", sig_input);
            }
        }

        if output.is_none() {
            output = Some(match &sig.output {
                ReturnType::Type(_, ty) => {
                    match self.try_parse_fn_output_type(ty).unwrap_or_else(|| {
                        panic!(
                            "Failed to parse function output type `{}`",
                            type_to_string(ty)
                        )
                    }) {
                        IrFuncOutput::ResultType(ty) => ty,
                        IrFuncOutput::Type(ty) => {
                            fallible = false;
                            ty
                        }
                    }
                }
                ReturnType::Default => {
                    fallible = false;
                    IrType::Primitive(IrTypePrimitive::Unit)
                }
            });
            mode = Some(
                if let Some(IrType::Delegate(IrTypeDelegate::SyncReturnVecU8)) = output {
                    IrFuncMode::Sync
                } else {
                    IrFuncMode::Normal
                },
            );
        }

        // let comments = func.attrs.iter().filter_map(extract_comments).collect();

        IrFunc {
            name: func_name,
            inputs,
            output: output.expect("unsupported output"),
            fallible,
            mode: mode.expect("unsupported mode"),
            comments: extract_comments(&func.attrs),
        }
    }
}

fn extract_fns_from_file(file: &File) -> Vec<&ItemFn> {
    let mut src_fns = Vec::new();

    for item in file.items.iter() {
        if let Item::Fn(ref item_fn) = item {
            if let Visibility::Public(_) = &item_fn.vis {
                src_fns.push(item_fn);
            }
        }
    }

    src_fns
}

fn extract_comments(attrs: &[Attribute]) -> Vec<IrComment> {
    attrs
        .iter()
        .filter_map(|attr| match attr.parse_meta() {
            Ok(Meta::NameValue(MetaNameValue {
                path,
                lit: Lit::Str(lit),
                ..
            })) if path.is_ident("doc") => Some(IrComment::from(lit.value().as_ref())),
            _ => None,
        })
        .collect()
}

pub mod frb_keyword {
    syn::custom_keyword!(mirror);
    syn::custom_keyword!(non_final);
    syn::custom_keyword!(dart_metadata);
    syn::custom_keyword!(import);
}

#[derive(Clone, Debug)]
pub struct NamedOption<K, V> {
    pub name: K,
    pub value: V,
}

impl<K: Parse + std::fmt::Debug, V: Parse> Parse for NamedOption<K, V> {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let name: K = input.parse()?;
        let _: Token![=] = input.parse()?;
        let value = input.parse()?;
        Ok(Self { name, value })
    }
}

#[derive(Clone, Debug)]
pub struct MirrorOption(Path);

impl Parse for MirrorOption {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let content;
        parenthesized!(content in input);
        let path: Path = content.parse()?;
        Ok(Self(path))
    }
}

#[derive(Clone, Debug)]
pub struct MetadataAnnotations(Vec<IrDartAnnotation>);

impl Parse for IrDartAnnotation {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let annotation: LitStr = input.parse()?;
        let library = if input.peek(frb_keyword::import) {
            let _ = input.parse::<frb_keyword::import>()?;
            let library: IrDartImport = input.parse()?;
            Some(library)
        } else {
            None
        };
        Ok(Self {
            content: annotation.value(),
            library,
        })
    }
}
impl Parse for MetadataAnnotations {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let content;
        parenthesized!(content in input);
        let annotations =
            Punctuated::<IrDartAnnotation, syn::Token![,]>::parse_terminated(&content)?
                .into_iter()
                .collect();
        Ok(Self(annotations))
    }
}

#[derive(Clone, Debug)]
pub struct DartImports(Vec<IrDartImport>);

impl Parse for IrDartImport {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let uri: LitStr = input.parse()?;
        let alias: Option<String> = if input.peek(token::As) {
            let _ = input.parse::<token::As>()?;
            let alias: Ident = input.parse()?;
            Some(alias.to_string())
        } else {
            None
        };
        Ok(Self {
            uri: uri.value(),
            alias,
        })
    }
}
impl Parse for DartImports {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let content;
        parenthesized!(content in input);
        let imports = Punctuated::<IrDartImport, syn::Token![,]>::parse_terminated(&content)?
            .into_iter()
            .collect();
        Ok(Self(imports))
    }
}

enum FrbOption {
    Mirror(MirrorOption),
    NonFinal,
    Metadata(NamedOption<frb_keyword::dart_metadata, MetadataAnnotations>),
}

impl Parse for FrbOption {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(frb_keyword::mirror) {
            input.parse().map(FrbOption::Mirror)
        } else if lookahead.peek(frb_keyword::non_final) {
            input
                .parse::<frb_keyword::non_final>()
                .map(|_| FrbOption::NonFinal)
        } else if lookahead.peek(frb_keyword::dart_metadata) {
            input.parse().map(FrbOption::Metadata)
        } else {
            Err(lookahead.error())
        }
    }
}
fn extract_metadata(attrs: &[Attribute]) -> Vec<IrDartAnnotation> {
    attrs
        .iter()
        .filter(|attr| attr.path.is_ident("frb"))
        .map(|attr| attr.parse_args::<FrbOption>())
        .flat_map(|frb_option| match frb_option {
            Ok(FrbOption::Metadata(NamedOption {
                name: _,
                value: MetadataAnnotations(annotations),
            })) => annotations,
            _ => vec![],
        })
        .collect()
}

/// syn -> string https://github.com/dtolnay/syn/issues/294
fn type_to_string(ty: &Type) -> String {
    quote!(#ty).to_string().replace(' ', "")
}
