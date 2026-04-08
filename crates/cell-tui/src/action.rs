use std::path::PathBuf;
use cell_core::model::CellPos;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Direction {
    Up, Down, Left, Right,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SearchDirection {
    Forward, Backward,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Noop,
    MoveCursor(Direction),
    MoveCursorTo(CellPos),
    EditCell(CellPos, String),
    ClearCell(CellPos),
    ClearRange { start: CellPos, end: CellPos },
    YankCell(CellPos),
    YankRange { start: CellPos, end: CellPos },
    Paste(CellPos),
    PasteBefore(CellPos),
    Undo,
    Redo,
    ChangeMode(Mode),
    Save(Option<PathBuf>),
    ForceSave(Option<PathBuf>),
    Open(PathBuf),
    Quit { force: bool },
    Sort { col: usize, ascending: bool },
    Search { pattern: String, direction: SearchDirection },
    SearchNext,
    SearchPrev,
    Resize,
    PageDown,
    PageUp,
    HalfPageDown,
    HalfPageUp,
    GotoFirstRow,
    GotoLastRow,
    GotoFirstCol,
    GotoLastCol,
    NextNonEmpty,
    PrevNonEmpty,
    DeleteRow(usize),
    YankRow(usize),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
    VisualBlock,
    Command,
}
