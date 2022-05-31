use crate::generator::rust::ty::*;
use crate::generator::rust::{generate_import, ExternFuncCollector};
use crate::ir::IrType::Primitive;
use crate::ir::*;
use crate::type_rust_generator_struct;

type_rust_generator_struct!(TypeBoxedGenerator, IrTypeBoxed);

impl TypeRustGeneratorTrait for TypeBoxedGenerator<'_> {
    fn wire2api_body(&self) -> Option<String> {
        let IrTypeBoxed {
            inner: box_inner,
            exist_in_real_api,
        } = &self.ir;
        Some(match (box_inner.as_ref(), exist_in_real_api) {
            (IrType::Primitive(_), false) => "unsafe { *support::box_from_leak_ptr(self) }".into(),
            (IrType::Primitive(_), true) => "unsafe { support::box_from_leak_ptr(self) }".into(),
            _ => {
                "let wrap = unsafe { support::box_from_leak_ptr(self) }; (*wrap).wire2api().into()"
                    .into()
            }
        })
    }

    fn wrapper_struct(&self) -> Option<String> {
        let src = TypeRustGenerator::new(*self.ir.inner.clone(), self.context.ir_file);
        src.wrapper_struct()
    }

    fn self_access(&self, obj: String) -> String {
        format!("(*{})", obj)
    }

    fn wrap_obj(&self, obj: String) -> String {
        let src = TypeRustGenerator::new(*self.ir.inner.clone(), self.context.ir_file);
        src.wrap_obj(self.self_access(obj))
    }

    fn allocate_funcs(&self, collector: &mut ExternFuncCollector) -> String {
        match &*self.ir.inner {
            Primitive(prim) => collector.generate(
                &format!("new_{}", self.ir.safe_ident()),
                &[&format!("value: {}", prim.rust_wire_type())],
                Some(&format!("*mut {}", prim.rust_wire_type())),
                "support::new_leak_box_ptr(value)",
            ),
            inner => collector.generate(
                &format!("new_{}", self.ir.safe_ident()),
                &[],
                Some(&[self.ir.rust_wire_modifier(), self.ir.rust_wire_type()].concat()),
                &format!(
                    "support::new_leak_box_ptr({}::new_with_null_ptr())",
                    inner.rust_wire_type()
                ),
            ),
        }
    }

    fn imports(&self) -> Option<String> {
        generate_import(&self.ir.inner, self.context.ir_file)
    }
}
