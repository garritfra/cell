use cell_sheet_core::model::CellPos;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SearchDirection {
    Forward,
    #[allow(dead_code)]
    Backward,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Noop,
    MoveCursor(Direction),
    #[allow(dead_code)]
    MoveCursorTo(CellPos),
    EditCell(CellPos, String),
    ClearCell(CellPos),
    ClearRange {
        start: CellPos,
        end: CellPos,
    },
    ChangeCell(CellPos),
    ChangeRange {
        start: CellPos,
        end: CellPos,
    },
    #[allow(dead_code)]
    YankCell(CellPos),
    YankRange {
        start: CellPos,
        end: CellPos,
    },
    Paste(CellPos),
    PasteBefore(CellPos),
    Undo,
    Redo,
    ChangeMode(Mode),
    Save(Option<PathBuf>),
    ForceSave(Option<PathBuf>),
    Open(PathBuf),
    Quit {
        force: bool,
    },
    Sort {
        col: usize,
        ascending: bool,
    },
    #[allow(dead_code)]
    Search {
        pattern: String,
        direction: SearchDirection,
    },
    SearchNext,
    SearchPrev,
    #[allow(dead_code)]
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
    ShowHelp(Option<String>),
    ScrollCursorTop,
    ScrollCursorCenter,
    ScrollCursorBottom,
    CursorToViewportTop,
    CursorToViewportMiddle,
    CursorToViewportBottom,
    ScrollLineDown,
    ScrollLineUp,
    SetMark(char),
    JumpToMark {
        name: char,
        line_wise: bool,
    },
    JumpBack,
    JumpForward,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
    VisualLine,
    VisualBlock,
    Command,
    Help,
}
