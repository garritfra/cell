use cell_core::model::CellPos;

pub struct Viewport {
    pub row_offset: usize,
    pub col_offset: usize,
    pub visible_rows: usize,
    pub visible_cols: usize,
}

impl Viewport {
    pub fn new() -> Self {
        Viewport {
            row_offset: 0,
            col_offset: 0,
            visible_rows: 20,
            visible_cols: 10,
        }
    }

    pub fn ensure_visible(&mut self, cursor: CellPos) {
        let (row, col) = cursor;
        if row < self.row_offset {
            self.row_offset = row;
        } else if row >= self.row_offset + self.visible_rows {
            self.row_offset = row - self.visible_rows + 1;
        }
        if col < self.col_offset {
            self.col_offset = col;
        } else if col >= self.col_offset + self.visible_cols {
            self.col_offset = col - self.visible_cols + 1;
        }
    }
}
