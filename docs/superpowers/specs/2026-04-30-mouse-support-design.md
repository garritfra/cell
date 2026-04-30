# Mouse Support — Design Spec

**Issue:** [#70](https://github.com/garritfra/cell/issues/70)
**Date:** 2026-04-30
**Status:** Approved, ready for implementation plan

---

## Background

cell is a keyboard-first, Vim-modal terminal spreadsheet. Today, every
interaction goes through `event::read` → `Event::Key` → a per-mode handler
in `crates/cell-sheet-tui/src/mode/`. There is no mouse capture, no
config-file infrastructure, and no notion of "where on the screen does
this cell live" outside of the render layer.

The geometry is owned by two places:

- `crates/cell-sheet-tui/src/render/grid.rs` — the column-header row,
  per-column widths via `sheet.col_widths`, the row-number gutter
  (`ROW_NUM_WIDTH = 5`), and where the grid is drawn within the frame.
- `crates/cell-sheet-tui/src/viewport.rs` — `row_offset` / `col_offset`
  for scrolling.

Adding mouse support is fundamentally an exercise in (a) capturing mouse
events and (b) translating screen coordinates back into logical
`(row, col)` cell positions using that geometry.

---

## Goals

1. **Opt-in mouse mode** — off by default, enabled via `:set mouse on`.
2. **Left-click** moves the cursor to the clicked cell.
3. **Click + drag** selects a range:
   - Inside the grid → Visual (Character) selection from anchor cell to
     current cell.
   - On a column header → whole-column range from anchor column to
     current column.
   - On a row header → whole-row range from anchor row to current row.
4. **Drag past visible edge** auto-scrolls the viewport one row/column
   per drag event.
5. **Scroll wheel** scrolls the viewport without moving the cursor.
   Horizontal scroll works on terminals that emit `ScrollLeft` /
   `ScrollRight` (e.g. Shift+wheel on most terminals).
6. **Double-click** on a cell enters Insert mode on it.
7. **Click while in another mode** transitions back to Normal first,
   then moves the cursor: Insert commits the in-progress edit, Command
   cancels the prompt, Visual exits the selection. (A click on
   `Outside` is always a no-op and never commits anything.)
8. **Shift+click** is a no-op so the terminal's native text selection
   keeps working for copy.
9. `:help mouse`, README, AGENTS.md, and CHANGELOG are updated.

## Non-goals

Explicitly deferred to follow-up issues:

- Drag column borders to resize columns.
- Right-click context menu.
- Click on status-line / mode indicator elements.
- Drag-to-select inside the cell editor while in Insert mode (mouse only
  acts at grid level for now).
- Persistent config file (e.g. `~/.config/cell/config.toml`). The only
  configuration mechanism is the runtime `:set` command, matching the
  existing `:set delimiter=` precedent.
- Legacy mouse protocols (X10, normal). Crossterm's `EnableMouseCapture`
  enables SGR 1006 by default, which works in every terminal we
  reasonably target.

---

## Approach

A new `mode/mouse.rs` module mirroring the keyboard-mode handlers, plus
a small `GridLayout` struct published from the render layer for
hit-testing. Three reasons for this shape:

1. **Pure hit-test seam.** `hit_test(layout, x, y) → MouseTarget` is a
   pure function — no terminal, no IO, fully unit-testable.
2. **Mode-handler symmetry.** `mode/mouse.rs` translates a `MouseEvent`
   into an `Action` exactly the way `mode/normal.rs` translates a
   `KeyEvent`. `run_loop` gains one new arm and stays a router.
3. **Single source of geometry.** The render layer already owns column
   widths and offsets; publishing a `GridLayout` per frame is cheaper and
   safer than duplicating that math in the event handler.

### Module layout

```
crates/cell-sheet-tui/src/
├─ main.rs                # EnableMouseCapture / DisableMouseCapture;
│                         # route Event::Mouse to mode::mouse::handle_mouse_event
├─ app.rs                 # add `mouse_enabled: bool`, last-frame `GridLayout`
├─ render/grid.rs         # publish GridLayout (area, header_height,
│                         # row_num_width, row_offset, col_offset,
│                         # visible_cols: Vec<(col_index, x, width)>)
├─ mode/mouse.rs          # NEW: MouseDragState, hit_test, handle_mouse_event
├─ mode/command.rs        # extend parse_command() with `set mouse on|off|toggle`
└─ action.rs              # add Action variants (see below)
```

### `MouseTarget`

```rust
pub enum MouseTarget {
    Cell(CellPos),
    ColHeader(usize),     // column index
    RowHeader(usize),     // 0-indexed row
    Outside,              // formula bar, status bar, command line, padding
}
```

### `MouseState`

Lives in `mode/mouse.rs` and is owned by `run_loop` exactly the way
`NormalState` and `Option<VisualState>` are today. Bundles drag state
and double-click detection (crossterm does not expose a click-count;
we track it ourselves):

```rust
pub struct MouseState {
    drag: MouseDragState,
    last_click: Option<LastClick>,
}

enum MouseDragState {
    Idle,
    DraggingCells   { anchor: CellPos },
    DraggingColumns { anchor_col: usize },
    DraggingRows    { anchor_row: usize },
}

struct LastClick {
    at: std::time::Instant,
    pos: CellPos,
}
```

A `Down(Left)` event qualifies as a double-click if `last_click` is
`Some` and:

- `now - at <= DOUBLE_CLICK_MS` (400ms; matches typical OS defaults), and
- the click landed on the same `CellPos`.

After dispatch, `last_click` is updated to the current click. A drag,
scroll, or non-cell click clears `last_click` to `None` so a stale
single-click can never combine with a much later one to spuriously
trigger Insert mode.

### New `Action` variants

```rust
Action::SetMouse(bool),                  // process via App::process_action
Action::MouseClickCell(CellPos),         // also commits/cancels prior mode
Action::MouseDragTo(CellPos),            // extends current Visual selection
Action::MouseSelectColumn(usize),
Action::MouseSelectRow(usize),
Action::MouseScroll { dx: i32, dy: i32 }, // viewport-only; cursor untouched
```

`MouseClickCell`, `MouseSelectColumn`, and `MouseSelectRow` reuse
existing range/visual primitives in `App::process_action` rather than
introducing parallel selection logic.

---

## Data flow

### Enable / disable

`:set mouse on|off|toggle` parses to `Action::SetMouse(bool)`.
`App::process_action` flips `app.mouse_enabled`. Whenever the flag
changes, `run_loop` issues `execute!(stdout, EnableMouseCapture)` or
`DisableMouseCapture`. Mouse is **off on startup** so existing users see
no behavior change until they opt in.

`run_tui` issues `DisableMouseCapture` symmetrically with
`LeaveAlternateScreen` on shutdown so mouse capture never leaks past
process exit. If `EnableMouseCapture` returns an `io::Error`, surface
via `Action::SetStatus("failed to enable mouse: <err>")` and leave
`mouse_enabled = false`.

### Per-event flow (when `mouse_enabled == true`)

1. `event::read()` returns `Event::Mouse(me)`.
2. `run_loop` reads the most recent `GridLayout` saved during the prior
   `terminal.draw`.
3. Calls `mode::mouse::handle_mouse_event(me, &mut mouse_state, &mut app, &layout)`.
4. The handler:
   1. **Bypass passthrough.** If `me.modifiers.contains(SHIFT)`, return
      `Action::Noop` immediately so the terminal's native selection
      wins.
   2. Runs `hit_test(layout, me.column, me.row) → MouseTarget`.
   3. Switches on `me.kind`:
      - `Down(Left)` →
        - If `target == Outside` → `Noop`. (Click on padding, status
          bar, or formula bar never commits an in-progress edit; only
          Esc or a real grid click does.)
        - If `target` is a `Cell(pos)` and the click qualifies as a
          double-click (see `MouseState` above), dispatch
          `Action::ChangeMode(Mode::Insert)` on `pos`. (No anchor armed
          for drag — the user is editing, not selecting.)
        - Otherwise, prepare a single `Action`:
          - In `Insert` mode: emit a synthetic commit (same code path
            as Enter today) before the click action — implemented as a
            small two-step action sequence in `run_loop` rather than a
            new compound `Action`.
          - In `Command` mode: cancel the prompt (clear `command_line`,
            return to Normal) before the click action.
          - In `Visual` / `VisualLine` / `VisualBlock` mode: exit Visual
            (clear `visual_state`, return to Normal) before the click
            action. Mirrors Vim's `mouse=a`.
          - Then dispatch by target: `Cell(pos)` ⇒ `MouseClickCell(pos)`
            and arm `DraggingCells { anchor: pos }`; `ColHeader(c)` ⇒
            `MouseSelectColumn(c)` and arm `DraggingColumns`;
            `RowHeader(r)` ⇒ `MouseSelectRow(r)` and arm `DraggingRows`.
      - `Drag(Left)` →
        - If drag is `Idle`, treat as `Noop` (a stray drag event with no
          armed anchor — should not happen in practice).
        - Otherwise dispatch `MouseDragTo(target_pos)` (or
          column/row-extend variants); the app converts this to a
          Visual selection from `anchor` to `target_pos`.
        - **Edge auto-scroll**: if `me.row` is past the last visible
          data row (or `me.column` past the last visible column),
          advance `viewport.row_offset` / `col_offset` by 1 *before*
          computing the target, then dispatch with the now-visible
          cell. Bounded to one row/column per event.
      - `Up(Left)` → reset `MouseDragState` to `Idle`. The visual
        selection persists exactly the way it does after releasing a
        keyboard `v`+motion sequence.
      - `ScrollDown` / `ScrollUp` → `Action::MouseScroll { dy: ±3, dx: 0 }`.
        `MOUSE_SCROLL_LINES = 3` (matches common terminal/Vim defaults).
        Cursor stays put even if it leaves the visible area.
      - `ScrollLeft` / `ScrollRight` → same with `dx: ±3, dy: 0`. On
        terminals that don't emit these, horizontal scrolling silently
        doesn't work.
5. The returned `Action` flows through the existing
   `App::process_action` pipeline; mode transitions, undo coalescing,
   and viewport recentering reuse current paths unchanged.

`App::process_action` gains arms for the new variants:

- `Action::SetMouse(b)` → set `app.mouse_enabled = b`. Also resets
  `MouseState` to `Idle` if `b == false`. The actual
  `Enable/DisableMouseCapture` syscall is issued by `run_loop` after
  observing the flag change (because `process_action` does not own
  stdout).
- `Action::MouseClickCell(pos)` → set `app.cursor = pos`,
  `viewport.ensure_visible(pos)`. (No selection.)
- `Action::MouseDragTo(pos)` → set `app.cursor = pos`,
  `viewport.ensure_visible(pos)`. The active `VisualState` (created on
  the prior `Down`) extends naturally because Visual selection is
  derived from `anchor` and `cursor`.
- `Action::MouseSelectColumn(c)` / `MouseSelectRow(r)` → enter
  `VisualBlock` with anchor and cursor spanning the full column or row,
  reusing the existing whole-column/whole-row selection paths if any
  exist; otherwise build the equivalent `VisualState` directly.
- `Action::MouseScroll { dx, dy }` → mutate `viewport.row_offset` and
  `viewport.col_offset` only. Cursor is **not** moved, even if it
  scrolls out of view (matches Vim).

---

## Configuration

| Knob                | Mechanism                          | Default | Persisted |
| ------------------- | ---------------------------------- | ------- | --------- |
| Mouse enabled       | `:set mouse on \| off \| toggle`   | `off`   | No (session-only) |

`set mouse <garbage>` is a soft error:
`Action::SetStatus("usage: :set mouse on|off|toggle")`. Same pattern as
the existing `:set delimiter=` errors in `parse_command`.

---

## Bypass-modifier strategy

Terminal text selection is the user's safety net for copying values out
of a cell or formula bar to paste elsewhere. When mouse capture is on,
most terminals stop doing native selection. The bypass modifier
restores it; which key serves as the bypass is terminal-specific.

Strategy:

1. **App-side passthrough.** When mouse mode is on and Shift is held,
   `handle_mouse_event` returns `Action::Noop`. Belt-and-suspenders:
   even if the terminal already does the right thing on Shift+click,
   the app does not consume the event.
2. **Documentation.** README and `:help mouse` list the bypass keys for
   common terminals (Linux: Shift; macOS Terminal/iTerm: Option/Alt;
   Windows Terminal: Shift; tmux/screen: configure
   `set -g mouse on` and use the terminal's bypass *plus* tmux's
   `copy-mode`).

---

## Edge cases

- **Terminals without mouse support.** `EnableMouseCapture` succeeds
  silently; no `Event::Mouse` ever arrives. No special detection
  needed.
- **Click on an empty row past populated data.** Cursor moves there,
  consistent with `j`-past-data behavior today.
- **Click on the top-left corner cell** (header row × row-number
  gutter) → `Outside`, no-op.
- **Mouse event in Help mode.** Always `Outside`; user must `q` out.
- **Resize between events.** `GridLayout` is rebuilt each frame, so
  the next mouse event uses fresh geometry. A click that arrives during
  a resize-in-flight may map to a stale cell once; acceptable.
- **Disabling mouse mid-drag.** Processing `Action::SetMouse(false)`
  resets `MouseDragState` to `Idle` *before* issuing
  `DisableMouseCapture` so no dangling drag anchor survives.
- **Restore on exit / panic.** `run_tui`'s cleanup block already
  disables raw mode and leaves the alternate screen; we add
  `DisableMouseCapture` to that block. (A panic during the loop still
  passes through this cleanup because `run_loop` returns a `Result`
  rather than panicking.)

---

## Testing

### Unit tests (no terminal needed)

`mode/mouse.rs::hit_test_*` — at minimum:

1. Click in a normal data cell.
2. Click on a column header.
3. Click on a row header.
4. Click on the top-left corner (header × gutter) → `Outside`.
5. Click in formula bar / status bar / command line → `Outside`.
6. Click in padding to the right of the last visible column → `Outside`.
7. Click below the last rendered row → `Outside`.
8. Click on a column with non-default width (verifies the per-column
   width math, not just `DEFAULT_COL_WIDTH`).

`mode/mouse.rs::handle_mouse_event_*`:

1. Cell click in Normal → `MouseClickCell`, drag state armed.
2. Cell drag in Normal → drag state transitions to `DraggingCells`,
   emits `MouseDragTo(target)`.
3. Column-header drag → emits `MouseSelectColumn` and extends to a
   range on subsequent drag events.
4. Double-click on a cell → `ChangeMode(Insert)`. Two `Down(Left)`
   events on the same cell within `DOUBLE_CLICK_MS` qualify; spaced
   further apart, they're two single clicks.
5. Two clicks on *different* cells within the threshold → two single
   clicks, not a double-click.
6. Scroll wheel → `MouseScroll { dy: ±3 }`, cursor unchanged even when
   it leaves the visible area.
7. Shift+click → `Noop`.
8. Click on `Outside` while in Insert mode → `Noop`, buffer is *not*
   committed.
9. Cell click while `Mode::Insert` → commit-then-move sequence.
10. Cell click while `Mode::Command` → cancel-then-move sequence.
11. Cell click while `Mode::Visual` → exit-Visual-then-move sequence.
12. Drag past visible edge → viewport advances one row/column, target
    uses the new visible cell.
13. `Action::SetMouse(false)` while drag is armed resets drag state to
    `Idle`.

`mode/command.rs::parse_command`:

1. `set mouse on`.
2. `set mouse off`.
3. `set mouse toggle`.
4. `set mouse bogus` → status error, no flag change.

### Integration smoke

Driving `App` with synthetic `MouseEvent`s constructed in code (no
terminal required). Asserts cursor position, viewport offsets, and
selection state after sequences of clicks, drags, and scrolls. Lives
in `crates/cell-sheet-tui/src/mode/mouse.rs` under `#[cfg(test)]` to
match the existing pattern.

CI on Linux/macOS/Windows already exercises the build with mouse code
paths compiled (crossterm is cross-platform). No new platform gates.

---

## Help-system updates

Per AGENTS.md, every user-visible binding must have a `HelpEntry`.

- Add `MOUSE_ENTRIES` in
  `crates/cell-sheet-core/src/help/entries.rs` with one entry per:
  enable/disable (`:set mouse on|off|toggle`), left-click, click+drag
  in grid / column header / row header, scroll wheel, double-click to
  edit, Shift+click bypass.
- Add a `Mouse` category to `HelpRegistry` in
  `crates/cell-sheet-core/src/help/mod.rs`.
- Render the new section in `crates/cell-sheet-tui/src/render/help.rs`.
- `:help mouse` topic resolves to the new category.

---

## Documentation

- **README.md** — new "Mouse support" subsection under Usage:
  opt-in via `:set mouse on`; list MVP interactions; the
  bypass-modifier note (Shift on most terminals; Option/Alt on macOS
  Terminal/iTerm; tmux/screen need their own bypass key); horizontal
  scroll depends on terminal support.
- **AGENTS.md** — short note in the *Things to avoid* list:
  "Do not enable mouse capture unconditionally; mouse is opt-in via the
  `mouse_enabled` runtime flag."
- **CHANGELOG.md** — `## Unreleased` → `Added`:
  > Optional mouse support (off by default; enable with
  > `:set mouse on`). Left-click moves the cursor; click+drag selects
  > a range; header clicks select rows/columns; scroll wheel scrolls
  > the viewport; double-click enters Insert mode. (#70)

---

## Acceptance criteria

- [ ] Mouse is off by default; opt-in via `:set mouse on` (also `off`,
      `toggle`).
- [ ] Left-click on a grid cell moves the cursor there, including from
      Insert mode (with commit), Command mode (with cancel), and
      Visual mode (with selection exit). Left-click on padding /
      formula bar / status bar is a no-op and does not commit.
- [ ] Click+drag inside the grid creates a Visual (Character) range
      from the click-down cell to the current cell.
- [ ] Click+drag on a column header selects whole columns from anchor
      to current.
- [ ] Click+drag on a row header selects whole rows from anchor to
      current.
- [ ] Drag past the visible edge auto-scrolls the viewport one
      row/column per event.
- [ ] Scroll wheel scrolls the viewport without moving the cursor;
      horizontal wheel scrolls horizontally on terminals that emit
      `ScrollLeft` / `ScrollRight`.
- [ ] Double-click on a cell enters Insert mode on it.
- [ ] Holding Shift while clicking is a `Noop` so the terminal's
      native selection works for copy.
- [ ] Mouse capture is properly disabled on `:q`, `:set mouse off`,
      and any normal exit.
- [ ] `:help mouse` lists every binding above; entries also appear in
      the Help screen.
- [ ] README, AGENTS.md, and CHANGELOG updated.
