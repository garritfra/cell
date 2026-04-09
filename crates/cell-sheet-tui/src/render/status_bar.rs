use crate::action::Mode;
use cell_sheet_core::model::CellPos;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Widget,
};

pub struct StatusBar<'a> {
    pub mode: Mode,
    pub row_count: usize,
    pub col_count: usize,
    pub cursor: CellPos,
    pub dirty: bool,
    pub file_name: Option<&'a str>,
    pub message: Option<&'a str>,
}

impl<'a> Widget for StatusBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }
        let style = Style::default().fg(Color::Black).bg(Color::White);
        for x in area.x..area.x + area.width {
            buf.set_string(x, area.y, " ", style);
        }
        let mode_str = match self.mode {
            Mode::Normal => " NORMAL ",
            Mode::Insert => " INSERT ",
            Mode::Visual => " VISUAL ",
            Mode::VisualLine => " V-LINE ",
            Mode::VisualBlock => " V-BLOCK ",
            Mode::Command => " COMMAND ",
            Mode::Help => " HELP ",
        };
        let mode_bg = match self.mode {
            Mode::Normal => Color::Rgb(68, 68, 68),
            Mode::Insert => Color::Rgb(95, 175, 0),
            Mode::Visual | Mode::VisualLine | Mode::VisualBlock => Color::Rgb(215, 135, 0),
            Mode::Command | Mode::Help => Color::Rgb(68, 68, 68),
        };
        let mode_fg = match self.mode {
            Mode::Normal | Mode::Command | Mode::Help => Color::White,
            Mode::Insert => Color::Black,
            Mode::Visual | Mode::VisualLine | Mode::VisualBlock => Color::Black,
        };
        let mode_style = Style::default()
            .fg(mode_fg)
            .bg(mode_bg)
            .add_modifier(Modifier::BOLD);
        buf.set_string(area.x, area.y, mode_str, mode_style);

        if let Some(msg) = self.message {
            let msg_x = area.x + mode_str.len() as u16 + 1;
            let msg_style = Style::default().fg(Color::Red).bg(Color::White);
            buf.set_string(msg_x, area.y, msg, msg_style);
            return;
        }

        let file_info = match self.file_name {
            Some(name) => {
                if self.dirty {
                    format!(" {} [+]", name)
                } else {
                    format!(" {}", name)
                }
            }
            None => {
                if self.dirty {
                    " [No Name] [+]".into()
                } else {
                    " [No Name]".into()
                }
            }
        };
        let info_x = area.x + mode_str.len() as u16;
        buf.set_string(info_x, area.y, &file_info, style);

        let right = format!(
            "{} rows x {} cols │ {}{} ",
            self.row_count,
            self.col_count,
            cell_sheet_core::model::col_index_to_label(self.cursor.1),
            self.cursor.0 + 1
        );
        let right_x = area.x + area.width.saturating_sub(right.len() as u16);
        if right_x > info_x + file_info.len() as u16 {
            buf.set_string(right_x, area.y, &right, style);
        }
    }
}
