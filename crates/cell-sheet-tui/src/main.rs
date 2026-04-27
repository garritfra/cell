mod action;
mod app;
mod clipboard;
mod headless;
mod mode;
mod render;
mod undo;
mod viewport;

use action::{Action, Mode};
use app::{App, FileFormat};
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "cell", version, about = "A terminal spreadsheet editor")]
struct Cli {
    /// File to open (CSV, TSV, or .cell)
    file: Option<PathBuf>,

    /// Print the computed value of a cell or range (e.g. A1, A1:B3).
    /// Repeat to read multiple refs. Ranges render as TSV.
    #[arg(long, value_name = "REF")]
    read: Vec<String>,

    /// Evaluate a formula against the loaded sheet without persisting.
    /// The leading `=` is optional. Repeat to evaluate multiple expressions.
    #[arg(long, value_name = "EXPR")]
    eval: Vec<String>,

    /// Set a cell to a value (auto-detects formula if it starts with `=`).
    /// Repeat to batch multiple writes into a single save.
    #[arg(long, value_names = ["REF", "VALUE"], num_args = 2)]
    write: Vec<String>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let opts = headless::Options {
        file: cli.file.clone().unwrap_or_default(),
        reads: cli.read,
        evals: cli.eval,
        writes: cli
            .write
            .chunks_exact(2)
            .map(|c| (c[0].clone(), c[1].clone()))
            .collect(),
    };

    if opts.is_active() {
        if cli.file.is_none() {
            eprintln!("error: a FILE argument is required for --read/--eval/--write");
            return ExitCode::from(2);
        }
        let mut stdout = io::stdout().lock();
        return match headless::run(&opts, &mut stdout) {
            Ok(()) => ExitCode::SUCCESS,
            Err(msg) => {
                eprintln!("error: {msg}");
                ExitCode::FAILURE
            }
        };
    }

    match run_tui(cli.file.as_deref()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run_tui(file: Option<&std::path::Path>) -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new();

    if let Some(path) = file {
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
    use cell_sheet_core::formula::deps::{recalculate, set_formula};

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "csv" => {
            let file = std::fs::File::open(path)?;
            app.sheet = cell_sheet_core::io::csv::read_csv(file, b',')?;
            app.file_format = FileFormat::Csv;
        }
        "tsv" => {
            let file = std::fs::File::open(path)?;
            app.sheet = cell_sheet_core::io::csv::read_csv(file, b'\t')?;
            app.file_format = FileFormat::Tsv;
        }
        "cell" => {
            let file = std::fs::File::open(path)?;
            app.sheet = cell_sheet_core::io::cell_format::read_cell_format(file)?;
            app.file_format = FileFormat::Cell;
        }
        _ => {
            let file = std::fs::File::open(path)?;
            app.sheet = cell_sheet_core::io::csv::read_csv(file, b',')?;
            app.file_format = FileFormat::Csv;
        }
    }
    app.file_path = Some(path.to_path_buf());

    // Register formulas in the dependency graph and evaluate them
    let formula_cells: Vec<_> = app
        .sheet
        .cells
        .iter()
        .filter(|(_, cell)| cell.raw.starts_with('='))
        .map(|(pos, cell)| (*pos, cell.raw.clone()))
        .collect();
    for (pos, raw) in formula_cells {
        set_formula(&mut app.sheet, &mut app.deps, pos, &raw);
    }
    recalculate(&mut app.sheet, &app.deps);

    Ok(())
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<(), Box<dyn std::error::Error>> {
    use mode::command::{handle_command_key, submit, CommandAction};
    use mode::help::HelpState;
    use mode::insert::{handle_insert_char, handle_insert_key, InsertAction};
    use mode::normal::NormalState;
    use mode::visual::{VisualKind, VisualState};

    let mut normal_state = NormalState::new();
    let mut visual_state: Option<VisualState> = None;
    let mut insert_cursor: usize = 0;
    let mut wq_pending = false;
    let mut help_state = HelpState::new();

    loop {
        let grid_height = terminal.size()?.height.saturating_sub(3) as usize;
        // The grid widget uses 1 row for column headers, so data rows = grid_height - 1.
        app.viewport.visible_rows = grid_height.saturating_sub(1);

        let selection = visual_state.as_ref().map(|vs| vs.selection(app.cursor));
        let ic = insert_cursor;
        terminal.draw(|frame| {
            render::render(frame, app, selection, ic);
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                app.status_message = None;

                let action = match app.mode {
                    Mode::Normal => normal_state.handle_key(key, app),
                    Mode::Insert => match key.code {
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
                                        insert_cursor =
                                            (insert_cursor + 1).min(app.insert_buffer.len());
                                    }
                                    InsertAction::CursorHome => {
                                        insert_cursor = 0;
                                    }
                                    InsertAction::CursorEnd => {
                                        insert_cursor = app.insert_buffer.len();
                                    }
                                }
                            }
                            Action::Noop
                        }
                    },
                    Mode::Visual | Mode::VisualLine | Mode::VisualBlock => {
                        if let Some(ref vs) = visual_state {
                            let action = vs.handle_key(key, app);
                            let exits = matches!(
                                action,
                                Action::ChangeMode(Mode::Normal)
                                    | Action::ClearRange { .. }
                                    | Action::YankRange { .. }
                                    | Action::ChangeRange { .. }
                            );
                            if exits {
                                let anchor = vs.anchor;
                                let kind = vs.kind;
                                app.record_last_visual(anchor, kind);
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
                        use action::{CommandKind, SearchDirection};
                        let cmd_action = handle_command_key(key, &app.command_line);
                        let search_dir = match app.command_kind {
                            CommandKind::Slash => Some(SearchDirection::Forward),
                            CommandKind::Question => Some(SearchDirection::Backward),
                            CommandKind::Colon => None,
                        };
                        match cmd_action {
                            CommandAction::InsertChar(c) => {
                                app.command_line.push(c);
                                if let Some(dir) = search_dir {
                                    Action::SearchIncremental {
                                        pattern: app.command_line.clone(),
                                        direction: dir,
                                    }
                                } else {
                                    Action::Noop
                                }
                            }
                            CommandAction::Backspace => {
                                app.command_line.pop();
                                if let Some(dir) = search_dir {
                                    Action::SearchIncremental {
                                        pattern: app.command_line.clone(),
                                        direction: dir,
                                    }
                                } else {
                                    Action::Noop
                                }
                            }
                            CommandAction::Execute(cmd) => {
                                let kind = app.command_kind;
                                let is_wq =
                                    matches!(kind, CommandKind::Colon) && cmd.trim() == "wq";
                                let parsed = submit(kind, &cmd);
                                app.command_line.clear();
                                if is_wq {
                                    wq_pending = true;
                                }
                                // Submitting any prompt returns to normal
                                // mode regardless of whether the action
                                // ends up moving the cursor.
                                app.mode = Mode::Normal;
                                parsed
                            }
                            CommandAction::Cancel => {
                                if search_dir.is_some() {
                                    Action::CancelSearch
                                } else {
                                    app.command_line.clear();
                                    Action::ChangeMode(Mode::Normal)
                                }
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
                    insert_cursor = app
                        .sheet
                        .get_cell(app.cursor)
                        .map(|c| c.raw.len())
                        .unwrap_or(0);
                }
                if matches!(&action, Action::ChangeCell(_) | Action::ChangeRange { .. }) {
                    insert_cursor = 0;
                }
                if matches!(&action, Action::ReselectLastVisual) {
                    if let Some(lv) = app.last_visual {
                        visual_state = Some(VisualState::new(lv.anchor, lv.kind));
                        app.cursor = lv.cursor;
                        app.mode = match lv.kind {
                            VisualKind::Character => Mode::Visual,
                            VisualKind::Line => Mode::VisualLine,
                            VisualKind::Block => Mode::VisualBlock,
                        };
                        app.viewport.ensure_visible(app.cursor);
                    }
                }

                app.process_action(action);

                if wq_pending && !app.dirty {
                    app.should_quit = true;
                    wq_pending = false;
                }
            }
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}
