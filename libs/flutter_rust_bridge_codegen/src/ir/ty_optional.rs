use crate::ir::IrType::*;
use crate::ir::*;

#[derive(Debug, Clone)]
pub struct IrTypeOptional {
    pub inner: Box<IrType>,
}

impl IrTypeOptional {
    pub fn new_prim(prim: IrTypePrimitive) -> Self {
        Self {
            inner: Box::new(Boxed(IrTypeBoxed {
                inner: Box::new(Primitive(prim)),
                exist_in_real_api: false,
            })),
        }
    }

    pub fn new_ptr(ptr: IrType) -> Self {
        Self {
            inner: Box::new(ptr),
        }
    }

    pub fn is_primitive(&self) -> bool {
        matches!(&*self.inner, Boxed(boxed) if matches!(*boxed.inner, IrType::Primitive(_)))
    }

    pub fn is_list(&self) -> bool {
        matches!(&*self.inner, GeneralList(_) | PrimitiveList(_))
    }

    pub fn is_delegate(&self) -> bool {
        matches!(&*self.inner, Delegate(_))
    }

    pub fn needs_initialization(&self) -> bool {
        !(self.is_primitive() || self.is_delegate())
    }
}

impl IrTypeTrait for IrTypeOptional {
    fn safe_ident(&self) -> String {
        format!("opt_{}", self.inner.safe_ident())
    }
    fn rust_wire_type(&self) -> String {
        self.inner.rust_wire_type()
    }
    fn rust_api_type(&self) -> String {
        format!("Option<{}>", self.inner.rust_api_type())
    }
    fn dart_wire_type(&self) -> String {
        self.inner.dart_wire_type()
    }
    fn dart_api_type(&self) -> String {
        format!("{}?", self.inner.dart_api_type())
    }
    fn rust_wire_is_pointer(&self) -> bool {
        true
    }

    fn visit_children_types<F: FnMut(&IrType) -> bool>(&self, f: &mut F, ir_file: &IrFile) {
        self.inner.visit_types(f, ir_file);
    }
}
