use crate::action::Action;
use crossterm::event::{KeyCode, KeyEvent};
use std::path::PathBuf;

pub fn parse_command(input: &str) -> Action {
    let input = input.trim();
    if input == "q" {
        Action::Quit { force: false }
    } else if input == "q!" {
        Action::Quit { force: true }
    } else if input == "w" || input == "wq" {
        Action::Save(None)
    } else if let Some(stripped) = input.strip_prefix("w ") {
        let path = stripped.trim();
        Action::Save(Some(PathBuf::from(path)))
    } else if let Some(stripped) = input.strip_prefix("w! ") {
        let path = stripped.trim();
        Action::ForceSave(Some(PathBuf::from(path)))
    } else if input == "w!" {
        Action::ForceSave(None)
    } else if let Some(stripped) = input.strip_prefix("e ") {
        let path = stripped.trim();
        Action::Open(PathBuf::from(path))
    } else if let Some(stripped) = input.strip_prefix("sort ") {
        parse_sort_command(stripped)
    } else if input == "help" {
        Action::ShowHelp(None)
    } else if let Some(stripped) = input.strip_prefix("help ") {
        let topic = stripped.trim();
        if topic.is_empty() {
            Action::ShowHelp(None)
        } else {
            Action::ShowHelp(Some(topic.to_string()))
        }
    } else {
        Action::Noop
    }
}

fn parse_sort_command(args: &str) -> Action {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.is_empty() {
        return Action::Noop;
    }
    let col = cell_core::model::col_label_to_index(parts[0]).unwrap_or(0);
    let ascending = parts.get(1).map(|&s| s != "desc").unwrap_or(true);
    Action::Sort { col, ascending }
}

pub fn handle_command_key(key: KeyEvent, command_buffer: &str) -> CommandAction {
    match key.code {
        KeyCode::Esc => CommandAction::Cancel,
        KeyCode::Enter => CommandAction::Execute(command_buffer.to_string()),
        KeyCode::Backspace => CommandAction::Backspace,
        KeyCode::Char(c) => CommandAction::InsertChar(c),
        _ => CommandAction::Noop,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommandAction {
    Noop,
    InsertChar(char),
    Backspace,
    Execute(String),
    Cancel,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    #[test]
    fn parse_quit() {
        assert_eq!(parse_command("q"), Action::Quit { force: false });
    }

    #[test]
    fn parse_force_quit() {
        assert_eq!(parse_command("q!"), Action::Quit { force: true });
    }

    #[test]
    fn parse_write() {
        assert_eq!(parse_command("w"), Action::Save(None));
    }

    #[test]
    fn parse_write_path() {
        assert_eq!(
            parse_command("w foo.csv"),
            Action::Save(Some(PathBuf::from("foo.csv")))
        );
    }

    #[test]
    fn parse_force_write() {
        assert_eq!(parse_command("w!"), Action::ForceSave(None));
    }

    #[test]
    fn parse_edit() {
        assert_eq!(
            parse_command("e data.csv"),
            Action::Open(PathBuf::from("data.csv"))
        );
    }

    #[test]
    fn parse_sort_asc() {
        assert_eq!(
            parse_command("sort A asc"),
            Action::Sort {
                col: 0,
                ascending: true
            }
        );
    }

    #[test]
    fn parse_sort_desc() {
        assert_eq!(
            parse_command("sort B desc"),
            Action::Sort {
                col: 1,
                ascending: false
            }
        );
    }

    #[test]
    fn parse_wq() {
        assert_eq!(parse_command("wq"), Action::Save(None));
    }

    #[test]
    fn parse_help_no_topic() {
        assert_eq!(parse_command("help"), Action::ShowHelp(None));
    }

    #[test]
    fn parse_help_with_topic() {
        assert_eq!(
            parse_command("help dd"),
            Action::ShowHelp(Some("dd".into()))
        );
    }

    #[test]
    fn parse_help_with_command_topic() {
        assert_eq!(
            parse_command("help :w"),
            Action::ShowHelp(Some(":w".into()))
        );
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn command_esc_cancels() {
        assert_eq!(
            handle_command_key(key(KeyCode::Esc), ""),
            CommandAction::Cancel
        );
    }

    #[test]
    fn command_enter_executes() {
        assert_eq!(
            handle_command_key(key(KeyCode::Enter), "w"),
            CommandAction::Execute("w".into())
        );
    }

    #[test]
    fn command_char_inserts() {
        assert_eq!(
            handle_command_key(key(KeyCode::Char('q')), ""),
            CommandAction::InsertChar('q')
        );
    }
}
