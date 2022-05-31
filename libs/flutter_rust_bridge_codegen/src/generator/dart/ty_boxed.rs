use crate::generator::dart::gen_wire2api_simple_type_cast;
use crate::generator::dart::ty::*;
use crate::ir::IrType::{EnumRef, Primitive, StructRef};
use crate::ir::*;
use crate::type_dart_generator_struct;

type_dart_generator_struct!(TypeBoxedGenerator, IrTypeBoxed);

impl TypeDartGeneratorTrait for TypeBoxedGenerator<'_> {
    fn api2wire_body(&self) -> Option<String> {
        Some(match &*self.ir.inner {
            Primitive(_) => {
                format!("return inner.new_{}(raw);", self.ir.safe_ident())
            }
            inner => {
                format!(
                    "final ptr = inner.new_{}();
                    _api_fill_to_wire_{}(raw, ptr.ref);
                    return ptr;",
                    self.ir.safe_ident(),
                    inner.safe_ident(),
                )
            }
        })
    }

    fn api_fill_to_wire_body(&self) -> Option<String> {
        if !matches!(*self.ir.inner, Primitive(_)) {
            Some(format!(
                " _api_fill_to_wire_{}(apiObj, wireObj.ref);",
                self.ir.inner.safe_ident()
            ))
        } else {
            None
        }
    }

    fn wire2api_body(&self) -> String {
        match &*self.ir.inner {
            StructRef(inner) => format!("return _wire2api_{}(raw);", inner.safe_ident()),
            EnumRef(inner) => format!("return _wire2api_{}(raw);", inner.safe_ident()),
            _ => gen_wire2api_simple_type_cast(&self.ir.dart_api_type()),
        }
    }
}
