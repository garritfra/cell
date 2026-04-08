mod app;
mod action;
mod viewport;
mod clipboard;
mod undo;
mod mode;

use std::io;
use std::path::PathBuf;
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    execute,
};
use ratatui::{Terminal, backend::CrosstermBackend};
use app::{App, FileFormat};
use action::{Action, Mode, Direction};

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

fn load_file(app: &mut App, path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
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
    app.file_path = Some(path.clone());
    Ok(())
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        terminal.draw(|frame| {
            let area = frame.area();
            let block = ratatui::widgets::Block::default()
                .title(format!(" cell — {:?} ", app.mode));
            frame.render_widget(block, area);
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                let action = match app.mode {
                    Mode::Normal => match key.code {
                        KeyCode::Char('q') => Action::Quit { force: false },
                        KeyCode::Char('h') => Action::MoveCursor(Direction::Left),
                        KeyCode::Char('j') => Action::MoveCursor(Direction::Down),
                        KeyCode::Char('k') => Action::MoveCursor(Direction::Up),
                        KeyCode::Char('l') => Action::MoveCursor(Direction::Right),
                        _ => Action::Noop,
                    },
                    _ => Action::Noop,
                };
                app.process_action(action);
            }
        }

        if app.should_quit { break; }
    }
    Ok(())
}
