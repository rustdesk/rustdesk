use crate::ir::*;

#[derive(Debug, Clone)]
pub enum IrTypePrimitive {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
    F32,
    F64,
    Bool,
    Unit,
    Usize,
}

impl IrTypeTrait for IrTypePrimitive {
    fn visit_children_types<F: FnMut(&IrType) -> bool>(&self, _f: &mut F, _ir_file: &IrFile) {}

    fn safe_ident(&self) -> String {
        self.rust_api_type()
    }

    fn dart_api_type(&self) -> String {
        match self {
            IrTypePrimitive::U8
            | IrTypePrimitive::I8
            | IrTypePrimitive::U16
            | IrTypePrimitive::I16
            | IrTypePrimitive::U32
            | IrTypePrimitive::I32
            | IrTypePrimitive::U64
            | IrTypePrimitive::I64
            | IrTypePrimitive::Usize => "int",
            IrTypePrimitive::F32 | IrTypePrimitive::F64 => "double",
            IrTypePrimitive::Bool => "bool",
            IrTypePrimitive::Unit => "void",
        }
        .to_string()
    }

    fn dart_wire_type(&self) -> String {
        match self {
            IrTypePrimitive::Bool => "int".to_owned(),
            _ => self.dart_api_type(),
        }
    }

    fn rust_api_type(&self) -> String {
        self.rust_wire_type()
    }

    fn rust_wire_type(&self) -> String {
        match self {
            IrTypePrimitive::U8 => "u8",
            IrTypePrimitive::I8 => "i8",
            IrTypePrimitive::U16 => "u16",
            IrTypePrimitive::I16 => "i16",
            IrTypePrimitive::U32 => "u32",
            IrTypePrimitive::I32 => "i32",
            IrTypePrimitive::U64 => "u64",
            IrTypePrimitive::I64 => "i64",
            IrTypePrimitive::F32 => "f32",
            IrTypePrimitive::F64 => "f64",
            IrTypePrimitive::Bool => "bool",
            IrTypePrimitive::Unit => "unit",
            IrTypePrimitive::Usize => "usize",
        }
        .to_string()
    }
}

impl IrTypePrimitive {
    /// Representations of primitives within Dart's pointers, e.g. `ffi.Pointer<ffi.Uint8>`.
    /// This is enforced on Dart's side, and should be used instead of `dart_wire_type`
    /// whenever primitives are put behind a pointer.
    pub fn dart_native_type(&self) -> &'static str {
        match self {
            IrTypePrimitive::U8 | IrTypePrimitive::Bool => "ffi.Uint8",
            IrTypePrimitive::I8 => "ffi.Int8",
            IrTypePrimitive::U16 => "ffi.Uint16",
            IrTypePrimitive::I16 => "ffi.Int16",
            IrTypePrimitive::U32 => "ffi.Uint32",
            IrTypePrimitive::I32 => "ffi.Int32",
            IrTypePrimitive::U64 => "ffi.Uint64",
            IrTypePrimitive::I64 => "ffi.Int64",
            IrTypePrimitive::F32 => "ffi.Float",
            IrTypePrimitive::F64 => "ffi.Double",
            IrTypePrimitive::Unit => "ffi.Void",
            IrTypePrimitive::Usize => "ffi.Usize",
        }
    }
    pub fn try_from_rust_str(s: &str) -> Option<Self> {
        match s {
            "u8" => Some(IrTypePrimitive::U8),
            "i8" => Some(IrTypePrimitive::I8),
            "u16" => Some(IrTypePrimitive::U16),
            "i16" => Some(IrTypePrimitive::I16),
            "u32" => Some(IrTypePrimitive::U32),
            "i32" => Some(IrTypePrimitive::I32),
            "u64" => Some(IrTypePrimitive::U64),
            "i64" => Some(IrTypePrimitive::I64),
            "f32" => Some(IrTypePrimitive::F32),
            "f64" => Some(IrTypePrimitive::F64),
            "bool" => Some(IrTypePrimitive::Bool),
            "()" => Some(IrTypePrimitive::Unit),
            "usize" => Some(IrTypePrimitive::Usize),
            _ => None,
        }
    }
}
