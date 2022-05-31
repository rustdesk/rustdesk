use crate::generator::rust::generate_import;
use crate::generator::rust::ty::*;
use crate::ir::*;
use crate::type_rust_generator_struct;

type_rust_generator_struct!(TypeOptionalGenerator, IrTypeOptional);

impl TypeRustGeneratorTrait for TypeOptionalGenerator<'_> {
    fn wire2api_body(&self) -> Option<String> {
        None
    }

    fn convert_to_dart(&self, obj: String) -> String {
        let inner = TypeRustGenerator::new(*self.ir.inner.clone(), self.context.ir_file);
        let obj = match inner.wrapper_struct() {
            Some(wrapper) => format!(
                "{}.map(|v| {}({}))",
                obj,
                wrapper,
                inner.self_access("v".to_owned())
            ),
            None => obj,
        };
        format!("{}.into_dart()", obj)
    }

    fn imports(&self) -> Option<String> {
        generate_import(&self.ir.inner, self.context.ir_file)
    }
}
