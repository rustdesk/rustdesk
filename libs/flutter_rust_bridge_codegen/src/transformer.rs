use log::debug;

use crate::ir::IrType::*;
use crate::ir::*;

pub fn transform(src: IrFile) -> IrFile {
    let dst_funcs = src
        .funcs
        .into_iter()
        .map(|src_func| IrFunc {
            inputs: src_func
                .inputs
                .into_iter()
                .map(transform_func_input_add_boxed)
                .collect(),
            ..src_func
        })
        .collect();

    IrFile {
        funcs: dst_funcs,
        ..src
    }
}

fn transform_func_input_add_boxed(input: IrField) -> IrField {
    match &input.ty {
        StructRef(_)
        | EnumRef(IrTypeEnumRef {
            is_struct: true, ..
        }) => {
            debug!(
                "transform_func_input_add_boxed wrap Boxed to field={:?}",
                input
            );
            IrField {
                ty: Boxed(IrTypeBoxed {
                    exist_in_real_api: false, // <--
                    inner: Box::new(input.ty.clone()),
                }),
                ..input
            }
        }
        _ => input,
    }
}
