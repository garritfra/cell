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

    // ── ensure_visible ───────────────────────────────────────────────────────

    #[test]
    fn ensure_visible_cursor_within_view_does_not_scroll() {
        let mut vp = Viewport::new();
        vp.visible_rows = 10;
        vp.row_offset = 5;
        vp.ensure_visible((10, 0)); // row 10 is within [5, 14]
        assert_eq!(vp.row_offset, 5);
    }

    #[test]
    fn ensure_visible_cursor_at_last_row_does_not_scroll() {
        let mut vp = Viewport::new();
        vp.visible_rows = 10;
        vp.row_offset = 0;
        vp.ensure_visible((9, 0)); // row 9 is the last visible row (0-indexed)
        assert_eq!(vp.row_offset, 0);
    }

    #[test]
    fn ensure_visible_cursor_one_past_last_row_scrolls_down() {
        // Regression for issue #29: the grid widget uses 1 row for the column
        // header, so visible_rows must equal (grid_area_height - 1). If it were
        // set one too high, a cursor at row `visible_rows` would not trigger a
        // scroll even though it falls outside the rendered data area.
        let mut vp = Viewport::new();
        vp.visible_rows = 10;
        vp.row_offset = 0;
        vp.ensure_visible((10, 0)); // row 10 is one past the last rendered row
        assert_eq!(vp.row_offset, 1);
    }

    #[test]
    fn ensure_visible_cursor_above_viewport_scrolls_up() {
        let mut vp = Viewport::new();
        vp.visible_rows = 10;
        vp.row_offset = 5;
        vp.ensure_visible((3, 0)); // row 3 is above offset 5
        assert_eq!(vp.row_offset, 3);
    }

    // ── top_on / center_on / bottom_on ──────────────────────────────────────

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
