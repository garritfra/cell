use super::{apply_case_op, App, FileFormat};
use crate::action::{Action, CaseOp, CommandKind, Direction, Mode, SearchDirection};
use crate::clipboard::Register;
use crate::undo::UndoEntry;
use cell_sheet_core::model::CellPos;

impl App {
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
                self.set_cell_raw(pos, &raw);
                if self.batch_depth == 0 {
                    self.recalculate();
                }
                self.dirty = true;
                self.last_change = Some(Action::EditCell(pos, raw));
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
                    self.command_history_idx = None;
                    self.command_history_scratch.clear();
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
                    self.write_cell_raw(pos, "");
                    if self.batch_depth == 0 {
                        self.recalculate();
                    }
                    self.dirty = true;
                }
                self.last_change = Some(Action::ClearCell(pos));
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
                    self.write_cell_raw(pos, "");
                    if self.batch_depth == 0 {
                        self.recalculate();
                    }
                    self.dirty = true;
                }
                self.insert_buffer = String::new();
                self.mode = Mode::Insert;
            }
            Action::ChangeRange { start, end } => {
                let max_col = end.1.min(self.sheet.col_count.saturating_sub(1));
                let mut changes = Vec::new();
                self.begin_batch();
                for row in start.0..=end.0 {
                    for col in start.1..=max_col {
                        let old_raw = self
                            .sheet
                            .get_cell((row, col))
                            .map(|c| c.raw.clone())
                            .unwrap_or_default();
                        if !old_raw.is_empty() {
                            changes.push(((row, col), old_raw, String::new()));
                            self.write_cell_raw((row, col), "");
                        }
                    }
                }
                if !changes.is_empty() {
                    self.undo_stack.push(UndoEntry::MultiCellEdit { changes });
                    self.dirty = true;
                }
                self.commit_batch();
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
                    let expected_delim = match format {
                        FileFormat::Csv => b',',
                        FileFormat::Tsv => b'\t',
                        FileFormat::Cell => 0,
                    };
                    if !matches!(format, FileFormat::Cell) && self.delimiter != expected_delim {
                        self.status_message = Some(format!(
                            "Non-standard delimiter '{}' will be used. Use :w! to force, or save as .tsv / .psv.",
                            self.delimiter as char
                        ));
                        return;
                    }
                    self.do_save(&path, format);
                } else {
                    self.status_message = Some("No file name".into());
                }
            }
            Action::ForceSave(path_opt) => {
                // ForceSave intentionally bypasses both the formula-flatten warning and the
                // non-standard-delimiter warning. The user has explicitly opted in via :w!
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
                self.command_history_idx = None;
                self.command_history_scratch.clear();
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
                cell_sheet_core::engine::SheetEngine::new(&mut self.sheet, &mut self.deps)
                    .sort_by_column_and_recalculate(col, ascending);
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
                self.begin_batch();
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
                    self.dirty = true;
                }
                self.commit_batch();
                self.last_change = Some(Action::ClearRange { start, end });
            }
            Action::DeleteRow { start, count } => {
                let count = count.max(1);
                let cols = self.sheet.col_count;
                let mut changes = Vec::new();
                self.begin_batch();
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
                            self.write_cell_raw((start, col), "");
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
                                self.write_cell_raw((r, col), "");
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
                self.commit_batch();
                self.last_change = Some(Action::DeleteRow { start, count });
            }
            Action::Paste(pos) | Action::PasteBefore(pos) => {
                let is_after = matches!(action, Action::Paste(_));
                if let Some(reg) = &self.register.clone() {
                    let mut changes: Vec<(CellPos, String, String)> = Vec::new();
                    self.begin_batch();
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
                        self.dirty = true;
                    }
                    self.commit_batch();
                }
                if is_after {
                    self.last_change = Some(Action::Paste(pos));
                } else {
                    self.last_change = Some(Action::PasteBefore(pos));
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
            Action::SetDelimiter(d) => {
                self.delimiter = d;
                self.status_message = Some(format!("Delimiter set to '{}'", d as char));
            }
            Action::SetMouse(b) => {
                self.mouse_enabled = b;
            }
            Action::ToggleMouse => {
                self.mouse_enabled = !self.mouse_enabled;
            }
            Action::MouseClickCell(pos) => {
                self.cursor = pos;
                self.viewport.ensure_visible(self.cursor);
            }
            Action::MouseDragTo(pos) => {
                self.cursor = pos;
                self.viewport.ensure_visible(self.cursor);
            }
            Action::MouseSelectColumn(col) => {
                // Cursor stays at the top of the clicked column so the
                // highlighted cell is where the user pointed; the visual
                // anchor at (last_row, _) makes the selection rectangle
                // cover the full column.
                self.cursor = (0, col);
                self.viewport.ensure_visible(self.cursor);
            }
            Action::MouseSelectRow(row) => {
                // Cursor stays at the leftmost cell of the clicked row;
                // the visual anchor at (_, last_col) makes the selection
                // rectangle cover the full row.
                self.cursor = (row, 0);
                self.viewport.ensure_visible(self.cursor);
            }
            Action::MouseScroll { dx, dy } => {
                if dy > 0 {
                    self.viewport.row_offset = self.viewport.row_offset.saturating_add(dy as usize);
                } else if dy < 0 {
                    self.viewport.row_offset =
                        self.viewport.row_offset.saturating_sub((-dy) as usize);
                }
                if dx > 0 {
                    self.viewport.col_offset = self.viewport.col_offset.saturating_add(dx as usize);
                } else if dx < 0 {
                    self.viewport.col_offset =
                        self.viewport.col_offset.saturating_sub((-dx) as usize);
                }
            }
            Action::SetStatus(msg) => {
                self.status_message = Some(msg);
            }
            Action::RepeatLastChange => {
                if let Some(change) = self.last_change.take() {
                    let saved = change.clone();
                    let rebound = self.rebind_change_to_cursor(change);
                    self.process_action(rebound);
                    self.last_change = Some(saved);
                }
            }
            Action::CaseOpCell { pos, op } => {
                let raw = self
                    .sheet
                    .get_cell(pos)
                    .map(|c| c.raw.clone())
                    .unwrap_or_default();
                if raw.is_empty() {
                    if op == CaseOp::ToggleFirst {
                        self.process_action(Action::MoveCursor(Direction::Right, 1));
                    }
                    return;
                }
                if raw.starts_with('=') {
                    self.status_message = Some("Cannot change case of a formula cell".into());
                    return;
                }
                let new_raw = apply_case_op(&raw, op);
                if new_raw != raw {
                    self.undo_stack.push(UndoEntry::CellEdit {
                        pos,
                        old_raw: raw,
                        new_raw: new_raw.clone(),
                    });
                    self.set_cell_raw(pos, &new_raw);
                    self.recalculate();
                    self.dirty = true;
                }
                self.last_change = Some(Action::CaseOpCell { pos, op });
                if op == CaseOp::ToggleFirst {
                    self.process_action(Action::MoveCursor(Direction::Right, 1));
                }
            }
            Action::CaseOpRange { start, end, op } => {
                let max_col = end.1.min(self.sheet.col_count.saturating_sub(1));
                let mut changes: Vec<(CellPos, String, String)> = Vec::new();
                for row in start.0..=end.0 {
                    for col in start.1..=max_col {
                        let raw = self
                            .sheet
                            .get_cell((row, col))
                            .map(|c| c.raw.clone())
                            .unwrap_or_default();
                        if raw.is_empty() || raw.starts_with('=') {
                            continue;
                        }
                        let new_raw = apply_case_op(&raw, op);
                        if new_raw != raw {
                            changes.push(((row, col), raw, new_raw));
                        }
                    }
                }
                if !changes.is_empty() {
                    for (pos, _, new_raw) in &changes {
                        self.set_cell_raw(*pos, new_raw);
                    }
                    self.recalculate();
                    self.undo_stack.push(UndoEntry::MultiCellEdit { changes });
                    self.dirty = true;
                }
                self.last_change = Some(Action::CaseOpRange { start, end, op });
            }
            Action::AdjustNumber { pos, delta } => {
                if let Some(cell) = self.sheet.get_cell(pos) {
                    let raw = cell.raw.clone();
                    if raw.starts_with('=') {
                        self.status_message = Some("E: Cannot increment a formula".into());
                    } else if let Ok(n) = raw.parse::<f64>() {
                        let new_raw = (n + delta as f64).to_string();
                        self.undo_stack.push(UndoEntry::CellEdit {
                            pos,
                            old_raw: raw,
                            new_raw: new_raw.clone(),
                        });
                        self.set_cell_raw(pos, &new_raw);
                        self.recalculate();
                        self.dirty = true;
                    }
                    // text or empty string: no-op
                }
                // empty cell (not in sheet): no-op
                self.last_change = Some(Action::AdjustNumber { pos, delta });
            }
            Action::Open(_) | Action::Resize => {}
        }
    }
}
