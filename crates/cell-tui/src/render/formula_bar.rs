use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};
use cell_core::model::{col_index_to_label, CellPos};

pub struct FormulaBar<'a> {
    pub cursor: CellPos,
    pub content: &'a str,
    #[allow(dead_code)]
    pub is_editing: bool,
}

impl<'a> Widget for FormulaBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 { return; }
        let addr = format!(" {}{} ", col_index_to_label(self.cursor.1), self.cursor.0 + 1);
        let addr_width = addr.len() as u16;
        let addr_style = Style::default().fg(Color::Black).bg(Color::White);
        buf.set_string(area.x, area.y, &addr, addr_style);
        let sep = " │ ";
        buf.set_string(area.x + addr_width, area.y, sep, Style::default());
        let content_x = area.x + addr_width + sep.len() as u16;
        let content_width = area.width.saturating_sub(addr_width + sep.len() as u16);
        let content = if self.content.len() > content_width as usize {
            &self.content[..content_width as usize]
        } else {
            self.content
        };
        buf.set_string(content_x, area.y, content, Style::default());
    }
}
