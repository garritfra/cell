use crate::action::Action;
use crate::app::App;
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle_insert_key(key: KeyEvent, app: &App) -> Action {
    match key.code {
        KeyCode::Esc => {
            let new_raw = app.insert_buffer.clone();
            Action::EditCell(app.cursor, new_raw)
        }
        KeyCode::Enter => {
            let new_raw = app.insert_buffer.clone();
            Action::EditCell(app.cursor, new_raw)
        }
        _ => Action::Noop,
    }
}

pub fn handle_insert_char(key: KeyEvent) -> Option<InsertAction> {
    match key.code {
        KeyCode::Char(c) => Some(InsertAction::InsertChar(c)),
        KeyCode::Backspace => Some(InsertAction::Backspace),
        KeyCode::Delete => Some(InsertAction::Delete),
        KeyCode::Left => Some(InsertAction::CursorLeft),
        KeyCode::Right => Some(InsertAction::CursorRight),
        KeyCode::Home => Some(InsertAction::CursorHome),
        KeyCode::End => Some(InsertAction::CursorEnd),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum InsertAction {
    InsertChar(char),
    Backspace,
    Delete,
    CursorLeft,
    CursorRight,
    CursorHome,
    CursorEnd,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn esc_confirms_edit() {
        let mut app = App::new();
        app.insert_buffer = "hello".into();
        let action = handle_insert_key(key(KeyCode::Esc), &app);
        assert_eq!(action, Action::EditCell((0, 0), "hello".into()));
    }

    #[test]
    fn enter_confirms_edit() {
        let mut app = App::new();
        app.insert_buffer = "42".into();
        let action = handle_insert_key(key(KeyCode::Enter), &app);
        assert_eq!(action, Action::EditCell((0, 0), "42".into()));
    }

    #[test]
    fn char_input() {
        assert_eq!(
            handle_insert_char(key(KeyCode::Char('a'))),
            Some(InsertAction::InsertChar('a'))
        );
    }

    #[test]
    fn backspace() {
        assert_eq!(
            handle_insert_char(key(KeyCode::Backspace)),
            Some(InsertAction::Backspace)
        );
    }
}
