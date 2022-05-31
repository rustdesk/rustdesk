#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct IrDartImport {
    pub uri: String,
    pub alias: Option<String>,
}
