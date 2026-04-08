#[derive(Debug, Clone)]
pub enum Register {
    Cell(String),
    Row(Vec<String>),
    Block(Vec<Vec<String>>),
}
