use crate::generator::rust::*;
use enum_dispatch::enum_dispatch;

#[enum_dispatch]
pub trait TypeRustGeneratorTrait {
    fn wire2api_body(&self) -> Option<String>;

    fn wire_struct_fields(&self) -> Option<Vec<String>> {
        None
    }

    fn static_checks(&self) -> Option<String> {
        None
    }

    fn wrapper_struct(&self) -> Option<String> {
        None
    }

    fn self_access(&self, obj: String) -> String {
        obj
    }

    fn wrap_obj(&self, obj: String) -> String {
        obj
    }

    fn convert_to_dart(&self, obj: String) -> String {
        format!("{}.into_dart()", obj)
    }

    fn structs(&self) -> String {
        "".to_string()
    }

    fn allocate_funcs(&self, _collector: &mut ExternFuncCollector) -> String {
        "".to_string()
    }

    fn impl_intodart(&self) -> String {
        "".to_string()
    }

    fn new_with_nullptr(&self, _collector: &mut ExternFuncCollector) -> String {
        "".to_string()
    }

    fn imports(&self) -> Option<String> {
        None
    }
}

#[derive(Debug, Clone)]
pub struct TypeGeneratorContext<'a> {
    pub ir_file: &'a IrFile,
}

#[macro_export]
macro_rules! type_rust_generator_struct {
    ($cls:ident, $ir_cls:ty) => {
        #[derive(Debug, Clone)]
        pub struct $cls<'a> {
            pub ir: $ir_cls,
            pub context: TypeGeneratorContext<'a>,
        }
    };
}

#[enum_dispatch(TypeRustGeneratorTrait)]
#[derive(Debug, Clone)]
pub enum TypeRustGenerator<'a> {
    Primitive(TypePrimitiveGenerator<'a>),
    Delegate(TypeDelegateGenerator<'a>),
    PrimitiveList(TypePrimitiveListGenerator<'a>),
    Optional(TypeOptionalGenerator<'a>),
    GeneralList(TypeGeneralListGenerator<'a>),
    StructRef(TypeStructRefGenerator<'a>),
    Boxed(TypeBoxedGenerator<'a>),
    EnumRef(TypeEnumRefGenerator<'a>),
}

impl<'a> TypeRustGenerator<'a> {
    pub fn new(ty: IrType, ir_file: &'a IrFile) -> Self {
        let context = TypeGeneratorContext { ir_file };
        match ty {
            Primitive(ir) => TypePrimitiveGenerator { ir, context }.into(),
            Delegate(ir) => TypeDelegateGenerator { ir, context }.into(),
            PrimitiveList(ir) => TypePrimitiveListGenerator { ir, context }.into(),
            Optional(ir) => TypeOptionalGenerator { ir, context }.into(),
            GeneralList(ir) => TypeGeneralListGenerator { ir, context }.into(),
            StructRef(ir) => TypeStructRefGenerator { ir, context }.into(),
            Boxed(ir) => TypeBoxedGenerator { ir, context }.into(),
            EnumRef(ir) => TypeEnumRefGenerator { ir, context }.into(),
        }
    }
}
