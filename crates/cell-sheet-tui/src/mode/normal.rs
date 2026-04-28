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

/// Cap the count buffer at a sane upper bound. Vim accepts huge counts
/// (and most ops degrade to "until end of buffer"), but we don't want a
/// stray `9999999999G` to wedge the loop, allocate gigantic vecs in
/// `dd` / `yy`, or overflow `usize` on 32-bit targets.
const COUNT_CAP: usize = 1_000_000;

pub struct NormalState {
    pub pending: Option<char>,
    /// Set after `f` or `F` so the next keypress is consumed as the target.
    pub pending_find: Option<FindKind>,
    /// Vim-style numeric count prefix accumulated from digit keys. `None`
    /// means "no count typed" — most commands then use a default of 1.
    /// `0` only starts a count *after* the first non-zero digit; before
    /// that, `0` is the `goto-first-column` motion.
    pub pending_count: Option<usize>,
    /// Count typed *after* an operator key in operator-pending mode (e.g.
    /// the `3` in `d3j`). When both `pending_count` (outer) and this inner
    /// count are present, they are multiplied: `5d2j` → effective count 10.
    pending_motion_count: Option<usize>,
}

impl NormalState {
    pub fn new() -> Self {
        NormalState {
            pending: None,
            pending_find: None,
            pending_count: None,
            pending_motion_count: None,
        }
    }

    /// Snapshot of the current pending count (for `showcmd`-style status
    /// rendering). Does not consume.
    pub fn pending_count(&self) -> Option<usize> {
        self.pending_count
    }

    /// Snapshot of the pending operator key (for `showcmd`-style status
    /// rendering). Does not consume.
    pub fn pending_op(&self) -> Option<char> {
        self.pending
    }

    /// Take the pending count, defaulting to 1, and clear the buffer.
    /// Use this when an action that respects counts is about to fire.
    fn take_count(&mut self) -> usize {
        self.pending_count.take().unwrap_or(1).max(1)
    }

    /// Discard any pending count without using it. Use when a non-counted
    /// command fires (e.g. `i`, `:`) so the next keypress starts fresh.
    fn discard_count(&mut self) {
        self.pending_count = None;
        self.pending_motion_count = None;
    }

    /// Append a digit to the outer (pre-operator) count buffer with overflow
    /// protection.
    fn push_digit(&mut self, d: u32) {
        let next = self
            .pending_count
            .unwrap_or(0)
            .saturating_mul(10)
            .saturating_add(d as usize)
            .min(COUNT_CAP);
        self.pending_count = Some(next);
    }

    /// Append a digit to the inner (post-operator) motion count buffer.
    fn push_motion_digit(&mut self, d: u32) {
        let next = self
            .pending_motion_count
            .unwrap_or(0)
            .saturating_mul(10)
            .saturating_add(d as usize)
            .min(COUNT_CAP);
        self.pending_motion_count = Some(next);
    }

    /// Consume both count buffers and return outer × inner (each defaulting to
    /// 1). Used by operator-pending-then-motion arms to implement vim's count
    /// multiplication rule (`5d2j` → 10).
    fn take_motion_count(&mut self) -> usize {
        let outer = self.pending_count.take().unwrap_or(1).max(1);
        let inner = self.pending_motion_count.take().unwrap_or(1).max(1);
        outer.saturating_mul(inner)
    }

    pub fn handle_key(&mut self, key: KeyEvent, app: &App) -> Action {
        // A pending `f`/`F` consumes the next keypress as its target,
        // regardless of modifier (so `f<Shift+a>` still hits 'A'). The
        // count prefix doesn't apply to f/F (out of scope for now).
        if let Some(kind) = self.pending_find.take() {
            self.discard_count();
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

        // Esc clears any partially-typed command (count + pending op).
        // Match vim: a half-typed `5d` followed by Esc cancels.
        if key.code == KeyCode::Esc {
            self.pending = None;
            self.pending_count = None;
            self.pending_motion_count = None;
            return Action::Noop;
        }

        // Handle Ctrl combinations first. Counts don't apply to most of
        // these (Vim itself supports `[count]Ctrl-d` etc., but that's not
        // in scope here — discard so the next keypress starts fresh).
        // Exception: Ctrl+a / Ctrl+x consume the count as the step size.
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('a') => {
                    let delta = self.take_count() as i64;
                    return Action::AdjustNumber {
                        pos: app.cursor,
                        delta,
                    };
                }
                KeyCode::Char('x') => {
                    let delta = self.take_count() as i64;
                    return Action::AdjustNumber {
                        pos: app.cursor,
                        delta: -delta,
                    };
                }
                _ => {
                    self.discard_count();
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
            }
        }

        // Digit handling: build up a count prefix.
        // - `1`-`9` always start/extend a count.
        // - `0` is `goto-first-column` ONLY when no count is pending and no
        //   operator is pending; otherwise it extends the count.
        // - When an operator is already pending (e.g. after `d`), digits
        //   feed the *motion* count so that `d3j` works as expected.
        if let KeyCode::Char(c) = key.code {
            if let Some(d) = c.to_digit(10) {
                if self.pending.is_some() {
                    // Operator-pending: digit belongs to the motion count.
                    self.push_motion_digit(d);
                    return Action::Noop;
                }
                if d == 0 && self.pending_count.is_none() {
                    return Action::GotoFirstCol;
                }
                self.push_digit(d);
                return Action::Noop;
            }
        }

        // Handle pending operator/prefix sequences. The outer count was
        // buffered before the operator key was pressed (e.g. `5dd`, `10gg`);
        // the motion count was buffered after it (e.g. `d3j`, `y2k`).
        // Marks/`'`/`` ` ``/`z` ignore counts.
        if let Some(prev) = self.pending.take() {
            return match (prev, key.code) {
                ('g', KeyCode::Char('g')) => {
                    let n = self.pending_count.take();
                    self.pending_motion_count = None;
                    match n {
                        Some(n) => Action::GotoRow(n),
                        None => Action::GotoFirstRow,
                    }
                }
                ('g', KeyCode::Char('v')) => {
                    self.discard_count();
                    Action::ReselectLastVisual
                }
                ('d', KeyCode::Char('d')) => {
                    let count = self.pending_count.take().unwrap_or(1).max(1);
                    self.pending_motion_count = None;
                    Action::DeleteRow {
                        start: app.cursor.0,
                        count,
                    }
                }
                ('y', KeyCode::Char('y')) => {
                    let count = self.pending_count.take().unwrap_or(1).max(1);
                    self.pending_motion_count = None;
                    Action::YankRow {
                        start: app.cursor.0,
                        count,
                    }
                }
                // Operator + directional motion: clear rows/cells in range.
                ('d', KeyCode::Char('j')) => {
                    let n = self.take_motion_count();
                    Action::ClearRange {
                        start: (app.cursor.0, 0),
                        end: (app.cursor.0.saturating_add(n), usize::MAX),
                    }
                }
                ('d', KeyCode::Char('k')) => {
                    let n = self.take_motion_count();
                    Action::ClearRange {
                        start: (app.cursor.0.saturating_sub(n), 0),
                        end: (app.cursor.0, usize::MAX),
                    }
                }
                ('d', KeyCode::Char('l')) => {
                    let n = self.take_motion_count();
                    Action::ClearRange {
                        start: app.cursor,
                        end: (app.cursor.0, app.cursor.1.saturating_add(n)),
                    }
                }
                ('d', KeyCode::Char('h')) => {
                    let n = self.take_motion_count();
                    Action::ClearRange {
                        start: (app.cursor.0, app.cursor.1.saturating_sub(n)),
                        end: app.cursor,
                    }
                }
                // Operator + directional motion: yank rows/cells in range.
                ('y', KeyCode::Char('j')) => {
                    let n = self.take_motion_count();
                    Action::YankRange {
                        start: (app.cursor.0, 0),
                        end: (app.cursor.0.saturating_add(n), usize::MAX),
                    }
                }
                ('y', KeyCode::Char('k')) => {
                    let n = self.take_motion_count();
                    Action::YankRange {
                        start: (app.cursor.0.saturating_sub(n), 0),
                        end: (app.cursor.0, usize::MAX),
                    }
                }
                ('y', KeyCode::Char('l')) => {
                    let n = self.take_motion_count();
                    Action::YankRange {
                        start: app.cursor,
                        end: (app.cursor.0, app.cursor.1.saturating_add(n)),
                    }
                }
                ('y', KeyCode::Char('h')) => {
                    let n = self.take_motion_count();
                    Action::YankRange {
                        start: (app.cursor.0, app.cursor.1.saturating_sub(n)),
                        end: app.cursor,
                    }
                }
                ('z', KeyCode::Char('z')) => {
                    self.discard_count();
                    Action::ScrollCursorCenter
                }
                ('z', KeyCode::Char('t')) => {
                    self.discard_count();
                    Action::ScrollCursorTop
                }
                ('z', KeyCode::Char('b')) => {
                    self.discard_count();
                    Action::ScrollCursorBottom
                }
                ('m', KeyCode::Char(c)) if c.is_ascii_lowercase() => {
                    self.discard_count();
                    Action::SetMark(c)
                }
                ('\'', KeyCode::Char(c)) if c.is_ascii_lowercase() => {
                    self.discard_count();
                    Action::JumpToMark {
                        name: c,
                        line_wise: true,
                    }
                }
                ('`', KeyCode::Char(c)) if c.is_ascii_lowercase() => {
                    self.discard_count();
                    Action::JumpToMark {
                        name: c,
                        line_wise: false,
                    }
                }
                _ => {
                    // Unknown sequence: reset everything (vim's behavior
                    // is to bell-and-cancel).
                    self.discard_count();
                    Action::Noop
                }
            };
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
            KeyCode::Char('H') => {
                self.discard_count();
                Action::CursorToViewportTop
            }
            KeyCode::Char('M') => {
                self.discard_count();
                Action::CursorToViewportMiddle
            }
            KeyCode::Char('L') => {
                self.discard_count();
                Action::CursorToViewportBottom
            }
            KeyCode::Char('G') => match self.pending_count.take() {
                Some(n) => Action::GotoRow(n),
                None => Action::GotoLastRow,
            },
            KeyCode::Char('$') => {
                self.discard_count();
                Action::GotoLastCol
            }
            KeyCode::Char('w') => Action::NextNonEmpty(self.take_count()),
            KeyCode::Char('b') => Action::PrevNonEmpty(self.take_count()),
            KeyCode::Char('{') => {
                self.discard_count();
                Action::BlockJumpUp
            }
            KeyCode::Char('}') => {
                self.discard_count();
                Action::BlockJumpDown
            }
            KeyCode::Char('i') | KeyCode::Char('a') => {
                self.discard_count();
                Action::ChangeMode(Mode::Insert)
            }
            KeyCode::Char('o') => {
                self.discard_count();
                Action::ChangeMode(Mode::Insert)
            }
            KeyCode::Char('v') => {
                self.discard_count();
                Action::ChangeMode(Mode::Visual)
            }
            KeyCode::Char('V') => {
                self.discard_count();
                Action::ChangeMode(Mode::VisualLine)
            }
            KeyCode::Char(':') => {
                self.discard_count();
                Action::ChangeMode(Mode::Command)
            }
            KeyCode::Char('c') => {
                self.discard_count();
                Action::ChangeCell(app.cursor)
            }
            KeyCode::Char('x') => {
                self.discard_count();
                Action::ClearCell(app.cursor)
            }
            KeyCode::Char('p') => {
                self.discard_count();
                Action::Paste(app.cursor)
            }
            KeyCode::Char('P') => {
                self.discard_count();
                Action::PasteBefore(app.cursor)
            }
            KeyCode::Char('u') => {
                self.discard_count();
                Action::Undo
            }
            KeyCode::Char('/') => {
                self.discard_count();
                Action::EnterSearch(SearchDirection::Forward)
            }
            KeyCode::Char('?') => {
                self.discard_count();
                Action::EnterSearch(SearchDirection::Backward)
            }
            KeyCode::Char('n') => {
                self.discard_count();
                Action::SearchNext
            }
            KeyCode::Char('N') => {
                self.discard_count();
                Action::SearchPrev
            }
            KeyCode::Char('f') => {
                self.pending_find = Some(FindKind::Forward);
                Action::Noop
            }
            KeyCode::Char('F') => {
                self.pending_find = Some(FindKind::Backward);
                Action::Noop
            }
            KeyCode::Char(';') => {
                self.discard_count();
                Action::RepeatFind { reversed: false }
            }
            KeyCode::Char(',') => {
                self.discard_count();
                Action::RepeatFind { reversed: true }
            }
            KeyCode::Char('*') => {
                self.discard_count();
                Action::SearchCellValue { backward: false }
            }
            KeyCode::Char('#') => {
                self.discard_count();
                Action::SearchCellValue { backward: true }
            }
            KeyCode::Tab => {
                self.discard_count();
                Action::JumpForward
            }
            KeyCode::Enter => {
                self.discard_count();
                Action::ChangeMode(Mode::Insert)
            }
            KeyCode::Char('.') => {
                self.discard_count();
                Action::RepeatLastChange
            }
            _ => {
                // Unknown key: throw away any pending count so we don't
                // silently apply it to the next valid command.
                self.discard_count();
                Action::Noop
            }
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
            Action::MoveCursor(Direction::Left, 1)
        );
        assert_eq!(
            state.handle_key(key(KeyCode::Char('j')), &app),
            Action::MoveCursor(Direction::Down, 1)
        );
        assert_eq!(
            state.handle_key(key(KeyCode::Char('k')), &app),
            Action::MoveCursor(Direction::Up, 1)
        );
        assert_eq!(
            state.handle_key(key(KeyCode::Char('l')), &app),
            Action::MoveCursor(Direction::Right, 1)
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
            Action::DeleteRow { start: 0, count: 1 }
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
            Action::YankRow { start: 0, count: 1 }
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
            Action::MoveCursor(Direction::Right, 1)
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
            Action::MoveCursor(Direction::Left, 1)
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
        assert_eq!(
            state.handle_key(ctrl_key('e'), &app),
            Action::ScrollLineDown
        );
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
    fn gv_reselects_last_visual() {
        let app = App::new();
        let mut state = NormalState::new();
        state.handle_key(key(KeyCode::Char('g')), &app);
        assert_eq!(
            state.handle_key(key(KeyCode::Char('v')), &app),
            Action::ReselectLastVisual
        );
    }

    #[test]
    fn star_searches_cell_value_forward() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            state.handle_key(key(KeyCode::Char('*')), &app),
            Action::SearchCellValue { backward: false }
        );
    }

    #[test]
    fn hash_searches_cell_value_backward() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            state.handle_key(key(KeyCode::Char('#')), &app),
            Action::SearchCellValue { backward: true }
        );
    }

    #[test]
    fn brace_open_jumps_block_up() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            state.handle_key(key(KeyCode::Char('{')), &app),
            Action::BlockJumpUp
        );
    }

    #[test]
    fn brace_close_jumps_block_down() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            state.handle_key(key(KeyCode::Char('}')), &app),
            Action::BlockJumpDown
        );
    }

    #[test]
    fn ctrl_o_jumps_back() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(state.handle_key(ctrl_key('o'), &app), Action::JumpBack);
    }

    #[test]
    fn ctrl_a_emits_adjust_number_increment() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            state.handle_key(ctrl_key('a'), &app),
            Action::AdjustNumber {
                pos: (0, 0),
                delta: 1
            }
        );
    }

    #[test]
    fn ctrl_x_emits_adjust_number_decrement() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            state.handle_key(ctrl_key('x'), &app),
            Action::AdjustNumber {
                pos: (0, 0),
                delta: -1
            }
        );
    }

    #[test]
    fn count_ctrl_a_uses_count_as_delta() {
        let app = App::new();
        let mut state = NormalState::new();
        let _ = state.handle_key(key(KeyCode::Char('5')), &app);
        assert_eq!(
            state.handle_key(ctrl_key('a'), &app),
            Action::AdjustNumber {
                pos: (0, 0),
                delta: 5
            }
        );
        assert!(state.pending_count.is_none(), "count must be consumed");
    }

    #[test]
    fn count_ctrl_x_uses_count_as_negative_delta() {
        let app = App::new();
        let mut state = NormalState::new();
        let _ = state.handle_key(key(KeyCode::Char('3')), &app);
        assert_eq!(
            state.handle_key(ctrl_key('x'), &app),
            Action::AdjustNumber {
                pos: (0, 0),
                delta: -3
            }
        );
        assert!(state.pending_count.is_none(), "count must be consumed");
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
    fn dot_emits_repeat_last_change() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            state.handle_key(key(KeyCode::Char('.')), &app),
            Action::RepeatLastChange
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

    // --- count prefix tests ----------------------------------------------

    fn feed(state: &mut NormalState, app: &App, keys: &str) -> Action {
        let mut last = Action::Noop;
        for c in keys.chars() {
            last = state.handle_key(key(KeyCode::Char(c)), app);
        }
        last
    }

    #[test]
    fn single_digit_count_repeats_motion() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            feed(&mut state, &app, "5j"),
            Action::MoveCursor(Direction::Down, 5)
        );
        assert!(state.pending_count.is_none(), "count must be consumed");
    }

    #[test]
    fn multi_digit_count_repeats_motion() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            feed(&mut state, &app, "42k"),
            Action::MoveCursor(Direction::Up, 42)
        );
    }

    #[test]
    fn zero_after_digit_extends_count_not_first_col() {
        let app = App::new();
        let mut state = NormalState::new();
        // `10j` must move down 10, not move to first column then down 1.
        assert_eq!(
            state.handle_key(key(KeyCode::Char('1')), &app),
            Action::Noop
        );
        assert_eq!(
            state.handle_key(key(KeyCode::Char('0')), &app),
            Action::Noop
        );
        assert_eq!(
            state.handle_key(key(KeyCode::Char('j')), &app),
            Action::MoveCursor(Direction::Down, 10)
        );
    }

    #[test]
    fn zero_alone_is_first_col() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            state.handle_key(key(KeyCode::Char('0')), &app),
            Action::GotoFirstCol
        );
        assert!(state.pending_count.is_none());
    }

    #[test]
    fn count_then_g_jumps_to_specific_row() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(feed(&mut state, &app, "5G"), Action::GotoRow(5));
    }

    #[test]
    fn bare_g_still_goes_to_last_row() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            state.handle_key(key(KeyCode::Char('G')), &app),
            Action::GotoLastRow
        );
    }

    #[test]
    fn count_then_gg_jumps_to_specific_row() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(feed(&mut state, &app, "7gg"), Action::GotoRow(7));
    }

    #[test]
    fn bare_gg_still_goes_to_first_row() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(feed(&mut state, &app, "gg"), Action::GotoFirstRow);
    }

    #[test]
    fn count_dd_deletes_n_rows() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            feed(&mut state, &app, "3dd"),
            Action::DeleteRow { start: 0, count: 3 }
        );
    }

    #[test]
    fn count_yy_yanks_n_rows() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            feed(&mut state, &app, "4yy"),
            Action::YankRow { start: 0, count: 4 }
        );
    }

    #[test]
    fn count_w_and_b() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(feed(&mut state, &app, "3w"), Action::NextNonEmpty(3));
        assert_eq!(feed(&mut state, &app, "2b"), Action::PrevNonEmpty(2));
    }

    #[test]
    fn count_resets_after_action() {
        let app = App::new();
        let mut state = NormalState::new();
        let _ = feed(&mut state, &app, "5j");
        // Next key without a count should be 1.
        assert_eq!(
            state.handle_key(key(KeyCode::Char('j')), &app),
            Action::MoveCursor(Direction::Down, 1)
        );
    }

    #[test]
    fn count_discarded_by_non_counted_command() {
        let app = App::new();
        let mut state = NormalState::new();
        // `5i` enters insert mode and silently drops the count.
        let action = feed(&mut state, &app, "5i");
        assert_eq!(action, Action::ChangeMode(Mode::Insert));
        assert!(state.pending_count.is_none());
    }

    #[test]
    fn esc_clears_pending_count_and_op() {
        let app = App::new();
        let mut state = NormalState::new();
        let _ = state.handle_key(key(KeyCode::Char('5')), &app);
        let _ = state.handle_key(key(KeyCode::Char('d')), &app);
        assert_eq!(state.pending_count, Some(5));
        assert_eq!(state.pending, Some('d'));
        let _ = state.handle_key(key(KeyCode::Esc), &app);
        assert!(state.pending_count.is_none());
        assert!(state.pending.is_none());
        assert!(state.pending_motion_count.is_none());
    }

    #[test]
    fn count_is_capped() {
        let app = App::new();
        let mut state = NormalState::new();
        // 10 nines = 9_999_999_999 — must clamp to COUNT_CAP.
        let huge = "9999999999j";
        let action = feed(&mut state, &app, huge);
        assert_eq!(action, Action::MoveCursor(Direction::Down, COUNT_CAP));
    }

    #[test]
    fn unknown_key_resets_count() {
        let app = App::new();
        let mut state = NormalState::new();
        let _ = state.handle_key(key(KeyCode::Char('5')), &app);
        // Some key that isn't bound: '~' is not handled in normal mode.
        let _ = state.handle_key(key(KeyCode::Char('~')), &app);
        assert!(state.pending_count.is_none());
        // Next motion should NOT carry the orphaned 5.
        assert_eq!(
            state.handle_key(key(KeyCode::Char('j')), &app),
            Action::MoveCursor(Direction::Down, 1)
        );
    }

    #[test]
    fn ctrl_combo_clears_count() {
        let app = App::new();
        let mut state = NormalState::new();
        let _ = state.handle_key(key(KeyCode::Char('5')), &app);
        // Ctrl-d shouldn't smuggle the count along.
        let _ = state.handle_key(ctrl_key('d'), &app);
        assert!(state.pending_count.is_none());
    }

    #[test]
    fn count_carries_through_pending_operator() {
        let app = App::new();
        let mut state = NormalState::new();
        // `5` `d` `d` — count must persist past the pending `d`.
        let _ = state.handle_key(key(KeyCode::Char('5')), &app);
        assert_eq!(state.pending_count, Some(5));
        let _ = state.handle_key(key(KeyCode::Char('d')), &app);
        assert_eq!(state.pending_count, Some(5), "count survives pending op");
        assert_eq!(
            state.handle_key(key(KeyCode::Char('d')), &app),
            Action::DeleteRow { start: 0, count: 5 }
        );
    }

    #[test]
    fn pending_count_accessor() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(state.pending_count(), None);
        let _ = state.handle_key(key(KeyCode::Char('1')), &app);
        let _ = state.handle_key(key(KeyCode::Char('2')), &app);
        assert_eq!(state.pending_count(), Some(12));
    }

    #[test]
    fn pending_op_accessor() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(state.pending_op(), None);
        let _ = state.handle_key(key(KeyCode::Char('d')), &app);
        assert_eq!(state.pending_op(), Some('d'));
    }

    // --- operator-pending + motion tests ---------------------------------

    #[test]
    fn d_j_clears_rows_downward() {
        // `d3j` from row 0 clears rows 0..=3 (inclusive).
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            feed(&mut state, &app, "d3j"),
            Action::ClearRange {
                start: (0, 0),
                end: (3, usize::MAX),
            }
        );
    }

    #[test]
    fn d_k_clears_rows_upward() {
        // `d2k` from row 5 clears rows 3..=5.
        let mut app = App::new();
        app.cursor = (5, 0);
        let mut state = NormalState::new();
        assert_eq!(
            feed(&mut state, &app, "d2k"),
            Action::ClearRange {
                start: (3, 0),
                end: (5, usize::MAX),
            }
        );
    }

    #[test]
    fn d_l_clears_cells_rightward() {
        // `d3l` from (0, 0) clears cells (0,0) through (0,3).
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            feed(&mut state, &app, "d3l"),
            Action::ClearRange {
                start: (0, 0),
                end: (0, 3),
            }
        );
    }

    #[test]
    fn y_l_yanks_cells_rightward() {
        // `y3l` from (0, 0) yanks cells (0,0) through (0,3).
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            feed(&mut state, &app, "y3l"),
            Action::YankRange {
                start: (0, 0),
                end: (0, 3),
            }
        );
    }

    #[test]
    fn y_j_yanks_rows_downward() {
        // `y2j` from row 1 yanks rows 1..=3.
        let mut app = App::new();
        app.cursor = (1, 2);
        let mut state = NormalState::new();
        assert_eq!(
            feed(&mut state, &app, "y2j"),
            Action::YankRange {
                start: (1, 0),
                end: (3, usize::MAX),
            }
        );
    }

    #[test]
    fn outer_inner_count_multiplication() {
        // `5d2j` — outer=5, inner=2, effective=10 → clears rows 0..=10.
        let app = App::new();
        let mut state = NormalState::new();
        let _ = state.handle_key(key(KeyCode::Char('5')), &app);
        let _ = state.handle_key(key(KeyCode::Char('d')), &app);
        let _ = state.handle_key(key(KeyCode::Char('2')), &app);
        assert_eq!(state.pending_motion_count, Some(2));
        let action = state.handle_key(key(KeyCode::Char('j')), &app);
        assert_eq!(
            action,
            Action::ClearRange {
                start: (0, 0),
                end: (10, usize::MAX),
            }
        );
        assert!(state.pending_count.is_none());
        assert!(state.pending_motion_count.is_none());
    }

    #[test]
    fn operator_no_motion_count_defaults_to_one() {
        // `dj` with no count clears 1 row below (rows 0..=1).
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(
            feed(&mut state, &app, "dj"),
            Action::ClearRange {
                start: (0, 0),
                end: (1, usize::MAX),
            }
        );
    }

    #[test]
    fn operator_then_esc_cancels_cleanly() {
        // `d5` then Esc: all state cleared, next `j` behaves normally.
        let app = App::new();
        let mut state = NormalState::new();
        let _ = state.handle_key(key(KeyCode::Char('d')), &app);
        let _ = state.handle_key(key(KeyCode::Char('5')), &app);
        assert_eq!(state.pending, Some('d'));
        assert_eq!(state.pending_motion_count, Some(5));
        let _ = state.handle_key(key(KeyCode::Esc), &app);
        assert!(state.pending.is_none());
        assert!(state.pending_count.is_none());
        assert!(state.pending_motion_count.is_none());
        // Next keypress should not carry any orphaned counts.
        assert_eq!(
            state.handle_key(key(KeyCode::Char('j')), &app),
            Action::MoveCursor(Direction::Down, 1)
        );
    }

    #[test]
    fn motion_digit_does_not_affect_outer_count() {
        // `5d3j`: outer count (5) and motion count (3) multiply → 15 rows.
        let app = App::new();
        let mut state = NormalState::new();
        let _ = state.handle_key(key(KeyCode::Char('5')), &app);
        assert_eq!(state.pending_count, Some(5));
        let _ = state.handle_key(key(KeyCode::Char('d')), &app);
        assert_eq!(state.pending_count, Some(5), "outer count survives 'd'");
        let _ = state.handle_key(key(KeyCode::Char('3')), &app);
        assert_eq!(
            state.pending_count,
            Some(5),
            "outer count unchanged by motion digit"
        );
        assert_eq!(state.pending_motion_count, Some(3));
        let action = state.handle_key(key(KeyCode::Char('j')), &app);
        assert_eq!(
            action,
            Action::ClearRange {
                start: (0, 0),
                end: (15, usize::MAX),
            }
        );
    }
}
