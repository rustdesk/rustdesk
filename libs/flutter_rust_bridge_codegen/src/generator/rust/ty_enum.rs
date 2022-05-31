use crate::generator::rust::ty::*;
use crate::generator::rust::ExternFuncCollector;
use crate::ir::*;
use crate::type_rust_generator_struct;

type_rust_generator_struct!(TypeEnumRefGenerator, IrTypeEnumRef);

impl TypeRustGeneratorTrait for TypeEnumRefGenerator<'_> {
    fn wire2api_body(&self) -> Option<String> {
        let enu = self.ir.get(self.context.ir_file);
        Some(if self.ir.is_struct {
            let variants = enu
                .variants()
                .iter()
                .enumerate()
                .map(|(idx, variant)| match &variant.kind {
                    IrVariantKind::Value => {
                        format!("{} => {}::{},", idx, enu.name, variant.name)
                    }
                    IrVariantKind::Struct(st) => {
                        let fields: Vec<_> = st
                            .fields
                            .iter()
                            .map(|field| {
                                if st.is_fields_named {
                                    format!("{0}: ans.{0}.wire2api()", field.name.rust_style())
                                } else {
                                    format!("ans.{}.wire2api()", field.name.rust_style())
                                }
                            })
                            .collect();
                        let (left, right) = st.brackets_pair();
                        format!(
                            "{} => unsafe {{
                                        let ans = support::box_from_leak_ptr(self.kind);
                                        let ans = support::box_from_leak_ptr(ans.{2});
                                        {}::{2}{3}{4}{5}
                                    }}",
                            idx,
                            enu.name,
                            variant.name,
                            left,
                            fields.join(","),
                            right
                        )
                    }
                })
                .collect::<Vec<_>>();
            format!(
                "match self.tag {{
                        {}
                        _ => unreachable!(),
                    }}",
                variants.join("\n"),
            )
        } else {
            let variants = enu
                .variants()
                .iter()
                .enumerate()
                .map(|(idx, variant)| format!("{} => {}::{},", idx, enu.name, variant.name))
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "match self {{
                        {}
                        _ => unreachable!(\"Invalid variant for {}: {{}}\", self),
                    }}",
                variants, enu.name
            )
        })
    }

    fn structs(&self) -> String {
        let src = self.ir.get(self.context.ir_file);
        if !src.is_struct() {
            return "".to_owned();
        }
        let variant_structs = src
            .variants()
            .iter()
            .map(|variant| {
                let fields = match &variant.kind {
                    IrVariantKind::Value => vec![],
                    IrVariantKind::Struct(s) => s
                        .fields
                        .iter()
                        .map(|field| {
                            format!(
                                "{}: {}{},",
                                field.name.rust_style(),
                                field.ty.rust_wire_modifier(),
                                field.ty.rust_wire_type()
                            )
                        })
                        .collect(),
                };
                format!(
                    "#[repr(C)]
                    #[derive(Clone)]
                    pub struct {}_{} {{ {} }}",
                    self.ir.name,
                    variant.name,
                    fields.join("\n")
                )
            })
            .collect::<Vec<_>>();
        let union_fields = src
            .variants()
            .iter()
            .map(|variant| format!("{0}: *mut {1}_{0},", variant.name, self.ir.name))
            .collect::<Vec<_>>();
        format!(
            "#[repr(C)]
            #[derive(Clone)]
            pub struct {0} {{ tag: i32, kind: *mut {1}Kind }}

            #[repr(C)]
            pub union {1}Kind {{
                {2}
            }}

            {3}",
            self.ir.rust_wire_type(),
            self.ir.name,
            union_fields.join("\n"),
            variant_structs.join("\n\n")
        )
    }

    fn static_checks(&self) -> Option<String> {
        let src = self.ir.get(self.context.ir_file);
        src.wrapper_name.as_ref()?;

        let branches: Vec<_> = src
            .variants()
            .iter()
            .map(|variant| match &variant.kind {
                IrVariantKind::Value => format!("{}::{} => {{}}", src.name, variant.name),
                IrVariantKind::Struct(s) => {
                    let pattern = s
                        .fields
                        .iter()
                        .map(|field| field.name.rust_style().to_owned())
                        .collect::<Vec<_>>();
                    let pattern = if s.is_fields_named {
                        format!("{}::{} {{ {} }}", src.name, variant.name, pattern.join(","))
                    } else {
                        format!("{}::{}({})", src.name, variant.name, pattern.join(","))
                    };
                    let checks = s
                        .fields
                        .iter()
                        .map(|field| {
                            format!(
                                "let _: {} = {};\n",
                                field.ty.rust_api_type(),
                                field.name.rust_style(),
                            )
                        })
                        .collect::<Vec<_>>();
                    format!("{} => {{ {} }}", pattern, checks.join(""))
                }
            })
            .collect();
        Some(format!(
            "match None::<{}>.unwrap() {{ {} }}",
            src.name,
            branches.join(","),
        ))
    }

    fn wrapper_struct(&self) -> Option<String> {
        let src = self.ir.get(self.context.ir_file);
        src.wrapper_name.as_ref().cloned()
    }

    fn self_access(&self, obj: String) -> String {
        let src = self.ir.get(self.context.ir_file);
        match &src.wrapper_name {
            Some(_) => format!("{}.0", obj),
            None => obj,
        }
    }

    fn wrap_obj(&self, obj: String) -> String {
        match self.wrapper_struct() {
            Some(wrapper) => format!("{}({})", wrapper, obj),
            None => obj,
        }
    }

    fn impl_intodart(&self) -> String {
        let src = self.ir.get(self.context.ir_file);

        let (name, self_path): (&str, &str) = match &src.wrapper_name {
            Some(wrapper) => (wrapper, &src.name),
            None => (&src.name, "Self"),
        };
        let self_ref = self.self_access("self".to_owned());
        if self.ir.is_struct {
            let variants = src
                .variants()
                .iter()
                .enumerate()
                .map(|(idx, variant)| {
                    let tag = format!("{}.into_dart()", idx);
                    match &variant.kind {
                        IrVariantKind::Value => {
                            format!("{}::{} => vec![{}],", self_path, variant.name, tag)
                        }
                        IrVariantKind::Struct(s) => {
                            let fields = Some(tag)
                                .into_iter()
                                .chain(s.fields.iter().map(|field| {
                                    let gen = TypeRustGenerator::new(
                                        field.ty.clone(),
                                        self.context.ir_file,
                                    );
                                    gen.convert_to_dart(field.name.rust_style().to_owned())
                                }))
                                .collect::<Vec<_>>();
                            let pattern = s
                                .fields
                                .iter()
                                .map(|field| field.name.rust_style().to_owned())
                                .collect::<Vec<_>>();
                            let (left, right) = s.brackets_pair();
                            format!(
                                "{}::{}{}{}{} => vec![{}],",
                                self_path,
                                variant.name,
                                left,
                                pattern.join(","),
                                right,
                                fields.join(",")
                            )
                        }
                    }
                })
                .collect::<Vec<_>>();
            format!(
                "impl support::IntoDart for {} {{
                    fn into_dart(self) -> support::DartCObject {{
                        match {} {{
                            {}
                        }}.into_dart()
                    }}
                }}
                impl support::IntoDartExceptPrimitive for {0} {{}}
                ",
                name,
                self_ref,
                variants.join("\n")
            )
        } else {
            let variants = src
                .variants()
                .iter()
                .enumerate()
                .map(|(idx, variant)| format!("{}::{} => {},", self_path, variant.name, idx))
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "impl support::IntoDart for {} {{
                fn into_dart(self) -> support::DartCObject {{
                    match {} {{
                        {}
                    }}.into_dart()
                }}
            }}
            ",
                name, self_ref, variants
            )
        }
    }

    fn new_with_nullptr(&self, collector: &mut ExternFuncCollector) -> String {
        if !self.ir.is_struct {
            return "".to_string();
        }

        fn init_of(ty: &IrType) -> &str {
            if ty.rust_wire_is_pointer() {
                "core::ptr::null_mut()"
            } else {
                "Default::default()"
            }
        }

        let src = self.ir.get(self.context.ir_file);

        let inflators = src
            .variants()
            .iter()
            .filter_map(|variant| {
                let typ = format!("{}_{}", self.ir.name, variant.name);
                let body: Vec<_> = if let IrVariantKind::Struct(st) = &variant.kind {
                    st.fields
                        .iter()
                        .map(|field| format!("{}: {}", field.name.rust_style(), init_of(&field.ty)))
                        .collect()
                } else {
                    return None;
                };
                Some(collector.generate(
                    &format!("inflate_{}", typ),
                    &[],
                    Some(&format!("*mut {}Kind", self.ir.name)),
                    &format!(
                        "support::new_leak_box_ptr({}Kind {{
                        {}: support::new_leak_box_ptr({} {{
                            {}
                        }})
                    }})",
                        self.ir.name,
                        variant.name.rust_style(),
                        typ,
                        body.join(",")
                    ),
                ))
            })
            .collect::<Vec<_>>();
        format!(
            "impl NewWithNullPtr for {} {{
                fn new_with_null_ptr() -> Self {{
                    Self {{
                        tag: -1,
                        kind: core::ptr::null_mut(),
                    }}
                }}
            }}
            {}",
            self.ir.rust_wire_type(),
            inflators.join("\n\n")
        )
    }

    fn imports(&self) -> Option<String> {
        let api_enum = self.ir.get(self.context.ir_file);
        Some(format!("use {};", api_enum.path.join("::")))
    }
}
