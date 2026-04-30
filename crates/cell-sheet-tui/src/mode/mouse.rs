use cell_sheet_core::model::CellPos;

/// Logical region a screen coordinate maps to. Built by [`hit_test`] from
/// a [`GridLayout`] published by the render layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseTarget {
    Cell(CellPos),
    ColHeader(usize),
    RowHeader(usize),
    Outside,
}

/// Geometry snapshot of the most recent grid render. Built by
/// `render::grid::Grid::render` and stashed on `App` so the next mouse
/// event can hit-test against accurate coordinates.
#[derive(Debug, Clone)]
pub struct GridLayout {
    /// Top-left corner of the grid widget (in terminal cells).
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    /// Width of the row-number gutter on the left edge of the grid.
    pub row_num_width: u16,
    /// Height of the column-header row at the top of the grid.
    pub header_height: u16,
    /// `viewport.row_offset` at render time.
    pub row_offset: usize,
    /// Visible columns in left-to-right order: `(col_index, screen_x, width)`.
    pub visible_cols: Vec<(usize, u16, u16)>,
}

/// Map a terminal coordinate to a [`MouseTarget`]. Pure.
pub fn hit_test(layout: &GridLayout, x: u16, y: u16) -> MouseTarget {
    let in_x = x >= layout.x && x < layout.x + layout.width;
    let in_y = y >= layout.y && y < layout.y + layout.height;
    if !in_x || !in_y {
        return MouseTarget::Outside;
    }

    let row_num_x_end = layout.x + layout.row_num_width;
    let header_y_end = layout.y + layout.header_height;
    let in_gutter = x < row_num_x_end;
    let in_header = y < header_y_end;

    if in_gutter && in_header {
        return MouseTarget::Outside;
    }
    if in_header {
        for &(col, cx, cw) in &layout.visible_cols {
            if x >= cx && x < cx + cw {
                return MouseTarget::ColHeader(col);
            }
        }
        return MouseTarget::Outside;
    }
    if in_gutter {
        let row_offset_y = y - header_y_end;
        return MouseTarget::RowHeader(layout.row_offset + row_offset_y as usize);
    }

    let row_offset_y = y - header_y_end;
    let row = layout.row_offset + row_offset_y as usize;
    for &(col, cx, cw) in &layout.visible_cols {
        if x >= cx && x < cx + cw {
            return MouseTarget::Cell((row, col));
        }
    }
    MouseTarget::Outside
}

use crate::action::Action;
use crate::app::App;
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::time::Instant;

// `DOUBLE_CLICK_MS` is consumed by Task 12's double-click logic; until then
// it's referenced only by tests.
#[allow(dead_code)]
pub const DOUBLE_CLICK_MS: u64 = 400;

pub const MOUSE_SCROLL_LINES: i32 = 3;

#[derive(Debug, Default)]
pub struct MouseState {
    pub drag: MouseDragState,
    pub last_click: Option<LastClick>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum MouseDragState {
    #[default]
    Idle,
    DraggingCells {
        anchor: CellPos,
    },
    DraggingColumns {
        anchor_col: usize,
    },
    DraggingRows {
        anchor_row: usize,
    },
}

// Fields are only read by Task 12's double-click logic; for now we just
// construct the struct on left-down.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct LastClick {
    pub at: Instant,
    pub pos: CellPos,
}

impl MouseState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Translate a single `MouseEvent` into an `Action`. The handler is the
/// only place that mutates `state` (drag tracking, double-click history).
/// `app` and `layout` are read-only; the handler does not own selection
/// or cursor mutations — those live in `App::process_action`.
pub fn handle_mouse_event(
    event: MouseEvent,
    state: &mut MouseState,
    _app: &App,
    layout: Option<&GridLayout>,
) -> Action {
    if event.modifiers.contains(KeyModifiers::SHIFT) {
        return Action::Noop;
    }
    let layout = match layout {
        Some(l) => l,
        None => return Action::Noop,
    };
    let target = hit_test(layout, event.column, event.row);

    match event.kind {
        MouseEventKind::Down(MouseButton::Left) => match target {
            MouseTarget::Cell(pos) => {
                state.drag = MouseDragState::DraggingCells { anchor: pos };
                state.last_click = Some(LastClick {
                    at: Instant::now(),
                    pos,
                });
                Action::MouseClickCell(pos)
            }
            MouseTarget::ColHeader(c) => {
                state.drag = MouseDragState::DraggingColumns { anchor_col: c };
                state.last_click = None;
                Action::MouseSelectColumn(c)
            }
            MouseTarget::RowHeader(r) => {
                state.drag = MouseDragState::DraggingRows { anchor_row: r };
                state.last_click = None;
                Action::MouseSelectRow(r)
            }
            _ => Action::Noop,
        },
        MouseEventKind::Drag(MouseButton::Left) => {
            if state.drag == MouseDragState::Idle {
                return Action::Noop;
            }
            // Edge auto-scroll: a cell drag landing outside the grid in
            // the direction of motion advances the viewport one step.
            // The user keeps holding the drag; the next event lands
            // inside the freshly-revealed row/column and the
            // per-drag-state matcher below extends the selection
            // normally. Only fires for cell drags — for column/row
            // drags the header/gutter is part of the natural
            // interaction surface and must not trigger a scroll.
            if matches!(state.drag, MouseDragState::DraggingCells { .. }) {
                let grid_bottom = layout.y + layout.height;
                let grid_right = layout.x + layout.width;
                let header_y_end = layout.y + layout.header_height;
                let row_num_x_end = layout.x + layout.row_num_width;
                if event.row >= grid_bottom {
                    return Action::MouseScroll { dx: 0, dy: 1 };
                }
                if event.row < header_y_end {
                    return Action::MouseScroll { dx: 0, dy: -1 };
                }
                if event.column >= grid_right {
                    return Action::MouseScroll { dx: 1, dy: 0 };
                }
                if event.column < row_num_x_end {
                    return Action::MouseScroll { dx: -1, dy: 0 };
                }
            }
            match state.drag {
                MouseDragState::DraggingCells { .. } => match target {
                    MouseTarget::Cell(pos) => Action::MouseDragTo(pos),
                    _ => Action::Noop,
                },
                MouseDragState::DraggingColumns { .. } => match target {
                    MouseTarget::ColHeader(c) | MouseTarget::Cell((_, c)) => {
                        Action::MouseSelectColumn(c)
                    }
                    _ => Action::Noop,
                },
                MouseDragState::DraggingRows { .. } => match target {
                    MouseTarget::RowHeader(r) | MouseTarget::Cell((r, _)) => {
                        Action::MouseSelectRow(r)
                    }
                    _ => Action::Noop,
                },
                MouseDragState::Idle => Action::Noop,
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            state.drag = MouseDragState::Idle;
            Action::Noop
        }
        MouseEventKind::ScrollDown => Action::MouseScroll {
            dx: 0,
            dy: MOUSE_SCROLL_LINES,
        },
        MouseEventKind::ScrollUp => Action::MouseScroll {
            dx: 0,
            dy: -MOUSE_SCROLL_LINES,
        },
        MouseEventKind::ScrollLeft => Action::MouseScroll {
            dx: -MOUSE_SCROLL_LINES,
            dy: 0,
        },
        MouseEventKind::ScrollRight => Action::MouseScroll {
            dx: MOUSE_SCROLL_LINES,
            dy: 0,
        },
        _ => Action::Noop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> GridLayout {
        // Grid at (0, 1) with 1-row header, row-number gutter of width 5.
        // Three visible columns of widths 10, 12, 8 starting at x = 6 (after
        // the gutter), with a 1-byte separator between each column (so col 1
        // starts at x = 17, col 2 at x = 30).
        GridLayout {
            x: 0,
            y: 1,
            width: 40,
            height: 10,
            row_num_width: 5,
            header_height: 1,
            row_offset: 0,
            visible_cols: vec![(0, 6, 10), (1, 17, 12), (2, 30, 8)],
        }
    }

    #[test]
    fn click_in_cell() {
        // x=8 is in column 0 (6..16), y=3 is in row 1 (after header at y=1).
        assert_eq!(hit_test(&fixture(), 8, 3), MouseTarget::Cell((1, 0)));
    }

    #[test]
    fn click_in_column_header() {
        // y=1 is the header row.
        assert_eq!(hit_test(&fixture(), 18, 1), MouseTarget::ColHeader(1));
    }

    #[test]
    fn click_in_row_header() {
        // x=2 is in the gutter, y=4 is row 2 (header_y_end=2 → row_offset_y=2).
        assert_eq!(hit_test(&fixture(), 2, 4), MouseTarget::RowHeader(2));
    }

    #[test]
    fn click_top_left_corner_is_outside() {
        assert_eq!(hit_test(&fixture(), 2, 1), MouseTarget::Outside);
    }

    #[test]
    fn click_outside_grid_widget() {
        // y=0 is above the grid widget.
        assert_eq!(hit_test(&fixture(), 8, 0), MouseTarget::Outside);
    }

    #[test]
    fn click_in_padding_right_of_last_visible_col() {
        // Last visible col ends at x=37 (30+8-1); x=39 is in padding.
        assert_eq!(hit_test(&fixture(), 39, 3), MouseTarget::Outside);
    }

    #[test]
    fn click_below_last_rendered_row() {
        // Layout height is 10, so y=11 is outside.
        assert_eq!(hit_test(&fixture(), 8, 11), MouseTarget::Outside);
    }

    #[test]
    fn click_uses_per_column_widths() {
        // Column 1 has width 12 (17..29). x=27 must hit col 1, not col 2.
        // y=5 → row_offset_y = 5 - 2 = 3.
        assert_eq!(hit_test(&fixture(), 27, 5), MouseTarget::Cell((3, 1)));
    }

    #[test]
    fn click_with_nonzero_row_offset() {
        let mut layout = fixture();
        layout.row_offset = 100;
        // y=2 → row_offset_y = 2 - 2 = 0 → row 100
        assert_eq!(hit_test(&layout, 8, 2), MouseTarget::Cell((100, 0)));
    }

    #[test]
    fn click_in_inter_column_gap_is_outside() {
        // x=16 sits between col 0 (6..16) and col 1 (17..29).
        assert_eq!(hit_test(&fixture(), 16, 3), MouseTarget::Outside);
    }

    use crate::action::Action;
    use crate::app::App;
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

    fn synth(kind: MouseEventKind, x: u16, y: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn fresh_mouse_state_is_idle() {
        let s = MouseState::new();
        assert_eq!(s.drag, MouseDragState::Idle);
        assert!(s.last_click.is_none());
    }

    #[test]
    fn down_left_on_cell_in_normal_emits_click() {
        let app = App::new();
        let mut state = MouseState::new();
        let layout = fixture();
        let event = synth(MouseEventKind::Down(MouseButton::Left), 8, 3);
        assert_eq!(
            handle_mouse_event(event, &mut state, &app, Some(&layout)),
            Action::MouseClickCell((1, 0))
        );
        assert_eq!(state.drag, MouseDragState::DraggingCells { anchor: (1, 0) });
    }

    #[test]
    fn shift_click_is_noop_for_terminal_passthrough() {
        let app = App::new();
        let mut state = MouseState::new();
        let layout = fixture();
        let event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 8,
            row: 3,
            modifiers: KeyModifiers::SHIFT,
        };
        assert_eq!(
            handle_mouse_event(event, &mut state, &app, Some(&layout)),
            Action::Noop
        );
        // Shift-click must NOT arm a drag.
        assert_eq!(state.drag, MouseDragState::Idle);
    }

    #[test]
    fn down_left_with_no_layout_is_noop() {
        let app = App::new();
        let mut state = MouseState::new();
        let event = synth(MouseEventKind::Down(MouseButton::Left), 8, 3);
        assert_eq!(
            handle_mouse_event(event, &mut state, &app, None),
            Action::Noop
        );
    }

    #[test]
    fn up_left_clears_drag_state() {
        let app = App::new();
        let mut state = MouseState::new();
        state.drag = MouseDragState::DraggingCells { anchor: (1, 0) };
        let layout = fixture();
        let event = synth(MouseEventKind::Up(MouseButton::Left), 8, 3);
        let _ = handle_mouse_event(event, &mut state, &app, Some(&layout));
        assert_eq!(state.drag, MouseDragState::Idle);
    }

    #[test]
    fn drag_after_down_emits_drag_to() {
        let app = App::new();
        let mut state = MouseState::new();
        let layout = fixture();
        let down = synth(MouseEventKind::Down(MouseButton::Left), 8, 3);
        handle_mouse_event(down, &mut state, &app, Some(&layout));
        let drag = synth(MouseEventKind::Drag(MouseButton::Left), 18, 5);
        assert_eq!(
            handle_mouse_event(drag, &mut state, &app, Some(&layout)),
            Action::MouseDragTo((3, 1))
        );
    }

    #[test]
    fn drag_without_down_is_noop() {
        let app = App::new();
        let mut state = MouseState::new();
        let layout = fixture();
        let drag = synth(MouseEventKind::Drag(MouseButton::Left), 18, 5);
        assert_eq!(
            handle_mouse_event(drag, &mut state, &app, Some(&layout)),
            Action::Noop
        );
    }

    #[test]
    fn drag_into_formula_bar_scrolls_up() {
        let app = App::new();
        let mut state = MouseState::new();
        let layout = fixture();
        let down = synth(MouseEventKind::Down(MouseButton::Left), 8, 3);
        handle_mouse_event(down, &mut state, &app, Some(&layout));
        // y=0 is above the grid (in the formula bar region), which the
        // edge auto-scroll logic treats as "past the top".
        let drag = synth(MouseEventKind::Drag(MouseButton::Left), 8, 0);
        assert_eq!(
            handle_mouse_event(drag, &mut state, &app, Some(&layout)),
            Action::MouseScroll { dx: 0, dy: -1 }
        );
    }

    #[test]
    fn click_then_drag_moves_cursor_to_target() {
        let mut app = App::new();
        app.process_action(Action::MouseClickCell((1, 0)));
        app.process_action(Action::MouseDragTo((3, 2)));
        assert_eq!(app.cursor, (3, 2));
    }

    #[test]
    fn down_on_column_header_emits_select_column() {
        let app = App::new();
        let mut state = MouseState::new();
        let layout = fixture();
        // x=18, y=1 → ColHeader(1).
        let event = synth(MouseEventKind::Down(MouseButton::Left), 18, 1);
        assert_eq!(
            handle_mouse_event(event, &mut state, &app, Some(&layout)),
            Action::MouseSelectColumn(1)
        );
        assert_eq!(
            state.drag,
            MouseDragState::DraggingColumns { anchor_col: 1 }
        );
    }

    #[test]
    fn drag_in_column_mode_extends_to_other_column() {
        let app = App::new();
        let mut state = MouseState::new();
        let layout = fixture();
        handle_mouse_event(
            synth(MouseEventKind::Down(MouseButton::Left), 18, 1),
            &mut state,
            &app,
            Some(&layout),
        );
        // x=32, y=5 → Cell(_, 2). Drag from col 1 to col 2.
        let drag = synth(MouseEventKind::Drag(MouseButton::Left), 32, 5);
        assert_eq!(
            handle_mouse_event(drag, &mut state, &app, Some(&layout)),
            Action::MouseSelectColumn(2)
        );
    }

    #[test]
    fn drag_in_column_mode_back_to_header_extends() {
        let app = App::new();
        let mut state = MouseState::new();
        let layout = fixture();
        handle_mouse_event(
            synth(MouseEventKind::Down(MouseButton::Left), 18, 1),
            &mut state,
            &app,
            Some(&layout),
        );
        // x=32, y=1 → ColHeader(2).
        let drag = synth(MouseEventKind::Drag(MouseButton::Left), 32, 1);
        assert_eq!(
            handle_mouse_event(drag, &mut state, &app, Some(&layout)),
            Action::MouseSelectColumn(2)
        );
    }

    #[test]
    fn down_on_row_header_emits_select_row() {
        let app = App::new();
        let mut state = MouseState::new();
        let layout = fixture();
        // x=2 is the gutter, y=4 → row_offset_y = 2 → row 2.
        let event = synth(MouseEventKind::Down(MouseButton::Left), 2, 4);
        assert_eq!(
            handle_mouse_event(event, &mut state, &app, Some(&layout)),
            Action::MouseSelectRow(2)
        );
        assert_eq!(state.drag, MouseDragState::DraggingRows { anchor_row: 2 });
    }

    #[test]
    fn drag_in_row_mode_extends_to_other_row() {
        let app = App::new();
        let mut state = MouseState::new();
        let layout = fixture();
        handle_mouse_event(
            synth(MouseEventKind::Down(MouseButton::Left), 2, 4),
            &mut state,
            &app,
            Some(&layout),
        );
        // x=8, y=6 → Cell((4, 0)). Drag from row 2 to row 4.
        let drag = synth(MouseEventKind::Drag(MouseButton::Left), 8, 6);
        assert_eq!(
            handle_mouse_event(drag, &mut state, &app, Some(&layout)),
            Action::MouseSelectRow(4)
        );
    }

    #[test]
    fn drag_in_row_mode_back_to_gutter_extends() {
        let app = App::new();
        let mut state = MouseState::new();
        let layout = fixture();
        handle_mouse_event(
            synth(MouseEventKind::Down(MouseButton::Left), 2, 4),
            &mut state,
            &app,
            Some(&layout),
        );
        // x=2 is gutter, y=6 → row 4 → RowHeader(4).
        let drag = synth(MouseEventKind::Drag(MouseButton::Left), 2, 6);
        assert_eq!(
            handle_mouse_event(drag, &mut state, &app, Some(&layout)),
            Action::MouseSelectRow(4)
        );
    }

    #[test]
    fn scroll_down_emits_positive_dy() {
        let app = App::new();
        let mut state = MouseState::new();
        let layout = fixture();
        let event = synth(MouseEventKind::ScrollDown, 8, 3);
        assert_eq!(
            handle_mouse_event(event, &mut state, &app, Some(&layout)),
            Action::MouseScroll { dx: 0, dy: 3 }
        );
    }

    #[test]
    fn scroll_up_emits_negative_dy() {
        let app = App::new();
        let mut state = MouseState::new();
        let layout = fixture();
        let event = synth(MouseEventKind::ScrollUp, 8, 3);
        assert_eq!(
            handle_mouse_event(event, &mut state, &app, Some(&layout)),
            Action::MouseScroll { dx: 0, dy: -3 }
        );
    }

    #[test]
    fn scroll_left_emits_negative_dx() {
        let app = App::new();
        let mut state = MouseState::new();
        let layout = fixture();
        let event = synth(MouseEventKind::ScrollLeft, 8, 3);
        assert_eq!(
            handle_mouse_event(event, &mut state, &app, Some(&layout)),
            Action::MouseScroll { dx: -3, dy: 0 }
        );
    }

    #[test]
    fn scroll_right_emits_positive_dx() {
        let app = App::new();
        let mut state = MouseState::new();
        let layout = fixture();
        let event = synth(MouseEventKind::ScrollRight, 8, 3);
        assert_eq!(
            handle_mouse_event(event, &mut state, &app, Some(&layout)),
            Action::MouseScroll { dx: 3, dy: 0 }
        );
    }

    #[test]
    fn scroll_does_not_move_cursor() {
        let mut app = App::new();
        app.cursor = (5, 2);
        app.process_action(Action::MouseScroll { dx: 0, dy: 3 });
        assert_eq!(app.cursor, (5, 2));
        assert_eq!(app.viewport.row_offset, 3);
    }

    #[test]
    fn scroll_up_at_top_saturates_to_zero() {
        let mut app = App::new();
        // viewport starts at row_offset=0; scrolling up should saturate.
        app.process_action(Action::MouseScroll { dx: 0, dy: -10 });
        assert_eq!(app.viewport.row_offset, 0);
    }

    #[test]
    fn drag_past_bottom_emits_scroll_down() {
        let app = App::new();
        let mut state = MouseState::new();
        let layout = fixture();
        // Arm DraggingCells first.
        handle_mouse_event(
            synth(MouseEventKind::Down(MouseButton::Left), 8, 3),
            &mut state,
            &app,
            Some(&layout),
        );
        // Layout: y=1, height=10 → grid_bottom = 11. y=11 is past the bottom.
        let drag = synth(MouseEventKind::Drag(MouseButton::Left), 8, 11);
        assert_eq!(
            handle_mouse_event(drag, &mut state, &app, Some(&layout)),
            Action::MouseScroll { dx: 0, dy: 1 }
        );
    }

    #[test]
    fn drag_past_top_emits_scroll_up() {
        let app = App::new();
        let mut state = MouseState::new();
        let layout = fixture();
        handle_mouse_event(
            synth(MouseEventKind::Down(MouseButton::Left), 8, 3),
            &mut state,
            &app,
            Some(&layout),
        );
        // Layout: y=1, header_height=1 → header_y_end = 2. y=1 is in header
        // (above the data rows), which counts as "past the top" for drag purposes.
        let drag = synth(MouseEventKind::Drag(MouseButton::Left), 8, 1);
        assert_eq!(
            handle_mouse_event(drag, &mut state, &app, Some(&layout)),
            Action::MouseScroll { dx: 0, dy: -1 }
        );
    }

    #[test]
    fn drag_past_right_emits_scroll_right() {
        let app = App::new();
        let mut state = MouseState::new();
        let layout = fixture();
        handle_mouse_event(
            synth(MouseEventKind::Down(MouseButton::Left), 8, 3),
            &mut state,
            &app,
            Some(&layout),
        );
        // Layout: x=0, width=40 → grid_right = 40. x=40 is past.
        let drag = synth(MouseEventKind::Drag(MouseButton::Left), 40, 5);
        assert_eq!(
            handle_mouse_event(drag, &mut state, &app, Some(&layout)),
            Action::MouseScroll { dx: 1, dy: 0 }
        );
    }

    #[test]
    fn drag_past_left_into_gutter_emits_scroll_left() {
        let app = App::new();
        let mut state = MouseState::new();
        let layout = fixture();
        handle_mouse_event(
            synth(MouseEventKind::Down(MouseButton::Left), 8, 3),
            &mut state,
            &app,
            Some(&layout),
        );
        // Layout: x=0, row_num_width=5 → row_num_x_end = 5. x=4 is in gutter.
        let drag = synth(MouseEventKind::Drag(MouseButton::Left), 4, 5);
        assert_eq!(
            handle_mouse_event(drag, &mut state, &app, Some(&layout)),
            Action::MouseScroll { dx: -1, dy: 0 }
        );
    }

    #[test]
    fn click_in_insert_committed_cell_then_moves() {
        // Simulate Task 11's run_loop logic by hand: commit Insert
        // edit, then dispatch MouseClickCell.
        let mut app = App::new();
        app.mode = crate::action::Mode::Insert;
        app.cursor = (0, 0);
        app.insert_buffer = "hello".to_string();

        let pos = app.cursor;
        let buf = std::mem::take(&mut app.insert_buffer);
        app.process_action(Action::EditCell(pos, buf));
        app.mode = crate::action::Mode::Normal;
        app.process_action(Action::MouseClickCell((3, 5)));

        assert_eq!(app.cursor, (3, 5));
        assert_eq!(app.mode, crate::action::Mode::Normal);
        assert_eq!(
            app.sheet.get_cell((0, 0)).map(|c| c.raw.clone()),
            Some("hello".into()),
            "buffer should have been committed"
        );
    }

    #[test]
    fn click_in_visual_records_last_visual_for_gv() {
        // Simulate Task 11's Visual-exit branch: snapshot last_visual
        // BEFORE clearing visual_state, so a later `gv` can re-enter.
        use crate::action::Mode;
        use crate::mode::visual::{VisualKind, VisualState};

        let mut app = App::new();
        app.cursor = (4, 6);
        app.mode = Mode::Visual;
        let vs = VisualState::new((2, 3), VisualKind::Character);

        // The exact two lines the run_loop performs on a click in Visual:
        app.record_last_visual(vs.anchor, vs.kind);
        app.mode = Mode::Normal;
        // ...and then the click action is dispatched:
        app.process_action(Action::MouseClickCell((9, 9)));

        let lv = app.last_visual.expect("last_visual must be recorded");
        assert_eq!(lv.anchor, (2, 3));
        // cursor snapshot must capture the pre-click position, not the
        // post-click position.
        assert_eq!(lv.cursor, (4, 6));
        assert_eq!(lv.kind, VisualKind::Character);
        assert_eq!(app.cursor, (9, 9));
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn click_in_command_clears_prompt_and_returns_to_normal() {
        // Simulate Task 11's Command-cancel branch for the colon prompt.
        use crate::action::{CommandKind, Mode};

        let mut app = App::new();
        app.mode = Mode::Command;
        app.command_kind = CommandKind::Colon;
        app.command_line = "se".into();
        app.command_history_idx = Some(0);
        app.command_history_scratch = "abc".into();

        // The mouse-cancel branch (colon path):
        app.command_history_idx = None;
        app.command_history_scratch.clear();
        app.command_line.clear();
        app.mode = Mode::Normal;
        app.process_action(Action::MouseClickCell((1, 1)));

        assert_eq!(app.mode, Mode::Normal);
        assert!(app.command_line.is_empty());
        assert!(app.command_history_idx.is_none());
        assert!(app.command_history_scratch.is_empty());
        assert_eq!(app.cursor, (1, 1));
    }
}
