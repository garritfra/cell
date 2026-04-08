use std::collections::HashMap;
use std::fmt;

pub type CellPos = (usize, usize);

#[derive(Debug, Clone, PartialEq)]
pub enum CellError {
    DivZero,
    Value,
    Ref,
    Circ,
    Name,
    Parse,
}

impl fmt::Display for CellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CellError::DivZero => write!(f, "#DIV/0!"),
            CellError::Value => write!(f, "#VALUE!"),
            CellError::Ref => write!(f, "#REF!"),
            CellError::Circ => write!(f, "#CIRC!"),
            CellError::Name => write!(f, "#NAME?"),
            CellError::Parse => write!(f, "#PARSE!"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CellValue {
    Number(f64),
    Text(String),
    Bool(bool),
    Error(CellError),
    Empty,
}

impl fmt::Display for CellValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CellValue::Number(n) => {
                if n.fract() == 0.0 {
                    write!(f, "{}", *n as i64)
                } else {
                    write!(f, "{}", n)
                }
            }
            CellValue::Text(s) => write!(f, "{}", s),
            CellValue::Bool(b) => write!(f, "{}", if *b { "TRUE" } else { "FALSE" }),
            CellValue::Error(e) => write!(f, "{}", e),
            CellValue::Empty => write!(f, ""),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Cell {
    pub raw: String,
    pub value: CellValue,
    pub dirty: bool,
}

pub struct Sheet {
    pub cells: HashMap<CellPos, Cell>,
    pub col_widths: Vec<u16>,
    pub row_count: usize,
    pub col_count: usize,
}

impl Default for Sheet {
    fn default() -> Self {
        Self::new()
    }
}

impl Sheet {
    pub fn new() -> Self {
        Sheet {
            cells: HashMap::new(),
            col_widths: Vec::new(),
            row_count: 0,
            col_count: 0,
        }
    }

    pub fn get_cell(&self, pos: CellPos) -> Option<&Cell> {
        self.cells.get(&pos)
    }

    pub fn set_cell(&mut self, pos: CellPos, raw: &str) {
        let value = if raw.is_empty() {
            CellValue::Empty
        } else if raw.starts_with('=') {
            // Formula — will be evaluated by formula engine later.
            // For now, store as text.
            CellValue::Text(raw.to_string())
        } else if let Ok(n) = raw.parse::<f64>() {
            CellValue::Number(n)
        } else {
            CellValue::Text(raw.to_string())
        };

        self.cells.insert(pos, Cell {
            raw: raw.to_string(),
            value,
            dirty: raw.starts_with('='),
        });

        self.row_count = self.row_count.max(pos.0 + 1);
        self.col_count = self.col_count.max(pos.1 + 1);
    }

    pub fn clear_cell(&mut self, pos: CellPos) {
        self.cells.remove(&pos);
    }

    pub fn sort_by_column(&mut self, col: usize, ascending: bool) {
        if self.row_count == 0 { return; }

        let mut row_indices: Vec<usize> = (0..self.row_count).collect();

        row_indices.sort_by(|&a, &b| {
            let cell_a = self.get_cell((a, col));
            let cell_b = self.get_cell((b, col));

            let ord = match (cell_a.map(|c| &c.value), cell_b.map(|c| &c.value)) {
                (Some(CellValue::Number(na)), Some(CellValue::Number(nb))) => {
                    na.partial_cmp(nb).unwrap_or(std::cmp::Ordering::Equal)
                }
                (Some(va), Some(vb)) => va.to_string().cmp(&vb.to_string()),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            };

            if ascending { ord } else { ord.reverse() }
        });

        let mut new_cells = std::collections::HashMap::new();
        for (new_row, &old_row) in row_indices.iter().enumerate() {
            for c in 0..self.col_count {
                if let Some(cell) = self.cells.get(&(old_row, c)) {
                    new_cells.insert((new_row, c), cell.clone());
                }
            }
        }
        self.cells = new_cells;
    }
}

pub fn col_index_to_label(mut col: usize) -> String {
    let mut label = String::new();
    loop {
        label.insert(0, (b'A' + (col % 26) as u8) as char);
        if col < 26 {
            break;
        }
        col = col / 26 - 1;
    }
    label
}

pub fn col_label_to_index(label: &str) -> Option<usize> {
    let mut index = 0usize;
    for (i, c) in label.chars().enumerate() {
        if !c.is_ascii_uppercase() {
            return None;
        }
        if i > 0 {
            index = (index + 1) * 26;
        }
        index += (c as usize) - ('A' as usize);
    }
    Some(index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_value_display_number() {
        assert_eq!(CellValue::Number(42.0).to_string(), "42");
        assert_eq!(CellValue::Number(3.14).to_string(), "3.14");
        assert_eq!(CellValue::Number(0.0).to_string(), "0");
    }

    #[test]
    fn cell_value_display_text() {
        assert_eq!(CellValue::Text("hello".into()).to_string(), "hello");
    }

    #[test]
    fn cell_value_display_bool() {
        assert_eq!(CellValue::Bool(true).to_string(), "TRUE");
        assert_eq!(CellValue::Bool(false).to_string(), "FALSE");
    }

    #[test]
    fn cell_value_display_errors() {
        assert_eq!(CellValue::Error(CellError::DivZero).to_string(), "#DIV/0!");
        assert_eq!(CellValue::Error(CellError::Value).to_string(), "#VALUE!");
        assert_eq!(CellValue::Error(CellError::Ref).to_string(), "#REF!");
        assert_eq!(CellValue::Error(CellError::Circ).to_string(), "#CIRC!");
        assert_eq!(CellValue::Error(CellError::Name).to_string(), "#NAME?");
        assert_eq!(CellValue::Error(CellError::Parse).to_string(), "#PARSE!");
    }

    #[test]
    fn cell_value_display_empty() {
        assert_eq!(CellValue::Empty.to_string(), "");
    }

    #[test]
    fn sheet_new_is_empty() {
        let sheet = Sheet::new();
        assert_eq!(sheet.row_count, 0);
        assert_eq!(sheet.col_count, 0);
        assert!(sheet.cells.is_empty());
    }

    #[test]
    fn sheet_set_and_get_cell() {
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), "hello");
        let cell = sheet.get_cell((0, 0)).unwrap();
        assert_eq!(cell.raw, "hello");
        assert_eq!(cell.value, CellValue::Text("hello".into()));
    }

    #[test]
    fn sheet_set_cell_updates_extent() {
        let mut sheet = Sheet::new();
        sheet.set_cell((5, 3), "x");
        assert_eq!(sheet.row_count, 6);
        assert_eq!(sheet.col_count, 4);
    }

    #[test]
    fn sheet_set_cell_parses_number() {
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), "42");
        assert_eq!(sheet.get_cell((0, 0)).unwrap().value, CellValue::Number(42.0));
    }

    #[test]
    fn sheet_set_cell_parses_float() {
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), "3.14");
        assert_eq!(sheet.get_cell((0, 0)).unwrap().value, CellValue::Number(3.14));
    }

    #[test]
    fn sheet_get_cell_empty() {
        let sheet = Sheet::new();
        assert!(sheet.get_cell((0, 0)).is_none());
    }

    #[test]
    fn sheet_clear_cell() {
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), "hello");
        sheet.clear_cell((0, 0));
        assert!(sheet.get_cell((0, 0)).is_none());
    }

    #[test]
    fn col_index_to_label_single() {
        assert_eq!(col_index_to_label(0), "A");
        assert_eq!(col_index_to_label(25), "Z");
    }

    #[test]
    fn col_index_to_label_double() {
        assert_eq!(col_index_to_label(26), "AA");
        assert_eq!(col_index_to_label(27), "AB");
        assert_eq!(col_index_to_label(51), "AZ");
        assert_eq!(col_index_to_label(52), "BA");
    }

    #[test]
    fn col_label_to_index_roundtrip() {
        for i in 0..100 {
            assert_eq!(col_label_to_index(&col_index_to_label(i)).unwrap(), i);
        }
    }

    #[test]
    fn cell_value_number_display_no_trailing_zeros() {
        assert_eq!(CellValue::Number(1.0).to_string(), "1");
        assert_eq!(CellValue::Number(1.10).to_string(), "1.1");
        assert_eq!(CellValue::Number(1.123).to_string(), "1.123");
    }

    #[test]
    fn sheet_sort_by_column_ascending() {
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), "Charlie");
        sheet.set_cell((0, 1), "3");
        sheet.set_cell((1, 0), "Alice");
        sheet.set_cell((1, 1), "1");
        sheet.set_cell((2, 0), "Bob");
        sheet.set_cell((2, 1), "2");
        sheet.sort_by_column(0, true);
        assert_eq!(sheet.get_cell((0, 0)).unwrap().raw, "Alice");
        assert_eq!(sheet.get_cell((1, 0)).unwrap().raw, "Bob");
        assert_eq!(sheet.get_cell((2, 0)).unwrap().raw, "Charlie");
        assert_eq!(sheet.get_cell((0, 1)).unwrap().raw, "1");
        assert_eq!(sheet.get_cell((1, 1)).unwrap().raw, "2");
        assert_eq!(sheet.get_cell((2, 1)).unwrap().raw, "3");
    }

    #[test]
    fn sheet_sort_numeric_column() {
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), "30");
        sheet.set_cell((1, 0), "5");
        sheet.set_cell((2, 0), "100");
        sheet.sort_by_column(0, true);
        assert_eq!(sheet.get_cell((0, 0)).unwrap().value, CellValue::Number(5.0));
        assert_eq!(sheet.get_cell((1, 0)).unwrap().value, CellValue::Number(30.0));
        assert_eq!(sheet.get_cell((2, 0)).unwrap().value, CellValue::Number(100.0));
    }
}
