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
    Backward,
}

/// Discriminator for the prompt rendered in `Mode::Command`.
///
/// `:` parses ex-style commands. `/` and `?` parse a search pattern that
/// is dispatched as `Action::Search` with the corresponding direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CommandKind {
    Colon,
    Slash,
    Question,
}

impl CommandKind {
    pub fn prefix(self) -> char {
        match self {
            CommandKind::Colon => ':',
            CommandKind::Slash => '/',
            CommandKind::Question => '?',
        }
    }
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
    Search {
        pattern: String,
        direction: SearchDirection,
    },
    SearchNext,
    SearchPrev,
    /// Switch to `Mode::Command` with a search prompt (`/` or `?`).
    /// Saves the current cursor as the search origin so Esc can restore it.
    EnterSearch(SearchDirection),
    /// Re-run the search from the saved origin as the user types in the
    /// `/` or `?` prompt (vim's `incsearch`). Empty pattern restores the
    /// cursor to the origin without committing a pattern.
    SearchIncremental {
        pattern: String,
        direction: SearchDirection,
    },
    /// Esc out of a `/` or `?` prompt: restore the cursor to the saved
    /// search origin and clear it.
    CancelSearch,
    /// Move the cursor to the next non-empty cell in the current row whose
    /// displayed value starts with `ch` (case-insensitive). `forward = false`
    /// scans left. `inclusive = true` lands on the match (`f`/`F`).
    FindCharInRow {
        ch: char,
        forward: bool,
        inclusive: bool,
    },
    /// Repeat the last `f`/`F` find. When `reversed`, flip the direction
    /// (i.e. vim's `,`).
    RepeatFind {
        reversed: bool,
    },
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
    BlockJumpDown,
    BlockJumpUp,
    SearchCellValue {
        backward: bool,
    },
    ReselectLastVisual,
    SetDelimiter(u8),
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
