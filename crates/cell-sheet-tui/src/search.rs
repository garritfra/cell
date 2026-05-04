use crate::action::{Action, CommandKind, Mode, SearchDirection};
use crate::app::App;
use cell_sheet_core::model::{CellPos, Sheet};

#[derive(Debug, Default, Clone)]
pub struct SearchState {
    pub pattern: Option<String>,
    /// Cursor position captured when the user opened a `/` or `?` prompt.
    /// `Some` for the lifetime of an open search prompt; consumed on Esc
    /// (cursor restored) or Enter with a non-empty pattern (committed).
    pub origin: Option<CellPos>,
    /// Last `f`/`F` target so `;` and `,` can replay it. Holds
    /// `(char, forward, inclusive)`.
    pub last_find: Option<(char, bool, bool)>,
}

impl App {
    pub(crate) fn process_search_action(&mut self, action: Action) {
        match action {
            Action::Search { pattern, direction } => {
                if pattern.is_empty() {
                    // Empty submit: restore cursor to where the prompt opened
                    // (matches vim's behavior of "no match -> no movement"),
                    // and don't overwrite an existing pattern.
                    if let Some(origin) = self.search.origin.take() {
                        self.cursor = origin;
                        self.viewport.ensure_visible(self.cursor);
                    }
                    return;
                }
                self.record_jump();
                self.search.pattern = Some(pattern.clone());
                let forward = direction == SearchDirection::Forward;
                // If a prompt is open (origin set), commit at the incremental
                // position by re-running the search from origin, including
                // origin as a candidate. Without an open prompt (e.g.
                // `Action::Search` dispatched directly), behave like vim's
                // `/<pattern>`: step past the current cell.
                let (origin, include_origin) = match self.search.origin.take() {
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
                self.search.origin = Some(self.cursor);
                self.command_history_idx = None;
                self.command_history_scratch.clear();
                self.mode = Mode::Command;
            }
            Action::SearchIncremental { pattern, direction } => {
                let Some(origin) = self.search.origin else {
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
                if let Some(origin) = self.search.origin.take() {
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
                self.search.last_find = Some((ch, forward, inclusive));
                self.find_char_in_row(ch, forward, inclusive);
            }
            Action::RepeatFind { reversed } => {
                if let Some((ch, forward, inclusive)) = self.search.last_find {
                    let dir = if reversed { !forward } else { forward };
                    self.find_char_in_row(ch, dir, inclusive);
                }
            }
            Action::SearchNext => {
                if self.search.pattern.is_some() {
                    self.record_jump();
                    self.find_next(true);
                }
            }
            Action::SearchPrev => {
                if self.search.pattern.is_some() {
                    self.record_jump();
                    self.find_next(false);
                }
            }
            Action::SearchCellValue { backward } => {
                let pattern = self
                    .sheet
                    .get_cell(self.cursor)
                    .map(|c| c.value.to_string());
                if let Some(p) = pattern.filter(|s| !s.is_empty()) {
                    let direction = if backward {
                        SearchDirection::Backward
                    } else {
                        SearchDirection::Forward
                    };
                    self.process_search_action(Action::Search {
                        pattern: p,
                        direction,
                    });
                } else {
                    self.status_message = Some("No string under cursor".into());
                }
            }
            _ => unreachable!("non-search action routed to search handler"),
        }
    }

    fn find_next(&mut self, forward: bool) {
        let pattern = match self.search.pattern.clone() {
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
    /// whether `origin` itself is a candidate - `true` for incremental search
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
}
