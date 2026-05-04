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
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, IsTerminal, Read};
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

    /// Field delimiter character (e.g. '|', ';'). Auto-detected from file
    /// content when omitted; .tsv files always default to tab.
    #[arg(long, value_name = "CHAR")]
    delimiter: Option<char>,
}

fn parse_delimiter(c: char) -> Result<u8, String> {
    if !c.is_ascii() {
        return Err(format!(
            "delimiter must be a single ASCII character, got {c:?}"
        ));
    }
    // Reject control characters (< 0x20 except tab), space, and double-quote
    // (reserved as the CSV quoting character). Tab is allowed: it is the
    // standard TSV delimiter.
    if c.is_alphanumeric() || c == '"' || c == ' ' || ((c as u8) < 0x20 && c != '\t') {
        return Err(format!("'{c}' is not a valid field delimiter"));
    }
    Ok(c as u8)
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let explicit_delimiter = match cli.delimiter.map(parse_delimiter) {
        Some(Err(msg)) => {
            eprintln!("error: {msg}");
            return ExitCode::from(2);
        }
        Some(Ok(b)) => Some(b),
        None => None,
    };

    // Read stdin before enabling raw mode. Only done when no FILE was given and
    // stdin is not an interactive terminal (i.e. data is being piped in).
    // Crossterm 0.29 opens /dev/tty for keyboard events when stdin is
    // redirected, so the TUI still receives input afterwards.
    let stdin_data: Option<Vec<u8>> = if cli.file.is_none() && !io::stdin().is_terminal() {
        let mut buf = Vec::new();
        if let Err(e) = io::stdin().lock().read_to_end(&mut buf) {
            eprintln!("error: failed to read stdin: {e}");
            return ExitCode::FAILURE;
        }
        Some(buf)
    } else {
        None
    };

    let has_headless_ops = !cli.read.is_empty() || !cli.eval.is_empty() || !cli.write.is_empty();

    if has_headless_ops {
        if cli.file.is_none() && stdin_data.is_none() {
            eprintln!("error: a FILE argument is required for --read/--eval/--write");
            return ExitCode::from(2);
        }
        let opts = headless::Options {
            file: cli.file.clone().unwrap_or_default(),
            stdin_data,
            reads: cli.read,
            evals: cli.eval,
            writes: cli
                .write
                .chunks_exact(2)
                .map(|c| (c[0].clone(), c[1].clone()))
                .collect(),
            delimiter: explicit_delimiter,
        };
        let mut stdout = io::stdout().lock();
        return match headless::run(&opts, &mut stdout) {
            Ok(()) => ExitCode::SUCCESS,
            Err(msg) => {
                eprintln!("error: {msg}");
                ExitCode::FAILURE
            }
        };
    }

    match run_tui(cli.file.as_deref(), explicit_delimiter, stdin_data) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run_tui(
    file: Option<&std::path::Path>,
    explicit_delimiter: Option<u8>,
    stdin_data: Option<Vec<u8>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new();

    if let Some(path) = file {
        load_file(&mut app, path, explicit_delimiter)?;
    } else if let Some(data) = stdin_data {
        load_stdin_data(&mut app, data, explicit_delimiter)?;
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, &mut app);

    if app.mouse_enabled {
        let _ = execute!(terminal.backend_mut(), DisableMouseCapture);
    }
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    result
}

fn load_file(
    app: &mut App,
    path: &std::path::Path,
    explicit_delimiter: Option<u8>,
) -> Result<(), Box<dyn std::error::Error>> {
    use cell_sheet_core::engine::SheetEngine;

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "cell" => {
            let file = std::fs::File::open(path)?;
            app.sheet = cell_sheet_core::io::cell_format::read_cell_format(file)?;
            app.file_format = FileFormat::Cell;
        }
        _ => {
            // Read entire file once; use the same bytes for sniffing and parsing.
            let data = std::fs::read(path)?;
            let delimiter = if let Some(d) = explicit_delimiter {
                d
            } else if ext == "tsv" {
                b'\t'
            } else {
                cell_sheet_core::io::csv::sniff_delimiter(&data)
            };
            app.sheet = cell_sheet_core::io::csv::read_csv(data.as_slice(), delimiter)?;
            app.file_format = if ext == "tsv" {
                FileFormat::Tsv
            } else {
                FileFormat::Csv
            };
            app.delimiter = delimiter;
        }
    }

    app.file_path = Some(path.to_path_buf());

    SheetEngine::new(&mut app.sheet, &mut app.deps).rebuild_formulas_and_recalculate();

    Ok(())
}

fn load_stdin_data(
    app: &mut App,
    data: Vec<u8>,
    explicit_delimiter: Option<u8>,
) -> Result<(), Box<dyn std::error::Error>> {
    use cell_sheet_core::engine::SheetEngine;

    // Detect the native .cell format by its magic header. Anything else is
    // treated as CSV/TSV with delimiter sniffing (or an explicit override).
    if data.starts_with(b"# cell v") {
        if explicit_delimiter.is_some() {
            return Err("--delimiter has no effect on .cell-format input piped to stdin".into());
        }
        app.sheet = cell_sheet_core::io::cell_format::read_cell_format(data.as_slice())?;
        app.file_format = FileFormat::Cell;
        // delimiter stays at its default; .cell format doesn't use one
    } else {
        let delimiter =
            explicit_delimiter.unwrap_or_else(|| cell_sheet_core::io::csv::sniff_delimiter(&data));
        app.sheet = cell_sheet_core::io::csv::read_csv(data.as_slice(), delimiter)?;
        app.file_format = FileFormat::Csv;
        app.delimiter = delimiter;
    }
    // file_path stays None — unnamed buffer; :w <path> still works to save

    SheetEngine::new(&mut app.sheet, &mut app.deps).rebuild_formulas_and_recalculate();

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
    let mut mouse_state = mode::mouse::MouseState::new();
    let mut mouse_capture_active = false;

    loop {
        if app.mouse_enabled != mouse_capture_active {
            if app.mouse_enabled {
                execute!(terminal.backend_mut(), EnableMouseCapture)?;
            } else {
                execute!(terminal.backend_mut(), DisableMouseCapture)?;
                mouse_state = mode::mouse::MouseState::new();
            }
            mouse_capture_active = app.mouse_enabled;
        }

        let grid_height = terminal.size()?.height.saturating_sub(3) as usize;
        // The grid widget uses 1 row for column headers, so data rows = grid_height - 1.
        app.viewport.visible_rows = grid_height.saturating_sub(1);

        let selection = visual_state.as_ref().map(|vs| vs.selection(app.cursor));
        let ic = insert_cursor;
        // Build vim-style `showcmd` for normal mode: e.g. `5`, `5d`, `g`,
        // `5g`. Other modes don't have a partial command to display.
        let partial_command = if app.mode == Mode::Normal {
            let mut s = String::new();
            if let Some(n) = normal_state.pending_count() {
                s.push_str(&n.to_string());
            }
            if let Some(c) = normal_state.pending_op() {
                s.push(c);
            }
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        } else {
            None
        };
        let partial_command_ref = partial_command.as_deref();
        terminal.draw(|frame| {
            render::render(frame, app, selection, ic, partial_command_ref);
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }

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
                            if let Some(ref mut vs) = visual_state {
                                let action = vs.handle_key(key, app);
                                let exits = matches!(
                                    action,
                                    Action::ChangeMode(Mode::Normal)
                                        | Action::ClearRange { .. }
                                        | Action::YankRange { .. }
                                        | Action::ChangeRange { .. }
                                        | Action::CaseOpRange { .. }
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
                                    // Push non-empty colon commands to history,
                                    // avoiding consecutive duplicates.
                                    if matches!(kind, CommandKind::Colon)
                                        && !cmd.trim().is_empty()
                                        && app.command_history.last().map(|s| s.as_str())
                                            != Some(cmd.trim())
                                    {
                                        app.command_history.push(cmd.trim().to_string());
                                    }
                                    app.command_history_idx = None;
                                    app.command_history_scratch.clear();
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
                                    app.command_history_idx = None;
                                    app.command_history_scratch.clear();
                                    if search_dir.is_some() {
                                        Action::CancelSearch
                                    } else {
                                        app.command_line.clear();
                                        Action::ChangeMode(Mode::Normal)
                                    }
                                }
                                CommandAction::HistoryPrev => {
                                    if app.command_history.is_empty() {
                                        Action::Noop
                                    } else {
                                        let new_idx = match app.command_history_idx {
                                            None => {
                                                // Save current in-progress text before browsing.
                                                app.command_history_scratch =
                                                    app.command_line.clone();
                                                app.command_history.len() - 1
                                            }
                                            Some(i) => i.saturating_sub(1),
                                        };
                                        app.command_history_idx = Some(new_idx);
                                        app.command_line = app.command_history[new_idx].clone();
                                        Action::Noop
                                    }
                                }
                                CommandAction::HistoryNext => {
                                    match app.command_history_idx {
                                        None => Action::Noop,
                                        Some(i) => {
                                            if i + 1 < app.command_history.len() {
                                                let new_idx = i + 1;
                                                app.command_history_idx = Some(new_idx);
                                                app.command_line =
                                                    app.command_history[new_idx].clone();
                                            } else {
                                                // Past the newest entry — restore scratch.
                                                app.command_history_idx = None;
                                                app.command_line =
                                                    app.command_history_scratch.clone();
                                            }
                                            Action::Noop
                                        }
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
                Event::Mouse(me) => {
                    if !app.mouse_enabled {
                        continue;
                    }
                    app.status_message = None;
                    let drag_was_idle = mouse_state.drag == mode::mouse::MouseDragState::Idle;
                    let layout = app.last_grid_layout.clone();
                    let action =
                        mode::mouse::handle_mouse_event(me, &mut mouse_state, app, layout.as_ref());

                    // First click on a grid target while in another mode:
                    // commit/cancel/exit before dispatching.
                    let is_first_grid_action = drag_was_idle
                        && matches!(
                            &action,
                            Action::MouseClickCell(_)
                                | Action::MouseSelectColumn(_)
                                | Action::MouseSelectRow(_)
                        );
                    if is_first_grid_action {
                        match app.mode {
                            Mode::Insert => {
                                let pos = app.cursor;
                                let buf = std::mem::take(&mut app.insert_buffer);
                                app.process_action(Action::EditCell(pos, buf));
                                app.mode = Mode::Normal;
                            }
                            Mode::Command => {
                                use action::CommandKind;
                                app.command_history_idx = None;
                                app.command_history_scratch.clear();
                                if matches!(
                                    app.command_kind,
                                    CommandKind::Slash | CommandKind::Question
                                ) {
                                    app.process_action(Action::CancelSearch);
                                } else {
                                    app.command_line.clear();
                                    app.mode = Mode::Normal;
                                }
                            }
                            Mode::Visual | Mode::VisualLine | Mode::VisualBlock => {
                                if let Some(vs) = &visual_state {
                                    app.record_last_visual(vs.anchor, vs.kind);
                                }
                                visual_state = None;
                                app.mode = Mode::Normal;
                            }
                            _ => {}
                        }
                    }

                    match &action {
                        Action::MouseDragTo(_) if app.mode == Mode::Normal => {
                            if let mode::mouse::MouseDragState::DraggingCells { anchor } =
                                mouse_state.drag
                            {
                                visual_state =
                                    Some(VisualState::new(anchor, VisualKind::Character));
                                app.mode = Mode::Visual;
                            }
                        }
                        Action::MouseSelectColumn(_) if app.mode != Mode::VisualBlock => {
                            if let mode::mouse::MouseDragState::DraggingColumns { anchor_col } =
                                mouse_state.drag
                            {
                                // Anchor at the bottom of the column so
                                // the selection rectangle covers the
                                // full column while the cursor (set by
                                // process_action to row 0) stays at the
                                // top — i.e. where the user clicked.
                                let last_row = app.sheet.row_count.saturating_sub(1);
                                visual_state = Some(VisualState::new(
                                    (last_row, anchor_col),
                                    VisualKind::Block,
                                ));
                                app.mode = Mode::VisualBlock;
                            }
                        }
                        Action::MouseSelectRow(_) if app.mode != Mode::VisualBlock => {
                            if let mode::mouse::MouseDragState::DraggingRows { anchor_row } =
                                mouse_state.drag
                            {
                                // Mirror image: anchor at the rightmost
                                // column, cursor at column 0.
                                let last_col = app.sheet.col_count.saturating_sub(1);
                                visual_state = Some(VisualState::new(
                                    (anchor_row, last_col),
                                    VisualKind::Block,
                                ));
                                app.mode = Mode::VisualBlock;
                            }
                        }
                        _ => {}
                    }

                    if let Action::ChangeMode(Mode::Insert) = &action {
                        insert_cursor = app
                            .sheet
                            .get_cell(app.cursor)
                            .map(|c| c.raw.len())
                            .unwrap_or(0);
                    }

                    app.process_action(action);
                }
                _ => {}
            }
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}
