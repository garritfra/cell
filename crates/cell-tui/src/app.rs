use std::path::PathBuf;
use cell_core::model::{Sheet, CellPos, CellValue};
use cell_core::formula::deps::{DepGraph, set_formula, mark_dirty, recalculate};
use crate::action::{Action, Mode, Direction};
use crate::viewport::Viewport;
use crate::undo::{UndoEntry, UndoStack};
use crate::clipboard::Register;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileFormat {
    Csv, Tsv, Cell,
}

pub struct App {
    pub sheet: Sheet,
    pub deps: DepGraph,
    pub viewport: Viewport,
    pub cursor: CellPos,
    pub mode: Mode,
    pub register: Option<Register>,
    pub undo_stack: UndoStack,
    pub command_line: String,
    pub status_message: Option<String>,
    pub search_pattern: Option<String>,
    pub file_path: Option<PathBuf>,
    pub file_format: FileFormat,
    pub dirty: bool,
    pub should_quit: bool,
    pub insert_buffer: String,
}

impl App {
    pub fn new() -> Self {
        App {
            sheet: Sheet::new(),
            deps: DepGraph::new(),
            viewport: Viewport::new(),
            cursor: (0, 0),
            mode: Mode::Normal,
            register: None,
            undo_stack: UndoStack::new(),
            command_line: String::new(),
            status_message: None,
            search_pattern: None,
            file_path: None,
            file_format: FileFormat::Csv,
            dirty: false,
            should_quit: false,
            insert_buffer: String::new(),
        }
    }

    pub fn has_formulas(&self) -> bool {
        self.sheet.cells.values().any(|c| c.raw.starts_with('='))
    }

    pub fn process_action(&mut self, action: Action) {
        match action {
            Action::Noop => {}
            Action::MoveCursor(dir) => {
                let (row, col) = self.cursor;
                self.cursor = match dir {
                    Direction::Up => (row.saturating_sub(1), col),
                    Direction::Down => (row + 1, col),
                    Direction::Left => (row, col.saturating_sub(1)),
                    Direction::Right => (row, col + 1),
                };
                self.viewport.ensure_visible(self.cursor);
            }
            Action::MoveCursorTo(pos) => {
                self.cursor = pos;
                self.viewport.ensure_visible(self.cursor);
            }
            Action::EditCell(pos, raw) => {
                let old_raw = self.sheet.get_cell(pos).map(|c| c.raw.clone()).unwrap_or_default();
                self.undo_stack.push(UndoEntry::CellEdit { pos, old_raw, new_raw: raw.clone() });
                if raw.starts_with('=') {
                    set_formula(&mut self.sheet, &mut self.deps, pos, &raw);
                } else {
                    self.sheet.set_cell(pos, &raw);
                }
                mark_dirty(&mut self.sheet, &self.deps, pos);
                recalculate(&mut self.sheet, &self.deps);
                self.dirty = true;
            }
            Action::ChangeMode(mode) => {
                if mode == Mode::Insert {
                    self.insert_buffer = self.sheet.get_cell(self.cursor).map(|c| c.raw.clone()).unwrap_or_default();
                }
                self.mode = mode;
            }
            Action::Quit { force } => {
                if !force && self.dirty {
                    self.status_message = Some("No write since last change (use :q! to override)".into());
                } else {
                    self.should_quit = true;
                }
            }
            Action::ClearCell(pos) => {
                let old_raw = self.sheet.get_cell(pos).map(|c| c.raw.clone()).unwrap_or_default();
                if !old_raw.is_empty() {
                    self.undo_stack.push(UndoEntry::CellEdit { pos, old_raw, new_raw: String::new() });
                    self.sheet.clear_cell(pos);
                    self.dirty = true;
                }
            }
            Action::GotoFirstRow => { self.cursor = (0, self.cursor.1); self.viewport.ensure_visible(self.cursor); }
            Action::GotoLastRow => {
                let last = if self.sheet.row_count > 0 { self.sheet.row_count - 1 } else { 0 };
                self.cursor = (last, self.cursor.1); self.viewport.ensure_visible(self.cursor);
            }
            Action::GotoFirstCol => { self.cursor = (self.cursor.0, 0); self.viewport.ensure_visible(self.cursor); }
            Action::GotoLastCol => {
                let last = if self.sheet.col_count > 0 { self.sheet.col_count - 1 } else { 0 };
                self.cursor = (self.cursor.0, last); self.viewport.ensure_visible(self.cursor);
            }
            Action::HalfPageDown => { self.cursor.0 += self.viewport.visible_rows / 2; self.viewport.ensure_visible(self.cursor); }
            Action::HalfPageUp => { self.cursor.0 = self.cursor.0.saturating_sub(self.viewport.visible_rows / 2); self.viewport.ensure_visible(self.cursor); }
            Action::PageDown => { self.cursor.0 += self.viewport.visible_rows; self.viewport.ensure_visible(self.cursor); }
            Action::PageUp => { self.cursor.0 = self.cursor.0.saturating_sub(self.viewport.visible_rows); self.viewport.ensure_visible(self.cursor); }
            Action::Undo => {
                if let Some(entry) = self.undo_stack.undo() {
                    self.apply_undo_entry(&entry, false);
                    self.dirty = true;
                }
            }
            Action::Redo => {
                if let Some(entry) = self.undo_stack.redo() {
                    self.apply_undo_entry(&entry, true);
                    self.dirty = true;
                }
            }
            _ => {
                // Remaining actions (Save, Search, Sort, Yank, Paste, etc.) will be
                // implemented in subsequent tasks
            }
        }
    }

    fn apply_undo_entry(&mut self, entry: &UndoEntry, redo: bool) {
        match entry {
            UndoEntry::CellEdit { pos, old_raw, new_raw } => {
                let raw = if redo { new_raw } else { old_raw };
                if raw.is_empty() {
                    self.sheet.clear_cell(*pos);
                } else if raw.starts_with('=') {
                    set_formula(&mut self.sheet, &mut self.deps, *pos, raw);
                } else {
                    self.sheet.set_cell(*pos, raw);
                }
                recalculate(&mut self.sheet, &self.deps);
            }
        }
    }
}
