use crate::model::col_label_to_index;

#[derive(Debug, Clone, PartialEq)]
pub struct CellRef {
    pub col: usize,
    pub row: usize,
    pub abs_col: bool,
    pub abs_row: bool,
}

impl CellRef {
    /// Create from display-style strings (col="A", row="1") where row is 1-indexed.
    pub fn from_display(col: &str, row: &str, abs_col: bool, abs_row: bool) -> Option<Self> {
        let col_idx = col_label_to_index(col)?;
        let row_idx: usize = row.parse::<usize>().ok()?.checked_sub(1)?;
        Some(CellRef {
            col: col_idx,
            row: row_idx,
            abs_col,
            abs_row,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Gt,
    Gte,
    Lt,
    Lte,
    Eq,
    Neq,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    Text(String),
    Bool(bool),
    CellRef(CellRef),
    Range {
        start: CellRef,
        end: CellRef,
    },
    BinaryOp {
        op: Op,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    UnaryNeg(Box<Expr>),
    FnCall {
        name: String,
        args: Vec<Expr>,
    },
}
