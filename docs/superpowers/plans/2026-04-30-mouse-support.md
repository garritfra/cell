# Mouse Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add opt-in mouse support to cell — left-click moves the cursor, click+drag selects a range, scroll wheel scrolls the viewport, double-click enters Insert mode — gated behind `:set mouse on`.

**Architecture:** A new `mode/mouse.rs` module mirrors the keyboard-mode handlers. A pure `hit_test(GridLayout, x, y) → MouseTarget` translates screen coordinates to logical cell positions. The render layer publishes `GridLayout` once per frame and stashes it on `App` for the next event. `run_loop` gains an `Event::Mouse` arm that delegates to `mouse::handle_mouse_event`, mirroring the existing keyboard dispatch.

**Tech Stack:** Rust 2021, crossterm 0.29 (`MouseEvent`, `EnableMouseCapture` / `DisableMouseCapture`), ratatui (already in tree), no new dependencies.

**Spec:** [`docs/superpowers/specs/2026-04-30-mouse-support-design.md`](../specs/2026-04-30-mouse-support-design.md) — Issue [#70](https://github.com/garritfra/cell/issues/70).

---

## File structure

| File | Action | Responsibility |
| --- | --- | --- |
| `crates/cell-sheet-tui/src/mode/mouse.rs` | **Create** | `MouseTarget`, `GridLayout`, `MouseState`, `MouseDragState`, `LastClick`, `hit_test`, `handle_mouse_event`. Owns nearly all mouse logic. |
| `crates/cell-sheet-tui/src/mode/mod.rs` | Modify | Add `pub mod mouse;`. |
| `crates/cell-sheet-tui/src/action.rs` | Modify | Add 6 new `Action` variants. |
| `crates/cell-sheet-tui/src/app.rs` | Modify | Add `mouse_enabled: bool` and `last_grid_layout: Option<GridLayout>`; new arms in `process_action`. |
| `crates/cell-sheet-tui/src/render/grid.rs` | Modify | Build a `GridLayout` while rendering and return it. |
| `crates/cell-sheet-tui/src/render/mod.rs` | Modify | Thread the `GridLayout` from grid render into `App`. |
| `crates/cell-sheet-tui/src/mode/command.rs` | Modify | Parse `set mouse on \| off \| toggle`. |
| `crates/cell-sheet-tui/src/main.rs` | Modify | Capture `Event::Mouse`, dispatch to handler; toggle `EnableMouseCapture` / `DisableMouseCapture` when the flag changes; clean up on shutdown. |
| `crates/cell-sheet-core/src/help/mod.rs` | Modify | Add `HelpCategory::Mouse` and wire `MOUSE_ENTRIES`. |
| `crates/cell-sheet-core/src/help/entries.rs` | Modify | Add `MOUSE_ENTRIES`. |
| `crates/cell-sheet-tui/src/render/help.rs` | Read-check | Verify the help renderer iterates `categories()` (no change needed if so). |
| `README.md` | Modify | Add a "Mouse support" section. |
| `AGENTS.md` | Modify | Add a one-liner under "Things to avoid". |
| `CHANGELOG.md` | Modify | Add an `Added` entry under `## Unreleased`. |

---

## Task 1: Foundation — `MouseTarget`, `GridLayout`, `hit_test`

Pure functions and types only. No event handling, no integration.

**Files:**
- Create: `crates/cell-sheet-tui/src/mode/mouse.rs`
- Modify: `crates/cell-sheet-tui/src/mode/mod.rs`

- [ ] **Step 1: Add the module to `mode/mod.rs`**

`crates/cell-sheet-tui/src/mode/mod.rs` currently:

```rust
pub mod command;
pub mod help;
pub mod insert;
pub mod normal;
pub mod visual;
```

Add one line:

```rust
pub mod command;
pub mod help;
pub mod insert;
pub mod mouse;
pub mod normal;
pub mod visual;
```

- [ ] **Step 2: Create `mode/mouse.rs` with the types and a stub `hit_test`**

```rust
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
    /// `viewport.col_offset` at render time.
    pub col_offset: usize,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> GridLayout {
        // Grid at (0, 1) with 2-row header (formula bar above it isn't ours),
        // row-number gutter of width 5, column-header height of 1.
        // Three visible columns of widths 10, 12, 8 starting at x = 6 (after
        // the gutter), with the column-separator that grid.rs draws between
        // each column accounted for as part of the column on its left.
        GridLayout {
            x: 0,
            y: 1,
            width: 40,
            height: 10,
            row_num_width: 5,
            header_height: 1,
            row_offset: 0,
            col_offset: 0,
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
        // x=2 is in the gutter, y=4 is row 3 (with row_offset=0).
        assert_eq!(hit_test(&fixture(), 2, 4), MouseTarget::RowHeader(3));
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
        assert_eq!(hit_test(&fixture(), 27, 5), MouseTarget::Cell((4, 1)));
    }

    #[test]
    fn click_with_nonzero_row_offset() {
        let mut layout = fixture();
        layout.row_offset = 100;
        // y=2 → row_offset_y=1 → row 101
        assert_eq!(hit_test(&layout, 8, 2), MouseTarget::Cell((101, 0)));
    }
}
```

- [ ] **Step 3: Run tests to verify the foundation**

```sh
cargo test -p cell-sheet-tui mode::mouse::tests
```

Expected: 9 passing.

- [ ] **Step 4: Run clippy and fmt**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --all-features
```

Expected: clean.

- [ ] **Step 5: Commit**

```sh
git add crates/cell-sheet-tui/src/mode/mod.rs crates/cell-sheet-tui/src/mode/mouse.rs
git commit -m "feat(tui): add MouseTarget, GridLayout, and hit_test (#70)

Pure foundation for mouse support. Translates terminal coordinates to
a logical cell / header / outside region. No integration yet."
```

---

## Task 2: `:set mouse on|off|toggle` + `Action::SetMouse` + `App.mouse_enabled`

Add the runtime toggle. No event handling yet — the flag just flips, with `Enable/DisableMouseCapture` syscalls deferred to Task 4.

**Files:**
- Modify: `crates/cell-sheet-tui/src/action.rs`
- Modify: `crates/cell-sheet-tui/src/app.rs`
- Modify: `crates/cell-sheet-tui/src/mode/command.rs`

- [ ] **Step 1: Add `Action::SetMouse(bool)` to `action.rs`**

In `crates/cell-sheet-tui/src/action.rs`, in the `Action` enum, immediately after the existing `SetDelimiter(u8)` variant:

```rust
    SetDelimiter(u8),
    /// Toggle the mouse-enabled flag. The actual `Enable/DisableMouseCapture`
    /// syscall is issued by `run_loop` after observing the flag change.
    SetMouse(bool),
    SetStatus(String),
```

- [ ] **Step 2: Add the `mouse_enabled` field to `App`**

In `crates/cell-sheet-tui/src/app.rs`, add to the `App` struct (after `pub delimiter: u8`):

```rust
    pub delimiter: u8,
    /// Off by default. Toggled by `:set mouse on|off|toggle`. When true,
    /// the run_loop has issued `EnableMouseCapture` to the terminal and
    /// is routing `Event::Mouse` to `mode::mouse::handle_mouse_event`.
    pub mouse_enabled: bool,
    pub help_scroll: usize,
```

…and in `App::new`:

```rust
            delimiter: b',',
            mouse_enabled: false,
            help_scroll: 0,
```

- [ ] **Step 3: Handle `Action::SetMouse` in `process_action`**

Find the existing `Action::SetDelimiter(d) => { ... }` arm in `process_action` and add immediately after it:

```rust
            Action::SetMouse(b) => {
                self.mouse_enabled = b;
            }
```

- [ ] **Step 4: Write failing tests for the `:set mouse` parser**

In `crates/cell-sheet-tui/src/mode/command.rs`, find the existing `#[cfg(test)] mod tests { ... }` (or add one if absent) and add:

```rust
#[test]
fn parse_set_mouse_on() {
    assert_eq!(parse_command("set mouse on"), Action::SetMouse(true));
}

#[test]
fn parse_set_mouse_off() {
    assert_eq!(parse_command("set mouse off"), Action::SetMouse(false));
}

#[test]
fn parse_set_mouse_toggle_returns_status_error_for_now() {
    // `toggle` is parsed in the next step; verify the no-op path here.
    assert!(matches!(parse_command("set mouse bogus"), Action::SetStatus(_)));
}
```

- [ ] **Step 5: Run the tests to verify they fail**

```sh
cargo test -p cell-sheet-tui mode::command::tests::parse_set_mouse
```

Expected: FAIL — `parse_command` does not yet recognize `set mouse`.

- [ ] **Step 6: Implement `set mouse` parsing**

In `crates/cell-sheet-tui/src/mode/command.rs::parse_command`, add a new branch *after* the existing `set delimiter=` block but *before* the final `else { Action::Noop }`:

```rust
    if let Some(rest) = input.strip_prefix("set mouse") {
        let arg = rest.trim();
        return match arg {
            "on" => Action::SetMouse(true),
            "off" => Action::SetMouse(false),
            "toggle" => Action::SetMouse(true), // overridden by run_loop using app.mouse_enabled
            "" => Action::SetStatus("usage: :set mouse on|off|toggle".into()),
            _ => Action::SetStatus("usage: :set mouse on|off|toggle".into()),
        };
    }
```

The `toggle` arm intentionally returns `SetMouse(true)` — `run_loop` rewrites it just below the parse, where it has access to the current `app.mouse_enabled`. Add a unit test for `toggle`:

```rust
#[test]
fn parse_set_mouse_toggle() {
    // Parser produces SetMouse(true); run_loop rewrites toggle vs current state.
    assert_eq!(parse_command("set mouse toggle"), Action::SetMouse(true));
}
```

- [ ] **Step 7: Toggle rewrite in `run_loop`**

In `crates/cell-sheet-tui/src/main.rs::run_loop`, in the `CommandAction::Execute(cmd)` branch, just before the existing `let parsed = submit(kind, &cmd);`, capture whether this is a toggle:

```rust
let is_toggle =
    matches!(kind, CommandKind::Colon) && cmd.trim() == "set mouse toggle";
let parsed = submit(kind, &cmd);
let parsed = if is_toggle {
    Action::SetMouse(!app.mouse_enabled)
} else {
    parsed
};
```

- [ ] **Step 8: Add tests for `process_action(SetMouse)`**

In `crates/cell-sheet-tui/src/app.rs::tests`:

```rust
#[test]
fn set_mouse_flag_starts_off_and_toggles() {
    let mut app = App::new();
    assert!(!app.mouse_enabled);
    app.process_action(Action::SetMouse(true));
    assert!(app.mouse_enabled);
    app.process_action(Action::SetMouse(false));
    assert!(!app.mouse_enabled);
}
```

- [ ] **Step 9: Run all tests, fmt, clippy**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --all-features
cargo test
```

Expected: all green.

- [ ] **Step 10: Commit**

```sh
git add crates/cell-sheet-tui/src/{action.rs,app.rs,main.rs,mode/command.rs}
git commit -m "feat(tui): add :set mouse on|off|toggle (#70)

Adds Action::SetMouse and App.mouse_enabled (off by default). Capture
syscall is wired in a later step. Toggle is resolved in run_loop with
access to the live flag."
```

---

## Task 3: Publish `GridLayout` from the render layer

The render layer constructs a `GridLayout` while drawing the grid and stashes it on `App` for the next mouse event.

**Files:**
- Modify: `crates/cell-sheet-tui/src/render/grid.rs`
- Modify: `crates/cell-sheet-tui/src/render/mod.rs`
- Modify: `crates/cell-sheet-tui/src/app.rs`
- Modify: `crates/cell-sheet-tui/src/mode/mouse.rs` (re-export `GridLayout` for `app.rs`)

- [ ] **Step 1: Add `last_grid_layout` to `App`**

In `crates/cell-sheet-tui/src/app.rs`:

```rust
use crate::mode::mouse::GridLayout;
```

Add a field to `App`:

```rust
    pub mouse_enabled: bool,
    /// Geometry from the most recent grid render, used by mouse hit-testing
    /// on the next event. `None` until the first frame is drawn.
    pub last_grid_layout: Option<GridLayout>,
    pub help_scroll: usize,
```

…and in `App::new`:

```rust
            mouse_enabled: false,
            last_grid_layout: None,
```

- [ ] **Step 2: Modify `Grid` to populate a layout**

In `crates/cell-sheet-tui/src/render/grid.rs`, change the `Grid` struct so the renderer can write back a layout. The simplest pattern that works with `Widget` (which takes `self` by value): give `Grid` an `&mut Option<GridLayout>` field.

```rust
use crate::mode::mouse::GridLayout;
use crate::viewport::Viewport;
use cell_sheet_core::model::{col_index_to_label, CellPos, CellValue, Sheet};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};

pub struct Grid<'a> {
    pub sheet: &'a Sheet,
    pub viewport: &'a Viewport,
    pub cursor: CellPos,
    pub selection: Option<(CellPos, CellPos)>,
    pub layout_out: &'a mut Option<GridLayout>,
}
```

Inside `impl<'a> Widget for Grid<'a>::render`, after the existing `for col in self.viewport.col_offset.. { ... }` loop that already builds `let mut visible_cols = Vec::new();`, populate the output:

```rust
        *self.layout_out = Some(GridLayout {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height,
            row_num_width: ROW_NUM_WIDTH,
            header_height: 1,
            row_offset: self.viewport.row_offset,
            col_offset: self.viewport.col_offset,
            visible_cols: visible_cols
                .iter()
                .map(|&(col, cx, cw)| (col, cx, cw))
                .collect(),
        });
```

(Place this right after the column-header loop completes — before the row-rendering loop is fine.)

- [ ] **Step 3: Thread the layout through `render::render`**

In `crates/cell-sheet-tui/src/render/mod.rs`, change the signature so the caller can read back the layout:

```rust
pub fn render(
    frame: &mut Frame,
    app: &mut App,
    selection: Option<(CellPos, CellPos)>,
    insert_cursor: usize,
    partial_command: Option<&str>,
) {
```

Note `app` is now `&mut App`. Update the `Grid` construction:

```rust
    let mut grid_layout: Option<GridLayout> = None;
    frame.render_widget(
        Grid {
            sheet: &app.sheet,
            viewport: &app.viewport,
            cursor: app.cursor,
            selection,
            layout_out: &mut grid_layout,
        },
        chunks[1],
    );
    app.last_grid_layout = grid_layout;
```

Add the import at the top:

```rust
use crate::mode::mouse::GridLayout;
```

- [ ] **Step 4: Update the `terminal.draw` call in `run_loop`**

In `crates/cell-sheet-tui/src/main.rs::run_loop`, the existing `terminal.draw(|frame| { render::render(frame, app, ...) })?;` already passes `app: &mut App` since `app` is `&mut App` in `run_loop`. Verify the call still type-checks; no edit needed unless `render::render` was being called with `&app`. If so, change to `app`.

- [ ] **Step 5: Add a render unit test**

In `crates/cell-sheet-tui/src/render/grid.rs`, add a `#[cfg(test)] mod tests`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    #[test]
    fn render_publishes_grid_layout() {
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), CellValue::Text("a".into()));
        sheet.set_cell((0, 1), CellValue::Text("b".into()));
        let viewport = Viewport::new();
        let area = Rect::new(0, 1, 30, 5);
        let mut buf = Buffer::empty(area);
        let mut layout = None;

        Grid {
            sheet: &sheet,
            viewport: &viewport,
            cursor: (0, 0),
            selection: None,
            layout_out: &mut layout,
        }
        .render(area, &mut buf);

        let l = layout.expect("layout should be published");
        assert_eq!(l.x, 0);
        assert_eq!(l.y, 1);
        assert_eq!(l.width, 30);
        assert_eq!(l.row_num_width, ROW_NUM_WIDTH);
        assert_eq!(l.header_height, 1);
        assert!(!l.visible_cols.is_empty());
        assert_eq!(l.visible_cols[0].0, 0);
    }
}
```

- [ ] **Step 6: Run tests, fmt, clippy**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --all-features
cargo test
```

Expected: all green.

- [ ] **Step 7: Commit**

```sh
git add crates/cell-sheet-tui/src
git commit -m "feat(tui): publish GridLayout from grid render (#70)

The renderer now writes a GridLayout snapshot back to App on every
frame. Mouse hit-testing reads this on the next event."
```

---

## Task 4: Wire mouse capture and route `Event::Mouse` to a stub handler

Enable/disable `MouseCapture` based on `app.mouse_enabled`, and route `Event::Mouse` to a stub `handle_mouse_event` that returns `Action::Noop`. End-to-end plumbing only.

**Files:**
- Modify: `crates/cell-sheet-tui/src/main.rs`
- Modify: `crates/cell-sheet-tui/src/mode/mouse.rs`

- [ ] **Step 1: Add `MouseState` and a stub `handle_mouse_event`**

Append to `crates/cell-sheet-tui/src/mode/mouse.rs`:

```rust
use crate::action::Action;
use crate::app::App;
use crossterm::event::{KeyModifiers, MouseEvent, MouseEventKind};
use std::time::{Duration, Instant};

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
    DraggingCells { anchor: CellPos },
    DraggingColumns { anchor_col: usize },
    DraggingRows { anchor_row: usize },
}

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
/// `app` is read-only for now; future steps may need `&mut App` for
/// mode-aware bookkeeping.
pub fn handle_mouse_event(
    _event: MouseEvent,
    _state: &mut MouseState,
    _app: &App,
    _layout: Option<&GridLayout>,
) -> Action {
    Action::Noop
}
```

- [ ] **Step 2: Wire enable/disable in `run_tui` and `run_loop`**

In `crates/cell-sheet-tui/src/main.rs`, change the import:

```rust
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseEvent,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
```

Update `run_tui`'s cleanup so mouse capture never leaks past process exit:

```rust
    let result = run_loop(&mut terminal, &mut app);

    if app.mouse_enabled {
        let _ = execute!(terminal.backend_mut(), DisableMouseCapture);
    }
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
```

In `run_loop`, near the top of the function, track whether mouse capture is currently engaged in the terminal so we can sync to `app.mouse_enabled`:

```rust
    let mut mouse_state = mode::mouse::MouseState::new();
    let mut mouse_capture_active = false;
```

At the top of the loop, *before* `terminal.draw(...)`, sync the terminal state:

```rust
    loop {
        if app.mouse_enabled != mouse_capture_active {
            if app.mouse_enabled {
                execute!(terminal.backend_mut(), EnableMouseCapture)?;
            } else {
                execute!(terminal.backend_mut(), DisableMouseCapture)?;
                mouse_state = mode::mouse::MouseState::new(); // drop stale drag
            }
            mouse_capture_active = app.mouse_enabled;
        }

        let grid_height = terminal.size()?.height.saturating_sub(3) as usize;
        // ... existing code
```

Add the `mod mouse;` reference is already there (Task 1). Now extend the `event::read()` arm to also accept mouse events. Below the existing `if let Event::Key(key) = event::read()? {` block, replace the structure with:

```rust
        if event::poll(std::time::Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    app.status_message = None;
                    // ... entire existing key-handling block, unchanged ...
                }
                Event::Mouse(me) => {
                    if !app.mouse_enabled {
                        continue;
                    }
                    app.status_message = None;
                    // Clone the layout so we don't hold an immutable
                    // borrow of `app` across the call (which also takes
                    // `app` for future read-only access).
                    let layout = app.last_grid_layout.clone();
                    let action = mode::mouse::handle_mouse_event(
                        me,
                        &mut mouse_state,
                        app,
                        layout.as_ref(),
                    );
                    app.process_action(action);
                }
                _ => {}
            }
        }
```

(The existing key-handling code — about 220 lines — moves verbatim into the `Event::Key(key) => { ... }` arm. This is the largest mechanical change in the plan; preserve every line.)

- [ ] **Step 3: Add a smoke test driving a synthetic mouse event**

In `crates/cell-sheet-tui/src/mode/mouse.rs::tests`, append:

```rust
use crate::app::App;
use crossterm::event::{KeyModifiers, MouseButton, MouseEventKind};

fn synth(kind: MouseEventKind, x: u16, y: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    }
}

#[test]
fn stub_handler_returns_noop() {
    let app = App::new();
    let mut state = MouseState::new();
    let layout = fixture();
    let event = synth(MouseEventKind::Down(MouseButton::Left), 8, 3);
    assert_eq!(
        handle_mouse_event(event, &mut state, &app, Some(&layout)),
        Action::Noop
    );
}
```

- [ ] **Step 4: Run tests, fmt, clippy**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --all-features
cargo test
```

Expected: all green. The TUI binary now toggles real mouse capture in response to `:set mouse on/off`, but does nothing with the events.

- [ ] **Step 5: Commit**

```sh
git add crates/cell-sheet-tui/src
git commit -m "feat(tui): route Event::Mouse to a stub handler (#70)

Mouse capture engages and disengages with :set mouse. Events flow into
mode::mouse::handle_mouse_event, which currently returns Noop. The next
tasks teach it to translate clicks, drags, and scrolls into Actions."
```

---

## Task 5: Left-click → `MouseClickCell` (Normal mode)

The simplest happy path: in Normal mode, a `Down(Left)` on a `Cell(pos)` moves the cursor.

**Files:**
- Modify: `crates/cell-sheet-tui/src/action.rs`
- Modify: `crates/cell-sheet-tui/src/app.rs`
- Modify: `crates/cell-sheet-tui/src/mode/mouse.rs`

- [ ] **Step 1: Add `Action::MouseClickCell`**

In `crates/cell-sheet-tui/src/action.rs`, after `SetMouse(bool)`:

```rust
    SetMouse(bool),
    MouseClickCell(CellPos),
    SetStatus(String),
```

- [ ] **Step 2: Handle it in `process_action`**

In `crates/cell-sheet-tui/src/app.rs`, after the `Action::SetMouse(b) => { ... }` arm:

```rust
            Action::MouseClickCell(pos) => {
                self.cursor = pos;
                self.viewport.ensure_visible(self.cursor);
            }
```

- [ ] **Step 3: Write the failing handler test**

In `crates/cell-sheet-tui/src/mode/mouse.rs::tests`:

```rust
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
```

- [ ] **Step 4: Run test to verify failure**

```sh
cargo test -p cell-sheet-tui mode::mouse::tests::down_left_on_cell
```

Expected: FAIL — handler returns `Noop`.

- [ ] **Step 5: Implement `Down(Left)` for Cell targets**

Replace the body of `handle_mouse_event` in `crates/cell-sheet-tui/src/mode/mouse.rs`:

```rust
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
        MouseEventKind::Down(crossterm::event::MouseButton::Left) => match target {
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
        MouseEventKind::Up(crossterm::event::MouseButton::Left) => {
            state.drag = MouseDragState::Idle;
            Action::Noop
        }
        _ => Action::Noop,
    }
}
```

- [ ] **Step 6: Run handler test to verify pass**

```sh
cargo test -p cell-sheet-tui mode::mouse::tests::down_left_on_cell
```

Expected: PASS.

- [ ] **Step 7: Add a process_action test**

In `crates/cell-sheet-tui/src/app.rs::tests`:

```rust
#[test]
fn mouse_click_cell_moves_cursor() {
    let mut app = App::new();
    app.process_action(Action::MouseClickCell((3, 5)));
    assert_eq!(app.cursor, (3, 5));
}
```

- [ ] **Step 8: Run all tests, fmt, clippy**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --all-features
cargo test
```

- [ ] **Step 9: Commit**

```sh
git add crates/cell-sheet-tui/src
git commit -m "feat(tui): left-click moves the cursor (#70)"
```

---

## Task 6: Click+drag inside grid → Visual selection via `MouseDragTo`

First `Drag(Left)` after a `Down(Left)` enters Visual mode with anchor at the click cell. Subsequent drags extend the cursor.

**Files:**
- Modify: `crates/cell-sheet-tui/src/action.rs`
- Modify: `crates/cell-sheet-tui/src/app.rs`
- Modify: `crates/cell-sheet-tui/src/mode/mouse.rs`
- Modify: `crates/cell-sheet-tui/src/main.rs`

- [ ] **Step 1: Add `Action::MouseDragTo`**

In `action.rs`:

```rust
    MouseClickCell(CellPos),
    MouseDragTo(CellPos),
    SetStatus(String),
```

- [ ] **Step 2: Handle in `process_action`**

In `app.rs`:

```rust
            Action::MouseDragTo(pos) => {
                self.cursor = pos;
                self.viewport.ensure_visible(self.cursor);
            }
```

The visual transition itself happens in `run_loop` (Step 4 below), mirroring how `Action::ChangeMode(Mode::Visual)` is handled today.

- [ ] **Step 3: Failing handler test**

In `crates/cell-sheet-tui/src/mode/mouse.rs::tests`:

```rust
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
```

- [ ] **Step 4: Implement the `Drag` arm**

Add a `MouseEventKind::Drag(...)` arm to the `match event.kind` in `handle_mouse_event`:

```rust
        MouseEventKind::Drag(crossterm::event::MouseButton::Left) => match state.drag {
            MouseDragState::DraggingCells { .. } => match target {
                MouseTarget::Cell(pos) => Action::MouseDragTo(pos),
                _ => Action::Noop,
            },
            _ => Action::Noop,
        },
```

- [ ] **Step 5: Run handler tests**

```sh
cargo test -p cell-sheet-tui mode::mouse::tests::drag
```

Expected: both PASS.

- [ ] **Step 6: Wire visual-mode transition in `run_loop`**

In `crates/cell-sheet-tui/src/main.rs`, after `Event::Mouse(me)` dispatches the action and *before* `app.process_action(action);`, mirror the existing keyboard pattern that turns `Action::ChangeMode(Mode::Visual)` into a real `visual_state`:

```rust
                Event::Mouse(me) => {
                    if !app.mouse_enabled {
                        continue;
                    }
                    app.status_message = None;
                    let layout = app.last_grid_layout.clone();
                    let action = mode::mouse::handle_mouse_event(
                        me,
                        &mut mouse_state,
                        app,
                        layout.as_ref(),
                    );
                    if let Action::MouseDragTo(_) = &action {
                        if app.mode == Mode::Normal {
                            if let mode::mouse::MouseDragState::DraggingCells { anchor } =
                                mouse_state.drag
                            {
                                visual_state =
                                    Some(VisualState::new(anchor, VisualKind::Character));
                                app.mode = Mode::Visual;
                            }
                        }
                    }
                    app.process_action(action);
                }
```

- [ ] **Step 7: Add an integration smoke test**

In `crates/cell-sheet-tui/src/mode/mouse.rs::tests`, drive the full pipeline through `App` (no terminal):

```rust
#[test]
fn click_then_drag_moves_cursor_to_target() {
    let mut app = App::new();
    app.process_action(Action::MouseClickCell((1, 0)));
    app.process_action(Action::MouseDragTo((3, 2)));
    assert_eq!(app.cursor, (3, 2));
}
```

(The Visual-mode transition itself is exercised end-to-end only via the run_loop; we verify the action-processing side here.)

- [ ] **Step 8: Run all tests, fmt, clippy, commit**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --all-features
cargo test
git add crates/cell-sheet-tui/src
git commit -m "feat(tui): click+drag selects a Visual range (#70)"
```

---

## Task 7: Click+drag on a column header → `MouseSelectColumn`

Click on a column header selects the whole column (all rows of that column). Drag extends column-by-column.

**Files:**
- Modify: `crates/cell-sheet-tui/src/action.rs`
- Modify: `crates/cell-sheet-tui/src/app.rs`
- Modify: `crates/cell-sheet-tui/src/mode/mouse.rs`
- Modify: `crates/cell-sheet-tui/src/main.rs`

- [ ] **Step 1: Add `MouseSelectColumn`**

In `action.rs`:

```rust
    MouseDragTo(CellPos),
    /// Anchor (or extend) a whole-column selection to column `col`.
    MouseSelectColumn(usize),
    SetStatus(String),
```

- [ ] **Step 2: Handle in `process_action`**

In `app.rs`:

```rust
            Action::MouseSelectColumn(col) => {
                let last_row = self.sheet.row_count.saturating_sub(1);
                self.cursor = (last_row, col);
                self.viewport.ensure_visible(self.cursor);
            }
```

The Visual-mode transition itself happens in `run_loop` (Step 5).

- [ ] **Step 3: Failing handler tests**

In `mode/mouse.rs::tests`:

```rust
#[test]
fn down_on_column_header_emits_select_column() {
    let app = App::new();
    let mut state = MouseState::new();
    let layout = fixture();
    let event = synth(MouseEventKind::Down(MouseButton::Left), 18, 1);
    assert_eq!(
        handle_mouse_event(event, &mut state, &app, Some(&layout)),
        Action::MouseSelectColumn(1)
    );
    assert_eq!(state.drag, MouseDragState::DraggingColumns { anchor_col: 1 });
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
    let drag = synth(MouseEventKind::Drag(MouseButton::Left), 32, 5);
    assert_eq!(
        handle_mouse_event(drag, &mut state, &app, Some(&layout)),
        Action::MouseSelectColumn(2)
    );
}
```

- [ ] **Step 4: Implement the `ColHeader` and `DraggingColumns` paths**

Update `handle_mouse_event`:

In the `Down(Left)` match: add a `ColHeader(c)` arm:

```rust
            MouseTarget::ColHeader(c) => {
                state.drag = MouseDragState::DraggingColumns { anchor_col: c };
                state.last_click = None;
                Action::MouseSelectColumn(c)
            }
```

In the `Drag(Left)` match: add a `DraggingColumns { .. }` arm:

```rust
            MouseDragState::DraggingColumns { .. } => match target {
                MouseTarget::ColHeader(c) | MouseTarget::Cell((_, c)) => {
                    Action::MouseSelectColumn(c)
                }
                _ => Action::Noop,
            },
```

- [ ] **Step 5: Wire visual-mode transition in `run_loop`**

In `main.rs::run_loop`, in the `Event::Mouse` arm, extend the visual transition logic:

```rust
                    match &action {
                        Action::MouseDragTo(_) if app.mode == Mode::Normal => {
                            if let mode::mouse::MouseDragState::DraggingCells { anchor } =
                                mouse_state.drag
                            {
                                visual_state =
                                    Some(VisualState::new(anchor, VisualKind::Character));
                                app.mode = Mode::Visual;
                            }
                        }
                        Action::MouseSelectColumn(_) => {
                            if let mode::mouse::MouseDragState::DraggingColumns { anchor_col } =
                                mouse_state.drag
                            {
                                let last_row = app.sheet.row_count.saturating_sub(1);
                                visual_state = Some(VisualState::new(
                                    (0, anchor_col),
                                    VisualKind::Block,
                                ));
                                app.cursor = (last_row, anchor_col);
                                app.mode = Mode::VisualBlock;
                            }
                        }
                        _ => {}
                    }
```

(Replace the `if let Action::MouseDragTo` block from Task 6 with this `match`.)

- [ ] **Step 6: Run tests, fmt, clippy, commit**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --all-features
cargo test
git add crates/cell-sheet-tui/src
git commit -m "feat(tui): click+drag on column header selects columns (#70)"
```

---

## Task 8: Click+drag on a row header → `MouseSelectRow`

Mirror image of Task 7.

**Files:**
- Modify: `crates/cell-sheet-tui/src/action.rs`
- Modify: `crates/cell-sheet-tui/src/app.rs`
- Modify: `crates/cell-sheet-tui/src/mode/mouse.rs`
- Modify: `crates/cell-sheet-tui/src/main.rs`

- [ ] **Step 1: Add `MouseSelectRow`**

```rust
    MouseSelectColumn(usize),
    MouseSelectRow(usize),
    SetStatus(String),
```

- [ ] **Step 2: Handle in `process_action`**

```rust
            Action::MouseSelectRow(row) => {
                let last_col = self.sheet.col_count.saturating_sub(1);
                self.cursor = (row, last_col);
                self.viewport.ensure_visible(self.cursor);
            }
```

- [ ] **Step 3: Failing handler tests**

```rust
#[test]
fn down_on_row_header_emits_select_row() {
    let app = App::new();
    let mut state = MouseState::new();
    let layout = fixture();
    let event = synth(MouseEventKind::Down(MouseButton::Left), 2, 4);
    assert_eq!(
        handle_mouse_event(event, &mut state, &app, Some(&layout)),
        Action::MouseSelectRow(3)
    );
    assert_eq!(state.drag, MouseDragState::DraggingRows { anchor_row: 3 });
}
```

- [ ] **Step 4: Implement the `RowHeader` and `DraggingRows` paths**

In `Down(Left)`:

```rust
            MouseTarget::RowHeader(r) => {
                state.drag = MouseDragState::DraggingRows { anchor_row: r };
                state.last_click = None;
                Action::MouseSelectRow(r)
            }
```

In `Drag(Left)`:

```rust
            MouseDragState::DraggingRows { .. } => match target {
                MouseTarget::RowHeader(r) | MouseTarget::Cell((r, _)) => {
                    Action::MouseSelectRow(r)
                }
                _ => Action::Noop,
            },
```

- [ ] **Step 5: Extend the `run_loop` visual transition**

In the `match &action` from Task 7, add:

```rust
                        Action::MouseSelectRow(_) => {
                            if let mode::mouse::MouseDragState::DraggingRows { anchor_row } =
                                mouse_state.drag
                            {
                                let last_col = app.sheet.col_count.saturating_sub(1);
                                visual_state = Some(VisualState::new(
                                    (anchor_row, 0),
                                    VisualKind::Block,
                                ));
                                app.cursor = (anchor_row, last_col);
                                app.mode = Mode::VisualBlock;
                            }
                        }
```

- [ ] **Step 6: Run tests, fmt, clippy, commit**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --all-features
cargo test
git add crates/cell-sheet-tui/src
git commit -m "feat(tui): click+drag on row header selects rows (#70)"
```

---

## Task 9: Scroll wheel → `MouseScroll`

Vertical scroll moves the viewport, cursor stays put. Horizontal via `ScrollLeft` / `ScrollRight` if the terminal emits them.

**Files:**
- Modify: `crates/cell-sheet-tui/src/action.rs`
- Modify: `crates/cell-sheet-tui/src/app.rs`
- Modify: `crates/cell-sheet-tui/src/mode/mouse.rs`

- [ ] **Step 1: Add `MouseScroll`**

In `action.rs`:

```rust
    MouseSelectRow(usize),
    /// Scroll the viewport. `dy > 0` scrolls toward higher row indices
    /// (i.e. content moves up, viewport reveals rows further down).
    /// `dx > 0` scrolls right. Cursor is not moved.
    MouseScroll { dx: i32, dy: i32 },
    SetStatus(String),
```

- [ ] **Step 2: Handle in `process_action`**

In `app.rs`:

```rust
            Action::MouseScroll { dx, dy } => {
                if dy > 0 {
                    self.viewport.row_offset =
                        self.viewport.row_offset.saturating_add(dy as usize);
                } else if dy < 0 {
                    self.viewport.row_offset =
                        self.viewport.row_offset.saturating_sub((-dy) as usize);
                }
                if dx > 0 {
                    self.viewport.col_offset =
                        self.viewport.col_offset.saturating_add(dx as usize);
                } else if dx < 0 {
                    self.viewport.col_offset =
                        self.viewport.col_offset.saturating_sub((-dx) as usize);
                }
            }
```

- [ ] **Step 3: Failing handler tests**

In `mode/mouse.rs::tests`:

```rust
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
fn scroll_does_not_move_cursor() {
    let mut app = App::new();
    app.cursor = (5, 2);
    app.process_action(Action::MouseScroll { dx: 0, dy: 3 });
    assert_eq!(app.cursor, (5, 2));
    assert_eq!(app.viewport.row_offset, 3);
}
```

- [ ] **Step 4: Implement the scroll arms**

Add a constant near the top of `mouse.rs`:

```rust
pub const MOUSE_SCROLL_LINES: i32 = 3;
```

Extend `handle_mouse_event`:

```rust
        MouseEventKind::ScrollDown => Action::MouseScroll { dx: 0, dy: MOUSE_SCROLL_LINES },
        MouseEventKind::ScrollUp => Action::MouseScroll { dx: 0, dy: -MOUSE_SCROLL_LINES },
        MouseEventKind::ScrollLeft => Action::MouseScroll { dx: -MOUSE_SCROLL_LINES, dy: 0 },
        MouseEventKind::ScrollRight => Action::MouseScroll { dx: MOUSE_SCROLL_LINES, dy: 0 },
```

- [ ] **Step 5: Add a test that disabling mouse mid-drag clears state**

The `mouse_capture_active` sync block in `run_loop` (Task 4) already calls `mouse_state = MouseState::new()` when the flag flips off. Verify it via a smaller direct test in `mode/mouse.rs::tests`:

```rust
#[test]
fn fresh_mouse_state_is_idle() {
    let s = MouseState::new();
    assert_eq!(s.drag, MouseDragState::Idle);
    assert!(s.last_click.is_none());
}
```

(The end-to-end "disable mid-drag" path is covered by the manual smoke test in Final Verification.)

- [ ] **Step 6: Run tests, fmt, clippy, commit**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --all-features
cargo test
git add crates/cell-sheet-tui/src
git commit -m "feat(tui): scroll wheel scrolls the viewport (#70)"
```

---

## Task 10: Edge auto-scroll on drag past the visible edge

When a `Drag(Left)` lands outside the grid in the direction of the drag, advance the viewport by one row/column so the next event has somewhere to land.

**Files:**
- Modify: `crates/cell-sheet-tui/src/mode/mouse.rs`

- [ ] **Step 1: Failing test**

```rust
#[test]
fn drag_past_bottom_advances_row_offset() {
    let mut app = App::new();
    // Build a sheet large enough that scrolling has somewhere to go.
    app.sheet.row_count = 100;
    app.sheet.col_count = 10;

    let mut state = MouseState::new();
    let layout = fixture();
    handle_mouse_event(
        synth(MouseEventKind::Down(MouseButton::Left), 8, 3),
        &mut state,
        &app,
        Some(&layout),
    );
    // y past the bottom of the grid (height=10, y_start=1 → bottom row at y=10)
    let drag_below = synth(MouseEventKind::Drag(MouseButton::Left), 8, 11);
    let action = handle_mouse_event(drag_below, &mut state, &app, Some(&layout));
    // The handler emits a MouseScroll(dy=+1) chained with MouseDragTo.
    // We model this as the handler returning a single MouseScroll and
    // using state to flag a deferred drag — but for simplicity in this
    // first implementation we ONLY emit the scroll when at the edge,
    // and rely on the next Drag event to land inside the freshly-revealed row.
    assert_eq!(action, Action::MouseScroll { dx: 0, dy: 1 });
}
```

- [ ] **Step 2: Implement edge detection**

Replace the `Drag(Left)` arm. When the drag lands outside the grid bounds, emit a one-row/one-column scroll instead of a drag. (Simpler than the spec's "advance offset then dispatch with new cell" — the next drag event lands correctly because the viewport has moved.)

```rust
        MouseEventKind::Drag(crossterm::event::MouseButton::Left) => {
            if state.drag == MouseDragState::Idle {
                return Action::Noop;
            }
            // Edge auto-scroll: a drag landing outside the grid in the
            // direction of motion advances the viewport one step.
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
```

- [ ] **Step 3: Run tests, fmt, clippy, commit**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --all-features
cargo test
git add crates/cell-sheet-tui/src
git commit -m "feat(tui): drag past edge auto-scrolls the viewport (#70)"
```

---

## Task 11: Mode-aware click — commit/cancel/exit before move; `Outside` is always Noop

A click on `Outside` is always a no-op (does NOT commit). A click on a real grid target while in `Insert` / `Command` / `Visual` first transitions back to Normal (committing or cancelling as appropriate), then dispatches the click action.

**Files:**
- Modify: `crates/cell-sheet-tui/src/main.rs`

This is a `run_loop` change, not a handler change: the handler is mode-agnostic; the `run_loop` already manages mode entry/exit state. We synthesize an Esc-equivalent before processing the mouse action.

- [ ] **Step 1: Update the `Event::Mouse` arm**

```rust
                Event::Mouse(me) => {
                    if !app.mouse_enabled {
                        continue;
                    }
                    app.status_message = None;
                    let layout = app.last_grid_layout.clone();
                    let action = mode::mouse::handle_mouse_event(
                        me,
                        &mut mouse_state,
                        app,
                        layout.as_ref(),
                    );

                    // Outside / Noop never causes a mode transition.
                    let is_grid_action = matches!(
                        &action,
                        Action::MouseClickCell(_)
                            | Action::MouseSelectColumn(_)
                            | Action::MouseSelectRow(_)
                    );
                    if is_grid_action {
                        match app.mode {
                            Mode::Insert => {
                                // Commit the in-progress edit, exactly the
                                // same path Enter takes today.
                                let pos = app.cursor;
                                let buf = std::mem::take(&mut app.insert_buffer);
                                app.process_action(Action::EditCell(pos, buf));
                                app.mode = Mode::Normal;
                            }
                            Mode::Command => {
                                app.command_line.clear();
                                app.command_history_idx = None;
                                app.command_history_scratch.clear();
                                app.mode = Mode::Normal;
                            }
                            Mode::Visual | Mode::VisualLine | Mode::VisualBlock => {
                                visual_state = None;
                                app.mode = Mode::Normal;
                            }
                            _ => {}
                        }
                    }

                    // Visual-mode-on-drag transition (from Tasks 6–8) goes here,
                    // unchanged.
                    match &action {
                        Action::MouseDragTo(_) if app.mode == Mode::Normal => {
                            if let mode::mouse::MouseDragState::DraggingCells { anchor } =
                                mouse_state.drag
                            {
                                visual_state =
                                    Some(VisualState::new(anchor, VisualKind::Character));
                                app.mode = Mode::Visual;
                            }
                        }
                        Action::MouseSelectColumn(_) => {
                            if let mode::mouse::MouseDragState::DraggingColumns { anchor_col } =
                                mouse_state.drag
                            {
                                visual_state = Some(VisualState::new(
                                    (0, anchor_col),
                                    VisualKind::Block,
                                ));
                                app.mode = Mode::VisualBlock;
                            }
                        }
                        Action::MouseSelectRow(_) => {
                            if let mode::mouse::MouseDragState::DraggingRows { anchor_row } =
                                mouse_state.drag
                            {
                                visual_state = Some(VisualState::new(
                                    (anchor_row, 0),
                                    VisualKind::Block,
                                ));
                                app.mode = Mode::VisualBlock;
                            }
                        }
                        _ => {}
                    }

                    app.process_action(action);
                }
```

- [ ] **Step 2: Run all tests, fmt, clippy**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --all-features
cargo test
```

- [ ] **Step 3: Commit**

```sh
git add crates/cell-sheet-tui/src/main.rs
git commit -m "feat(tui): click in Insert/Command/Visual exits the mode first (#70)"
```

---

## Task 12: Double-click on a cell → enter Insert mode

Implement double-click detection in `MouseState`. Two `Down(Left)` events on the same cell within `DOUBLE_CLICK_MS` fire `Action::ChangeMode(Mode::Insert)` instead of `MouseClickCell`.

**Files:**
- Modify: `crates/cell-sheet-tui/src/mode/mouse.rs`

- [ ] **Step 1: Failing test**

```rust
#[test]
fn double_click_enters_insert_mode() {
    let app = App::new();
    let mut state = MouseState::new();
    let layout = fixture();
    let event = synth(MouseEventKind::Down(MouseButton::Left), 8, 3);
    let _ = handle_mouse_event(event, &mut state, &app, Some(&layout));
    let event2 = synth(MouseEventKind::Down(MouseButton::Left), 8, 3);
    assert_eq!(
        handle_mouse_event(event2, &mut state, &app, Some(&layout)),
        Action::ChangeMode(crate::action::Mode::Insert)
    );
}

#[test]
fn click_then_different_cell_is_two_singles() {
    let app = App::new();
    let mut state = MouseState::new();
    let layout = fixture();
    let _ = handle_mouse_event(
        synth(MouseEventKind::Down(MouseButton::Left), 8, 3),
        &mut state,
        &app,
        Some(&layout),
    );
    let event2 = synth(MouseEventKind::Down(MouseButton::Left), 18, 5);
    assert_eq!(
        handle_mouse_event(event2, &mut state, &app, Some(&layout)),
        Action::MouseClickCell((3, 1))
    );
}
```

- [ ] **Step 2: Implement double-click detection**

In the `Down(Left)` arm of `handle_mouse_event`, before the existing `MouseTarget::Cell` arm:

```rust
            MouseTarget::Cell(pos) => {
                let now = Instant::now();
                let is_double = state
                    .last_click
                    .map(|lc| {
                        lc.pos == pos
                            && now.duration_since(lc.at) <= Duration::from_millis(DOUBLE_CLICK_MS)
                    })
                    .unwrap_or(false);
                if is_double {
                    state.drag = MouseDragState::Idle;
                    state.last_click = None;
                    Action::ChangeMode(Mode::Insert)
                } else {
                    state.drag = MouseDragState::DraggingCells { anchor: pos };
                    state.last_click = Some(LastClick { at: now, pos });
                    Action::MouseClickCell(pos)
                }
            }
```

Also clear `last_click` whenever `Down(Left)` lands on `ColHeader` / `RowHeader` (drag, scroll, or non-cell click) so a stale click can't combine with a much later one (already done in Task 7/8 for header clicks; verify and keep).

Add the `Mode` import:

```rust
use crate::action::{Action, Mode};
```

- [ ] **Step 3: Run tests, fmt, clippy, commit**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --all-features
cargo test
git add crates/cell-sheet-tui/src
git commit -m "feat(tui): double-click on a cell enters Insert mode (#70)"
```

---

## Task 13: Help-system entries — `MOUSE_ENTRIES` + `HelpCategory::Mouse`

Per AGENTS.md, every user-visible binding must have a `HelpEntry`.

**Files:**
- Modify: `crates/cell-sheet-core/src/help/mod.rs`
- Modify: `crates/cell-sheet-core/src/help/entries.rs`
- Read-check: `crates/cell-sheet-tui/src/render/help.rs`

- [ ] **Step 1: Add `Mouse` to `HelpCategory`**

In `crates/cell-sheet-core/src/help/mod.rs`:

```rust
pub enum HelpCategory {
    Normal,
    Insert,
    Visual,
    Command,
    Formula,
    Mouse,
}
```

```rust
    pub fn label(&self) -> &'static str {
        match self {
            HelpCategory::Normal => "NORMAL MODE",
            HelpCategory::Insert => "INSERT MODE",
            HelpCategory::Visual => "VISUAL MODE",
            HelpCategory::Command => "COMMANDS",
            HelpCategory::Formula => "FORMULAS",
            HelpCategory::Mouse => "MOUSE",
        }
    }
```

In `categories()`:

```rust
        let order = [Normal, Insert, Visual, Command, Formula, Mouse];
```

In `HelpRegistry::new()`:

```rust
        Self::from_entries(&[
            NORMAL_ENTRIES,
            INSERT_ENTRIES,
            VISUAL_ENTRIES,
            COMMAND_ENTRIES,
            FORMULA_ENTRIES,
            MOUSE_ENTRIES,
        ])
```

- [ ] **Step 2: Add `MOUSE_ENTRIES`**

In `crates/cell-sheet-core/src/help/entries.rs`, append:

```rust
pub static MOUSE_ENTRIES: &[HelpEntry] = &[
    HelpEntry {
        tags: &["mouse", ":set mouse"],
        category: HelpCategory::Mouse,
        summary: "Enable / disable mouse support",
        detail: "Mouse support is OFF by default. Toggle at runtime:\n\
                 \n\
                 :set mouse on        Enable mouse capture.\n\
                 :set mouse off       Disable mouse capture.\n\
                 :set mouse toggle    Flip the current state.\n\
                 \n\
                 When mouse mode is on, the terminal stops doing native\n\
                 text selection. Hold your terminal's bypass modifier to\n\
                 fall back to native selection for copy:\n\
                 \n\
                 - Linux & Windows Terminal: Shift\n\
                 - macOS Terminal.app and iTerm2: Option/Alt\n\
                 - tmux/screen: see your terminal's docs",
    },
    HelpEntry {
        tags: &["mouse-click", "click"],
        category: HelpCategory::Mouse,
        summary: "Left-click moves the cursor",
        detail: "Left-click on a grid cell moves the cursor there. From\n\
                 Insert mode the in-progress edit is committed first;\n\
                 from Command mode the prompt is cancelled; from Visual\n\
                 the selection is exited.\n\
                 \n\
                 Click on the formula bar, status bar, or padding around\n\
                 the grid is a no-op and never commits an edit.",
    },
    HelpEntry {
        tags: &["mouse-drag", "drag"],
        category: HelpCategory::Mouse,
        summary: "Click + drag selects a range",
        detail: "Drag inside the grid: enters Visual mode and extends the\n\
                 selection from the click cell to the current cell.\n\
                 \n\
                 Drag on a column header: selects whole columns.\n\
                 Drag on a row header: selects whole rows.\n\
                 \n\
                 Dragging past the visible edge auto-scrolls the\n\
                 viewport one row/column per drag event.",
    },
    HelpEntry {
        tags: &["mouse-scroll", "scroll-wheel", "wheel"],
        category: HelpCategory::Mouse,
        summary: "Scroll wheel scrolls the viewport",
        detail: "The scroll wheel scrolls the viewport up or down by 3\n\
                 rows. The cursor does not move, even if it scrolls out\n\
                 of view (matches Vim's mouse behaviour).\n\
                 \n\
                 Horizontal scroll (Shift+wheel on most terminals)\n\
                 scrolls the viewport left or right when the terminal\n\
                 emits ScrollLeft / ScrollRight events.",
    },
    HelpEntry {
        tags: &["mouse-double-click", "double-click", "edit-cell"],
        category: HelpCategory::Mouse,
        summary: "Double-click enters Insert mode",
        detail: "Two left-clicks on the same cell within ~400ms enter\n\
                 Insert mode on that cell. A second click on a different\n\
                 cell, or after the threshold, is treated as a fresh\n\
                 single click.",
    },
    HelpEntry {
        tags: &["mouse-bypass", "shift-click"],
        category: HelpCategory::Mouse,
        summary: "Shift+click bypasses mouse capture",
        detail: "Holding Shift while clicking is treated as a no-op by\n\
                 cell, allowing the terminal's native text selection to\n\
                 take over. Use this to copy a cell value or formula\n\
                 string out to your system clipboard. (On macOS\n\
                 Terminal.app and iTerm2 the bypass modifier is\n\
                 typically Option/Alt — check your terminal settings.)",
    },
];
```

- [ ] **Step 3: Verify the help renderer doesn't need a change**

```sh
rg "categories\(\)|HelpCategory::" crates/cell-sheet-tui/src/render/help.rs
```

If the renderer iterates `registry.categories()` (it should, per the existing pattern), no change is required. If it hard-codes the category list, add `HelpCategory::Mouse` there.

- [ ] **Step 4: Update the `full_registry_has_expected_tags` test**

In `crates/cell-sheet-core/src/help/mod.rs::tests::full_registry_has_expected_tags`:

```rust
        assert!(registry.find("mouse").is_some(), "missing mouse");
        assert!(registry.find("mouse-click").is_some(), "missing mouse-click");
        assert!(registry.find("mouse-scroll").is_some(), "missing mouse-scroll");
```

- [ ] **Step 5: Run tests, fmt, clippy, commit**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --all-features
cargo test
git add crates/cell-sheet-core/src/help crates/cell-sheet-tui/src/render/help.rs
git commit -m "docs(help): add MOUSE_ENTRIES and Mouse category (#70)"
```

---

## Task 14: README, AGENTS.md, CHANGELOG

Wrap up with the documentation surface.

**Files:**
- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: README — add a "Mouse support" subsection**

Find the existing "Usage" or "Keybindings" section in `README.md` and add a subsection. Place it after the keybindings list and before the file-formats section:

```markdown
## Mouse support

Mouse support is **off by default** so the terminal's native text
selection keeps working. Enable it at runtime:

```
:set mouse on        # enable
:set mouse off       # disable
:set mouse toggle    # flip
```

When enabled:

- **Left-click** on a cell moves the cursor.
- **Click + drag** inside the grid selects a Visual range.
- **Click + drag** on a column header selects whole columns.
- **Click + drag** on a row header selects whole rows.
- **Scroll wheel** scrolls the viewport (cursor stays put). Horizontal
  scroll works when the terminal emits `ScrollLeft` / `ScrollRight`
  (commonly bound to Shift + wheel).
- **Double-click** a cell to enter Insert mode on it.
- **Drag past the visible edge** auto-scrolls the viewport.

To copy a cell value out to your system clipboard while mouse mode is
on, hold your terminal's bypass modifier when clicking and dragging:

| Terminal | Bypass |
| --- | --- |
| Linux terminals (gnome-terminal, alacritty, kitty, …) | Shift |
| Windows Terminal | Shift |
| macOS Terminal.app, iTerm2 | Option/Alt |
| tmux / screen | configure per their docs |
```

- [ ] **Step 2: AGENTS.md — add a one-liner under "Things to avoid"**

In the existing `### Things to avoid` list, add:

```markdown
- Enabling mouse capture unconditionally. Mouse support is opt-in via
  the `mouse_enabled` runtime flag set by `:set mouse on`; do not
  bypass it.
```

- [ ] **Step 3: CHANGELOG — add an entry under `## Unreleased`**

```markdown
## Unreleased

### Added

- Optional mouse support (off by default; enable with `:set mouse on`).
  Left-click moves the cursor; click+drag selects a range; clicks on
  column/row headers select whole columns/rows; scroll wheel scrolls
  the viewport without moving the cursor; double-click enters Insert
  mode. Hold Shift (or your terminal's bypass modifier) to use native
  text selection for copy. (#70)
```

- [ ] **Step 4: Run final verification**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --all-features
cargo test
```

Expected: clean across the board.

- [ ] **Step 5: Commit**

```sh
git add README.md AGENTS.md CHANGELOG.md
git commit -m "docs: document mouse support (#70)"
```

---

## Final verification

- [ ] **Run the full pre-commit pipeline one more time**

Per AGENTS.md, before claiming work is done, run all three:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --all-features
cargo test
```

Each must pass clean (CI uses `RUSTFLAGS=-Dwarnings`).

- [ ] **Smoke test the binary by hand**

```sh
cargo run -- examples/sample.csv  # or any test sheet
```

Then in the TUI:

1. `:set mouse on` — status line should not error.
2. Click a cell several rows/columns away — cursor should jump there.
3. Click and drag across a few cells — should enter Visual and select the range.
4. Click on a column header — should select the whole column (VisualBlock).
5. Click on a row header — should select the whole row.
6. Scroll the wheel — viewport should scroll, cursor should not move.
7. Double-click a cell — should enter Insert mode.
8. Press Esc, then Shift+click — should be a no-op (terminal selection works).
9. `:set mouse off` — clicks should stop having any effect.
10. `:q` — terminal should exit cleanly with no lingering mouse capture.

- [ ] **All acceptance criteria from the spec are now true:**

Cross-reference [`docs/superpowers/specs/2026-04-30-mouse-support-design.md`](../specs/2026-04-30-mouse-support-design.md) "Acceptance criteria" section. Each box should be checkable.
