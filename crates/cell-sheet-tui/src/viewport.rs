use cell_sheet_core::model::CellPos;

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

    /// Scroll so `row` is at the top of the viewport (vim `zt`).
    pub fn top_on(&mut self, row: usize) {
        self.row_offset = row;
    }

    /// Scroll so `row` is centered in the viewport (vim `zz`). Clamps to 0.
    pub fn center_on(&mut self, row: usize) {
        let half = self.visible_rows / 2;
        self.row_offset = row.saturating_sub(half);
    }

    /// Scroll so `row` is at the bottom of the viewport (vim `zb`).
    pub fn bottom_on(&mut self, row: usize) {
        self.row_offset = (row + 1).saturating_sub(self.visible_rows);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_on_places_row_at_top() {
        let mut vp = Viewport::new();
        vp.visible_rows = 10;
        vp.top_on(50);
        assert_eq!(vp.row_offset, 50);
    }

    #[test]
    fn center_on_places_cursor_in_middle() {
        let mut vp = Viewport::new();
        vp.visible_rows = 10;
        vp.center_on(50);
        assert_eq!(vp.row_offset, 45);
    }

    #[test]
    fn center_on_clamps_to_zero_near_top() {
        let mut vp = Viewport::new();
        vp.visible_rows = 10;
        vp.center_on(2);
        assert_eq!(vp.row_offset, 0);
    }

    #[test]
    fn bottom_on_places_row_at_bottom() {
        let mut vp = Viewport::new();
        vp.visible_rows = 10;
        vp.bottom_on(50);
        // row 50 visible at bottom => offset 41 (rows 41..50 = 10 rows ending at 50)
        assert_eq!(vp.row_offset, 41);
    }

    #[test]
    fn bottom_on_clamps_to_zero_near_top() {
        let mut vp = Viewport::new();
        vp.visible_rows = 10;
        vp.bottom_on(3);
        assert_eq!(vp.row_offset, 0);
    }
}
