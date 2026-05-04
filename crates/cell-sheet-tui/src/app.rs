mod dispatch;

use crate::action::{Action, CaseOp, Mode};
use crate::clipboard::Register;
use crate::command_state::CommandState;
use crate::file_format::FileFormat;
use crate::mode::mouse::GridLayout;
use crate::mode::visual::VisualKind;
use crate::search::SearchState;
use crate::status::StatusManager;
use crate::undo::{UndoEntry, UndoStack};
use crate::viewport::Viewport;
use cell_sheet_core::engine::SheetEngine;
use cell_sheet_core::formula::deps::DepGraph;
use cell_sheet_core::help::HelpRegistry;
use cell_sheet_core::model::{CellPos, Sheet};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct App {
    pub sheet: Sheet,
    pub deps: DepGraph,
    pub viewport: Viewport,
    pub cursor: CellPos,
    pub mode: Mode,
    pub register: Option<Register>,
    pub undo_stack: UndoStack,
    pub command: CommandState,
    pub status: StatusManager,
    pub search: SearchState,
    pub file_path: Option<PathBuf>,
    pub file_format: FileFormat,
    pub dirty: bool,
    pub should_quit: bool,
    pub insert_buffer: String,
    pub delimiter: u8,
    /// Off by default. Toggled by `:set mouse on|off|toggle`. When true,
    /// the run_loop has issued `EnableMouseCapture` to the terminal and
    /// is routing `Event::Mouse` to `mode::mouse::handle_mouse_event`.
    pub mouse_enabled: bool,
    /// Geometry from the most recent grid render, used by mouse hit-testing
    /// on the next event. `None` until the first frame is drawn.
    pub last_grid_layout: Option<GridLayout>,
    pub help_scroll: usize,
    pub help_topic: Option<String>,
    pub help_registry: HelpRegistry,
    pub marks: HashMap<char, CellPos>,
    pub jump_list: Vec<CellPos>,
    pub jump_idx: usize,
    pub last_visual: Option<LastVisual>,
    /// Depth counter for deferred recalculation batches.
    /// When > 0, `EditCell` skips `recalculate`; `commit_batch` triggers it
    /// once when the outermost batch is closed.
    batch_depth: u32,
    pub last_change: Option<Action>,
}

const JUMP_LIST_CAP: usize = 100;

#[derive(Debug, Clone, Copy)]
pub struct LastVisual {
    pub anchor: CellPos,
    pub cursor: CellPos,
    pub kind: VisualKind,
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
            command: CommandState::default(),
            status: StatusManager::default(),
            search: SearchState::default(),
            file_path: None,
            file_format: FileFormat::Csv,
            dirty: false,
            should_quit: false,
            insert_buffer: String::new(),
            delimiter: b',',
            mouse_enabled: false,
            last_grid_layout: None,
            help_scroll: 0,
            help_topic: None,
            help_registry: HelpRegistry::new(),
            marks: HashMap::new(),
            jump_list: Vec::new(),
            jump_idx: 0,
            last_visual: None,
            batch_depth: 0,
            last_change: None,
        }
    }

    /// Snapshot the current visual selection so a later `gv` can re-enter it.
    pub fn record_last_visual(&mut self, anchor: CellPos, kind: VisualKind) {
        self.last_visual = Some(LastVisual {
            anchor,
            cursor: self.cursor,
            kind,
        });
    }

    /// Push the current cursor onto the jump list as a "from" position
    /// before a big jump. Following vim, recording mid-stack truncates the
    /// forward history. Capped at `JUMP_LIST_CAP` entries.
    pub fn record_jump(&mut self) {
        if self.jump_idx < self.jump_list.len() {
            self.jump_list.truncate(self.jump_idx);
        }
        self.jump_list.push(self.cursor);
        if self.jump_list.len() > JUMP_LIST_CAP {
            let overflow = self.jump_list.len() - JUMP_LIST_CAP;
            self.jump_list.drain(0..overflow);
        }
        self.jump_idx = self.jump_list.len();
    }

    pub fn has_formulas(&self) -> bool {
        self.sheet.cells.values().any(|c| c.raw.starts_with('='))
    }

    fn rebind_change_to_cursor(&self, action: Action) -> Action {
        let cursor = self.cursor;
        match action {
            Action::EditCell(_, raw) => Action::EditCell(cursor, raw),
            Action::ClearCell(_) => Action::ClearCell(cursor),
            Action::ClearRange { start, end } => {
                let dr = end.0.saturating_sub(start.0);
                let dc = end.1.saturating_sub(start.1);
                Action::ClearRange {
                    start: cursor,
                    end: (cursor.0 + dr, cursor.1 + dc),
                }
            }
            Action::DeleteRow { count, .. } => Action::DeleteRow {
                start: cursor.0,
                count,
            },
            Action::Paste(_) => Action::Paste(cursor),
            Action::PasteBefore(_) => Action::PasteBefore(cursor),
            Action::CaseOpCell { op, .. } => Action::CaseOpCell { pos: cursor, op },
            Action::AdjustNumber { delta, .. } => Action::AdjustNumber { pos: cursor, delta },
            _ => action,
        }
    }

    fn format_from_path(path: &Path) -> FileFormat {
        FileFormat::from_path(path)
    }

    fn do_save(&mut self, path: &PathBuf, format: FileFormat) {
        let delimiter = self.delimiter;
        let result = match format {
            FileFormat::Csv | FileFormat::Tsv => std::fs::File::create(path)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
                .and_then(|f| cell_sheet_core::io::csv::write_csv(&self.sheet, f, delimiter)),
            FileFormat::Cell => std::fs::File::create(path)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
                .and_then(|f| cell_sheet_core::io::cell_format::write_cell_format(&self.sheet, f)),
        };

        match result {
            Ok(()) => {
                self.file_path = Some(path.clone());
                self.file_format = format;
                self.dirty = false;
                self.status.set(format!("Written to {}", path.display()));
            }
            Err(e) => {
                self.status.set(format!("Error saving: {}", e));
            }
        }
    }

    fn cell_is_non_empty(&self, row: usize, col: usize) -> bool {
        self.sheet.get_cell((row, col)).is_some()
    }

    /// Spec: `}` from a non-empty cell jumps to the first empty row at-or-after
    /// the current block's last non-empty row. From an empty cell, jumps to
    /// the first non-empty row below. Returns `None` at the sheet boundary.
    pub fn block_jump_down(&self) -> Option<usize> {
        let (row, col) = self.cursor;
        let n = self.sheet.row_count;
        if row + 1 >= n {
            return None;
        }
        let start_filled = self.cell_is_non_empty(row, col);
        let mut r = row + 1;
        while r < n && self.cell_is_non_empty(r, col) == start_filled {
            r += 1;
        }
        if r < n {
            Some(r)
        } else {
            None
        }
    }

    /// Symmetric to `block_jump_down`.
    pub fn block_jump_up(&self) -> Option<usize> {
        let (row, col) = self.cursor;
        if row == 0 {
            return None;
        }
        let start_filled = self.cell_is_non_empty(row, col);
        let mut r = row - 1;
        loop {
            if self.cell_is_non_empty(r, col) != start_filled {
                return Some(r);
            }
            if r == 0 {
                return None;
            }
            r -= 1;
        }
    }

    fn apply_undo_entry(&mut self, entry: &UndoEntry, redo: bool) {
        match entry {
            UndoEntry::CellEdit {
                pos,
                old_raw,
                new_raw,
            } => {
                let raw = if redo { new_raw } else { old_raw };
                self.write_cell_raw(*pos, raw);
                if self.batch_depth == 0 {
                    self.recalculate();
                }
            }
            UndoEntry::MultiCellEdit { changes } => {
                self.begin_batch();
                for (pos, old_raw, new_raw) in changes {
                    let raw = if redo { new_raw } else { old_raw };
                    self.write_cell_raw(*pos, raw);
                }
                self.commit_batch();
            }
        }
    }

    /// Start a deferred-recalculation batch. While a batch is active,
    /// `EditCell` writes cells and marks them dirty but defers the full
    /// topological recalc. The recalc runs once when `commit_batch` returns
    /// the depth to zero. Batches nest: every `begin_batch` must have a
    /// matching `commit_batch`.
    pub fn begin_batch(&mut self) {
        self.batch_depth += 1;
    }

    /// End a deferred-recalculation batch. Triggers `recalculate` exactly
    /// once when the outermost batch is committed (depth returns to zero).
    pub fn commit_batch(&mut self) {
        if self.batch_depth == 0 {
            return;
        }
        self.batch_depth -= 1;
        if self.batch_depth == 0 {
            self.recalculate();
        }
    }

    fn recalculate(&mut self) {
        SheetEngine::new(&mut self.sheet, &mut self.deps).recalculate();
    }

    fn set_cell_raw(&mut self, pos: CellPos, raw: &str) {
        SheetEngine::new(&mut self.sheet, &mut self.deps).set_cell_raw(pos, raw);
    }

    /// Write `raw` into `pos`, keeping the dep graph consistent and marking
    /// dependents dirty. Does not call `recalculate` — callers batch that.
    ///
    /// - empty raw → clear cell + drop graph entry
    /// - formula  → register/replace dependencies
    /// - value    → drop any stale graph entry from a prior formula
    fn write_cell_raw(&mut self, pos: CellPos, raw: &str) {
        SheetEngine::new(&mut self.sheet, &mut self.deps).write_cell_raw(pos, raw);
    }
}

fn apply_case_op(s: &str, op: CaseOp) -> String {
    match op {
        CaseOp::ToLower => s.to_lowercase(),
        CaseOp::ToUpper => s.to_uppercase(),
        CaseOp::ToggleAll => s
            .chars()
            .map(|c| {
                if c.is_uppercase() {
                    c.to_lowercase().next().unwrap_or(c)
                } else if c.is_lowercase() {
                    c.to_uppercase().next().unwrap_or(c)
                } else {
                    c
                }
            })
            .collect(),
        CaseOp::ToggleFirst => {
            let mut chars = s.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => {
                    let toggled = if c.is_uppercase() {
                        c.to_lowercase().next().unwrap_or(c)
                    } else if c.is_lowercase() {
                        c.to_uppercase().next().unwrap_or(c)
                    } else {
                        c
                    };
                    std::iter::once(toggled).chain(chars).collect()
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Action;

    // ── ClearRange (visual-mode d) ──────────────────────────────────────────

    #[test]
    fn clear_range_can_be_undone() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "a".into()));
        app.process_action(Action::EditCell((0, 1), "b".into()));
        app.process_action(Action::EditCell((1, 0), "c".into()));
        app.process_action(Action::EditCell((1, 1), "d".into()));
        app.process_action(Action::ClearRange {
            start: (0, 0),
            end: (1, 1),
        });
        assert!(app.sheet.get_cell((0, 0)).is_none());
        app.process_action(Action::Undo);
        assert_eq!(
            app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
            Some("a")
        );
        assert_eq!(
            app.sheet.get_cell((0, 1)).map(|c| c.raw.as_str()),
            Some("b")
        );
        assert_eq!(
            app.sheet.get_cell((1, 0)).map(|c| c.raw.as_str()),
            Some("c")
        );
        assert_eq!(
            app.sheet.get_cell((1, 1)).map(|c| c.raw.as_str()),
            Some("d")
        );
    }

    #[test]
    fn clear_range_can_be_redone() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "a".into()));
        app.process_action(Action::EditCell((0, 1), "b".into()));
        app.process_action(Action::ClearRange {
            start: (0, 0),
            end: (0, 1),
        });
        app.process_action(Action::Undo);
        assert_eq!(
            app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
            Some("a")
        );
        app.process_action(Action::Redo);
        assert!(app.sheet.get_cell((0, 0)).is_none());
        assert!(app.sheet.get_cell((0, 1)).is_none());
    }

    #[test]
    fn clear_range_undo_preserves_formula() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "=1+1".into()));
        app.process_action(Action::ClearRange {
            start: (0, 0),
            end: (0, 0),
        });
        app.process_action(Action::Undo);
        assert_eq!(
            app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
            Some("=1+1")
        );
    }

    // ── ChangeRange (visual-mode c) ────────────────────────────────────────────

    #[test]
    fn change_range_single_undo_restores_all_cells() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "a".into()));
        app.process_action(Action::EditCell((0, 1), "b".into()));
        app.process_action(Action::EditCell((1, 0), "c".into()));
        app.process_action(Action::EditCell((1, 1), "d".into()));
        app.process_action(Action::ChangeRange {
            start: (0, 0),
            end: (1, 1),
        });
        assert!(app.sheet.get_cell((0, 0)).is_none());
        app.process_action(Action::Undo);
        assert_eq!(
            app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
            Some("a")
        );
        assert_eq!(
            app.sheet.get_cell((0, 1)).map(|c| c.raw.as_str()),
            Some("b")
        );
        assert_eq!(
            app.sheet.get_cell((1, 0)).map(|c| c.raw.as_str()),
            Some("c")
        );
        assert_eq!(
            app.sheet.get_cell((1, 1)).map(|c| c.raw.as_str()),
            Some("d")
        );
    }

    #[test]
    fn change_range_undo_preserves_formula() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "=1+1".into()));
        app.process_action(Action::ChangeRange {
            start: (0, 0),
            end: (0, 0),
        });
        app.process_action(Action::Undo);
        assert_eq!(
            app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
            Some("=1+1")
        );
        let val = app.sheet.get_cell((0, 0)).map(|c| c.value.to_string());
        assert_eq!(val.as_deref(), Some("2"));
    }

    #[test]
    fn change_range_can_be_redone() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "a".into()));
        app.process_action(Action::EditCell((0, 1), "b".into()));
        app.process_action(Action::ChangeRange {
            start: (0, 0),
            end: (0, 1),
        });
        app.process_action(Action::Undo);
        assert_eq!(
            app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
            Some("a")
        );
        assert_eq!(
            app.sheet.get_cell((0, 1)).map(|c| c.raw.as_str()),
            Some("b")
        );
        app.process_action(Action::Redo);
        assert!(app.sheet.get_cell((0, 0)).is_none());
        assert!(app.sheet.get_cell((0, 1)).is_none());
    }

    #[test]
    fn change_range_of_empty_cells_no_undo_entry() {
        let mut app = App::new();
        // Set up one prior edit so the undo stack has exactly one entry.
        app.process_action(Action::EditCell((5, 5), "prior".into()));
        app.sheet.col_count = 2;
        app.sheet.row_count = 2;
        // ChangeRange on a range with no non-empty cells: should push nothing.
        app.process_action(Action::ChangeRange {
            start: (0, 0),
            end: (1, 1),
        });
        // The only undo step is the prior EditCell — not the ChangeRange.
        app.process_action(Action::Undo);
        assert!(
            app.sheet.get_cell((5, 5)).is_none(),
            "undo should have reverted the prior edit, not a no-op ChangeRange"
        );
    }

    // ── Paste / PasteBefore ─────────────────────────────────────────────────

    fn raw_at(app: &App, pos: CellPos) -> Option<String> {
        app.sheet.get_cell(pos).map(|c| c.raw.clone())
    }

    #[test]
    fn paste_cell_register_can_be_undone_into_empty() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "hello".into()));
        app.process_action(Action::YankCell((0, 0)));
        app.process_action(Action::Paste((0, 1)));
        assert_eq!(raw_at(&app, (0, 1)).as_deref(), Some("hello"));
        app.process_action(Action::Undo);
        assert!(app.sheet.get_cell((0, 1)).is_none());
    }

    #[test]
    fn paste_cell_register_undo_restores_prior_content() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "new".into()));
        app.process_action(Action::EditCell((0, 1), "old".into()));
        app.process_action(Action::YankCell((0, 0)));
        app.process_action(Action::Paste((0, 1)));
        assert_eq!(raw_at(&app, (0, 1)).as_deref(), Some("new"));
        app.process_action(Action::Undo);
        assert_eq!(raw_at(&app, (0, 1)).as_deref(), Some("old"));
    }

    #[test]
    fn paste_cell_register_can_be_redone() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "hello".into()));
        app.process_action(Action::YankCell((0, 0)));
        app.process_action(Action::Paste((0, 1)));
        app.process_action(Action::Undo);
        app.process_action(Action::Redo);
        assert_eq!(raw_at(&app, (0, 1)).as_deref(), Some("hello"));
    }

    #[test]
    fn paste_cell_identical_content_is_a_noop() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "same".into()));
        app.process_action(Action::EditCell((0, 1), "same".into()));
        app.process_action(Action::YankCell((0, 0)));
        app.process_action(Action::Paste((0, 1)));
        // Undo of the prior EditCell should restore (0,1) to empty,
        // proving that the no-op paste did not consume an undo slot.
        app.process_action(Action::Undo);
        assert!(app.sheet.get_cell((0, 1)).is_none());
    }

    #[test]
    fn paste_row_register_can_be_undone() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "x".into()));
        app.process_action(Action::EditCell((0, 1), "y".into()));
        app.process_action(Action::YankRow { start: 0, count: 1 });
        // P on row 1 puts the yanked row on row 1.
        app.process_action(Action::PasteBefore((1, 0)));
        assert_eq!(raw_at(&app, (1, 0)).as_deref(), Some("x"));
        assert_eq!(raw_at(&app, (1, 1)).as_deref(), Some("y"));
        app.process_action(Action::Undo);
        assert!(app.sheet.get_cell((1, 0)).is_none());
        assert!(app.sheet.get_cell((1, 1)).is_none());
    }

    #[test]
    fn paste_row_register_can_be_redone() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "x".into()));
        app.process_action(Action::EditCell((0, 1), "y".into()));
        app.process_action(Action::YankRow { start: 0, count: 1 });
        app.process_action(Action::PasteBefore((1, 0)));
        app.process_action(Action::Undo);
        app.process_action(Action::Redo);
        assert_eq!(raw_at(&app, (1, 0)).as_deref(), Some("x"));
        assert_eq!(raw_at(&app, (1, 1)).as_deref(), Some("y"));
    }

    #[test]
    fn paste_row_register_undo_restores_prior_content() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "x".into()));
        app.process_action(Action::EditCell((0, 1), "y".into()));
        app.process_action(Action::EditCell((1, 0), "old0".into()));
        app.process_action(Action::EditCell((1, 1), "old1".into()));
        app.process_action(Action::YankRow { start: 0, count: 1 });
        app.process_action(Action::PasteBefore((1, 0)));
        assert_eq!(raw_at(&app, (1, 0)).as_deref(), Some("x"));
        assert_eq!(raw_at(&app, (1, 1)).as_deref(), Some("y"));
        app.process_action(Action::Undo);
        assert_eq!(raw_at(&app, (1, 0)).as_deref(), Some("old0"));
        assert_eq!(raw_at(&app, (1, 1)).as_deref(), Some("old1"));
    }

    #[test]
    fn paste_block_register_can_be_undone() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "a".into()));
        app.process_action(Action::EditCell((0, 1), "b".into()));
        app.process_action(Action::YankRange {
            start: (0, 0),
            end: (0, 1),
        });
        app.process_action(Action::Paste((1, 0)));
        assert_eq!(raw_at(&app, (1, 0)).as_deref(), Some("a"));
        assert_eq!(raw_at(&app, (1, 1)).as_deref(), Some("b"));
        app.process_action(Action::Undo);
        assert!(app.sheet.get_cell((1, 0)).is_none());
        assert!(app.sheet.get_cell((1, 1)).is_none());
    }

    #[test]
    fn paste_block_register_can_be_redone() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "a".into()));
        app.process_action(Action::EditCell((0, 1), "b".into()));
        app.process_action(Action::YankRange {
            start: (0, 0),
            end: (0, 1),
        });
        app.process_action(Action::Paste((1, 0)));
        app.process_action(Action::Undo);
        app.process_action(Action::Redo);
        assert_eq!(raw_at(&app, (1, 0)).as_deref(), Some("a"));
        assert_eq!(raw_at(&app, (1, 1)).as_deref(), Some("b"));
    }

    #[test]
    fn paste_block_register_undo_restores_prior_content() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "a".into()));
        app.process_action(Action::EditCell((0, 1), "b".into()));
        app.process_action(Action::EditCell((1, 0), "old_a".into()));
        app.process_action(Action::EditCell((1, 1), "old_b".into()));
        app.process_action(Action::YankRange {
            start: (0, 0),
            end: (0, 1),
        });
        app.process_action(Action::Paste((1, 0)));
        assert_eq!(raw_at(&app, (1, 0)).as_deref(), Some("a"));
        app.process_action(Action::Undo);
        assert_eq!(raw_at(&app, (1, 0)).as_deref(), Some("old_a"));
        assert_eq!(raw_at(&app, (1, 1)).as_deref(), Some("old_b"));
    }

    #[test]
    fn paste_formula_can_be_undone() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "=1+1".into()));
        app.process_action(Action::YankCell((0, 0)));
        app.process_action(Action::Paste((0, 1)));
        app.process_action(Action::Undo);
        assert!(app.sheet.get_cell((0, 1)).is_none());
    }

    #[test]
    fn pasted_formula_is_evaluated() {
        use cell_sheet_core::model::CellValue;
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "=1+1".into()));
        app.process_action(Action::YankCell((0, 0)));
        app.process_action(Action::Paste((0, 1)));
        // Pasted formula should be evaluated, not left at its default.
        assert_eq!(
            app.sheet.get_cell((0, 1)).map(|c| c.value.clone()),
            Some(CellValue::Number(2.0))
        );
    }

    #[test]
    fn clear_range_of_only_empty_cells_does_not_dirty() {
        let mut app = App::new();
        app.sheet.col_count = 2;
        app.sheet.row_count = 2;
        app.dirty = false;
        app.process_action(Action::ClearRange {
            start: (0, 0),
            end: (1, 1),
        });
        assert!(!app.dirty);
    }

    #[test]
    fn paste_block_of_n_cells_is_single_undo_step() {
        let mut app = App::new();
        // Fill a 3×3 source block.
        for row in 0..3_usize {
            for col in 0..3_usize {
                let val = format!("r{}c{}", row, col);
                app.process_action(Action::EditCell((row, col), val));
            }
        }
        app.process_action(Action::YankRange {
            start: (0, 0),
            end: (2, 2),
        });
        // Paste at (3, 0): fills rows 3–5, cols 0–2 (9 cells).
        app.process_action(Action::Paste((3, 0)));
        for row in 3..6_usize {
            for col in 0..3_usize {
                assert!(
                    app.sheet.get_cell((row, col)).is_some(),
                    "expected cell ({row},{col}) to be filled after paste"
                );
            }
        }
        // A single undo should clear all 9 pasted cells.
        app.process_action(Action::Undo);
        for row in 3..6_usize {
            for col in 0..3_usize {
                assert!(
                    app.sheet.get_cell((row, col)).is_none(),
                    "expected cell ({row},{col}) to be empty after single undo"
                );
            }
        }
    }

    // ── Help ────────────────────────────────────────────────────────────────

    #[test]
    fn show_help_toc_sets_help_mode() {
        let mut app = App::new();
        app.process_action(Action::ShowHelp(None));
        assert_eq!(app.mode, Mode::Help);
        assert_eq!(app.help_topic, None);
        assert_eq!(app.help_scroll, 0);
    }

    #[test]
    fn show_help_valid_topic() {
        let mut app = App::new();
        app.process_action(Action::ShowHelp(Some("dd".into())));
        assert_eq!(app.mode, Mode::Help);
        assert_eq!(app.help_topic, Some("dd".into()));
    }

    #[test]
    fn show_help_invalid_topic_stays_normal() {
        let mut app = App::new();
        app.mode = Mode::Normal;
        app.process_action(Action::ShowHelp(Some("nonexistent".into())));
        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(app.status.as_deref(), Some("No help for 'nonexistent'"));
    }

    #[test]
    fn help_mode_back_to_normal() {
        let mut app = App::new();
        app.process_action(Action::ShowHelp(None));
        assert_eq!(app.mode, Mode::Help);
        app.process_action(Action::ChangeMode(Mode::Normal));
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn dd_can_be_undone() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "hello".into()));
        app.process_action(Action::EditCell((0, 1), "world".into()));
        app.process_action(Action::EditCell((1, 0), "keep".into()));

        app.process_action(Action::DeleteRow { start: 0, count: 1 });
        assert!(app.sheet.get_cell((0, 0)).is_none());
        assert!(app.sheet.get_cell((0, 1)).is_none());
        assert_eq!(
            app.sheet.get_cell((1, 0)).map(|c| c.raw.as_str()),
            Some("keep")
        );

        app.process_action(Action::Undo);
        assert_eq!(
            app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
            Some("hello")
        );
        assert_eq!(
            app.sheet.get_cell((0, 1)).map(|c| c.raw.as_str()),
            Some("world")
        );
        assert_eq!(
            app.sheet.get_cell((1, 0)).map(|c| c.raw.as_str()),
            Some("keep")
        );
    }

    #[test]
    fn dd_can_be_redone() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "hello".into()));
        app.process_action(Action::EditCell((0, 1), "world".into()));

        app.process_action(Action::DeleteRow { start: 0, count: 1 });
        app.process_action(Action::Undo);
        app.process_action(Action::Redo);

        assert!(app.sheet.get_cell((0, 0)).is_none());
        assert!(app.sheet.get_cell((0, 1)).is_none());
    }

    // ── Search prompt (/?) ──────────────────────────────────────────────────

    #[test]
    fn enter_search_forward_sets_command_mode_with_slash_prefix() {
        use crate::action::{CommandKind, SearchDirection};
        let mut app = App::new();
        app.process_action(Action::EnterSearch(SearchDirection::Forward));
        assert_eq!(app.mode, Mode::Command);
        assert_eq!(app.command.kind, CommandKind::Slash);
        assert!(app.command.line.is_empty());
    }

    #[test]
    fn enter_search_backward_sets_question_prefix() {
        use crate::action::{CommandKind, SearchDirection};
        let mut app = App::new();
        app.process_action(Action::EnterSearch(SearchDirection::Backward));
        assert_eq!(app.mode, Mode::Command);
        assert_eq!(app.command.kind, CommandKind::Question);
    }

    #[test]
    fn change_mode_command_resets_to_colon() {
        use crate::action::{CommandKind, SearchDirection};
        let mut app = App::new();
        // Enter search, then user backs out and re-enters via `:`.
        app.process_action(Action::EnterSearch(SearchDirection::Forward));
        app.process_action(Action::ChangeMode(Mode::Normal));
        app.process_action(Action::ChangeMode(Mode::Command));
        assert_eq!(app.command.kind, CommandKind::Colon);
    }

    #[test]
    fn forward_search_populates_pattern_and_moves_cursor() {
        use crate::action::SearchDirection;
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "alpha".into()));
        app.process_action(Action::EditCell((0, 1), "beta".into()));
        app.process_action(Action::EditCell((1, 0), "gamma".into()));
        app.cursor = (0, 0);
        app.process_action(Action::Search {
            pattern: "gamma".into(),
            direction: SearchDirection::Forward,
        });
        assert_eq!(app.search.pattern.as_deref(), Some("gamma"));
        assert_eq!(app.cursor, (1, 0));
    }

    #[test]
    fn backward_search_walks_backwards() {
        use crate::action::SearchDirection;
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "alpha".into()));
        app.process_action(Action::EditCell((1, 0), "beta".into()));
        app.process_action(Action::EditCell((2, 0), "alpha".into()));
        app.cursor = (1, 0);
        app.process_action(Action::Search {
            pattern: "alpha".into(),
            direction: SearchDirection::Backward,
        });
        assert_eq!(app.cursor, (0, 0));
    }

    #[test]
    fn search_next_after_search_keeps_stepping() {
        use crate::action::SearchDirection;
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "x".into()));
        app.process_action(Action::EditCell((1, 0), "x".into()));
        app.process_action(Action::EditCell((2, 0), "x".into()));
        app.cursor = (0, 0);
        app.process_action(Action::Search {
            pattern: "x".into(),
            direction: SearchDirection::Forward,
        });
        assert_eq!(app.cursor, (1, 0));
        app.process_action(Action::SearchNext);
        assert_eq!(app.cursor, (2, 0));
    }

    #[test]
    fn empty_pattern_does_not_overwrite_search_pattern() {
        use crate::action::SearchDirection;
        let mut app = App::new();
        app.process_action(Action::Search {
            pattern: "first".into(),
            direction: SearchDirection::Forward,
        });
        app.process_action(Action::Search {
            pattern: String::new(),
            direction: SearchDirection::Forward,
        });
        assert_eq!(app.search.pattern.as_deref(), Some("first"));
    }

    // ── Incremental search ──────────────────────────────────────────────────

    #[test]
    fn enter_search_records_origin() {
        use crate::action::SearchDirection;
        let mut app = App::new();
        app.cursor = (3, 2);
        app.process_action(Action::EnterSearch(SearchDirection::Forward));
        assert_eq!(app.search.origin, Some((3, 2)));
    }

    #[test]
    fn incremental_search_moves_cursor_as_user_types() {
        use crate::action::SearchDirection;
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "alpha".into()));
        app.process_action(Action::EditCell((1, 0), "beta".into()));
        app.process_action(Action::EditCell((2, 0), "gamma".into()));
        app.cursor = (0, 0);
        app.process_action(Action::EnterSearch(SearchDirection::Forward));
        // User types `g` — should jump to "gamma" already.
        app.process_action(Action::SearchIncremental {
            pattern: "g".into(),
            direction: SearchDirection::Forward,
        });
        assert_eq!(app.cursor, (2, 0));
        // User adds `am` — still on gamma.
        app.process_action(Action::SearchIncremental {
            pattern: "gam".into(),
            direction: SearchDirection::Forward,
        });
        assert_eq!(app.cursor, (2, 0));
    }

    #[test]
    fn incremental_search_does_not_compound_movement() {
        use crate::action::SearchDirection;
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "alpha".into()));
        app.process_action(Action::EditCell((1, 0), "alpine".into()));
        app.process_action(Action::EditCell((2, 0), "ant".into()));
        app.cursor = (0, 0);
        app.process_action(Action::EnterSearch(SearchDirection::Forward));
        // First match for "a" with origin=(0,0) and include_origin=true is
        // (0,0) itself. Without origin tracking, a second char would search
        // again from (0,0) and stay; with origin tracking, "al" should still
        // anchor on the origin and find (0,0) (alpha contains "al").
        app.process_action(Action::SearchIncremental {
            pattern: "a".into(),
            direction: SearchDirection::Forward,
        });
        assert_eq!(app.cursor, (0, 0));
        app.process_action(Action::SearchIncremental {
            pattern: "alp".into(),
            direction: SearchDirection::Forward,
        });
        assert_eq!(app.cursor, (0, 0));
        app.process_action(Action::SearchIncremental {
            pattern: "alpi".into(),
            direction: SearchDirection::Forward,
        });
        assert_eq!(app.cursor, (1, 0));
    }

    #[test]
    fn incremental_search_empty_pattern_restores_to_origin() {
        use crate::action::SearchDirection;
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "alpha".into()));
        app.process_action(Action::EditCell((2, 0), "gamma".into()));
        app.cursor = (0, 0);
        app.process_action(Action::EnterSearch(SearchDirection::Forward));
        app.process_action(Action::SearchIncremental {
            pattern: "g".into(),
            direction: SearchDirection::Forward,
        });
        assert_eq!(app.cursor, (2, 0));
        // User backspaces back to empty — cursor should snap back to origin.
        app.process_action(Action::SearchIncremental {
            pattern: String::new(),
            direction: SearchDirection::Forward,
        });
        assert_eq!(app.cursor, (0, 0));
    }

    #[test]
    fn incremental_search_no_match_snaps_back_to_origin() {
        use crate::action::SearchDirection;
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "alpha".into()));
        app.process_action(Action::EditCell((2, 0), "gamma".into()));
        app.cursor = (0, 0);
        app.process_action(Action::EnterSearch(SearchDirection::Forward));
        app.process_action(Action::SearchIncremental {
            pattern: "g".into(),
            direction: SearchDirection::Forward,
        });
        assert_eq!(app.cursor, (2, 0));
        // Add a char that breaks the match — cursor should reset to origin.
        app.process_action(Action::SearchIncremental {
            pattern: "gz".into(),
            direction: SearchDirection::Forward,
        });
        assert_eq!(app.cursor, (0, 0));
    }

    #[test]
    fn cancel_search_restores_cursor_and_returns_to_normal() {
        use crate::action::SearchDirection;
        let mut app = App::new();
        app.process_action(Action::EditCell((2, 0), "gamma".into()));
        app.cursor = (0, 0);
        app.process_action(Action::EnterSearch(SearchDirection::Forward));
        app.process_action(Action::SearchIncremental {
            pattern: "g".into(),
            direction: SearchDirection::Forward,
        });
        assert_eq!(app.cursor, (2, 0));
        app.process_action(Action::CancelSearch);
        assert_eq!(app.cursor, (0, 0));
        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(app.search.origin, None);
    }

    #[test]
    fn submit_search_commits_at_incremental_position() {
        use crate::action::SearchDirection;
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "alpha".into()));
        app.process_action(Action::EditCell((2, 0), "gamma".into()));
        app.cursor = (0, 0);
        app.process_action(Action::EnterSearch(SearchDirection::Forward));
        app.process_action(Action::SearchIncremental {
            pattern: "g".into(),
            direction: SearchDirection::Forward,
        });
        // Enter pressed: Action::Search submitted with the typed pattern.
        app.process_action(Action::Search {
            pattern: "g".into(),
            direction: SearchDirection::Forward,
        });
        assert_eq!(app.cursor, (2, 0));
        assert_eq!(app.search.pattern.as_deref(), Some("g"));
        assert_eq!(app.search.origin, None);
    }

    #[test]
    fn submit_search_with_no_match_returns_to_origin() {
        use crate::action::SearchDirection;
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "alpha".into()));
        app.cursor = (0, 0);
        app.process_action(Action::EnterSearch(SearchDirection::Forward));
        app.process_action(Action::Search {
            pattern: "zzz".into(),
            direction: SearchDirection::Forward,
        });
        assert_eq!(app.cursor, (0, 0));
        assert!(app.status.as_deref().is_some());
    }

    #[test]
    fn submit_empty_search_after_typing_restores_to_origin() {
        use crate::action::SearchDirection;
        let mut app = App::new();
        app.process_action(Action::EditCell((2, 0), "gamma".into()));
        app.cursor = (0, 0);
        app.process_action(Action::EnterSearch(SearchDirection::Forward));
        app.process_action(Action::SearchIncremental {
            pattern: "g".into(),
            direction: SearchDirection::Forward,
        });
        // User backspaces to empty then hits Enter.
        app.process_action(Action::Search {
            pattern: String::new(),
            direction: SearchDirection::Forward,
        });
        assert_eq!(app.cursor, (0, 0));
        assert_eq!(app.search.origin, None);
    }

    // ── f / F / ; / , ───────────────────────────────────────────────────────

    fn setup_row(app: &mut App, cells: &[(usize, &str)]) {
        for (col, raw) in cells {
            app.process_action(Action::EditCell((0, *col), (*raw).into()));
        }
    }

    #[test]
    fn find_char_forward_lands_on_starts_with_match() {
        let mut app = App::new();
        setup_row(
            &mut app,
            &[(0, "alpha"), (1, "beta"), (2, "gamma"), (3, "delta")],
        );
        app.cursor = (0, 0);
        app.process_action(Action::FindCharInRow {
            ch: 'g',
            forward: true,
            inclusive: true,
        });
        assert_eq!(app.cursor, (0, 2));
    }

    #[test]
    fn find_char_is_case_insensitive() {
        let mut app = App::new();
        setup_row(&mut app, &[(0, "alpha"), (1, "Beta"), (2, "gamma")]);
        app.cursor = (0, 0);
        app.process_action(Action::FindCharInRow {
            ch: 'b',
            forward: true,
            inclusive: true,
        });
        assert_eq!(app.cursor, (0, 1));
    }

    #[test]
    fn find_char_backward_lands_on_match() {
        let mut app = App::new();
        setup_row(
            &mut app,
            &[(0, "alpha"), (1, "beta"), (2, "gamma"), (3, "delta")],
        );
        app.cursor = (0, 3);
        app.process_action(Action::FindCharInRow {
            ch: 'a',
            forward: false,
            inclusive: true,
        });
        assert_eq!(app.cursor, (0, 0));
    }

    #[test]
    fn find_char_no_match_keeps_cursor() {
        let mut app = App::new();
        setup_row(&mut app, &[(0, "alpha"), (1, "beta"), (2, "gamma")]);
        app.cursor = (0, 0);
        app.process_action(Action::FindCharInRow {
            ch: 'z',
            forward: true,
            inclusive: true,
        });
        assert_eq!(app.cursor, (0, 0));
    }

    #[test]
    fn find_char_skips_empty_cells() {
        let mut app = App::new();
        // Row has gaps at col 1 and 3.
        setup_row(&mut app, &[(0, "alpha"), (2, "beta"), (4, "berry")]);
        app.cursor = (0, 0);
        app.process_action(Action::FindCharInRow {
            ch: 'b',
            forward: true,
            inclusive: true,
        });
        assert_eq!(app.cursor, (0, 2));
    }

    #[test]
    fn find_char_remembers_last_target_for_repeat() {
        let mut app = App::new();
        setup_row(&mut app, &[(0, "alpha"), (1, "beta"), (2, "berry")]);
        app.cursor = (0, 0);
        app.process_action(Action::FindCharInRow {
            ch: 'b',
            forward: true,
            inclusive: true,
        });
        assert_eq!(app.cursor, (0, 1));
        app.process_action(Action::RepeatFind { reversed: false });
        assert_eq!(app.cursor, (0, 2));
    }

    #[test]
    fn comma_repeats_find_reversed_direction() {
        let mut app = App::new();
        setup_row(
            &mut app,
            &[(0, "alpha"), (1, "beta"), (2, "berry"), (3, "delta")],
        );
        app.cursor = (0, 0);
        app.process_action(Action::FindCharInRow {
            ch: 'b',
            forward: true,
            inclusive: true,
        });
        assert_eq!(app.cursor, (0, 1));
        // ; would step forward to col 2; , flips and steps back to col... but
        // there's no `b` to the left of col 1. Cursor should stay put.
        app.process_action(Action::RepeatFind { reversed: true });
        assert_eq!(app.cursor, (0, 1));
        // Now move right past berry, then , should walk back to it.
        app.cursor = (0, 3);
        app.process_action(Action::RepeatFind { reversed: true });
        assert_eq!(app.cursor, (0, 2));
    }

    #[test]
    fn repeat_find_with_no_prior_find_is_noop() {
        let mut app = App::new();
        setup_row(&mut app, &[(0, "alpha"), (1, "beta")]);
        app.cursor = (0, 0);
        app.process_action(Action::RepeatFind { reversed: false });
        assert_eq!(app.cursor, (0, 0));
    }

    #[test]
    fn find_char_uses_displayed_value_not_raw() {
        // Formula displayed value should drive the match, not the raw `=…`.
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "alpha".into()));
        app.process_action(Action::EditCell((0, 1), "=\"banana\"".into()));
        app.cursor = (0, 0);
        app.process_action(Action::FindCharInRow {
            ch: 'b',
            forward: true,
            inclusive: true,
        });
        assert_eq!(app.cursor, (0, 1));
    }

    // ── Viewport navigation (#30) ───────────────────────────────────────────

    #[test]
    fn zt_scrolls_cursor_to_top() {
        let mut app = App::new();
        app.viewport.visible_rows = 10;
        app.cursor = (50, 0);
        app.process_action(Action::ScrollCursorTop);
        assert_eq!(app.viewport.row_offset, 50);
        assert_eq!(app.cursor, (50, 0));
    }

    #[test]
    fn zz_centers_cursor() {
        let mut app = App::new();
        app.viewport.visible_rows = 10;
        app.cursor = (50, 0);
        app.process_action(Action::ScrollCursorCenter);
        assert_eq!(app.viewport.row_offset, 45);
    }

    #[test]
    fn zb_scrolls_cursor_to_bottom() {
        let mut app = App::new();
        app.viewport.visible_rows = 10;
        app.cursor = (50, 0);
        app.process_action(Action::ScrollCursorBottom);
        assert_eq!(app.viewport.row_offset, 41);
    }

    #[test]
    fn capital_h_jumps_cursor_to_viewport_top() {
        let mut app = App::new();
        app.viewport.visible_rows = 10;
        app.viewport.row_offset = 20;
        app.cursor = (25, 3);
        app.process_action(Action::CursorToViewportTop);
        assert_eq!(app.cursor, (20, 3));
    }

    #[test]
    fn capital_m_jumps_cursor_to_viewport_middle() {
        let mut app = App::new();
        app.viewport.visible_rows = 10;
        app.viewport.row_offset = 20;
        app.cursor = (20, 3);
        app.process_action(Action::CursorToViewportMiddle);
        assert_eq!(app.cursor, (25, 3));
    }

    #[test]
    fn capital_l_jumps_cursor_to_viewport_bottom() {
        let mut app = App::new();
        app.viewport.visible_rows = 10;
        app.viewport.row_offset = 20;
        app.cursor = (20, 3);
        app.process_action(Action::CursorToViewportBottom);
        assert_eq!(app.cursor, (29, 3));
    }

    #[test]
    fn ctrl_e_scrolls_viewport_without_moving_cursor() {
        let mut app = App::new();
        app.viewport.visible_rows = 10;
        app.cursor = (5, 0);
        app.process_action(Action::ScrollLineDown);
        assert_eq!(app.viewport.row_offset, 1);
        assert_eq!(app.cursor, (5, 0));
    }

    #[test]
    fn ctrl_y_scrolls_up_clamped_at_zero() {
        let mut app = App::new();
        app.viewport.visible_rows = 10;
        app.viewport.row_offset = 0;
        app.process_action(Action::ScrollLineUp);
        assert_eq!(app.viewport.row_offset, 0);
    }

    // ── Marks (#31) ─────────────────────────────────────────────────────────

    #[test]
    fn set_mark_records_cursor_position() {
        let mut app = App::new();
        app.cursor = (5, 3);
        app.process_action(Action::SetMark('a'));
        assert_eq!(app.marks.get(&'a'), Some(&(5, 3)));
    }

    #[test]
    fn backtick_jump_returns_to_exact_cell() {
        let mut app = App::new();
        app.cursor = (5, 3);
        app.process_action(Action::SetMark('a'));
        app.cursor = (0, 0);
        app.process_action(Action::JumpToMark {
            name: 'a',
            line_wise: false,
        });
        assert_eq!(app.cursor, (5, 3));
    }

    #[test]
    fn apostrophe_jump_returns_to_marked_row_column_zero() {
        let mut app = App::new();
        app.cursor = (5, 3);
        app.process_action(Action::SetMark('a'));
        app.cursor = (0, 0);
        app.process_action(Action::JumpToMark {
            name: 'a',
            line_wise: true,
        });
        assert_eq!(app.cursor, (5, 0));
    }

    #[test]
    fn jump_to_unset_mark_is_noop_with_status() {
        let mut app = App::new();
        app.cursor = (1, 1);
        app.process_action(Action::JumpToMark {
            name: 'q',
            line_wise: false,
        });
        assert_eq!(app.cursor, (1, 1));
        assert!(app.status.as_deref().unwrap().contains("Mark not set"));
    }

    #[test]
    fn set_mark_rejects_non_lowercase() {
        let mut app = App::new();
        app.cursor = (5, 3);
        app.process_action(Action::SetMark('A'));
        assert!(!app.marks.contains_key(&'A'));
        app.process_action(Action::SetMark('1'));
        assert!(!app.marks.contains_key(&'1'));
    }

    // ── Jump list (#32) ─────────────────────────────────────────────────────

    #[test]
    fn ctrl_o_returns_to_position_before_jump() {
        let mut app = App::new();
        app.sheet.row_count = 100;
        app.cursor = (0, 0);
        app.process_action(Action::GotoLastRow);
        assert_eq!(app.cursor, (99, 0));
        app.process_action(Action::JumpBack);
        assert_eq!(app.cursor, (0, 0));
    }

    #[test]
    fn ctrl_i_returns_to_jumped_position() {
        let mut app = App::new();
        app.sheet.row_count = 100;
        app.cursor = (0, 0);
        app.process_action(Action::GotoLastRow);
        app.process_action(Action::JumpBack);
        assert_eq!(app.cursor, (0, 0));
        app.process_action(Action::JumpForward);
        assert_eq!(app.cursor, (99, 0));
    }

    #[test]
    fn jump_list_capped_at_100_entries() {
        let mut app = App::new();
        app.sheet.row_count = 200;
        for r in 0..150 {
            app.cursor = (r, 0);
            app.process_action(Action::GotoLastRow);
        }
        assert!(app.jump_list.len() <= 100);
    }

    #[test]
    fn jump_back_with_empty_list_is_noop() {
        let mut app = App::new();
        app.cursor = (5, 0);
        app.process_action(Action::JumpBack);
        assert_eq!(app.cursor, (5, 0));
    }

    #[test]
    fn search_records_jump() {
        let mut app = App::new();
        app.process_action(Action::EditCell((5, 0), "foo".into()));
        app.cursor = (0, 0);
        app.process_action(Action::Search {
            pattern: "foo".into(),
            direction: crate::action::SearchDirection::Forward,
        });
        assert_eq!(app.cursor, (5, 0));
        app.process_action(Action::JumpBack);
        assert_eq!(app.cursor, (0, 0));
    }

    #[test]
    fn marks_jump_records_jump() {
        let mut app = App::new();
        app.cursor = (5, 5);
        app.process_action(Action::SetMark('a'));
        app.cursor = (0, 0);
        app.process_action(Action::JumpToMark {
            name: 'a',
            line_wise: false,
        });
        assert_eq!(app.cursor, (5, 5));
        app.process_action(Action::JumpBack);
        assert_eq!(app.cursor, (0, 0));
    }

    // ── Block jump (#35) ────────────────────────────────────────────────────

    #[test]
    fn block_jump_down_from_inside_block_to_first_empty() {
        let mut app = App::new();
        for r in 0..3 {
            app.process_action(Action::EditCell((r, 0), "x".into()));
        }
        app.process_action(Action::EditCell((4, 0), "y".into()));
        app.cursor = (1, 0);
        app.process_action(Action::BlockJumpDown);
        assert_eq!(app.cursor, (3, 0));
    }

    #[test]
    fn block_jump_down_from_empty_to_first_non_empty() {
        let mut app = App::new();
        app.sheet.row_count = 6;
        app.process_action(Action::EditCell((4, 0), "y".into()));
        app.cursor = (1, 0);
        app.process_action(Action::BlockJumpDown);
        assert_eq!(app.cursor, (4, 0));
    }

    #[test]
    fn block_jump_up_symmetric_finds_first_empty_above() {
        let mut app = App::new();
        for r in 0..2 {
            app.process_action(Action::EditCell((r, 0), "x".into()));
        }
        app.process_action(Action::EditCell((4, 0), "y".into()));
        app.cursor = (4, 0);
        app.process_action(Action::BlockJumpUp);
        // (4,0) is non-empty, so jump up to first empty row → row 3.
        assert_eq!(app.cursor, (3, 0));
    }

    #[test]
    fn block_jump_up_from_empty_to_first_non_empty() {
        let mut app = App::new();
        for r in 0..2 {
            app.process_action(Action::EditCell((r, 0), "x".into()));
        }
        app.process_action(Action::EditCell((5, 0), "y".into()));
        app.cursor = (4, 0);
        app.process_action(Action::BlockJumpUp);
        // (4,0) is empty, so jump up to first non-empty row → row 1.
        assert_eq!(app.cursor, (1, 0));
    }

    #[test]
    fn block_jump_up_at_top_is_noop() {
        let mut app = App::new();
        app.sheet.row_count = 5;
        app.cursor = (0, 0);
        app.process_action(Action::BlockJumpUp);
        assert_eq!(app.cursor, (0, 0));
    }

    #[test]
    fn block_jump_down_at_bottom_is_noop() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "x".into()));
        app.cursor = (0, 0);
        app.process_action(Action::BlockJumpDown);
        assert_eq!(app.cursor, (0, 0));
    }

    // ── gv reselect last visual (#37) ───────────────────────────────────────

    #[test]
    fn record_last_visual_saves_anchor_cursor_kind() {
        let mut app = App::new();
        app.cursor = (3, 4);
        app.record_last_visual((1, 2), VisualKind::Line);
        let lv = app.last_visual.unwrap();
        assert_eq!(lv.anchor, (1, 2));
        assert_eq!(lv.cursor, (3, 4));
        assert!(matches!(lv.kind, VisualKind::Line));
    }

    #[test]
    fn reselect_last_visual_action_is_a_noop_in_app() {
        let mut app = App::new();
        app.cursor = (1, 1);
        app.process_action(Action::ReselectLastVisual);
        assert_eq!(app.cursor, (1, 1));
        assert_eq!(app.mode, Mode::Normal);
    }

    // ── * / # search current cell value (#36) ───────────────────────────────

    #[test]
    fn star_searches_for_current_cell_value() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "foo".into()));
        app.process_action(Action::EditCell((3, 2), "foo".into()));
        app.cursor = (0, 0);
        app.process_action(Action::SearchCellValue { backward: false });
        assert_eq!(app.cursor, (3, 2));
        assert_eq!(app.search.pattern.as_deref(), Some("foo"));
    }

    #[test]
    fn hash_searches_backward_for_current_cell_value() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "foo".into()));
        app.process_action(Action::EditCell((3, 2), "foo".into()));
        app.cursor = (3, 2);
        app.process_action(Action::SearchCellValue { backward: true });
        assert_eq!(app.cursor, (0, 0));
    }

    #[test]
    fn star_on_empty_cell_is_noop() {
        let mut app = App::new();
        app.cursor = (0, 0);
        app.process_action(Action::SearchCellValue { backward: false });
        assert_eq!(app.search.pattern, None);
        assert_eq!(app.status.as_deref(), Some("No string under cursor"));
    }

    #[test]
    fn set_delimiter_updates_field_and_status() {
        let mut app = App::new();
        assert_eq!(app.delimiter, b',', "default delimiter should be comma");
        app.process_action(Action::SetDelimiter(b'|'));
        assert_eq!(app.delimiter, b'|');
        assert_eq!(app.status.as_deref(), Some("Delimiter set to '|'"));
    }

    #[test]
    fn set_delimiter_tab() {
        let mut app = App::new();
        app.process_action(Action::SetDelimiter(b'\t'));
        assert_eq!(app.delimiter, b'\t');
    }

    #[test]
    fn set_mouse_action_writes_flag() {
        let mut app = App::new();
        assert!(!app.mouse_enabled);
        app.process_action(Action::SetMouse(true));
        assert!(app.mouse_enabled);
        app.process_action(Action::SetMouse(false));
        assert!(!app.mouse_enabled);
    }

    #[test]
    fn toggle_mouse_flips_flag_from_either_state() {
        let mut app = App::new();
        assert!(!app.mouse_enabled);
        app.process_action(Action::ToggleMouse);
        assert!(app.mouse_enabled);
        app.process_action(Action::ToggleMouse);
        assert!(!app.mouse_enabled);
    }

    #[test]
    fn mouse_click_cell_moves_cursor_and_ensures_visible() {
        let mut app = App::new();
        app.process_action(Action::MouseClickCell((50, 5)));
        assert_eq!(app.cursor, (50, 5));
        // Viewport must scroll to keep the cursor in view.
        assert!(app.viewport.row_offset > 0);
    }

    #[test]
    fn dd_undo_preserves_formula() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "10".into()));
        app.process_action(Action::EditCell((0, 1), "=A1*2".into()));

        app.process_action(Action::DeleteRow { start: 0, count: 1 });
        app.process_action(Action::Undo);

        assert_eq!(
            app.sheet.get_cell((0, 1)).map(|c| c.raw.as_str()),
            Some("=A1*2")
        );
        // Formula should re-evaluate after restoration.
        let val = app.sheet.get_cell((0, 1)).map(|c| c.value.to_string());
        assert_eq!(val.as_deref(), Some("20"));
    }

    // ── numeric count prefix ([count]j, [count]G, [count]dd, [count]yy) ──

    #[test]
    fn move_cursor_with_count_advances_n_steps() {
        let mut app = App::new();
        app.cursor = (2, 3);
        app.process_action(Action::MoveCursor(crate::action::Direction::Down, 5));
        assert_eq!(app.cursor, (7, 3));
        app.process_action(Action::MoveCursor(crate::action::Direction::Right, 4));
        assert_eq!(app.cursor, (7, 7));
        app.process_action(Action::MoveCursor(crate::action::Direction::Up, 100));
        assert_eq!(app.cursor, (0, 7), "saturating_sub clamps at 0");
        app.process_action(Action::MoveCursor(crate::action::Direction::Left, 100));
        assert_eq!(app.cursor, (0, 0));
    }

    #[test]
    fn move_cursor_count_zero_treated_as_one() {
        let mut app = App::new();
        app.cursor = (0, 0);
        app.process_action(Action::MoveCursor(crate::action::Direction::Down, 0));
        assert_eq!(app.cursor, (1, 0));
    }

    #[test]
    fn goto_row_jumps_to_one_indexed_row() {
        let mut app = App::new();
        app.cursor = (0, 2);
        app.process_action(Action::GotoRow(10));
        assert_eq!(app.cursor, (9, 2), "10G goes to row index 9");
    }

    #[test]
    fn goto_row_zero_clamps_to_first() {
        let mut app = App::new();
        app.cursor = (5, 0);
        app.process_action(Action::GotoRow(0));
        assert_eq!(app.cursor, (0, 0));
    }

    #[test]
    fn goto_row_records_jump_for_ctrl_o() {
        let mut app = App::new();
        app.cursor = (5, 0);
        app.process_action(Action::GotoRow(20));
        // Jump back should return us to (5, 0).
        app.process_action(Action::JumpBack);
        assert_eq!(app.cursor, (5, 0));
    }

    #[test]
    fn delete_multiple_rows_clears_them_all() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "a".into()));
        app.process_action(Action::EditCell((1, 0), "b".into()));
        app.process_action(Action::EditCell((2, 0), "c".into()));
        app.process_action(Action::EditCell((3, 0), "keep".into()));

        app.process_action(Action::DeleteRow { start: 0, count: 3 });
        assert!(app.sheet.get_cell((0, 0)).is_none());
        assert!(app.sheet.get_cell((1, 0)).is_none());
        assert!(app.sheet.get_cell((2, 0)).is_none());
        assert_eq!(
            app.sheet.get_cell((3, 0)).map(|c| c.raw.as_str()),
            Some("keep")
        );
    }

    #[test]
    fn delete_multiple_rows_undoes_atomically() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "a".into()));
        app.process_action(Action::EditCell((1, 0), "b".into()));
        app.process_action(Action::EditCell((2, 0), "c".into()));

        app.process_action(Action::DeleteRow { start: 0, count: 3 });
        app.process_action(Action::Undo);
        // Single undo restores all three rows (single MultiCellEdit entry).
        assert_eq!(
            app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
            Some("a")
        );
        assert_eq!(
            app.sheet.get_cell((1, 0)).map(|c| c.raw.as_str()),
            Some("b")
        );
        assert_eq!(
            app.sheet.get_cell((2, 0)).map(|c| c.raw.as_str()),
            Some("c")
        );
    }

    #[test]
    fn yank_multiple_rows_then_paste_below_drops_block() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "x".into()));
        app.process_action(Action::EditCell((0, 1), "y".into()));
        app.process_action(Action::EditCell((1, 0), "z".into()));
        app.cursor = (0, 0);

        app.process_action(Action::YankRow { start: 0, count: 2 });
        app.process_action(Action::Paste((0, 0)));
        // Pasted starting at row 1 (below row 0): row 1 should be x/y,
        // row 2 should be z.
        assert_eq!(
            app.sheet.get_cell((1, 0)).map(|c| c.raw.as_str()),
            Some("x")
        );
        assert_eq!(
            app.sheet.get_cell((1, 1)).map(|c| c.raw.as_str()),
            Some("y")
        );
        assert_eq!(
            app.sheet.get_cell((2, 0)).map(|c| c.raw.as_str()),
            Some("z")
        );
    }

    #[test]
    fn delete_then_paste_multi_row_restores_via_register() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "a".into()));
        app.process_action(Action::EditCell((1, 0), "b".into()));
        app.process_action(Action::EditCell((2, 0), "c".into()));
        app.cursor = (0, 0);

        app.process_action(Action::DeleteRow { start: 0, count: 3 });
        // After 3dd at row 0, the register holds 3 rows. PasteBefore
        // (P) places them starting at the current row.
        app.process_action(Action::PasteBefore((0, 0)));
        assert_eq!(
            app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
            Some("a")
        );
        assert_eq!(
            app.sheet.get_cell((1, 0)).map(|c| c.raw.as_str()),
            Some("b")
        );
        assert_eq!(
            app.sheet.get_cell((2, 0)).map(|c| c.raw.as_str()),
            Some("c")
        );
    }

    #[test]
    fn next_non_empty_with_count_hops_n_cells() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 1), "a".into()));
        app.process_action(Action::EditCell((0, 3), "b".into()));
        app.process_action(Action::EditCell((0, 5), "c".into()));
        app.cursor = (0, 0);

        app.process_action(Action::NextNonEmpty(2));
        assert_eq!(app.cursor, (0, 3));
        app.process_action(Action::NextNonEmpty(1));
        assert_eq!(app.cursor, (0, 5));
        // Asking for more than available: stay where we got to (last).
        let before = app.cursor;
        app.process_action(Action::NextNonEmpty(99));
        assert_eq!(app.cursor, before, "no more non-empty cells, stay put");
    }

    #[test]
    fn prev_non_empty_with_count_hops_n_cells() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 1), "a".into()));
        app.process_action(Action::EditCell((0, 3), "b".into()));
        app.process_action(Action::EditCell((0, 5), "c".into()));
        app.cursor = (0, 5);

        app.process_action(Action::PrevNonEmpty(2));
        assert_eq!(app.cursor, (0, 1));
    }

    // ── Delimiter save warnings ──────────────────────────────────────────────

    #[test]
    fn save_csv_with_non_comma_delimiter_warns() {
        let mut app = App::new();
        // Use a non-existent path so the actual write would fail, but the delimiter
        // warning must fire *before* the write attempt.
        app.file_path = Some(std::path::PathBuf::from("data.csv"));
        app.delimiter = b'|';
        app.process_action(Action::Save(None));
        let msg = app.status.as_deref().unwrap_or("");
        assert!(
            msg.contains("Non-standard delimiter"),
            "expected delimiter warning, got: {msg:?}"
        );
    }

    #[test]
    fn save_tsv_with_non_tab_delimiter_warns() {
        let mut app = App::new();
        app.file_path = Some(std::path::PathBuf::from("data.tsv"));
        app.delimiter = b'|';
        app.process_action(Action::Save(None));
        let msg = app.status.as_deref().unwrap_or("");
        assert!(
            msg.contains("Non-standard delimiter"),
            "expected delimiter warning, got: {msg:?}"
        );
    }

    #[test]
    fn save_csv_with_comma_delimiter_no_delimiter_warning() {
        let mut app = App::new();
        // delimiter is b',' (default) — no delimiter warning should fire.
        // The write may fail for other reasons (path doesn't exist), but the
        // error message should NOT contain "Non-standard delimiter".
        app.file_path = Some(std::path::PathBuf::from("data.csv"));
        app.delimiter = b',';
        app.process_action(Action::Save(None));
        let msg = app.status.as_deref().unwrap_or("");
        assert!(
            !msg.contains("Non-standard delimiter"),
            "unexpected delimiter warning: {msg:?}"
        );
    }

    #[test]
    fn force_save_csv_with_non_comma_delimiter_skips_warning() {
        let mut app = App::new();
        app.file_path = Some(std::path::PathBuf::from("data.csv"));
        app.delimiter = b'|';
        // ForceSave must NOT produce the delimiter warning.
        // (The write may produce an I/O error if the path can't be written, which is fine.)
        app.process_action(Action::ForceSave(None));
        let msg = app.status.as_deref().unwrap_or("");
        assert!(
            !msg.contains("Non-standard delimiter"),
            "ForceSave should bypass delimiter warning, got: {msg:?}"
        );
    }

    // ── ChangeRange recalculation ───────────────────────────────────────────

    #[test]
    fn change_range_updates_dependents() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 1), "99".into()));
        app.process_action(Action::EditCell((0, 0), "=B1".into()));
        assert_eq!(
            app.sheet.get_cell((0, 0)).unwrap().value,
            cell_sheet_core::model::CellValue::Number(99.0)
        );
        app.process_action(Action::ChangeRange {
            start: (0, 1),
            end: (0, 1),
        });
        // B1 was cleared; =B1 must not keep the stale 99 value
        let val = app.sheet.get_cell((0, 0)).map(|c| c.value.clone());
        assert_ne!(
            val,
            Some(cell_sheet_core::model::CellValue::Number(99.0)),
            "A1 must not keep stale value after B1 is cleared by ChangeRange"
        );
    }

    #[test]
    fn clear_cell_updates_dependents() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 1), "99".into()));
        app.process_action(Action::EditCell((0, 0), "=B1".into()));
        assert_eq!(
            app.sheet.get_cell((0, 0)).unwrap().value,
            cell_sheet_core::model::CellValue::Number(99.0)
        );
        app.process_action(Action::ClearCell((0, 1)));
        assert_eq!(
            app.sheet.get_cell((0, 0)).map(|c| c.value.clone()),
            Some(cell_sheet_core::model::CellValue::Number(0.0))
        );
    }

    // ── DeleteRow recalculation ─────────────────────────────────────────────

    #[test]
    fn delete_row_updates_dependents() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "10".into()));
        app.process_action(Action::EditCell((0, 1), "20".into()));
        app.process_action(Action::EditCell((1, 0), "=A1+B1".into()));
        assert_eq!(
            app.sheet.get_cell((1, 0)).unwrap().value,
            cell_sheet_core::model::CellValue::Number(30.0)
        );
        app.process_action(Action::DeleteRow { start: 0, count: 1 });
        let val = app.sheet.get_cell((1, 0)).map(|c| c.value.clone());
        assert_ne!(
            val,
            Some(cell_sheet_core::model::CellValue::Number(30.0)),
            "formula depending on deleted row must not keep stale value"
        );
    }

    #[test]
    fn delete_multiple_rows_updates_dependents() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "5".into()));
        app.process_action(Action::EditCell((1, 0), "7".into()));
        app.process_action(Action::EditCell((2, 0), "=A1+A2".into()));
        assert_eq!(
            app.sheet.get_cell((2, 0)).unwrap().value,
            cell_sheet_core::model::CellValue::Number(12.0)
        );
        app.process_action(Action::DeleteRow { start: 0, count: 2 });
        let val = app.sheet.get_cell((2, 0)).map(|c| c.value.clone());
        assert_ne!(
            val,
            Some(cell_sheet_core::model::CellValue::Number(12.0)),
            "formula depending on deleted rows must not keep stale value"
        );
    }

    #[test]
    fn sort_rebuilds_formula_dependencies() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "2".into()));
        app.process_action(Action::EditCell((1, 0), "1".into()));
        app.process_action(Action::EditCell((0, 1), "=A1*10".into()));

        app.process_action(Action::Sort {
            col: 0,
            ascending: true,
        });
        app.process_action(Action::EditCell((0, 0), "3".into()));

        assert_eq!(
            app.sheet.get_cell((1, 1)).map(|c| c.value.clone()),
            Some(cell_sheet_core::model::CellValue::Number(30.0))
        );
    }

    // ── begin_batch / commit_batch ──────────────────────────────────────────

    #[test]
    fn begin_commit_batch_defers_recalc() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "1".into()));
        app.process_action(Action::EditCell((1, 0), "=A1".into()));
        assert_eq!(
            app.sheet.get_cell((1, 0)).unwrap().value,
            cell_sheet_core::model::CellValue::Number(1.0)
        );

        app.begin_batch();
        app.process_action(Action::EditCell((0, 0), "42".into()));
        // Inside the batch, A2 must be dirty but not yet recalculated.
        assert!(
            app.sheet.get_cell((1, 0)).map(|c| c.dirty).unwrap_or(false),
            "A2 should be dirty inside a batch"
        );
        app.commit_batch();
        assert_eq!(
            app.sheet.get_cell((1, 0)).unwrap().value,
            cell_sheet_core::model::CellValue::Number(42.0),
            "A2 must be recalculated after commit_batch"
        );
    }

    #[test]
    fn batch_inter_dependent_cells() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "5".into()));
        app.process_action(Action::EditCell((0, 1), "=A1".into()));
        app.process_action(Action::EditCell((0, 2), "=B1".into()));

        app.begin_batch();
        app.process_action(Action::EditCell((0, 0), "99".into()));
        app.commit_batch();

        assert_eq!(
            app.sheet.get_cell((0, 1)).unwrap().value,
            cell_sheet_core::model::CellValue::Number(99.0)
        );
        assert_eq!(
            app.sheet.get_cell((0, 2)).unwrap().value,
            cell_sheet_core::model::CellValue::Number(99.0)
        );
    }

    #[test]
    fn nested_batches_recalc_on_outermost_commit() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "1".into()));
        app.process_action(Action::EditCell((1, 0), "=A1".into()));

        app.begin_batch();
        app.begin_batch();
        app.process_action(Action::EditCell((0, 0), "10".into()));
        // First commit — still inside outer batch; no recalc yet.
        app.commit_batch();
        assert!(
            app.sheet.get_cell((1, 0)).map(|c| c.dirty).unwrap_or(false),
            "A2 must still be dirty after inner commit"
        );
        // Second commit — outermost; recalc fires.
        app.commit_batch();
        assert_eq!(
            app.sheet.get_cell((1, 0)).unwrap().value,
            cell_sheet_core::model::CellValue::Number(10.0),
            "A2 must be updated after outermost commit"
        );
    }

    // ── dot-repeat (#33) ────────────────────────────────────────────────────

    #[test]
    fn dot_with_no_last_change_is_noop() {
        let mut app = App::new();
        app.cursor = (2, 3);
        app.process_action(Action::RepeatLastChange);
        assert_eq!(app.cursor, (2, 3));
        assert!(app.sheet.get_cell((2, 3)).is_none());
    }

    #[test]
    fn dot_repeats_edit_cell_at_new_cursor() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "x".into()));
        app.cursor = (0, 1);
        app.process_action(Action::RepeatLastChange);
        assert_eq!(
            app.sheet.get_cell((0, 1)).map(|c| c.raw.as_str()),
            Some("x")
        );
    }

    #[test]
    fn dot_preserves_last_change_for_next_dot() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "x".into()));
        app.cursor = (0, 1);
        app.process_action(Action::RepeatLastChange);
        app.cursor = (0, 2);
        app.process_action(Action::RepeatLastChange);
        assert_eq!(
            app.sheet.get_cell((0, 2)).map(|c| c.raw.as_str()),
            Some("x")
        );
    }

    #[test]
    fn dot_repeats_clear_cell() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "a".into()));
        app.process_action(Action::EditCell((0, 1), "b".into()));
        app.process_action(Action::ClearCell((0, 0)));
        app.cursor = (0, 1);
        app.process_action(Action::RepeatLastChange);
        assert!(app.sheet.get_cell((0, 1)).is_none());
    }

    #[test]
    fn dot_repeats_dd_at_new_row() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "a".into()));
        app.process_action(Action::EditCell((1, 0), "b".into()));
        app.process_action(Action::DeleteRow { start: 0, count: 1 });
        app.cursor = (1, 0);
        app.process_action(Action::RepeatLastChange);
        assert!(app.sheet.get_cell((1, 0)).is_none());
    }

    #[test]
    fn dot_after_undo_does_not_undo_again() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "x".into()));
        app.process_action(Action::Undo);
        assert!(app.sheet.get_cell((0, 0)).is_none());
        app.cursor = (0, 0);
        app.process_action(Action::RepeatLastChange);
        assert_eq!(
            app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
            Some("x")
        );
    }

    #[test]
    fn dot_repeats_paste() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "hello".into()));
        app.process_action(Action::YankCell((0, 0)));
        app.process_action(Action::Paste((0, 1)));
        app.cursor = (0, 2);
        app.process_action(Action::RepeatLastChange);
        assert_eq!(
            app.sheet.get_cell((0, 2)).map(|c| c.raw.as_str()),
            Some("hello")
        );
    }

    #[test]
    fn dot_repeats_paste_before() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "hello".into()));
        app.process_action(Action::YankCell((0, 0)));
        app.process_action(Action::PasteBefore((0, 1)));
        app.cursor = (0, 2);
        app.process_action(Action::RepeatLastChange);
        assert_eq!(
            app.sheet.get_cell((0, 2)).map(|c| c.raw.as_str()),
            Some("hello")
        );
    }

    // ── CaseOpCell / CaseOpRange ─────────────────────────────────────────────

    #[test]
    fn toggle_first_uppercases_first_char_of_lowercase() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "hello".into()));
        app.process_action(Action::CaseOpCell {
            pos: (0, 0),
            op: CaseOp::ToggleFirst,
        });
        assert_eq!(
            app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
            Some("Hello")
        );
    }

    #[test]
    fn toggle_first_advances_cursor_right() {
        let mut app = App::new();
        app.cursor = (0, 0);
        app.process_action(Action::EditCell((0, 0), "hello".into()));
        app.process_action(Action::CaseOpCell {
            pos: (0, 0),
            op: CaseOp::ToggleFirst,
        });
        assert_eq!(app.cursor, (0, 1));
    }

    #[test]
    fn to_upper_uppercases_entire_cell() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "foo".into()));
        app.process_action(Action::CaseOpCell {
            pos: (0, 0),
            op: CaseOp::ToUpper,
        });
        assert_eq!(
            app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
            Some("FOO")
        );
    }

    #[test]
    fn to_lower_lowercases_entire_cell() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "HELLO".into()));
        app.process_action(Action::CaseOpCell {
            pos: (0, 0),
            op: CaseOp::ToLower,
        });
        assert_eq!(
            app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
            Some("hello")
        );
    }

    #[test]
    fn toggle_all_toggles_every_char() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "HeLLo".into()));
        app.process_action(Action::CaseOpCell {
            pos: (0, 0),
            op: CaseOp::ToggleAll,
        });
        assert_eq!(
            app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
            Some("hEllO")
        );
    }

    #[test]
    fn case_op_cell_formula_is_noop_with_status() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "=SUM(A1:A5)".into()));
        app.process_action(Action::CaseOpCell {
            pos: (0, 0),
            op: CaseOp::ToUpper,
        });
        assert_eq!(
            app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
            Some("=SUM(A1:A5)")
        );
        assert!(app.status.as_deref().is_some());
    }

    #[test]
    fn case_op_cell_is_undoable() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "hello".into()));
        app.process_action(Action::CaseOpCell {
            pos: (0, 0),
            op: CaseOp::ToUpper,
        });
        assert_eq!(
            app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
            Some("HELLO")
        );
        app.process_action(Action::Undo);
        assert_eq!(
            app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
            Some("hello")
        );
    }

    #[test]
    fn case_op_range_uppercases_all_cells() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "foo".into()));
        app.process_action(Action::EditCell((0, 1), "BAR".into()));
        app.process_action(Action::EditCell((0, 2), "baz".into()));
        app.process_action(Action::CaseOpRange {
            start: (0, 0),
            end: (0, 2),
            op: CaseOp::ToUpper,
        });
        assert_eq!(
            app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
            Some("FOO")
        );
        assert_eq!(
            app.sheet.get_cell((0, 1)).map(|c| c.raw.as_str()),
            Some("BAR")
        );
        assert_eq!(
            app.sheet.get_cell((0, 2)).map(|c| c.raw.as_str()),
            Some("BAZ")
        );
    }

    #[test]
    fn case_op_range_skips_formula_cells() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "hello".into()));
        app.process_action(Action::EditCell((0, 1), "=A1".into()));
        app.process_action(Action::CaseOpRange {
            start: (0, 0),
            end: (0, 1),
            op: CaseOp::ToUpper,
        });
        assert_eq!(
            app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
            Some("HELLO")
        );
        assert_eq!(
            app.sheet.get_cell((0, 1)).map(|c| c.raw.as_str()),
            Some("=A1")
        );
    }

    #[test]
    fn case_op_range_is_undoable() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "foo".into()));
        app.process_action(Action::EditCell((0, 1), "bar".into()));
        app.process_action(Action::CaseOpRange {
            start: (0, 0),
            end: (0, 1),
            op: CaseOp::ToUpper,
        });
        assert_eq!(
            app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
            Some("FOO")
        );
        app.process_action(Action::Undo);
        assert_eq!(
            app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
            Some("foo")
        );
        assert_eq!(
            app.sheet.get_cell((0, 1)).map(|c| c.raw.as_str()),
            Some("bar")
        );
    }

    #[test]
    fn dot_repeats_case_op_cell() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "hello".into()));
        app.process_action(Action::EditCell((0, 1), "world".into()));
        app.process_action(Action::CaseOpCell {
            pos: (0, 0),
            op: CaseOp::ToUpper,
        });
        app.cursor = (0, 1);
        app.process_action(Action::RepeatLastChange);
        assert_eq!(
            app.sheet.get_cell((0, 1)).map(|c| c.raw.as_str()),
            Some("WORLD")
        );
    }

    #[test]
    fn dot_repeats_clear_range_with_same_shape() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "a".into()));
        app.process_action(Action::EditCell((0, 1), "b".into()));
        app.process_action(Action::EditCell((1, 0), "c".into()));
        app.process_action(Action::EditCell((1, 1), "d".into()));
        // Clear 1×2 range at row 0
        app.process_action(Action::ClearRange {
            start: (0, 0),
            end: (0, 1),
        });
        // Dot at (1, 0) should clear (1,0)–(1,1)
        app.cursor = (1, 0);
        app.process_action(Action::RepeatLastChange);
        assert!(app.sheet.get_cell((1, 0)).is_none());
        assert!(app.sheet.get_cell((1, 1)).is_none());
    }

    // ── Ctrl+a / Ctrl+x increment/decrement (#34) ───────────────────────

    #[test]
    fn adjust_number_increments_numeric_cell() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "5".into()));
        app.process_action(Action::AdjustNumber {
            pos: (0, 0),
            delta: 1,
        });
        assert_eq!(
            app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
            Some("6")
        );
    }

    #[test]
    fn adjust_number_decrements_numeric_cell() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "10".into()));
        app.process_action(Action::AdjustNumber {
            pos: (0, 0),
            delta: -1,
        });
        assert_eq!(
            app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
            Some("9")
        );
    }

    #[test]
    fn adjust_number_with_count_applies_full_delta() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "3".into()));
        app.process_action(Action::AdjustNumber {
            pos: (0, 0),
            delta: 5,
        });
        assert_eq!(
            app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
            Some("8")
        );
    }

    #[test]
    fn adjust_number_triggers_dependent_recalculation() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "5".into()));
        app.process_action(Action::EditCell((0, 1), "=A1+1".into()));
        app.process_action(Action::AdjustNumber {
            pos: (0, 0),
            delta: 1,
        });
        // A1 is now 6, so B1 (=A1+1) should be 7
        assert_eq!(
            app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
            Some("6")
        );
        let b1_val = app.sheet.get_cell((0, 1)).map(|c| c.value.clone()).unwrap();
        assert_eq!(b1_val, cell_sheet_core::model::CellValue::Number(7.0));
    }

    #[test]
    fn adjust_number_on_formula_cell_is_noop_with_status() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "=1+1".into()));
        let raw_before = app.sheet.get_cell((0, 0)).map(|c| c.raw.clone());
        app.process_action(Action::AdjustNumber {
            pos: (0, 0),
            delta: 1,
        });
        // Cell unchanged
        assert_eq!(
            app.sheet.get_cell((0, 0)).map(|c| c.raw.clone()),
            raw_before
        );
        // Status message set
        assert_eq!(app.status.as_deref(), Some("E: Cannot increment a formula"));
    }

    #[test]
    fn adjust_number_on_text_cell_is_noop() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "hello".into()));
        app.process_action(Action::AdjustNumber {
            pos: (0, 0),
            delta: 1,
        });
        assert_eq!(
            app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
            Some("hello")
        );
    }

    #[test]
    fn adjust_number_on_empty_cell_is_noop() {
        let mut app = App::new();
        app.process_action(Action::AdjustNumber {
            pos: (0, 0),
            delta: 1,
        });
        assert!(app.sheet.get_cell((0, 0)).is_none());
    }

    #[test]
    fn adjust_number_is_undoable() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "5".into()));
        app.process_action(Action::AdjustNumber {
            pos: (0, 0),
            delta: 1,
        });
        app.process_action(Action::Undo);
        assert_eq!(
            app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
            Some("5")
        );
    }

    #[test]
    fn adjust_number_sets_last_change_for_dot_repeat() {
        let mut app = App::new();
        app.process_action(Action::EditCell((0, 0), "5".into()));
        app.process_action(Action::EditCell((1, 0), "10".into()));
        app.process_action(Action::AdjustNumber {
            pos: (0, 0),
            delta: 1,
        });
        // Move cursor and repeat with `.` — should increment (1,0) by 1
        app.cursor = (1, 0);
        app.process_action(Action::RepeatLastChange);
        assert_eq!(
            app.sheet.get_cell((1, 0)).map(|c| c.raw.as_str()),
            Some("11")
        );
    }
}
