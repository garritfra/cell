use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use cell_core::model::CellPos;
use crate::action::{Action, Direction, Mode};
use crate::app::App;

pub struct VisualState {
    pub anchor: CellPos,
    pub is_block: bool,
}

impl VisualState {
    pub fn new(anchor: CellPos, is_block: bool) -> Self {
        VisualState { anchor, is_block }
    }

    pub fn selection(&self, cursor: CellPos) -> (CellPos, CellPos) {
        let r1 = self.anchor.0.min(cursor.0);
        let r2 = self.anchor.0.max(cursor.0);
        let c1 = self.anchor.1.min(cursor.1);
        let c2 = self.anchor.1.max(cursor.1);
        ((r1, c1), (r2, c2))
    }

    pub fn handle_key(&self, key: KeyEvent, app: &App) -> Action {
        let (start, end) = self.selection(app.cursor);
        match key.code {
            KeyCode::Char('h') | KeyCode::Left => Action::MoveCursor(Direction::Left),
            KeyCode::Char('j') | KeyCode::Down => Action::MoveCursor(Direction::Down),
            KeyCode::Char('k') | KeyCode::Up => Action::MoveCursor(Direction::Up),
            KeyCode::Char('l') | KeyCode::Right => Action::MoveCursor(Direction::Right),
            KeyCode::Char('d') => Action::ClearRange { start, end },
            KeyCode::Char('y') => Action::YankRange { start, end },
            KeyCode::Esc => Action::ChangeMode(Mode::Normal),
            _ => Action::Noop,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent { KeyEvent::new(code, KeyModifiers::NONE) }

    #[test]
    fn selection_normalized() {
        let state = VisualState::new((2, 3), false);
        let (start, end) = state.selection((0, 1));
        assert_eq!(start, (0, 1));
        assert_eq!(end, (2, 3));
    }

    #[test]
    fn selection_same_cell() {
        let state = VisualState::new((1, 1), false);
        let (start, end) = state.selection((1, 1));
        assert_eq!(start, (1, 1));
        assert_eq!(end, (1, 1));
    }

    #[test]
    fn hjkl_in_visual() {
        let app = App::new();
        let state = VisualState::new((0, 0), false);
        assert_eq!(state.handle_key(key(KeyCode::Char('j')), &app), Action::MoveCursor(Direction::Down));
    }

    #[test]
    fn d_clears_range() {
        let mut app = App::new();
        app.cursor = (2, 2);
        let state = VisualState::new((0, 0), false);
        assert_eq!(state.handle_key(key(KeyCode::Char('d')), &app), Action::ClearRange { start: (0, 0), end: (2, 2) });
    }

    #[test]
    fn y_yanks_range() {
        let mut app = App::new();
        app.cursor = (1, 1);
        let state = VisualState::new((0, 0), false);
        assert_eq!(state.handle_key(key(KeyCode::Char('y')), &app), Action::YankRange { start: (0, 0), end: (1, 1) });
    }

    #[test]
    fn esc_exits_visual() {
        let app = App::new();
        let state = VisualState::new((0, 0), false);
        assert_eq!(state.handle_key(key(KeyCode::Esc), &app), Action::ChangeMode(Mode::Normal));
    }
}
