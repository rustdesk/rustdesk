use crate::ir::*;

#[derive(Debug, Clone)]
pub struct IrDartAnnotation {
    pub content: String,
    pub library: Option<IrDartImport>,
}
