use crate::action::{Action, CommandKind, Direction, Mode, SearchDirection};
use crate::clipboard::Register;
use crate::mode::visual::VisualKind;
use crate::undo::{UndoEntry, UndoStack};
use crate::viewport::Viewport;
use cell_sheet_core::formula::deps::{mark_dirty, recalculate, set_formula, DepGraph};
use cell_sheet_core::help::HelpRegistry;
use cell_sheet_core::model::{CellPos, Sheet};
use std::collections::HashMap;
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
    pub command_kind: CommandKind,
    pub status_message: Option<String>,
    pub search_pattern: Option<String>,
    /// Cursor position captured when the user opened a `/` or `?` prompt.
    /// `Some` for the lifetime of an open search prompt; consumed on Esc
    /// (cursor restored) or Enter with a non-empty pattern (committed).
    pub search_origin: Option<CellPos>,
    /// Last `f`/`F` target so `;` and `,` can replay it. Holds
    /// `(char, forward, inclusive)`.
    pub last_find: Option<(char, bool, bool)>,
    pub file_path: Option<PathBuf>,
    pub file_format: FileFormat,
    pub dirty: bool,
    pub should_quit: bool,
    pub insert_buffer: String,
    pub help_scroll: usize,
    pub help_topic: Option<String>,
    pub help_registry: HelpRegistry,
    pub marks: HashMap<char, CellPos>,
    pub jump_list: Vec<CellPos>,
    pub jump_idx: usize,
    pub last_visual: Option<LastVisual>,
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
            command_line: String::new(),
            command_kind: CommandKind::Colon,
            status_message: None,
            search_pattern: None,
            search_origin: None,
            last_find: None,
            file_path: None,
            file_format: FileFormat::Csv,
            dirty: false,
            should_quit: false,
            insert_buffer: String::new(),
            help_scroll: 0,
            help_topic: None,
            help_registry: HelpRegistry::new(),
            marks: HashMap::new(),
            jump_list: Vec::new(),
            jump_idx: 0,
            last_visual: None,
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

    pub fn process_action(&mut self, action: Action) {
        match action {
            Action::Noop => {}
            Action::MoveCursor(dir, count) => {
                let count = count.max(1);
                let (row, col) = self.cursor;
                self.cursor = match dir {
                    Direction::Up => (row.saturating_sub(count), col),
                    Direction::Down => (row.saturating_add(count), col),
                    Direction::Left => (row, col.saturating_sub(count)),
                    Direction::Right => (row, col.saturating_add(count)),
                };
                self.viewport.ensure_visible(self.cursor);
            }
            Action::MoveCursorTo(pos) => {
                self.record_jump();
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
                if mode == Mode::Command {
                    self.command_kind = CommandKind::Colon;
                    self.command_line.clear();
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
                self.record_jump();
                self.cursor = (0, self.cursor.1);
                self.viewport.ensure_visible(self.cursor);
            }
            Action::GotoLastRow => {
                self.record_jump();
                let last = if self.sheet.row_count > 0 {
                    self.sheet.row_count - 1
                } else {
                    0
                };
                self.cursor = (last, self.cursor.1);
                self.viewport.ensure_visible(self.cursor);
            }
            Action::GotoRow(target_1based) => {
                self.record_jump();
                let row = target_1based.saturating_sub(1);
                self.cursor = (row, self.cursor.1);
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
                if pattern.is_empty() {
                    // Empty submit: restore cursor to where the prompt opened
                    // (matches vim's behavior of "no match → no movement"),
                    // and don't overwrite an existing pattern.
                    if let Some(origin) = self.search_origin.take() {
                        self.cursor = origin;
                        self.viewport.ensure_visible(self.cursor);
                    }
                    return;
                }
                self.record_jump();
                self.search_pattern = Some(pattern.clone());
                let forward = direction == SearchDirection::Forward;
                // If a prompt is open (origin set), commit at the incremental
                // position by re-running the search from origin, including
                // origin as a candidate. Without an open prompt (e.g.
                // `Action::Search` dispatched directly), behave like vim's
                // `/<pattern>`: step past the current cell.
                let (origin, include_origin) = match self.search_origin.take() {
                    Some(o) => (o, true),
                    None => (self.cursor, false),
                };
                if !self.find_from(&pattern, forward, origin, include_origin) {
                    if include_origin {
                        self.cursor = origin;
                        self.viewport.ensure_visible(self.cursor);
                    }
                    self.status_message = Some(format!("Pattern not found: {}", pattern));
                }
            }
            Action::EnterSearch(direction) => {
                self.command_line.clear();
                self.command_kind = match direction {
                    SearchDirection::Forward => CommandKind::Slash,
                    SearchDirection::Backward => CommandKind::Question,
                };
                self.search_origin = Some(self.cursor);
                self.mode = Mode::Command;
            }
            Action::SearchIncremental { pattern, direction } => {
                let Some(origin) = self.search_origin else {
                    return;
                };
                if pattern.is_empty() {
                    self.cursor = origin;
                    self.viewport.ensure_visible(self.cursor);
                    return;
                }
                let forward = direction == SearchDirection::Forward;
                if !self.find_from(&pattern, forward, origin, true) {
                    // No match: snap back to origin so the user sees they're
                    // not on a stale earlier match.
                    self.cursor = origin;
                    self.viewport.ensure_visible(self.cursor);
                }
            }
            Action::CancelSearch => {
                if let Some(origin) = self.search_origin.take() {
                    self.cursor = origin;
                    self.viewport.ensure_visible(self.cursor);
                }
                self.command_line.clear();
                self.mode = Mode::Normal;
            }
            Action::FindCharInRow {
                ch,
                forward,
                inclusive,
            } => {
                self.last_find = Some((ch, forward, inclusive));
                self.find_char_in_row(ch, forward, inclusive);
            }
            Action::RepeatFind { reversed } => {
                if let Some((ch, forward, inclusive)) = self.last_find {
                    let dir = if reversed { !forward } else { forward };
                    self.find_char_in_row(ch, dir, inclusive);
                }
            }
            Action::SearchNext => {
                if self.search_pattern.is_some() {
                    self.record_jump();
                    self.find_next(true);
                }
            }
            Action::SearchPrev => {
                if self.search_pattern.is_some() {
                    self.record_jump();
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
            Action::YankRow { start, count } => {
                let count = count.max(1);
                let cols = self.sheet.col_count;
                if count == 1 {
                    let mut cells = Vec::with_capacity(cols);
                    for col in 0..cols {
                        let raw = self
                            .sheet
                            .get_cell((start, col))
                            .map(|c| c.raw.clone())
                            .unwrap_or_default();
                        cells.push(raw);
                    }
                    self.register = Some(Register::Row(cells));
                } else {
                    let mut rows: Vec<Vec<String>> = Vec::with_capacity(count);
                    for r in start..start.saturating_add(count) {
                        let mut row_cells = Vec::with_capacity(cols);
                        for col in 0..cols {
                            let raw = self
                                .sheet
                                .get_cell((r, col))
                                .map(|c| c.raw.clone())
                                .unwrap_or_default();
                            row_cells.push(raw);
                        }
                        rows.push(row_cells);
                    }
                    self.register = Some(Register::Rows(rows));
                }
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
                let mut changes = Vec::new();
                for row in start.0..=end.0 {
                    let mut row_data = Vec::new();
                    for col in start.1..=max_col {
                        let raw = self
                            .sheet
                            .get_cell((row, col))
                            .map(|c| c.raw.clone())
                            .unwrap_or_default();
                        if !raw.is_empty() {
                            changes.push(((row, col), raw.clone(), String::new()));
                            self.write_cell_raw((row, col), "");
                        }
                        row_data.push(raw);
                    }
                    block.push(row_data);
                }
                self.register = Some(Register::Block(block));
                if !changes.is_empty() {
                    self.undo_stack.push(UndoEntry::MultiCellEdit { changes });
                    recalculate(&mut self.sheet, &self.deps);
                    self.dirty = true;
                }
            }
            Action::DeleteRow { start, count } => {
                let count = count.max(1);
                let cols = self.sheet.col_count;
                let mut changes = Vec::new();
                if count == 1 {
                    let mut cells = Vec::with_capacity(cols);
                    for col in 0..cols {
                        let raw = self
                            .sheet
                            .get_cell((start, col))
                            .map(|c| c.raw.clone())
                            .unwrap_or_default();
                        if !raw.is_empty() {
                            changes.push(((start, col), raw.clone(), String::new()));
                            self.sheet.clear_cell((start, col));
                        }
                        cells.push(raw);
                    }
                    self.register = Some(Register::Row(cells));
                } else {
                    let mut rows: Vec<Vec<String>> = Vec::with_capacity(count);
                    for r in start..start.saturating_add(count) {
                        let mut row_cells = Vec::with_capacity(cols);
                        for col in 0..cols {
                            let raw = self
                                .sheet
                                .get_cell((r, col))
                                .map(|c| c.raw.clone())
                                .unwrap_or_default();
                            if !raw.is_empty() {
                                changes.push(((r, col), raw.clone(), String::new()));
                                self.sheet.clear_cell((r, col));
                            }
                            row_cells.push(raw);
                        }
                        rows.push(row_cells);
                    }
                    self.register = Some(Register::Rows(rows));
                }
                if !changes.is_empty() {
                    self.undo_stack.push(UndoEntry::MultiCellEdit { changes });
                    self.dirty = true;
                }
            }
            Action::Paste(pos) | Action::PasteBefore(pos) => {
                let is_after = matches!(action, Action::Paste(_));
                if let Some(reg) = &self.register.clone() {
                    let mut changes: Vec<(CellPos, String, String)> = Vec::new();
                    match reg {
                        Register::Cell(raw) => {
                            let adjusted = crate::clipboard::adjust_formula(raw, 0, 0);
                            let old_raw = self
                                .sheet
                                .get_cell(pos)
                                .map(|c| c.raw.clone())
                                .unwrap_or_default();
                            if adjusted != old_raw {
                                changes.push((pos, old_raw, adjusted.clone()));
                                self.write_cell_raw(pos, &adjusted);
                            }
                        }
                        Register::Row(cells) => {
                            // Row (yy/dd): p pastes on line below, P on current line
                            let dest_row = if is_after { pos.0 + 1 } else { pos.0 };
                            for (col, raw) in cells.iter().enumerate() {
                                if raw.is_empty() {
                                    continue;
                                }
                                let adjusted = crate::clipboard::adjust_formula(
                                    raw,
                                    dest_row as isize - pos.0 as isize,
                                    0,
                                );
                                let dest = (dest_row, col);
                                let old_raw = self
                                    .sheet
                                    .get_cell(dest)
                                    .map(|c| c.raw.clone())
                                    .unwrap_or_default();
                                if adjusted != old_raw {
                                    changes.push((dest, old_raw, adjusted.clone()));
                                    self.write_cell_raw(dest, &adjusted);
                                }
                            }
                        }
                        Register::Rows(rows) => {
                            // Multi-row line-wise (3dd/3yy): p pastes the whole
                            // block starting at the line below; P starts at the
                            // current line. Cursor column is ignored.
                            let dest_row_start = if is_after { pos.0 + 1 } else { pos.0 };
                            for (r_off, row_data) in rows.iter().enumerate() {
                                let dest_row = dest_row_start + r_off;
                                for (col, raw) in row_data.iter().enumerate() {
                                    if raw.is_empty() {
                                        continue;
                                    }
                                    let adjusted = crate::clipboard::adjust_formula(
                                        raw,
                                        dest_row as isize - (pos.0 as isize + r_off as isize),
                                        0,
                                    );
                                    let dest = (dest_row, col);
                                    let old_raw = self
                                        .sheet
                                        .get_cell(dest)
                                        .map(|c| c.raw.clone())
                                        .unwrap_or_default();
                                    if adjusted != old_raw {
                                        changes.push((dest, old_raw, adjusted.clone()));
                                        self.write_cell_raw(dest, &adjusted);
                                    }
                                }
                            }
                        }
                        Register::Block(block) => {
                            // Block (visual selection): p pastes at cursor position
                            for (r_off, row_data) in block.iter().enumerate() {
                                for (c_off, raw) in row_data.iter().enumerate() {
                                    if raw.is_empty() {
                                        continue;
                                    }
                                    let adjusted = crate::clipboard::adjust_formula(
                                        raw,
                                        r_off as isize,
                                        c_off as isize,
                                    );
                                    let dest = (pos.0 + r_off, pos.1 + c_off);
                                    let old_raw = self
                                        .sheet
                                        .get_cell(dest)
                                        .map(|c| c.raw.clone())
                                        .unwrap_or_default();
                                    if adjusted != old_raw {
                                        changes.push((dest, old_raw, adjusted.clone()));
                                        self.write_cell_raw(dest, &adjusted);
                                    }
                                }
                            }
                        }
                    }
                    if !changes.is_empty() {
                        self.undo_stack.push(UndoEntry::MultiCellEdit { changes });
                        recalculate(&mut self.sheet, &self.deps);
                        self.dirty = true;
                    }
                }
            }
            Action::NextNonEmpty(count) => {
                let count = count.max(1);
                let (row, col) = self.cursor;
                let mut last = col;
                let mut hops = 0;
                let mut c = col + 1;
                while c < self.sheet.col_count && hops < count {
                    if self.sheet.get_cell((row, c)).is_some() {
                        last = c;
                        hops += 1;
                    }
                    c += 1;
                }
                if hops > 0 {
                    self.cursor = (row, last);
                    self.viewport.ensure_visible(self.cursor);
                }
            }
            Action::PrevNonEmpty(count) => {
                let count = count.max(1);
                let (row, col) = self.cursor;
                if col == 0 {
                    return;
                }
                let mut last = col;
                let mut hops = 0;
                for c in (0..col).rev() {
                    if self.sheet.get_cell((row, c)).is_some() {
                        last = c;
                        hops += 1;
                        if hops >= count {
                            break;
                        }
                    }
                }
                if hops > 0 {
                    self.cursor = (row, last);
                    self.viewport.ensure_visible(self.cursor);
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
            Action::ScrollCursorTop => {
                self.viewport.top_on(self.cursor.0);
            }
            Action::ScrollCursorCenter => {
                self.viewport.center_on(self.cursor.0);
            }
            Action::ScrollCursorBottom => {
                self.viewport.bottom_on(self.cursor.0);
            }
            Action::CursorToViewportTop => {
                self.cursor = (self.viewport.row_offset, self.cursor.1);
                self.viewport.ensure_visible(self.cursor);
            }
            Action::CursorToViewportMiddle => {
                let mid = self
                    .viewport
                    .row_offset
                    .saturating_add(self.viewport.visible_rows / 2);
                let last_row = self.viewport.row_offset + self.viewport.visible_rows;
                let target = mid.min(last_row.saturating_sub(1));
                self.cursor = (target, self.cursor.1);
                self.viewport.ensure_visible(self.cursor);
            }
            Action::CursorToViewportBottom => {
                let bottom =
                    (self.viewport.row_offset + self.viewport.visible_rows).saturating_sub(1);
                self.cursor = (bottom, self.cursor.1);
                self.viewport.ensure_visible(self.cursor);
            }
            Action::ScrollLineDown => {
                self.viewport.row_offset = self.viewport.row_offset.saturating_add(1);
            }
            Action::ScrollLineUp => {
                self.viewport.row_offset = self.viewport.row_offset.saturating_sub(1);
            }
            Action::SetMark(name) => {
                if name.is_ascii_lowercase() {
                    self.marks.insert(name, self.cursor);
                }
            }
            Action::JumpToMark { name, line_wise } => match self.marks.get(&name).copied() {
                Some(pos) => {
                    self.record_jump();
                    self.cursor = if line_wise { (pos.0, 0) } else { pos };
                    self.viewport.ensure_visible(self.cursor);
                }
                None => {
                    self.status_message = Some(format!("E20: Mark not set: {}", name));
                }
            },
            Action::JumpBack => {
                if self.jump_idx == self.jump_list.len() && !self.jump_list.is_empty() {
                    let cur = self.cursor;
                    self.jump_list.push(cur);
                }
                if self.jump_idx > 0 {
                    self.jump_idx -= 1;
                    self.cursor = self.jump_list[self.jump_idx];
                    self.viewport.ensure_visible(self.cursor);
                }
            }
            Action::JumpForward => {
                if self.jump_idx + 1 < self.jump_list.len() {
                    self.jump_idx += 1;
                    self.cursor = self.jump_list[self.jump_idx];
                    self.viewport.ensure_visible(self.cursor);
                }
            }
            Action::BlockJumpDown => {
                if let Some(row) = self.block_jump_down() {
                    self.cursor = (row, self.cursor.1);
                    self.viewport.ensure_visible(self.cursor);
                }
            }
            Action::BlockJumpUp => {
                if let Some(row) = self.block_jump_up() {
                    self.cursor = (row, self.cursor.1);
                    self.viewport.ensure_visible(self.cursor);
                }
            }
            Action::ReselectLastVisual => {
                // Re-entry into visual mode is orchestrated in main.rs because
                // it owns the live `VisualState`. App only stores the snapshot.
            }
            Action::SearchCellValue { backward } => {
                let pattern = self
                    .sheet
                    .get_cell(self.cursor)
                    .map(|c| c.value.to_string());
                if let Some(p) = pattern.filter(|s| !s.is_empty()) {
                    let direction = if backward {
                        crate::action::SearchDirection::Backward
                    } else {
                        crate::action::SearchDirection::Forward
                    };
                    self.process_action(Action::Search {
                        pattern: p,
                        direction,
                    });
                } else {
                    self.status_message = Some("No string under cursor".into());
                }
            }
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

    fn find_next(&mut self, forward: bool) {
        let pattern = match self.search_pattern.clone() {
            Some(p) => p,
            None => return,
        };
        if !self.find_from(&pattern, forward, self.cursor, false) {
            self.status_message = Some(format!("Pattern not found: {}", pattern));
        }
    }

    /// Scan cells starting at `origin` in row-major order (wrapping) for the
    /// first cell whose displayed value contains `pattern` (case-insensitive).
    /// On a hit, moves the cursor and returns `true`. `include_origin` decides
    /// whether `origin` itself is a candidate — `true` for incremental search
    /// so typing the first matching char already lands on the origin if it
    /// matches; `false` for `n`/`N` so they always step.
    fn find_from(
        &mut self,
        pattern: &str,
        forward: bool,
        origin: CellPos,
        include_origin: bool,
    ) -> bool {
        let total_cells = self.sheet.row_count * self.sheet.col_count.max(1);
        if total_cells == 0 {
            return false;
        }
        let cols = self.sheet.col_count.max(1);
        let needle = pattern.to_lowercase();
        let start = if include_origin { 0 } else { 1 };

        for offset in start..=total_cells {
            let flat = origin.0 * cols + origin.1;
            let next_flat = if forward {
                (flat + offset) % total_cells
            } else {
                (flat + total_cells - offset) % total_cells
            };
            let row = next_flat / cols;
            let col = next_flat % cols;

            if let Some(cell) = self.sheet.get_cell((row, col)) {
                if cell.value.to_string().to_lowercase().contains(&needle) {
                    self.cursor = (row, col);
                    self.viewport.ensure_visible(self.cursor);
                    return true;
                }
            }
        }
        false
    }

    /// Move the cursor along the current row to the next non-empty cell whose
    /// displayed value starts with `ch` (case-insensitive). On no match the
    /// cursor stays put. `inclusive = false` lands one cell short (vim `t`).
    fn find_char_in_row(&mut self, ch: char, forward: bool, inclusive: bool) {
        let target = ch.to_lowercase().next().unwrap_or(ch);
        let (row, col) = self.cursor;
        let cols = self.sheet.col_count;
        if cols == 0 {
            return;
        }

        let cell_starts_with = |sheet: &Sheet, pos: CellPos, t: char| -> bool {
            let Some(cell) = sheet.get_cell(pos) else {
                return false;
            };
            let s = cell.value.to_string();
            if s.is_empty() {
                return false;
            }
            s.chars()
                .next()
                .map(|c| c.to_lowercase().next().unwrap_or(c) == t)
                .unwrap_or(false)
        };

        if forward {
            for c in (col + 1)..cols {
                if cell_starts_with(&self.sheet, (row, c), target) {
                    let landing = if inclusive {
                        c
                    } else {
                        c.saturating_sub(1).max(col)
                    };
                    self.cursor = (row, landing);
                    self.viewport.ensure_visible(self.cursor);
                    return;
                }
            }
        } else {
            for c in (0..col).rev() {
                if cell_starts_with(&self.sheet, (row, c), target) {
                    let landing = if inclusive { c } else { (c + 1).min(col) };
                    self.cursor = (row, landing);
                    self.viewport.ensure_visible(self.cursor);
                    return;
                }
            }
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
                recalculate(&mut self.sheet, &self.deps);
            }
            UndoEntry::MultiCellEdit { changes } => {
                for (pos, old_raw, new_raw) in changes {
                    let raw = if redo { new_raw } else { old_raw };
                    self.write_cell_raw(*pos, raw);
                }
                recalculate(&mut self.sheet, &self.deps);
            }
        }
    }

    /// Write `raw` into `pos`, keeping the dep graph consistent and marking
    /// dependents dirty. Does not call `recalculate` — callers batch that.
    ///
    /// - empty raw → clear cell + drop graph entry
    /// - formula  → register/replace dependencies
    /// - value    → drop any stale graph entry from a prior formula
    fn write_cell_raw(&mut self, pos: CellPos, raw: &str) {
        if raw.is_empty() {
            self.sheet.clear_cell(pos);
            self.deps.remove(pos);
        } else if raw.starts_with('=') {
            set_formula(&mut self.sheet, &mut self.deps, pos, raw);
        } else {
            self.sheet.set_cell(pos, raw);
            self.deps.remove(pos);
        }
        mark_dirty(&mut self.sheet, &self.deps, pos);
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
        assert_eq!(app.command_kind, CommandKind::Slash);
        assert!(app.command_line.is_empty());
    }

    #[test]
    fn enter_search_backward_sets_question_prefix() {
        use crate::action::{CommandKind, SearchDirection};
        let mut app = App::new();
        app.process_action(Action::EnterSearch(SearchDirection::Backward));
        assert_eq!(app.mode, Mode::Command);
        assert_eq!(app.command_kind, CommandKind::Question);
    }

    #[test]
    fn change_mode_command_resets_to_colon() {
        use crate::action::{CommandKind, SearchDirection};
        let mut app = App::new();
        // Enter search, then user backs out and re-enters via `:`.
        app.process_action(Action::EnterSearch(SearchDirection::Forward));
        app.process_action(Action::ChangeMode(Mode::Normal));
        app.process_action(Action::ChangeMode(Mode::Command));
        assert_eq!(app.command_kind, CommandKind::Colon);
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
        assert_eq!(app.search_pattern.as_deref(), Some("gamma"));
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
        assert_eq!(app.search_pattern.as_deref(), Some("first"));
    }

    // ── Incremental search ──────────────────────────────────────────────────

    #[test]
    fn enter_search_records_origin() {
        use crate::action::SearchDirection;
        let mut app = App::new();
        app.cursor = (3, 2);
        app.process_action(Action::EnterSearch(SearchDirection::Forward));
        assert_eq!(app.search_origin, Some((3, 2)));
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
        assert_eq!(app.search_origin, None);
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
        assert_eq!(app.search_pattern.as_deref(), Some("g"));
        assert_eq!(app.search_origin, None);
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
        assert!(app.status_message.is_some());
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
        assert_eq!(app.search_origin, None);
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
        assert!(app
            .status_message
            .as_deref()
            .unwrap()
            .contains("Mark not set"));
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
        assert_eq!(app.search_pattern.as_deref(), Some("foo"));
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
        assert_eq!(app.search_pattern, None);
        assert_eq!(
            app.status_message.as_deref(),
            Some("No string under cursor")
        );
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
}
