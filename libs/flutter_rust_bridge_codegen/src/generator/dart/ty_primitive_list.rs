use crate::generator::dart::gen_wire2api_simple_type_cast;
use crate::generator::dart::ty::*;
use crate::ir::*;
use crate::type_dart_generator_struct;

type_dart_generator_struct!(TypePrimitiveListGenerator, IrTypePrimitiveList);

impl TypeDartGeneratorTrait for TypePrimitiveListGenerator<'_> {
    fn api2wire_body(&self) -> Option<String> {
        // NOTE Dart code *only* allocates memory. It never *release* memory by itself.
        // Instead, Rust receives that pointer and now it is in control of Rust.
        // Therefore, *never* continue to use this pointer after you have passed the pointer
        // to Rust.
        // NOTE WARN: Never use the [calloc] provided by Dart FFI to allocate any memory.
        // Instead, ask Rust to allocate some memory and return raw pointers. Otherwise,
        // memory will be allocated in one dylib (e.g. libflutter.so), and then be released
        // by another dylib (e.g. my_rust_code.so), especially in Android platform. It can be
        // undefined behavior.
        Some(format!(
            "final ans = inner.new_{}(raw.length);
                ans.ref.ptr.asTypedList(raw.length).setAll(0, raw);
                return ans;",
            self.ir.safe_ident(),
        ))
    }

    fn wire2api_body(&self) -> String {
        gen_wire2api_simple_type_cast(&self.ir.dart_api_type())
    }
}
