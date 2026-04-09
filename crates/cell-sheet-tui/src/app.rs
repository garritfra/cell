use crate::action::{Action, Direction, Mode};
use crate::clipboard::Register;
use crate::undo::{UndoEntry, UndoStack};
use crate::viewport::Viewport;
use cell_sheet_core::formula::deps::{mark_dirty, recalculate, set_formula, DepGraph};
use cell_sheet_core::help::HelpRegistry;
use cell_sheet_core::model::{CellPos, Sheet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileFormat {
    Csv,
    Tsv,
    Cell,
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
    pub help_scroll: usize,
    pub help_topic: Option<String>,
    pub help_registry: HelpRegistry,
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
            help_scroll: 0,
            help_topic: None,
            help_registry: HelpRegistry::new(),
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
                let old_raw = self
                    .sheet
                    .get_cell(pos)
                    .map(|c| c.raw.clone())
                    .unwrap_or_default();
                self.undo_stack.push(UndoEntry::CellEdit {
                    pos,
                    old_raw,
                    new_raw: raw.clone(),
                });
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
                    self.insert_buffer = self
                        .sheet
                        .get_cell(self.cursor)
                        .map(|c| c.raw.clone())
                        .unwrap_or_default();
                }
                self.mode = mode;
            }
            Action::Quit { force } => {
                if !force && self.dirty {
                    self.status_message =
                        Some("No write since last change (use :q! to override)".into());
                } else {
                    self.should_quit = true;
                }
            }
            Action::ClearCell(pos) => {
                let old_raw = self
                    .sheet
                    .get_cell(pos)
                    .map(|c| c.raw.clone())
                    .unwrap_or_default();
                if !old_raw.is_empty() {
                    self.undo_stack.push(UndoEntry::CellEdit {
                        pos,
                        old_raw,
                        new_raw: String::new(),
                    });
                    self.sheet.clear_cell(pos);
                    self.dirty = true;
                }
            }
            Action::ChangeCell(pos) => {
                let old_raw = self
                    .sheet
                    .get_cell(pos)
                    .map(|c| c.raw.clone())
                    .unwrap_or_default();
                if !old_raw.is_empty() {
                    self.undo_stack.push(UndoEntry::CellEdit {
                        pos,
                        old_raw,
                        new_raw: String::new(),
                    });
                    self.sheet.clear_cell(pos);
                    self.dirty = true;
                }
                self.insert_buffer = String::new();
                self.mode = Mode::Insert;
            }
            Action::ChangeRange { start, end } => {
                let max_col = end.1.min(self.sheet.col_count.saturating_sub(1));
                for row in start.0..=end.0 {
                    for col in start.1..=max_col {
                        let old_raw = self
                            .sheet
                            .get_cell((row, col))
                            .map(|c| c.raw.clone())
                            .unwrap_or_default();
                        if !old_raw.is_empty() {
                            self.undo_stack.push(UndoEntry::CellEdit {
                                pos: (row, col),
                                old_raw,
                                new_raw: String::new(),
                            });
                            self.sheet.clear_cell((row, col));
                        }
                    }
                }
                self.dirty = true;
                self.insert_buffer = String::new();
                self.mode = Mode::Insert;
            }
            Action::GotoFirstRow => {
                self.cursor = (0, self.cursor.1);
                self.viewport.ensure_visible(self.cursor);
            }
            Action::GotoLastRow => {
                let last = if self.sheet.row_count > 0 {
                    self.sheet.row_count - 1
                } else {
                    0
                };
                self.cursor = (last, self.cursor.1);
                self.viewport.ensure_visible(self.cursor);
            }
            Action::GotoFirstCol => {
                self.cursor = (self.cursor.0, 0);
                self.viewport.ensure_visible(self.cursor);
            }
            Action::GotoLastCol => {
                let last = if self.sheet.col_count > 0 {
                    self.sheet.col_count - 1
                } else {
                    0
                };
                self.cursor = (self.cursor.0, last);
                self.viewport.ensure_visible(self.cursor);
            }
            Action::HalfPageDown => {
                self.cursor.0 += self.viewport.visible_rows / 2;
                self.viewport.ensure_visible(self.cursor);
            }
            Action::HalfPageUp => {
                self.cursor.0 = self.cursor.0.saturating_sub(self.viewport.visible_rows / 2);
                self.viewport.ensure_visible(self.cursor);
            }
            Action::PageDown => {
                self.cursor.0 += self.viewport.visible_rows;
                self.viewport.ensure_visible(self.cursor);
            }
            Action::PageUp => {
                self.cursor.0 = self.cursor.0.saturating_sub(self.viewport.visible_rows);
                self.viewport.ensure_visible(self.cursor);
            }
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
            Action::Save(path_opt) => {
                let path = path_opt.or(self.file_path.clone());
                if let Some(path) = path {
                    let format = Self::format_from_path(&path);
                    if !matches!(format, FileFormat::Cell) && self.has_formulas() {
                        self.status_message = Some(
                            "Sheet contains formulas that will be lost. Use :w file.cell to preserve, or :w! to save as CSV anyway.".into()
                        );
                        return;
                    }
                    self.do_save(&path, format);
                } else {
                    self.status_message = Some("No file name".into());
                }
            }
            Action::ForceSave(path_opt) => {
                let path = path_opt.or(self.file_path.clone());
                if let Some(path) = path {
                    let format = Self::format_from_path(&path);
                    self.do_save(&path, format);
                } else {
                    self.status_message = Some("No file name".into());
                }
            }
            Action::Search { pattern, direction } => {
                self.search_pattern = Some(pattern.clone());
                self.find_next(direction == crate::action::SearchDirection::Forward);
            }
            Action::SearchNext => {
                if self.search_pattern.is_some() {
                    self.find_next(true);
                }
            }
            Action::SearchPrev => {
                if self.search_pattern.is_some() {
                    self.find_next(false);
                }
            }
            Action::Sort { col, ascending } => {
                self.sheet.sort_by_column(col, ascending);
                self.dirty = true;
                self.status_message = Some(format!(
                    "Sorted by column {} {}",
                    cell_sheet_core::model::col_index_to_label(col),
                    if ascending { "ascending" } else { "descending" }
                ));
            }
            Action::YankCell(pos) => {
                if let Some(cell) = self.sheet.get_cell(pos) {
                    self.register = Some(Register::Cell(cell.raw.clone()));
                }
            }
            Action::YankRow(row) => {
                let mut cells = Vec::new();
                for col in 0..self.sheet.col_count {
                    let raw = self
                        .sheet
                        .get_cell((row, col))
                        .map(|c| c.raw.clone())
                        .unwrap_or_default();
                    cells.push(raw);
                }
                self.register = Some(Register::Row(cells));
            }
            Action::YankRange { start, end } => {
                let max_col = end.1.min(self.sheet.col_count.saturating_sub(1));
                let mut block = Vec::new();
                for row in start.0..=end.0 {
                    let mut row_data = Vec::new();
                    for col in start.1..=max_col {
                        let raw = self
                            .sheet
                            .get_cell((row, col))
                            .map(|c| c.raw.clone())
                            .unwrap_or_default();
                        row_data.push(raw);
                    }
                    block.push(row_data);
                }
                self.register = Some(Register::Block(block));
            }
            Action::ClearRange { start, end } => {
                let max_col = end.1.min(self.sheet.col_count.saturating_sub(1));
                let mut block = Vec::new();
                for row in start.0..=end.0 {
                    let mut row_data = Vec::new();
                    for col in start.1..=max_col {
                        let raw = self
                            .sheet
                            .get_cell((row, col))
                            .map(|c| c.raw.clone())
                            .unwrap_or_default();
                        row_data.push(raw);
                        self.sheet.clear_cell((row, col));
                    }
                    block.push(row_data);
                }
                self.register = Some(Register::Block(block));
                self.dirty = true;
            }
            Action::DeleteRow(row) => {
                let mut cells = Vec::new();
                for col in 0..self.sheet.col_count {
                    let raw = self
                        .sheet
                        .get_cell((row, col))
                        .map(|c| c.raw.clone())
                        .unwrap_or_default();
                    cells.push(raw);
                    self.sheet.clear_cell((row, col));
                }
                self.register = Some(Register::Row(cells));
                self.dirty = true;
            }
            Action::Paste(pos) | Action::PasteBefore(pos) => {
                let is_after = matches!(action, Action::Paste(_));
                if let Some(reg) = &self.register.clone() {
                    match reg {
                        Register::Cell(raw) => {
                            let adjusted = crate::clipboard::adjust_formula(raw, 0, 0);
                            if adjusted.starts_with('=') {
                                cell_sheet_core::formula::deps::set_formula(
                                    &mut self.sheet,
                                    &mut self.deps,
                                    pos,
                                    &adjusted,
                                );
                            } else {
                                self.sheet.set_cell(pos, &adjusted);
                            }
                            self.dirty = true;
                        }
                        Register::Row(cells) => {
                            // Row (yy/dd): p pastes on line below, P on current line
                            let dest_row = if is_after { pos.0 + 1 } else { pos.0 };
                            for (col, raw) in cells.iter().enumerate() {
                                if !raw.is_empty() {
                                    let adjusted = crate::clipboard::adjust_formula(
                                        raw,
                                        dest_row as isize - pos.0 as isize,
                                        0,
                                    );
                                    self.sheet.set_cell((dest_row, col), &adjusted);
                                }
                            }
                            self.dirty = true;
                        }
                        Register::Block(block) => {
                            // Block (visual selection): p pastes at cursor position
                            for (r_off, row_data) in block.iter().enumerate() {
                                for (c_off, raw) in row_data.iter().enumerate() {
                                    if !raw.is_empty() {
                                        let adjusted = crate::clipboard::adjust_formula(
                                            raw,
                                            r_off as isize,
                                            c_off as isize,
                                        );
                                        self.sheet
                                            .set_cell((pos.0 + r_off, pos.1 + c_off), &adjusted);
                                    }
                                }
                            }
                            self.dirty = true;
                        }
                    }
                }
            }
            Action::NextNonEmpty => {
                let (row, col) = self.cursor;
                for c in (col + 1)..self.sheet.col_count {
                    if self.sheet.get_cell((row, c)).is_some() {
                        self.cursor = (row, c);
                        self.viewport.ensure_visible(self.cursor);
                        return;
                    }
                }
            }
            Action::PrevNonEmpty => {
                let (row, col) = self.cursor;
                if col > 0 {
                    for c in (0..col).rev() {
                        if self.sheet.get_cell((row, c)).is_some() {
                            self.cursor = (row, c);
                            self.viewport.ensure_visible(self.cursor);
                            return;
                        }
                    }
                }
            }
            Action::ShowHelp(topic) => match topic {
                Some(ref tag) => {
                    if self.help_registry.find(tag).is_some() {
                        self.help_topic = topic;
                        self.help_scroll = 0;
                        self.mode = Mode::Help;
                    } else {
                        self.status_message = Some(format!("No help for '{}'", tag));
                    }
                }
                None => {
                    self.help_topic = None;
                    self.help_scroll = 0;
                    self.mode = Mode::Help;
                }
            },
            Action::Open(_) | Action::Resize => {}
        }
    }

    fn format_from_path(path: &Path) -> FileFormat {
        match path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase()
            .as_str()
        {
            "tsv" => FileFormat::Tsv,
            "cell" => FileFormat::Cell,
            _ => FileFormat::Csv,
        }
    }

    fn do_save(&mut self, path: &PathBuf, format: FileFormat) {
        let result = match format {
            FileFormat::Csv => std::fs::File::create(path)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
                .and_then(|f| cell_sheet_core::io::csv::write_csv(&self.sheet, f, b',')),
            FileFormat::Tsv => std::fs::File::create(path)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
                .and_then(|f| cell_sheet_core::io::csv::write_csv(&self.sheet, f, b'\t')),
            FileFormat::Cell => std::fs::File::create(path)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
                .and_then(|f| cell_sheet_core::io::cell_format::write_cell_format(&self.sheet, f)),
        };

        match result {
            Ok(()) => {
                self.file_path = Some(path.clone());
                self.file_format = format;
                self.dirty = false;
                self.status_message = Some(format!("Written to {}", path.display()));
            }
            Err(e) => {
                self.status_message = Some(format!("Error saving: {}", e));
            }
        }
    }

    fn find_next(&mut self, forward: bool) {
        let pattern = match &self.search_pattern {
            Some(p) => p.to_lowercase(),
            None => return,
        };

        let total_cells = self.sheet.row_count * self.sheet.col_count.max(1);
        if total_cells == 0 {
            return;
        }

        let (start_row, start_col) = self.cursor;
        let cols = self.sheet.col_count.max(1);

        for offset in 1..=total_cells {
            let flat = start_row * cols + start_col;
            let next_flat = if forward {
                (flat + offset) % total_cells
            } else {
                (flat + total_cells - offset) % total_cells
            };
            let row = next_flat / cols;
            let col = next_flat % cols;

            if let Some(cell) = self.sheet.get_cell((row, col)) {
                if cell.value.to_string().to_lowercase().contains(&pattern) {
                    self.cursor = (row, col);
                    self.viewport.ensure_visible(self.cursor);
                    return;
                }
            }
        }
        self.status_message = Some(format!("Pattern not found: {}", pattern));
    }

    fn apply_undo_entry(&mut self, entry: &UndoEntry, redo: bool) {
        match entry {
            UndoEntry::CellEdit {
                pos,
                old_raw,
                new_raw,
            } => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Action;

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
        assert_eq!(app.status_message, Some("No help for 'nonexistent'".into()));
    }

    #[test]
    fn help_mode_back_to_normal() {
        let mut app = App::new();
        app.process_action(Action::ShowHelp(None));
        assert_eq!(app.mode, Mode::Help);
        app.process_action(Action::ChangeMode(Mode::Normal));
        assert_eq!(app.mode, Mode::Normal);
    }
}
