use crate::ir::*;
use convert_case::{Case, Casing};

#[derive(Debug, Clone)]
pub struct IrTypeStructRef {
    pub name: String,
    pub freezed: bool,
}

impl IrTypeStructRef {
    pub fn get<'a>(&self, f: &'a IrFile) -> &'a IrStruct {
        &f.struct_pool[&self.name]
    }
}

impl IrTypeTrait for IrTypeStructRef {
    fn visit_children_types<F: FnMut(&IrType) -> bool>(&self, f: &mut F, ir_file: &IrFile) {
        for field in &self.get(ir_file).fields {
            field.ty.visit_types(f, ir_file);
        }
    }

    fn safe_ident(&self) -> String {
        self.dart_api_type().to_case(Case::Snake)
    }
    fn dart_api_type(&self) -> String {
        self.name.to_string()
    }

    fn dart_wire_type(&self) -> String {
        self.rust_wire_type()
    }

    fn rust_api_type(&self) -> String {
        self.name.to_string()
    }

    fn rust_wire_type(&self) -> String {
        format!("wire_{}", self.name)
    }
}

#[derive(Debug, Clone)]
pub struct IrStruct {
    pub name: String,
    pub wrapper_name: Option<String>,
    pub path: Option<Vec<String>>,
    pub fields: Vec<IrField>,
    pub is_fields_named: bool,
    pub dart_metadata: Vec<IrDartAnnotation>,
    pub comments: Vec<IrComment>,
}

impl IrStruct {
    pub fn brackets_pair(&self) -> (char, char) {
        if self.is_fields_named {
            ('{', '}')
        } else {
            ('(', ')')
        }
    }

    pub fn using_freezed(&self) -> bool {
        self.dart_metadata.iter().any(|it| it.content == "freezed")
    }
}
