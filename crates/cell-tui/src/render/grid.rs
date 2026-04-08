use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};
use cell_core::model::{Sheet, CellPos, CellValue, col_index_to_label};
use crate::viewport::Viewport;

pub struct Grid<'a> {
    pub sheet: &'a Sheet,
    pub viewport: &'a Viewport,
    pub cursor: CellPos,
    pub selection: Option<(CellPos, CellPos)>,
}

const ROW_NUM_WIDTH: u16 = 5;
const DEFAULT_COL_WIDTH: u16 = 10;

impl<'a> Grid<'a> {
    fn col_width(&self, col: usize) -> u16 {
        self.sheet.col_widths.get(col).copied().unwrap_or(DEFAULT_COL_WIDTH)
    }
}

impl<'a> Widget for Grid<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 2 || area.width < ROW_NUM_WIDTH + 2 { return; }

        let header_style = Style::default().fg(Color::Black).bg(Color::DarkGray);
        let cursor_style = Style::default().fg(Color::Black).bg(Color::Yellow);
        let selection_style = Style::default().fg(Color::Black).bg(Color::Blue);
        let normal_style = Style::default();

        // Column headers
        let mut x = area.x + ROW_NUM_WIDTH + 1;
        let mut visible_cols = Vec::new();
        for col in self.viewport.col_offset.. {
            if x >= area.x + area.width { break; }
            let w = self.col_width(col);
            let label = col_index_to_label(col);
            let display = format!("{:^width$}", label, width = w as usize);
            let truncated = &display[..display.len().min((area.x + area.width - x) as usize)];
            buf.set_string(x, area.y, truncated, header_style);
            visible_cols.push((col, x, w));
            x += w + 1;
        }

        // Rows
        for row_offset in 0..area.height.saturating_sub(1) {
            let row = self.viewport.row_offset + row_offset as usize;
            let y = area.y + 1 + row_offset;
            if y >= area.y + area.height { break; }

            let row_num = format!("{:>width$}", row + 1, width = ROW_NUM_WIDTH as usize);
            buf.set_string(area.x, y, &row_num, header_style);

            for &(col, col_x, col_w) in &visible_cols {
                let pos = (row, col);
                let is_cursor = pos == self.cursor;
                let is_selected = self.selection.is_some_and(|(start, end)| {
                    row >= start.0 && row <= end.0 && col >= start.1 && col <= end.1
                });
                let style = if is_cursor { cursor_style }
                    else if is_selected { selection_style }
                    else { normal_style };

                let display_val = match self.sheet.get_cell(pos) {
                    Some(cell) => cell.value.to_string(),
                    None => String::new(),
                };
                let is_number = matches!(
                    self.sheet.get_cell(pos).map(|c| &c.value),
                    Some(CellValue::Number(_))
                );
                let formatted = if is_number {
                    format!("{:>width$}", display_val, width = col_w as usize)
                } else {
                    format!("{:<width$}", display_val, width = col_w as usize)
                };
                let truncated = if formatted.len() > col_w as usize {
                    let mut s = formatted[..col_w as usize - 1].to_string();
                    s.push('…');
                    s
                } else {
                    formatted
                };
                let max_chars = (area.x + area.width).saturating_sub(col_x) as usize;
                let truncated = &truncated[..truncated.len().min(max_chars)];

                for cx in 0..col_w.min(area.x + area.width - col_x) {
                    buf.set_string(col_x + cx, y, " ", style);
                }
                buf.set_string(col_x, y, truncated, style);
            }
        }
    }
}
