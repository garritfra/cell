use crate::action::{Action, CaseOp, Direction, Mode};
use crate::app::App;
use cell_sheet_core::model::CellPos;
use crossterm::event::{KeyCode, KeyEvent};

const COUNT_CAP: usize = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VisualKind {
    Character,
    Line,
    Block,
}

pub struct VisualState {
    pub anchor: CellPos,
    pub kind: VisualKind,
    /// Numeric count accumulated from digit keys while in visual mode, so
    /// that `5j` extends the selection by 5 rows instead of 1.
    pending_count: Option<usize>,
}

impl VisualState {
    pub fn new(anchor: CellPos, kind: VisualKind) -> Self {
        VisualState {
            anchor,
            kind,
            pending_count: None,
        }
    }

    pub fn selection(&self, cursor: CellPos) -> (CellPos, CellPos) {
        let r1 = self.anchor.0.min(cursor.0);
        let r2 = self.anchor.0.max(cursor.0);
        match self.kind {
            VisualKind::Line => {
                // Select entire rows — use 0 to usize::MAX so all visible columns match
                ((r1, 0), (r2, usize::MAX))
            }
            VisualKind::Character | VisualKind::Block => {
                let c1 = self.anchor.1.min(cursor.1);
                let c2 = self.anchor.1.max(cursor.1);
                ((r1, c1), (r2, c2))
            }
        }
    }

    /// Consume the pending count, defaulting to 1.
    fn take_count(&mut self) -> usize {
        self.pending_count.take().unwrap_or(1).max(1)
    }

    pub fn handle_key(&mut self, key: KeyEvent, app: &App) -> Action {
        let (start, end) = self.selection(app.cursor);

        // Digit keys accumulate a count prefix for motions (`5j`, `3l`).
        if let KeyCode::Char(c) = key.code {
            if let Some(d) = c.to_digit(10) {
                let next = self
                    .pending_count
                    .unwrap_or(0)
                    .saturating_mul(10)
                    .saturating_add(d as usize)
                    .min(COUNT_CAP);
                self.pending_count = Some(next);
                return Action::Noop;
            }
        }

        match key.code {
            KeyCode::Char('h') | KeyCode::Left => {
                Action::MoveCursor(Direction::Left, self.take_count())
            }
            KeyCode::Char('j') | KeyCode::Down => {
                Action::MoveCursor(Direction::Down, self.take_count())
            }
            KeyCode::Char('k') | KeyCode::Up => {
                Action::MoveCursor(Direction::Up, self.take_count())
            }
            KeyCode::Char('l') | KeyCode::Right => {
                Action::MoveCursor(Direction::Right, self.take_count())
            }
            KeyCode::Char('c') => {
                self.pending_count = None;
                Action::ChangeRange { start, end }
            }
            KeyCode::Char('d') => {
                self.pending_count = None;
                Action::ClearRange { start, end }
            }
            KeyCode::Char('y') => {
                self.pending_count = None;
                Action::YankRange { start, end }
            }
            KeyCode::Char('u') => {
                self.pending_count = None;
                Action::CaseOpRange {
                    start,
                    end,
                    op: CaseOp::ToLower,
                }
            }
            KeyCode::Char('U') => {
                self.pending_count = None;
                Action::CaseOpRange {
                    start,
                    end,
                    op: CaseOp::ToUpper,
                }
            }
            KeyCode::Char('~') => {
                self.pending_count = None;
                Action::CaseOpRange {
                    start,
                    end,
                    op: CaseOp::ToggleAll,
                }
            }
            KeyCode::Esc => Action::ChangeMode(Mode::Normal),
            _ => {
                self.pending_count = None;
                Action::Noop
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn selection_normalized() {
        let state = VisualState::new((2, 3), VisualKind::Character);
        let (start, end) = state.selection((0, 1));
        assert_eq!(start, (0, 1));
        assert_eq!(end, (2, 3));
    }

    #[test]
    fn selection_same_cell() {
        let state = VisualState::new((1, 1), VisualKind::Character);
        let (start, end) = state.selection((1, 1));
        assert_eq!(start, (1, 1));
        assert_eq!(end, (1, 1));
    }

    #[test]
    fn selection_line_selects_full_rows() {
        let state = VisualState::new((1, 3), VisualKind::Line);
        let (start, end) = state.selection((3, 1));
        assert_eq!(start, (1, 0));
        assert_eq!(end, (3, usize::MAX));
    }

    #[test]
    fn hjkl_in_visual() {
        let app = App::new();
        let mut state = VisualState::new((0, 0), VisualKind::Character);
        assert_eq!(
            state.handle_key(key(KeyCode::Char('j')), &app),
            Action::MoveCursor(Direction::Down, 1)
        );
    }

    #[test]
    fn d_clears_range() {
        let mut app = App::new();
        app.cursor = (2, 2);
        let mut state = VisualState::new((0, 0), VisualKind::Character);
        assert_eq!(
            state.handle_key(key(KeyCode::Char('d')), &app),
            Action::ClearRange {
                start: (0, 0),
                end: (2, 2)
            }
        );
    }

    #[test]
    fn y_yanks_range() {
        let mut app = App::new();
        app.cursor = (1, 1);
        let mut state = VisualState::new((0, 0), VisualKind::Character);
        assert_eq!(
            state.handle_key(key(KeyCode::Char('y')), &app),
            Action::YankRange {
                start: (0, 0),
                end: (1, 1)
            }
        );
    }

    #[test]
    fn esc_exits_visual() {
        let app = App::new();
        let mut state = VisualState::new((0, 0), VisualKind::Character);
        assert_eq!(
            state.handle_key(key(KeyCode::Esc), &app),
            Action::ChangeMode(Mode::Normal)
        );
    }

    // --- count-prefix tests ----------------------------------------------

    #[test]
    fn count_j_extends_selection_by_count() {
        // `v` then `3j` should move cursor down by 3 (selection extends).
        let app = App::new();
        let mut state = VisualState::new((0, 0), VisualKind::Character);
        let _ = state.handle_key(key(KeyCode::Char('3')), &app);
        assert_eq!(
            state.handle_key(key(KeyCode::Char('j')), &app),
            Action::MoveCursor(Direction::Down, 3)
        );
    }

    #[test]
    fn count_l_moves_right_by_count() {
        // `v` then `3l` selects current cell + 3 cells to the right.
        let app = App::new();
        let mut state = VisualState::new((0, 0), VisualKind::Character);
        let _ = state.handle_key(key(KeyCode::Char('3')), &app);
        assert_eq!(
            state.handle_key(key(KeyCode::Char('l')), &app),
            Action::MoveCursor(Direction::Right, 3)
        );
    }

    #[test]
    fn multi_digit_count_in_visual() {
        let app = App::new();
        let mut state = VisualState::new((0, 0), VisualKind::Character);
        let _ = state.handle_key(key(KeyCode::Char('1')), &app);
        let _ = state.handle_key(key(KeyCode::Char('2')), &app);
        assert_eq!(
            state.handle_key(key(KeyCode::Char('k')), &app),
            Action::MoveCursor(Direction::Up, 12)
        );
    }

    #[test]
    fn count_cleared_after_motion() {
        let app = App::new();
        let mut state = VisualState::new((0, 0), VisualKind::Character);
        let _ = state.handle_key(key(KeyCode::Char('5')), &app);
        let _ = state.handle_key(key(KeyCode::Char('j')), &app);
        // Count was consumed; next motion defaults to 1.
        assert_eq!(
            state.handle_key(key(KeyCode::Char('j')), &app),
            Action::MoveCursor(Direction::Down, 1)
        );
    }

    #[test]
    fn count_cleared_on_operator() {
        // A pending count is discarded when `d`, `y`, or `c` is pressed.
        let mut app = App::new();
        app.cursor = (1, 1);
        let mut state = VisualState::new((0, 0), VisualKind::Character);
        let _ = state.handle_key(key(KeyCode::Char('3')), &app);
        // `d` fires the range op and discards the count.
        assert_eq!(
            state.handle_key(key(KeyCode::Char('d')), &app),
            Action::ClearRange {
                start: (0, 0),
                end: (1, 1),
            }
        );
        assert!(state.pending_count.is_none());
    }

    // --- visual case-op key-binding tests ---------------------------------

    #[test]
    fn u_emits_to_lower_range() {
        let mut app = App::new();
        app.cursor = (1, 2);
        let mut state = VisualState::new((0, 0), VisualKind::Character);
        assert_eq!(
            state.handle_key(key(KeyCode::Char('u')), &app),
            Action::CaseOpRange {
                start: (0, 0),
                end: (1, 2),
                op: CaseOp::ToLower,
            }
        );
    }

    #[test]
    fn shift_u_emits_to_upper_range() {
        let mut app = App::new();
        app.cursor = (0, 3);
        let mut state = VisualState::new((0, 0), VisualKind::Character);
        assert_eq!(
            state.handle_key(key(KeyCode::Char('U')), &app),
            Action::CaseOpRange {
                start: (0, 0),
                end: (0, 3),
                op: CaseOp::ToUpper,
            }
        );
    }

    #[test]
    fn tilde_emits_toggle_all_range() {
        let mut app = App::new();
        app.cursor = (2, 1);
        let mut state = VisualState::new((0, 0), VisualKind::Character);
        assert_eq!(
            state.handle_key(key(KeyCode::Char('~')), &app),
            Action::CaseOpRange {
                start: (0, 0),
                end: (2, 1),
                op: CaseOp::ToggleAll,
            }
        );
    }
}
