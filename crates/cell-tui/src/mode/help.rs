use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::action::{Action, Mode};
use crate::app::App;

pub struct HelpState {
    pending_g: bool,
}

impl HelpState {
    pub fn new() -> Self {
        HelpState { pending_g: false }
    }

    pub fn handle_key(&mut self, key: KeyEvent, app: &mut App, page_height: usize) -> Action {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return match key.code {
                KeyCode::Char('d') => {
                    app.help_scroll += page_height / 2;
                    Action::Noop
                }
                KeyCode::Char('u') => {
                    app.help_scroll = app.help_scroll.saturating_sub(page_height / 2);
                    Action::Noop
                }
                _ => Action::Noop,
            };
        }

        if self.pending_g {
            self.pending_g = false;
            if key.code == KeyCode::Char('g') {
                app.help_scroll = 0;
            }
            return Action::Noop;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => Action::ChangeMode(Mode::Normal),
            KeyCode::Char('j') | KeyCode::Down => {
                app.help_scroll += 1;
                Action::Noop
            }
            KeyCode::Char('k') | KeyCode::Up => {
                app.help_scroll = app.help_scroll.saturating_sub(1);
                Action::Noop
            }
            KeyCode::Char('g') => {
                self.pending_g = true;
                Action::Noop
            }
            KeyCode::Char('G') => {
                app.help_scroll = usize::MAX; // clamped by renderer
                Action::Noop
            }
            KeyCode::Char(':') => Action::ChangeMode(Mode::Command),
            _ => Action::Noop,
        }
    }
}
