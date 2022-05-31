use crate::generator::dart::ty::*;
use crate::generator::dart::{dart_comments, dart_metadata};
use crate::ir::*;
use crate::type_dart_generator_struct;

type_dart_generator_struct!(TypeStructRefGenerator, IrTypeStructRef);

impl TypeDartGeneratorTrait for TypeStructRefGenerator<'_> {
    fn api2wire_body(&self) -> Option<String> {
        None
    }

    fn api_fill_to_wire_body(&self) -> Option<String> {
        let s = self.ir.get(self.context.ir_file);
        Some(
            s.fields
                .iter()
                .map(|field| {
                    format!(
                        "wireObj.{} = _api2wire_{}(apiObj.{});",
                        field.name.rust_style(),
                        field.ty.safe_ident(),
                        field.name.dart_style()
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }

    fn wire2api_body(&self) -> String {
        let s = self.ir.get(self.context.ir_file);
        let inner = s
            .fields
            .iter()
            .enumerate()
            .map(|(idx, field)| {
                format!(
                    "{}: _wire2api_{}(arr[{}]),",
                    field.name.dart_style(),
                    field.ty.safe_ident(),
                    idx
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "final arr = raw as List<dynamic>;
                if (arr.length != {}) throw Exception('unexpected arr length: expect {} but see ${{arr.length}}');
                return {}({});",
            s.fields.len(),
            s.fields.len(),
            s.name, inner,
        )
    }

    fn structs(&self) -> String {
        let src = self.ir.get(self.context.ir_file);
        let comments = dart_comments(&src.comments);
        let metadata = dart_metadata(&src.dart_metadata);

        if src.using_freezed() {
            let constructor_params = src
                .fields
                .iter()
                .map(|f| {
                    format!(
                        "{} {} {},",
                        f.ty.dart_required_modifier(),
                        f.ty.dart_api_type(),
                        f.name.dart_style()
                    )
                })
                .collect::<Vec<_>>()
                .join("");

            format!(
                "{}{}class {} with _${} {{
                const factory {}({{{}}}) = _{};
            }}",
                comments,
                metadata,
                self.ir.name,
                self.ir.name,
                self.ir.name,
                constructor_params,
                self.ir.name
            )
        } else {
            let field_declarations = src
                .fields
                .iter()
                .map(|f| {
                    let comments = dart_comments(&f.comments);
                    format!(
                        "{}{} {} {};",
                        comments,
                        if f.is_final { "final" } else { "" },
                        f.ty.dart_api_type(),
                        f.name.dart_style()
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");

            let constructor_params = src
                .fields
                .iter()
                .map(|f| {
                    format!(
                        "{}this.{},",
                        f.ty.dart_required_modifier(),
                        f.name.dart_style()
                    )
                })
                .collect::<Vec<_>>()
                .join("");

            format!(
                "{}{}class {} {{
                {}

                {}({{{}}});
            }}",
                comments,
                metadata,
                self.ir.name,
                field_declarations,
                self.ir.name,
                constructor_params
            )
        }
    }
}
