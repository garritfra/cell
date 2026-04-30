pub mod command_line;
pub mod formula_bar;
pub mod grid;
pub mod help;
pub mod status_bar;

use crate::action::Mode;
use crate::app::App;
use crate::mode::mouse::GridLayout;
use cell_sheet_core::model::CellPos;
use command_line::CommandLine;
use formula_bar::FormulaBar;
use grid::Grid;
use help::HelpView;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};
use status_bar::StatusBar;

pub fn render(
    frame: &mut Frame,
    app: &mut App,
    selection: Option<(CellPos, CellPos)>,
    insert_cursor: usize,
    partial_command: Option<&str>,
) {
    if app.mode == Mode::Help {
        frame.render_widget(
            HelpView {
                registry: &app.help_registry,
                topic: app.help_topic.as_deref(),
                scroll: app.help_scroll,
            },
            frame.area(),
        );
        return;
    }

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

    let cell_content = app
        .sheet
        .get_cell(app.cursor)
        .map(|c| c.raw.as_str())
        .unwrap_or("");
    let is_editing = app.mode == Mode::Insert;
    let display_content = if is_editing {
        &app.insert_buffer
    } else {
        cell_content
    };
    let formula_bar = FormulaBar {
        cursor: app.cursor,
        content: display_content,
        is_editing,
    };
    let content_x = formula_bar.content_x();
    frame.render_widget(formula_bar, chunks[0]);

    if is_editing {
        let cursor_x = chunks[0].x + content_x + insert_cursor as u16;
        let cursor_y = chunks[0].y;
        if cursor_x < chunks[0].x + chunks[0].width {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }

    let mut grid_layout: Option<GridLayout> = None;
    frame.render_widget(
        Grid {
            sheet: &app.sheet,
            viewport: &app.viewport,
            cursor: app.cursor,
            selection,
            layout_out: &mut grid_layout,
        },
        chunks[1],
    );
    app.last_grid_layout = grid_layout;

    let file_name = app
        .file_path
        .as_ref()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str());
    frame.render_widget(
        StatusBar {
            mode: app.mode,
            row_count: app.sheet.row_count,
            col_count: app.sheet.col_count,
            cursor: app.cursor,
            dirty: app.dirty,
            file_name,
            message: app.status_message.as_deref(),
            partial_command,
        },
        chunks[2],
    );

    let is_command = app.mode == Mode::Command;
    frame.render_widget(
        CommandLine {
            content: &app.command_line,
            prefix: app.command_kind.prefix(),
            active: is_command,
        },
        chunks[3],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn render_writes_layout_to_app_and_overwrites_on_second_call() {
        let backend = TestBackend::new(40, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new();
        app.sheet.set_cell((0, 0), "a");
        app.sheet.set_cell((0, 1), "b");

        terminal
            .draw(|frame| {
                render(frame, &mut app, None, 0, None);
            })
            .unwrap();
        let first = app
            .last_grid_layout
            .clone()
            .expect("layout populated after first render");
        assert!(!first.visible_cols.is_empty());
        assert_eq!(first.header_height, 1);

        // A second render must OVERWRITE, not stack/append. Mutating the
        // viewport between renders is the simplest way to prove it.
        app.viewport.row_offset = 5;
        terminal
            .draw(|frame| {
                render(frame, &mut app, None, 0, None);
            })
            .unwrap();
        let second = app
            .last_grid_layout
            .as_ref()
            .expect("layout still populated after second render");
        assert_eq!(second.row_offset, 5);
    }
}
