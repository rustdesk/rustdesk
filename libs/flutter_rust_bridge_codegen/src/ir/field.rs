use crate::ir::*;

#[derive(Debug, Clone)]
pub struct IrField {
    pub ty: IrType,
    pub name: IrIdent,
    pub is_final: bool,
    pub comments: Vec<IrComment>,
}
