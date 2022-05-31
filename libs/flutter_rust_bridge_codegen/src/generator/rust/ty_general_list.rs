use crate::generator::rust::ty::*;
use crate::generator::rust::{generate_import, generate_list_allocate_func, ExternFuncCollector};
use crate::ir::*;
use crate::type_rust_generator_struct;

type_rust_generator_struct!(TypeGeneralListGenerator, IrTypeGeneralList);

impl TypeGeneralListGenerator<'_> {
    pub const WIRE2API_BODY: &'static str = "
            let vec = unsafe {
                let wrap = support::box_from_leak_ptr(self);
                support::vec_from_leak_ptr(wrap.ptr, wrap.len)
            };
            vec.into_iter().map(Wire2Api::wire2api).collect()";
}

impl TypeRustGeneratorTrait for TypeGeneralListGenerator<'_> {
    fn wire2api_body(&self) -> Option<String> {
        Some(TypeGeneralListGenerator::WIRE2API_BODY.to_string())
    }

    fn wire_struct_fields(&self) -> Option<Vec<String>> {
        Some(vec![
            format!(
                "ptr: *mut {}{}",
                self.ir.inner.rust_ptr_modifier(),
                self.ir.inner.rust_wire_type()
            ),
            "len: i32".to_string(),
        ])
    }

    fn wrap_obj(&self, obj: String) -> String {
        let inner = TypeRustGenerator::new(*self.ir.inner.clone(), self.context.ir_file);
        inner
            .wrapper_struct()
            .map(|wrapper| {
                format!(
                    "{}.into_iter().map(|v| {}({})).collect::<Vec<_>>()",
                    obj,
                    wrapper,
                    inner.self_access("v".to_owned())
                )
            })
            .unwrap_or(obj)
    }

    fn allocate_funcs(&self, collector: &mut ExternFuncCollector) -> String {
        generate_list_allocate_func(collector, &self.ir.safe_ident(), &self.ir, &self.ir.inner)
    }

    fn imports(&self) -> Option<String> {
        generate_import(&self.ir.inner, self.context.ir_file)
    }
}
