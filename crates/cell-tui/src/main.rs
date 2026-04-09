mod app;
mod action;
mod viewport;
mod clipboard;
mod undo;
mod mode;
mod render;

use std::io;
use std::path::PathBuf;
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    execute,
};
use ratatui::{Terminal, backend::CrosstermBackend};
use app::{App, FileFormat};
use action::{Action, Mode};

#[derive(Parser)]
#[command(name = "cell", version, about = "A terminal spreadsheet editor")]
struct Cli {
    /// File to open (CSV, TSV, or .cell)
    file: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let mut app = App::new();

    if let Some(path) = &cli.file {
        load_file(&mut app, path)?;
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    result
}

fn load_file(app: &mut App, path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    match ext.as_str() {
        "csv" => {
            let file = std::fs::File::open(path)?;
            app.sheet = cell_core::io::csv::read_csv(file, b',')?;
            app.file_format = FileFormat::Csv;
        }
        "tsv" => {
            let file = std::fs::File::open(path)?;
            app.sheet = cell_core::io::csv::read_csv(file, b'\t')?;
            app.file_format = FileFormat::Tsv;
        }
        "cell" => {
            let file = std::fs::File::open(path)?;
            app.sheet = cell_core::io::cell_format::read_cell_format(file)?;
            app.file_format = FileFormat::Cell;
        }
        _ => {
            let file = std::fs::File::open(path)?;
            app.sheet = cell_core::io::csv::read_csv(file, b',')?;
            app.file_format = FileFormat::Csv;
        }
    }
    app.file_path = Some(path.to_path_buf());
    Ok(())
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<(), Box<dyn std::error::Error>> {
    use mode::normal::NormalState;
    use mode::insert::{handle_insert_key, handle_insert_char, InsertAction};
    use mode::command::{handle_command_key, parse_command, CommandAction};
    use mode::visual::{VisualState, VisualKind};
    use mode::help::HelpState;

    let mut normal_state = NormalState::new();
    let mut visual_state: Option<VisualState> = None;
    let mut insert_cursor: usize = 0;
    let mut search_mode = false;
    let mut wq_pending = false;
    let mut help_state = HelpState::new();

    loop {
        let grid_height = terminal.size()?.height.saturating_sub(3) as usize;
        app.viewport.visible_rows = grid_height;

        let selection = visual_state.as_ref().map(|vs| vs.selection(app.cursor));
        terminal.draw(|frame| {
            render::render(frame, app, selection);
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                app.status_message = None;

                let action = match app.mode {
                    Mode::Normal => normal_state.handle_key(key, app),
                    Mode::Insert => {
                        match key.code {
                            KeyCode::Esc | KeyCode::Enter => {
                                let edit_action = handle_insert_key(key, app);
                                app.process_action(edit_action);
                                Action::ChangeMode(Mode::Normal)
                            }
                            _ => {
                                if let Some(insert_action) = handle_insert_char(key) {
                                    match insert_action {
                                        InsertAction::InsertChar(c) => {
                                            app.insert_buffer.insert(insert_cursor, c);
                                            insert_cursor += 1;
                                        }
                                        InsertAction::Backspace => {
                                            if insert_cursor > 0 {
                                                insert_cursor -= 1;
                                                app.insert_buffer.remove(insert_cursor);
                                            }
                                        }
                                        InsertAction::Delete => {
                                            if insert_cursor < app.insert_buffer.len() {
                                                app.insert_buffer.remove(insert_cursor);
                                            }
                                        }
                                        InsertAction::CursorLeft => {
                                            insert_cursor = insert_cursor.saturating_sub(1);
                                        }
                                        InsertAction::CursorRight => {
                                            insert_cursor = (insert_cursor + 1).min(app.insert_buffer.len());
                                        }
                                        InsertAction::CursorHome => { insert_cursor = 0; }
                                        InsertAction::CursorEnd => { insert_cursor = app.insert_buffer.len(); }
                                    }
                                }
                                Action::Noop
                            }
                        }
                    }
                    Mode::Visual | Mode::VisualLine | Mode::VisualBlock => {
                        if let Some(ref vs) = visual_state {
                            let action = vs.handle_key(key, app);
                            let exits = matches!(
                                action,
                                Action::ChangeMode(Mode::Normal)
                                    | Action::ClearRange { .. }
                                    | Action::YankRange { .. }
                            );
                            if exits {
                                visual_state = None;
                                app.mode = Mode::Normal;
                            }
                            action
                        } else {
                            Action::ChangeMode(Mode::Normal)
                        }
                    }
                    Mode::Help => help_state.handle_key(key, app, grid_height),
                    Mode::Command => {
                        let cmd_action = handle_command_key(key, &app.command_line);
                        match cmd_action {
                            CommandAction::InsertChar(c) => {
                                app.command_line.push(c);
                                Action::Noop
                            }
                            CommandAction::Backspace => {
                                app.command_line.pop();
                                Action::Noop
                            }
                            CommandAction::Execute(cmd) => {
                                if search_mode {
                                    search_mode = false;
                                    let pattern = app.command_line.clone();
                                    app.command_line.clear();
                                    app.search_pattern = Some(pattern);
                                    Action::ChangeMode(Mode::Normal)
                                } else {
                                    let is_wq = cmd.trim() == "wq";
                                    let parsed = parse_command(&cmd);
                                    app.command_line.clear();
                                    if is_wq { wq_pending = true; }
                                    parsed
                                }
                            }
                            CommandAction::Cancel => {
                                app.command_line.clear();
                                search_mode = false;
                                Action::ChangeMode(Mode::Normal)
                            }
                            CommandAction::Noop => Action::Noop,
                        }
                    }
                };

                if let Action::ChangeMode(Mode::Visual) = &action {
                    visual_state = Some(VisualState::new(app.cursor, VisualKind::Character));
                }
                if let Action::ChangeMode(Mode::VisualLine) = &action {
                    visual_state = Some(VisualState::new(app.cursor, VisualKind::Line));
                }
                if let Action::ChangeMode(Mode::VisualBlock) = &action {
                    visual_state = Some(VisualState::new(app.cursor, VisualKind::Block));
                }
                if let Action::ChangeMode(Mode::Insert) = &action {
                    insert_cursor = app.sheet.get_cell(app.cursor)
                        .map(|c| c.raw.len()).unwrap_or(0);
                }
                if let Action::ChangeMode(Mode::Command) = &action {
                    if key.code == KeyCode::Char('/') {
                        search_mode = true;
                    }
                }

                app.process_action(action);

                if wq_pending && !app.dirty {
                    app.should_quit = true;
                    wq_pending = false;
                }
            }
        }

        if app.should_quit { break; }
    }
    Ok(())
}
