use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::Widget,
};

pub struct CommandLine<'a> {
    pub content: &'a str,
    pub prefix: char,
    pub active: bool,
}

impl<'a> Widget for CommandLine<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || !self.active { return; }
        let display = format!("{}{}", self.prefix, self.content);
        buf.set_string(area.x, area.y, &display, Style::default());
    }
}
