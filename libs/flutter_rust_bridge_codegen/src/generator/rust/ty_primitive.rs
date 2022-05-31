use crate::generator::rust::ty::*;
use crate::ir::*;
use crate::type_rust_generator_struct;

type_rust_generator_struct!(TypePrimitiveGenerator, IrTypePrimitive);

impl TypeRustGeneratorTrait for TypePrimitiveGenerator<'_> {
    fn wire2api_body(&self) -> Option<String> {
        Some("self".into())
    }
}
