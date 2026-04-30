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
    // `DraggingColumns` / `DraggingRows` are armed by header-drag logic in
    // Tasks 7+. Keep the variants here so the state machine is complete.
    #[allow(dead_code)]
    DraggingColumns {
        anchor_col: usize,
    },
    #[allow(dead_code)]
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
            _ => Action::Noop,
        },
        MouseEventKind::Up(MouseButton::Left) => {
            state.drag = MouseDragState::Idle;
            Action::Noop
        }
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
}
