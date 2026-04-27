use cell_sheet_core::model::CellPos;

#[derive(Debug, Clone)]
pub enum UndoEntry {
    CellEdit {
        pos: CellPos,
        old_raw: String,
        new_raw: String,
    },
    /// A group of cell edits applied atomically (e.g. `dd` deleting a row).
    /// Each tuple is `(pos, old_raw, new_raw)`.
    MultiCellEdit {
        changes: Vec<(CellPos, String, String)>,
    },
}

pub struct UndoStack {
    undo: Vec<UndoEntry>,
    redo: Vec<UndoEntry>,
}

impl UndoStack {
    pub fn new() -> Self {
        UndoStack {
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }
    pub fn push(&mut self, entry: UndoEntry) {
        self.undo.push(entry);
        self.redo.clear();
    }
    pub fn undo(&mut self) -> Option<UndoEntry> {
        let entry = self.undo.pop()?;
        self.redo.push(entry.clone());
        Some(entry)
    }
    pub fn redo(&mut self) -> Option<UndoEntry> {
        let entry = self.redo.pop()?;
        self.undo.push(entry.clone());
        Some(entry)
    }
}
