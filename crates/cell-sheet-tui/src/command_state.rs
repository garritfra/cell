use crate::action::CommandKind;

#[derive(Debug, Clone)]
pub struct CommandState {
    pub line: String,
    pub kind: CommandKind,
    /// Previously executed colon commands, oldest first.
    pub history: Vec<String>,
    /// Index into `history` while the user is cycling with up/down.
    /// `None` means the user is not currently browsing history.
    pub history_idx: Option<usize>,
    /// The in-progress command line saved when the user first presses up,
    /// restored when they press down past the most recent entry.
    pub history_scratch: String,
}

impl Default for CommandState {
    fn default() -> Self {
        Self {
            line: String::new(),
            kind: CommandKind::Colon,
            history: Vec::new(),
            history_idx: None,
            history_scratch: String::new(),
        }
    }
}

impl CommandState {
    pub fn enter(&mut self, kind: CommandKind) {
        self.kind = kind;
        self.line.clear();
        self.reset_history_browse();
    }

    pub fn clear_line(&mut self) {
        self.line.clear();
    }

    pub fn insert_char(&mut self, c: char) {
        self.line.push(c);
    }

    pub fn backspace(&mut self) {
        self.line.pop();
    }

    pub fn reset_history_browse(&mut self) {
        self.history_idx = None;
        self.history_scratch.clear();
    }

    pub fn record_submitted_colon_command(&mut self, command: &str) {
        let command = command.trim();
        if !command.is_empty() && self.history.last().map(|s| s.as_str()) != Some(command) {
            self.history.push(command.to_string());
        }
        self.reset_history_browse();
    }

    pub fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let new_idx = match self.history_idx {
            None => {
                // Save current in-progress text before browsing.
                self.history_scratch = self.line.clone();
                self.history.len() - 1
            }
            Some(i) => i.saturating_sub(1),
        };
        self.history_idx = Some(new_idx);
        self.line = self.history[new_idx].clone();
    }

    pub fn history_next(&mut self) {
        let Some(i) = self.history_idx else {
            return;
        };
        if i + 1 < self.history.len() {
            let new_idx = i + 1;
            self.history_idx = Some(new_idx);
            self.line = self.history[new_idx].clone();
        } else {
            // Past the newest entry - restore scratch.
            self.history_idx = None;
            self.line = self.history_scratch.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_trimmed_colon_commands_without_consecutive_duplicates() {
        let mut state = CommandState::default();

        state.record_submitted_colon_command("  write.csv  ");
        state.record_submitted_colon_command("write.csv");
        state.record_submitted_colon_command("");

        assert_eq!(state.history, vec!["write.csv"]);
        assert_eq!(state.history_idx, None);
        assert!(state.history_scratch.is_empty());
    }

    #[test]
    fn history_prev_saves_scratch_and_walks_back() {
        let mut state = CommandState {
            history: vec!["first".into(), "second".into()],
            line: "scratch".into(),
            ..Default::default()
        };

        state.history_prev();
        assert_eq!(state.line, "second");
        assert_eq!(state.history_idx, Some(1));
        assert_eq!(state.history_scratch, "scratch");

        state.history_prev();
        assert_eq!(state.line, "first");
        assert_eq!(state.history_idx, Some(0));
    }

    #[test]
    fn history_next_restores_scratch_after_newest_entry() {
        let mut state = CommandState {
            history: vec!["first".into(), "second".into()],
            line: "scratch".into(),
            ..Default::default()
        };

        state.history_prev();
        state.history_next();

        assert_eq!(state.line, "scratch");
        assert_eq!(state.history_idx, None);
    }
}
