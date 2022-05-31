use crate::generator::rust::ty::*;
use crate::generator::rust::ExternFuncCollector;
use crate::ir::*;
use crate::type_rust_generator_struct;

type_rust_generator_struct!(TypePrimitiveListGenerator, IrTypePrimitiveList);

impl TypeRustGeneratorTrait for TypePrimitiveListGenerator<'_> {
    fn wire2api_body(&self) -> Option<String> {
        Some(
            "unsafe {
            let wrap = support::box_from_leak_ptr(self);
            support::vec_from_leak_ptr(wrap.ptr, wrap.len)
        }"
            .into(),
        )
    }

    fn wire_struct_fields(&self) -> Option<Vec<String>> {
        Some(vec![
            format!("ptr: *mut {}", self.ir.primitive.rust_wire_type()),
            "len: i32".to_string(),
        ])
    }

    fn allocate_funcs(&self, collector: &mut ExternFuncCollector) -> String {
        collector.generate(
            &format!("new_{}", self.ir.safe_ident()),
            &["len: i32"],
            Some(&format!(
                "{}{}",
                self.ir.rust_wire_modifier(),
                self.ir.rust_wire_type()
            )),
            &format!(
                "let ans = {} {{ ptr: support::new_leak_vec_ptr(Default::default(), len), len }};
                support::new_leak_box_ptr(ans)",
                self.ir.rust_wire_type(),
            ),
        )
    }
}
