pub mod grid;
pub mod formula_bar;
pub mod status_bar;
pub mod command_line;

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Direction},
};
use crate::app::App;
use crate::action::Mode;
use grid::Grid;
use formula_bar::FormulaBar;
use status_bar::StatusBar;
use command_line::CommandLine;

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    let cell_content = app.sheet.get_cell(app.cursor).map(|c| c.raw.as_str()).unwrap_or("");
    let display_content = if app.mode == Mode::Insert { &app.insert_buffer } else { cell_content };
    frame.render_widget(FormulaBar {
        cursor: app.cursor, content: display_content, is_editing: app.mode == Mode::Insert,
    }, chunks[0]);

    frame.render_widget(Grid {
        sheet: &app.sheet, viewport: &app.viewport, cursor: app.cursor, selection: None,
    }, chunks[1]);

    let file_name = app.file_path.as_ref().and_then(|p| p.file_name()).and_then(|n| n.to_str());
    frame.render_widget(StatusBar {
        mode: app.mode, row_count: app.sheet.row_count, col_count: app.sheet.col_count,
        cursor: app.cursor, dirty: app.dirty, file_name, message: app.status_message.as_deref(),
    }, chunks[2]);

    let is_command = app.mode == Mode::Command;
    frame.render_widget(CommandLine {
        content: &app.command_line, prefix: ':', active: is_command,
    }, chunks[3]);
}
