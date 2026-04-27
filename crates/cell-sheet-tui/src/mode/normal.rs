use crate::action::{Action, Direction, Mode};
use crate::app::App;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub struct NormalState {
    pub pending: Option<char>,
}

impl NormalState {
    pub fn new() -> Self {
        NormalState { pending: None }
    }

    pub fn handle_key(&mut self, key: KeyEvent, app: &App) -> Action {
        // Handle Ctrl combinations first
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return match key.code {
                KeyCode::Char('d') => Action::HalfPageDown,
                KeyCode::Char('u') => Action::HalfPageUp,
                KeyCode::Char('f') => Action::PageDown,
                KeyCode::Char('b') => Action::PageUp,
                KeyCode::Char('r') => Action::Redo,
                KeyCode::Char('v') => Action::ChangeMode(Mode::VisualBlock),
                KeyCode::Char('e') => Action::ScrollLineDown,
                KeyCode::Char('y') => Action::ScrollLineUp,
                KeyCode::Char('o') => Action::JumpBack,
                _ => Action::Noop,
            };
        }

        // Handle pending sequences
        if let Some(prev) = self.pending.take() {
            return match (prev, key.code) {
                ('g', KeyCode::Char('g')) => Action::GotoFirstRow,
                ('d', KeyCode::Char('d')) => Action::DeleteRow(app.cursor.0),
                ('y', KeyCode::Char('y')) => Action::YankRow(app.cursor.0),
                ('z', KeyCode::Char('z')) => Action::ScrollCursorCenter,
                ('z', KeyCode::Char('t')) => Action::ScrollCursorTop,
                ('z', KeyCode::Char('b')) => Action::ScrollCursorBottom,
                ('m', KeyCode::Char(c)) if c.is_ascii_lowercase() => Action::SetMark(c),
                ('\'', KeyCode::Char(c)) if c.is_ascii_lowercase() => Action::JumpToMark {
                    name: c,
                    line_wise: true,
                },
                ('`', KeyCode::Char(c)) if c.is_ascii_lowercase() => Action::JumpToMark {
                    name: c,
                    line_wise: false,
                },
                _ => Action::Noop,
            };
        }

        match key.code {
            KeyCode::Char('h') | KeyCode::Left => Action::MoveCursor(Direction::Left),
            KeyCode::Char('j') | KeyCode::Down => Action::MoveCursor(Direction::Down),
            KeyCode::Char('k') | KeyCode::Up => Action::MoveCursor(Direction::Up),
            KeyCode::Char('l') | KeyCode::Right => Action::MoveCursor(Direction::Right),
            KeyCode::Char('g') => {
                self.pending = Some('g');
                Action::Noop
            }
            KeyCode::Char('d') => {
                self.pending = Some('d');
                Action::Noop
            }
            KeyCode::Char('y') => {
                self.pending = Some('y');
                Action::Noop
            }
            KeyCode::Char('z') => {
                self.pending = Some('z');
                Action::Noop
            }
            KeyCode::Char('m') => {
                self.pending = Some('m');
                Action::Noop
            }
            KeyCode::Char('\'') => {
                self.pending = Some('\'');
                Action::Noop
            }
            KeyCode::Char('`') => {
                self.pending = Some('`');
                Action::Noop
            }
            KeyCode::Char('H') => Action::CursorToViewportTop,
            KeyCode::Char('M') => Action::CursorToViewportMiddle,
            KeyCode::Char('L') => Action::CursorToViewportBottom,
            KeyCode::Char('G') => Action::GotoLastRow,
            KeyCode::Char('0') => Action::GotoFirstCol,
            KeyCode::Char('$') => Action::GotoLastCol,
            KeyCode::Char('w') => Action::NextNonEmpty,
            KeyCode::Char('b') => Action::PrevNonEmpty,
            KeyCode::Char('i') | KeyCode::Char('a') => Action::ChangeMode(Mode::Insert),
            KeyCode::Char('o') => Action::ChangeMode(Mode::Insert),
            KeyCode::Char('v') => Action::ChangeMode(Mode::Visual),
            KeyCode::Char('V') => Action::ChangeMode(Mode::VisualLine),
            KeyCode::Char(':') => Action::ChangeMode(Mode::Command),
            KeyCode::Char('c') => Action::ChangeCell(app.cursor),
            KeyCode::Char('x') => Action::ClearCell(app.cursor),
            KeyCode::Char('p') => Action::Paste(app.cursor),
            KeyCode::Char('P') => Action::PasteBefore(app.cursor),
            KeyCode::Char('u') => Action::Undo,
            KeyCode::Char('/') => Action::ChangeMode(Mode::Command),
            KeyCode::Char('n') => Action::SearchNext,
            KeyCode::Char('N') => Action::SearchPrev,
            KeyCode::Tab => Action::JumpForward,
            KeyCode::Enter => Action::ChangeMode(Mode::Insert),
            _ => Action::Noop,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEvent;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl_key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    #[test]
    fn hjkl_navigation() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            state.handle_key(key(KeyCode::Char('h')), &app),
            Action::MoveCursor(Direction::Left)
        );
        assert_eq!(
            state.handle_key(key(KeyCode::Char('j')), &app),
            Action::MoveCursor(Direction::Down)
        );
        assert_eq!(
            state.handle_key(key(KeyCode::Char('k')), &app),
            Action::MoveCursor(Direction::Up)
        );
        assert_eq!(
            state.handle_key(key(KeyCode::Char('l')), &app),
            Action::MoveCursor(Direction::Right)
        );
    }

    #[test]
    fn gg_goes_to_first_row() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            state.handle_key(key(KeyCode::Char('g')), &app),
            Action::Noop
        );
        assert_eq!(
            state.handle_key(key(KeyCode::Char('g')), &app),
            Action::GotoFirstRow
        );
    }

    #[test]
    fn shift_g_goes_to_last_row() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            state.handle_key(key(KeyCode::Char('G')), &app),
            Action::GotoLastRow
        );
    }

    #[test]
    fn dd_deletes_row() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            state.handle_key(key(KeyCode::Char('d')), &app),
            Action::Noop
        );
        assert_eq!(
            state.handle_key(key(KeyCode::Char('d')), &app),
            Action::DeleteRow(0)
        );
    }

    #[test]
    fn yy_yanks_row() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            state.handle_key(key(KeyCode::Char('y')), &app),
            Action::Noop
        );
        assert_eq!(
            state.handle_key(key(KeyCode::Char('y')), &app),
            Action::YankRow(0)
        );
    }

    #[test]
    fn i_enters_insert_mode() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            state.handle_key(key(KeyCode::Char('i')), &app),
            Action::ChangeMode(Mode::Insert)
        );
    }

    #[test]
    fn colon_enters_command_mode() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            state.handle_key(key(KeyCode::Char(':')), &app),
            Action::ChangeMode(Mode::Command)
        );
    }

    #[test]
    fn x_clears_cell() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            state.handle_key(key(KeyCode::Char('x')), &app),
            Action::ClearCell((0, 0))
        );
    }

    #[test]
    fn ctrl_d_half_page_down() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(state.handle_key(ctrl_key('d'), &app), Action::HalfPageDown);
    }

    #[test]
    fn zz_scrolls_cursor_to_center() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            state.handle_key(key(KeyCode::Char('z')), &app),
            Action::Noop
        );
        assert_eq!(
            state.handle_key(key(KeyCode::Char('z')), &app),
            Action::ScrollCursorCenter
        );
    }

    #[test]
    fn zt_scrolls_cursor_to_top() {
        let app = App::new();
        let mut state = NormalState::new();
        state.handle_key(key(KeyCode::Char('z')), &app);
        assert_eq!(
            state.handle_key(key(KeyCode::Char('t')), &app),
            Action::ScrollCursorTop
        );
    }

    #[test]
    fn zb_scrolls_cursor_to_bottom() {
        let app = App::new();
        let mut state = NormalState::new();
        state.handle_key(key(KeyCode::Char('z')), &app);
        assert_eq!(
            state.handle_key(key(KeyCode::Char('b')), &app),
            Action::ScrollCursorBottom
        );
    }

    #[test]
    fn z_followed_by_unknown_is_noop_and_clears_pending() {
        let app = App::new();
        let mut state = NormalState::new();
        state.handle_key(key(KeyCode::Char('z')), &app);
        assert_eq!(
            state.handle_key(key(KeyCode::Char('q')), &app),
            Action::Noop
        );
        assert!(state.pending.is_none());
    }

    #[test]
    fn capital_h_m_l_jump_within_viewport() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            state.handle_key(key(KeyCode::Char('H')), &app),
            Action::CursorToViewportTop
        );
        assert_eq!(
            state.handle_key(key(KeyCode::Char('M')), &app),
            Action::CursorToViewportMiddle
        );
        assert_eq!(
            state.handle_key(key(KeyCode::Char('L')), &app),
            Action::CursorToViewportBottom
        );
    }

    #[test]
    fn ctrl_e_scrolls_line_down() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(state.handle_key(ctrl_key('e'), &app), Action::ScrollLineDown);
    }

    #[test]
    fn ctrl_y_scrolls_line_up() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(state.handle_key(ctrl_key('y'), &app), Action::ScrollLineUp);
    }

    #[test]
    fn ma_sets_mark() {
        let app = App::new();
        let mut state = NormalState::new();
        state.handle_key(key(KeyCode::Char('m')), &app);
        assert_eq!(
            state.handle_key(key(KeyCode::Char('a')), &app),
            Action::SetMark('a')
        );
    }

    #[test]
    fn apostrophe_a_jumps_line_wise() {
        let app = App::new();
        let mut state = NormalState::new();
        state.handle_key(key(KeyCode::Char('\'')), &app);
        assert_eq!(
            state.handle_key(key(KeyCode::Char('a')), &app),
            Action::JumpToMark {
                name: 'a',
                line_wise: true
            }
        );
    }

    #[test]
    fn backtick_a_jumps_cell_wise() {
        let app = App::new();
        let mut state = NormalState::new();
        state.handle_key(key(KeyCode::Char('`')), &app);
        assert_eq!(
            state.handle_key(key(KeyCode::Char('a')), &app),
            Action::JumpToMark {
                name: 'a',
                line_wise: false
            }
        );
    }

    #[test]
    fn ctrl_o_jumps_back() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(state.handle_key(ctrl_key('o'), &app), Action::JumpBack);
    }

    #[test]
    fn tab_jumps_forward() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            state.handle_key(key(KeyCode::Tab), &app),
            Action::JumpForward
        );
    }

    #[test]
    fn mark_rejects_non_lowercase_char() {
        let app = App::new();
        let mut state = NormalState::new();
        state.handle_key(key(KeyCode::Char('m')), &app);
        assert_eq!(
            state.handle_key(key(KeyCode::Char('A')), &app),
            Action::Noop
        );
    }
}
