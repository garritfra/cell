use crate::action::{Action, Direction, Mode, SearchDirection};
use crate::app::App;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FindKind {
    /// `f<char>` — forward, inclusive.
    Forward,
    /// `F<char>` — backward, inclusive.
    Backward,
}

pub struct NormalState {
    pub pending: Option<char>,
    /// Set after `f` or `F` so the next keypress is consumed as the target.
    pub pending_find: Option<FindKind>,
}

impl NormalState {
    pub fn new() -> Self {
        NormalState {
            pending: None,
            pending_find: None,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent, app: &App) -> Action {
        // A pending `f`/`F` consumes the next keypress as its target,
        // regardless of modifier (so `f<Shift+a>` still hits 'A').
        if let Some(kind) = self.pending_find.take() {
            return match key.code {
                KeyCode::Char(c) => {
                    let forward = matches!(kind, FindKind::Forward);
                    Action::FindCharInRow {
                        ch: c,
                        forward,
                        inclusive: true,
                    }
                }
                _ => Action::Noop,
            };
        }

        // Handle Ctrl combinations first
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return match key.code {
                KeyCode::Char('d') => Action::HalfPageDown,
                KeyCode::Char('u') => Action::HalfPageUp,
                KeyCode::Char('f') => Action::PageDown,
                KeyCode::Char('b') => Action::PageUp,
                KeyCode::Char('r') => Action::Redo,
                KeyCode::Char('v') => Action::ChangeMode(Mode::VisualBlock),
                _ => Action::Noop,
            };
        }

        // Handle pending sequences
        if let Some(prev) = self.pending.take() {
            return match (prev, key.code) {
                ('g', KeyCode::Char('g')) => Action::GotoFirstRow,
                ('d', KeyCode::Char('d')) => Action::DeleteRow(app.cursor.0),
                ('y', KeyCode::Char('y')) => Action::YankRow(app.cursor.0),
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
            KeyCode::Char('/') => Action::EnterSearch(SearchDirection::Forward),
            KeyCode::Char('?') => Action::EnterSearch(SearchDirection::Backward),
            KeyCode::Char('n') => Action::SearchNext,
            KeyCode::Char('N') => Action::SearchPrev,
            KeyCode::Char('f') => {
                self.pending_find = Some(FindKind::Forward);
                Action::Noop
            }
            KeyCode::Char('F') => {
                self.pending_find = Some(FindKind::Backward);
                Action::Noop
            }
            KeyCode::Char(';') => Action::RepeatFind { reversed: false },
            KeyCode::Char(',') => Action::RepeatFind { reversed: true },
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
    fn slash_enters_forward_search_prompt() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            state.handle_key(key(KeyCode::Char('/')), &app),
            Action::EnterSearch(SearchDirection::Forward)
        );
    }

    #[test]
    fn question_enters_backward_search_prompt() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            state.handle_key(key(KeyCode::Char('?')), &app),
            Action::EnterSearch(SearchDirection::Backward)
        );
    }

    #[test]
    fn f_then_char_emits_forward_find() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            state.handle_key(key(KeyCode::Char('f')), &app),
            Action::Noop
        );
        assert_eq!(
            state.handle_key(key(KeyCode::Char('a')), &app),
            Action::FindCharInRow {
                ch: 'a',
                forward: true,
                inclusive: true,
            }
        );
    }

    #[test]
    fn shift_f_then_char_emits_backward_find() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            state.handle_key(key(KeyCode::Char('F')), &app),
            Action::Noop
        );
        assert_eq!(
            state.handle_key(key(KeyCode::Char('Z')), &app),
            Action::FindCharInRow {
                ch: 'Z',
                forward: false,
                inclusive: true,
            }
        );
    }

    #[test]
    fn pending_find_consumes_only_one_key() {
        let app = App::new();
        let mut state = NormalState::new();
        state.handle_key(key(KeyCode::Char('f')), &app);
        state.handle_key(key(KeyCode::Char('a')), &app);
        // After the find resolves, hjkl should resume working normally.
        assert_eq!(
            state.handle_key(key(KeyCode::Char('l')), &app),
            Action::MoveCursor(Direction::Right)
        );
    }

    #[test]
    fn pending_find_swallows_non_char_keys() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            state.handle_key(key(KeyCode::Char('f')), &app),
            Action::Noop
        );
        // Esc-as-target cancels the find without dispatching anything.
        assert_eq!(state.handle_key(key(KeyCode::Esc), &app), Action::Noop);
        // And it doesn't leave the state stuck.
        assert_eq!(
            state.handle_key(key(KeyCode::Char('h')), &app),
            Action::MoveCursor(Direction::Left)
        );
    }

    #[test]
    fn semicolon_repeats_find() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            state.handle_key(key(KeyCode::Char(';')), &app),
            Action::RepeatFind { reversed: false }
        );
    }

    #[test]
    fn comma_repeats_find_reversed() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            state.handle_key(key(KeyCode::Char(',')), &app),
            Action::RepeatFind { reversed: true }
        );
    }
}
