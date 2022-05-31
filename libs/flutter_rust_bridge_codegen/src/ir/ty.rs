use crate::ir::*;
use enum_dispatch::enum_dispatch;
use IrType::*;

/// Remark: "Ty" instead of "Type", since "type" is a reserved word in Rust.
#[enum_dispatch(IrTypeTrait)]
#[derive(Debug, Clone)]
pub enum IrType {
    Primitive(IrTypePrimitive),
    Delegate(IrTypeDelegate),
    PrimitiveList(IrTypePrimitiveList),
    Optional(IrTypeOptional),
    GeneralList(IrTypeGeneralList),
    StructRef(IrTypeStructRef),
    Boxed(IrTypeBoxed),
    EnumRef(IrTypeEnumRef),
}

impl IrType {
    pub fn visit_types<F: FnMut(&IrType) -> bool>(&self, f: &mut F, ir_file: &IrFile) {
        if f(self) {
            return;
        }

        self.visit_children_types(f, ir_file);
    }

    #[inline]
    pub fn dart_required_modifier(&self) -> &'static str {
        match self {
            Optional(_) => "",
            _ => "required ",
        }
    }

    /// Additional indirection for types put behind a vector
    #[inline]
    pub fn rust_ptr_modifier(&self) -> &'static str {
        match self {
            Optional(_) | Delegate(IrTypeDelegate::String) => "*mut ",
            _ => "",
        }
    }
}

#[enum_dispatch]
pub trait IrTypeTrait {
    fn visit_children_types<F: FnMut(&IrType) -> bool>(&self, f: &mut F, ir_file: &IrFile);

    fn safe_ident(&self) -> String;

    fn dart_api_type(&self) -> String;

    fn dart_wire_type(&self) -> String;

    fn rust_api_type(&self) -> String;

    fn rust_wire_type(&self) -> String;

    fn rust_wire_modifier(&self) -> String {
        if self.rust_wire_is_pointer() {
            "*mut ".to_string()
        } else {
            "".to_string()
        }
    }

    fn rust_wire_is_pointer(&self) -> bool {
        false
    }
}

pub fn optional_boundary_index(types: &[&IrType]) -> Option<usize> {
    types
        .iter()
        .enumerate()
        .find(|ty| matches!(ty.1, Optional(_)))
        .and_then(|(idx, _)| {
            (&types[idx..])
                .iter()
                .all(|ty| matches!(ty, Optional(_)))
                .then(|| idx)
        })
}
