use crate::action::{Action, CommandKind, SearchDirection};
use crossterm::event::{KeyCode, KeyEvent};
use std::path::PathBuf;

/// Turn a submitted command line into an `Action`. The prompt prefix
/// (`:`, `/`, `?`) decides how `input` is parsed: ex-style command vs
/// search pattern.
pub fn submit(kind: CommandKind, input: &str) -> Action {
    match kind {
        CommandKind::Colon => parse_command(input),
        CommandKind::Slash => parse_search(input, SearchDirection::Forward),
        CommandKind::Question => parse_search(input, SearchDirection::Backward),
    }
}

fn parse_search(input: &str, direction: SearchDirection) -> Action {
    let pattern = input.trim();
    if pattern.is_empty() {
        Action::Noop
    } else {
        Action::Search {
            pattern: pattern.to_string(),
            direction,
        }
    }
}

pub fn parse_command(input: &str) -> Action {
    let input = input.trim_start();

    // Handle before trimming the end: the delimiter character may itself be
    // whitespace (e.g. Tab), so trim_end() must not run before this check.
    if let Some(stripped) = input.strip_prefix("set delimiter=") {
        let mut chars = stripped.chars();
        return match (chars.next(), chars.next()) {
            // Tab (0x09) is the standard TSV delimiter and is explicitly
            // allowed even though it is a control character (< 0x20).
            (Some(c), None)
                if c.is_ascii()
                    && !c.is_alphanumeric()
                    && c != '"'
                    && c != ' '
                    && ((c as u8) >= 0x20 || c == '\t') =>
            {
                Action::SetDelimiter(c as u8)
            }
            _ => Action::SetStatus(
                "Invalid delimiter: must be a single non-alphanumeric ASCII character (not \", \\n, \\r).".into()
            ),
        };
    }

    // For all other commands, trailing whitespace is ignored (Vim behaviour).
    let input = input.trim_end();

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
    } else if let Some(rest) = input.strip_prefix("set mouse") {
        let arg = rest.trim();
        match arg {
            "on" => Action::SetMouse(true),
            "off" => Action::SetMouse(false),
            // The toggle case is rewritten in run_loop, where we have
            // access to the live mouse_enabled flag. Returning
            // SetMouse(true) here is a placeholder; run_loop replaces it.
            "toggle" => Action::SetMouse(true),
            _ => Action::SetStatus("usage: :set mouse on|off|toggle".into()),
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
    let col = cell_sheet_core::model::col_label_to_index(parts[0]).unwrap_or(0);
    let ascending = parts.get(1).map(|&s| s != "desc").unwrap_or(true);
    Action::Sort { col, ascending }
}

pub fn handle_command_key(key: KeyEvent, command_buffer: &str) -> CommandAction {
    match key.code {
        KeyCode::Esc => CommandAction::Cancel,
        KeyCode::Enter => CommandAction::Execute(command_buffer.to_string()),
        KeyCode::Backspace => CommandAction::Backspace,
        KeyCode::Up => CommandAction::HistoryPrev,
        KeyCode::Down => CommandAction::HistoryNext,
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
    HistoryPrev,
    HistoryNext,
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

    #[test]
    fn parse_set_delimiter_pipe() {
        assert_eq!(parse_command("set delimiter=|"), Action::SetDelimiter(b'|'));
    }

    #[test]
    fn parse_set_delimiter_semicolon() {
        assert_eq!(parse_command("set delimiter=;"), Action::SetDelimiter(b';'));
    }

    #[test]
    fn parse_set_delimiter_tab() {
        assert_eq!(
            parse_command("set delimiter=\t"),
            Action::SetDelimiter(b'\t')
        );
    }

    #[test]
    fn parse_set_delimiter_empty_is_error() {
        assert!(matches!(
            parse_command("set delimiter="),
            Action::SetStatus(_)
        ));
    }

    #[test]
    fn parse_set_delimiter_alphanumeric_is_error() {
        assert!(matches!(
            parse_command("set delimiter=a"),
            Action::SetStatus(_)
        ));
        assert!(matches!(
            parse_command("set delimiter=1"),
            Action::SetStatus(_)
        ));
    }

    #[test]
    fn parse_set_delimiter_multi_char_is_error() {
        assert!(matches!(
            parse_command("set delimiter=||"),
            Action::SetStatus(_)
        ));
    }

    #[test]
    fn parse_set_delimiter_non_ascii_is_error() {
        assert!(matches!(
            parse_command("set delimiter=€"),
            Action::SetStatus(_)
        ));
    }

    #[test]
    fn parse_quit_with_trailing_space() {
        // Trailing whitespace must be tolerated (Vim behaviour)
        assert_eq!(parse_command("q "), Action::Quit { force: false });
        assert_eq!(parse_command("wq "), Action::Save(None));
    }

    #[test]
    fn parse_set_delimiter_quote_is_error() {
        assert!(matches!(
            parse_command("set delimiter=\""),
            Action::SetStatus(_)
        ));
    }

    #[test]
    fn parse_set_delimiter_space_is_error() {
        assert!(matches!(
            parse_command("set delimiter= "),
            Action::SetStatus(_)
        ));
    }

    #[test]
    fn parse_set_delimiter_nul_is_error() {
        assert!(matches!(
            parse_command("set delimiter=\0"),
            Action::SetStatus(_)
        ));
    }

    #[test]
    fn parse_set_mouse_on() {
        assert_eq!(parse_command("set mouse on"), Action::SetMouse(true));
    }

    #[test]
    fn parse_set_mouse_off() {
        assert_eq!(parse_command("set mouse off"), Action::SetMouse(false));
    }

    #[test]
    fn parse_set_mouse_bogus_returns_status_error() {
        assert!(matches!(
            parse_command("set mouse bogus"),
            Action::SetStatus(_)
        ));
    }

    #[test]
    fn parse_set_mouse_toggle() {
        // Parser produces SetMouse(true); run_loop rewrites toggle vs current state.
        assert_eq!(parse_command("set mouse toggle"), Action::SetMouse(true));
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

    #[test]
    fn submit_colon_routes_to_parse_command() {
        assert_eq!(
            submit(CommandKind::Colon, "q"),
            Action::Quit { force: false }
        );
    }

    #[test]
    fn submit_slash_dispatches_forward_search() {
        assert_eq!(
            submit(CommandKind::Slash, "foo"),
            Action::Search {
                pattern: "foo".into(),
                direction: SearchDirection::Forward,
            }
        );
    }

    #[test]
    fn submit_question_dispatches_backward_search() {
        assert_eq!(
            submit(CommandKind::Question, "bar"),
            Action::Search {
                pattern: "bar".into(),
                direction: SearchDirection::Backward,
            }
        );
    }

    #[test]
    fn submit_empty_search_is_noop() {
        assert_eq!(submit(CommandKind::Slash, ""), Action::Noop);
        assert_eq!(submit(CommandKind::Question, "   "), Action::Noop);
    }

    #[test]
    fn submit_search_trims_whitespace() {
        assert_eq!(
            submit(CommandKind::Slash, "  hello  "),
            Action::Search {
                pattern: "hello".into(),
                direction: SearchDirection::Forward,
            }
        );
    }
}
