# Vim Quick-Wins Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land six small, mechanical vim motions (issues #30, #31, #32, #35, #36, #37) in a single PR on `feat/vim-quickwins`.

**Architecture:** Each motion is one or two new `Action` variants dispatched from `NormalState::handle_key`, handled in `App::process_action`. State that persists across actions (marks, jump list, last visual) lives on `App`. Tests live alongside in `#[cfg(test)]` modules in `app.rs` or the relevant mode file — pure Rust logic, no terminal needed.

**Tech Stack:** Rust workspace (`cell-sheet-core`, `cell-sheet-tui`), `crossterm` for key events, no new deps.

---

## Conventions for every task

- TDD: write the test, run it, see it fail, then implement.
- Each task ends with one commit (Conventional Commits, `feat:` or `fix:`).
- After each task, run `cargo test -p cell-sheet-tui` to confirm green.
- After all six tasks, run `cargo fmt --all`, `cargo clippy --workspace --all-targets --all-features`, `cargo test`. All three must be clean.
- Add a `Changed`/`Added` entry to `CHANGELOG.md` after each task is in.

## File map

| File | Touched by |
|------|------------|
| `crates/cell-sheet-tui/src/action.rs` | every task — new `Action` variants |
| `crates/cell-sheet-tui/src/app.rs` | every task — handler logic, new `App` fields |
| `crates/cell-sheet-tui/src/viewport.rs` | #30 only — viewport math helpers |
| `crates/cell-sheet-tui/src/mode/normal.rs` | every task — keybindings + new pending sequences |
| `crates/cell-sheet-tui/src/main.rs` | #37 only — re-create `VisualState` from `last_visual` |
| `CHANGELOG.md` | once at the end |

---

## Task 1: Viewport navigation (#30) — `zz`/`zt`/`zb`, `H`/`M`/`L`, `Ctrl-e`/`Ctrl-y`

**Files:**
- Modify: `crates/cell-sheet-tui/src/action.rs` — add 8 actions
- Modify: `crates/cell-sheet-tui/src/viewport.rs` — add `center_on`, `top_on`, `bottom_on`
- Modify: `crates/cell-sheet-tui/src/app.rs` — handle new actions
- Modify: `crates/cell-sheet-tui/src/mode/normal.rs` — bindings + `pending: Some('z')` branch

**Actions:**

```rust
ScrollCursorTop,        // zt
ScrollCursorCenter,     // zz
ScrollCursorBottom,     // zb
CursorToViewportTop,    // H
CursorToViewportMiddle, // M
CursorToViewportBottom, // L
ScrollLineDown,         // Ctrl-e
ScrollLineUp,           // Ctrl-y
```

- [ ] **Step 1: Test for `zz` (center)** in `viewport.rs`

```rust
#[test]
fn center_on_places_cursor_in_middle() {
    let mut vp = Viewport::new();
    vp.visible_rows = 10;
    vp.center_on(50);
    assert_eq!(vp.row_offset, 45);
}

#[test]
fn center_on_clamps_to_zero() {
    let mut vp = Viewport::new();
    vp.visible_rows = 10;
    vp.center_on(2);
    assert_eq!(vp.row_offset, 0);
}
```

- [ ] **Step 2: Implement `center_on`/`top_on`/`bottom_on`** in `viewport.rs`

```rust
pub fn top_on(&mut self, row: usize) {
    self.row_offset = row;
}
pub fn center_on(&mut self, row: usize) {
    let half = self.visible_rows / 2;
    self.row_offset = row.saturating_sub(half);
}
pub fn bottom_on(&mut self, row: usize) {
    self.row_offset = (row + 1).saturating_sub(self.visible_rows);
}
```

- [ ] **Step 3: Test for `Ctrl-e` and `Ctrl-y`** in `app.rs`

```rust
#[test]
fn scroll_line_down_moves_viewport_only() {
    let mut app = App::new();
    app.viewport.visible_rows = 10;
    app.cursor = (5, 0);
    app.process_action(Action::ScrollLineDown);
    assert_eq!(app.viewport.row_offset, 1);
    assert_eq!(app.cursor, (5, 0));
}
```

- [ ] **Step 4: Test for `H`/`M`/`L`** — assert cursor lands on the correct row given a viewport.

- [ ] **Step 5: Implement all 8 actions in `app.rs`** — straightforward viewport math.

- [ ] **Step 6: Test for `pending = 'z'` flow** in `normal.rs` — `z` then `z` produces `ScrollCursorCenter`; `z` then `t` produces `ScrollCursorTop`; `z` then `q` (unknown) produces `Noop`.

- [ ] **Step 7: Wire keybindings in `normal.rs`.** Extend the `pending` match with the `z` arm; bind `H`, `M`, `L`; add `Ctrl-e` and `Ctrl-y` to the Ctrl branch.

- [ ] **Step 8: Run `cargo test -p cell-sheet-tui` — must pass.**

- [ ] **Step 9: Commit.** `feat: add zz/zt/zb, H/M/L, Ctrl-e/Ctrl-y viewport motions (#30)`

## Task 2: Marks (#31) — `m{a-z}`, `'{a-z}`, `` `{a-z} ``

**Files:**
- Modify: `crates/cell-sheet-tui/src/action.rs`
- Modify: `crates/cell-sheet-tui/src/app.rs` — add `marks: HashMap<char, CellPos>` field
- Modify: `crates/cell-sheet-tui/src/mode/normal.rs`

**Actions:**

```rust
SetMark(char),
JumpToMark { name: char, line_wise: bool },
```

- [ ] **Step 1: Test setting a mark** in `app.rs`:

```rust
#[test]
fn set_mark_records_cursor_position() {
    let mut app = App::new();
    app.cursor = (5, 3);
    app.process_action(Action::SetMark('a'));
    assert_eq!(app.marks.get(&'a'), Some(&(5, 3)));
}
```

- [ ] **Step 2: Test cell-wise mark jump:**

```rust
#[test]
fn backtick_jump_returns_to_exact_cell() {
    let mut app = App::new();
    app.cursor = (5, 3);
    app.process_action(Action::SetMark('a'));
    app.cursor = (0, 0);
    app.process_action(Action::JumpToMark { name: 'a', line_wise: false });
    assert_eq!(app.cursor, (5, 3));
}
```

- [ ] **Step 3: Test line-wise mark jump (column reset to 0):**

```rust
#[test]
fn apostrophe_jump_returns_to_marked_row_column_zero() {
    let mut app = App::new();
    app.cursor = (5, 3);
    app.process_action(Action::SetMark('a'));
    app.cursor = (0, 0);
    app.process_action(Action::JumpToMark { name: 'a', line_wise: true });
    assert_eq!(app.cursor, (5, 0));
}
```

- [ ] **Step 4: Test unset mark is a no-op + status message:**

```rust
#[test]
fn jump_to_unset_mark_shows_status_and_keeps_cursor() {
    let mut app = App::new();
    app.cursor = (1, 1);
    app.process_action(Action::JumpToMark { name: 'q', line_wise: false });
    assert_eq!(app.cursor, (1, 1));
    assert!(app.status_message.as_deref().unwrap().contains("Mark not set"));
}
```

- [ ] **Step 5: Implement.** Add `marks: HashMap<char, CellPos>` to `App`, initialize empty in `new`. Reject non-`a..=z` chars in handlers (no-op + status).

- [ ] **Step 6: Test pending sequences in `normal.rs`** — `m` then `a` produces `SetMark('a')`; `'` then `a` produces `JumpToMark { line_wise: true }`; `` ` `` then `a` produces `JumpToMark { line_wise: false }`.

- [ ] **Step 7: Wire `pending = 'm' | '\'' | '`'`** in `normal.rs`. Reject any non-`a..=z` follow-up with `Noop`.

- [ ] **Step 8: `cargo test -p cell-sheet-tui` — green.**

- [ ] **Step 9: Commit.** `feat: add marks (m/'/\`) for bookmarking and jumping (#31)`

## Task 3: Jump list (#32) — `Ctrl-o` / `Ctrl-i`

**Files:**
- Modify: `crates/cell-sheet-tui/src/action.rs` — add `JumpBack`, `JumpForward`
- Modify: `crates/cell-sheet-tui/src/app.rs` — add `jump_list: Vec<CellPos>`, `jump_idx: usize`, `record_jump` helper, push from each big-jump action
- Modify: `crates/cell-sheet-tui/src/mode/normal.rs`

The "big jumps" that push to the list (entry = position **before** the jump):
- `GotoFirstRow`, `GotoLastRow`
- `JumpToMark` (any)
- `Search`, `SearchNext`, `SearchPrev`
- `MoveCursorTo` (currently dead, but future-proof)

`MoveCursor` (single-step `hjkl`) does **not** push.

- [ ] **Step 1: Test that `Ctrl-o` walks back to the previous position:**

```rust
#[test]
fn ctrl_o_returns_to_position_before_jump() {
    let mut app = App::new();
    app.sheet.row_count = 100;
    app.cursor = (0, 0);
    app.process_action(Action::GotoLastRow);
    assert_eq!(app.cursor, (99, 0));
    app.process_action(Action::JumpBack);
    assert_eq!(app.cursor, (0, 0));
}
```

- [ ] **Step 2: Test that `Ctrl-i` walks forward again:**

```rust
#[test]
fn ctrl_i_returns_to_jumped_position() {
    let mut app = App::new();
    app.sheet.row_count = 100;
    app.cursor = (0, 0);
    app.process_action(Action::GotoLastRow);
    app.process_action(Action::JumpBack);
    assert_eq!(app.cursor, (0, 0));
    app.process_action(Action::JumpForward);
    assert_eq!(app.cursor, (99, 0));
}
```

- [ ] **Step 3: Test that the list is capped:**

```rust
#[test]
fn jump_list_capped_at_100_entries() {
    let mut app = App::new();
    app.sheet.row_count = 200;
    for r in 0..150 {
        app.cursor = (r, 0);
        app.process_action(Action::GotoLastRow);
    }
    assert!(app.jump_list.len() <= 100);
}
```

- [ ] **Step 4: Implement.** Helper `record_jump(&mut self)` pushes `self.cursor` to the list, truncating to 100. New `Action::JumpBack`/`Action::JumpForward`. Each "big-jump" action calls `record_jump` **before** changing cursor. `JumpBack` decrements idx and seeks; `JumpForward` increments idx.

  Detail: maintain `jump_list` as a Vec. When recording mid-stack (after some Ctrl-o), truncate everything after `jump_idx` first, then push, then move idx to the end (vim-faithful).

- [ ] **Step 5: Test that `Ctrl-i` (received as `Tab`) is bound** in `normal.rs`:

```rust
#[test]
fn tab_is_bound_to_jump_forward() {
    let app = App::new();
    let mut state = NormalState::new();
    assert_eq!(
        state.handle_key(key(KeyCode::Tab), &app),
        Action::JumpForward
    );
}
```

  Note the bug magnet: `Ctrl-i` is reported as `KeyCode::Tab`, not `KeyCode::Char('i')` with Ctrl modifier.

- [ ] **Step 6: Wire bindings.** `Ctrl-o` → `JumpBack` in the Ctrl branch; `KeyCode::Tab` → `JumpForward` at top level.

- [ ] **Step 7: `cargo test -p cell-sheet-tui` — green.**

- [ ] **Step 8: Commit.** `feat: add jump list (Ctrl-o / Ctrl-i) for cursor history (#32)`

## Task 4: Block jump (#35) — `{` / `}`

**Files:**
- Modify: `crates/cell-sheet-tui/src/action.rs` — add `BlockJumpUp`, `BlockJumpDown`
- Modify: `crates/cell-sheet-tui/src/app.rs`
- Modify: `crates/cell-sheet-tui/src/mode/normal.rs`

Definition: a **block** is a maximal run of consecutive non-empty cells in the current column. `}` from inside a block jumps to the first **empty** row at-or-after the block's last non-empty row. `}` from an empty cell jumps to the first non-empty row at-or-below.

Algorithm (down):
1. If current cell is non-empty, walk down until we find an empty cell (or hit `row_count`). That's the destination.
2. If current cell is empty, walk down until we find a non-empty cell. That's the destination.
3. If we hit `row_count`, no-op.

`{` is the symmetric inverse.

- [ ] **Step 1: Tests.**

```rust
#[test]
fn block_jump_down_from_inside_block_to_first_empty() {
    let mut app = App::new();
    for r in 0..3 {
        app.process_action(Action::EditCell((r, 0), "x".into()));
    }
    // row 3 is empty
    app.process_action(Action::EditCell((4, 0), "y".into()));
    app.cursor = (1, 0);
    app.process_action(Action::BlockJumpDown);
    assert_eq!(app.cursor, (3, 0));
}

#[test]
fn block_jump_down_from_empty_to_first_non_empty() {
    let mut app = App::new();
    app.sheet.row_count = 6;
    app.process_action(Action::EditCell((4, 0), "y".into()));
    app.cursor = (1, 0);
    app.process_action(Action::BlockJumpDown);
    assert_eq!(app.cursor, (4, 0));
}

#[test]
fn block_jump_up_symmetric() {
    let mut app = App::new();
    for r in 0..2 {
        app.process_action(Action::EditCell((r, 0), "x".into()));
    }
    app.process_action(Action::EditCell((4, 0), "y".into()));
    app.cursor = (4, 0);
    app.process_action(Action::BlockJumpUp);
    // Above (4,0) is empty (row 3, 2). First non-empty above is row 1.
    assert_eq!(app.cursor, (1, 0));
}

#[test]
fn block_jump_at_boundary_is_noop() {
    let mut app = App::new();
    app.sheet.row_count = 5;
    app.cursor = (0, 0);
    app.process_action(Action::BlockJumpUp);
    assert_eq!(app.cursor, (0, 0));
}
```

- [ ] **Step 2: Implement `block_jump_down` / `block_jump_up`** as helpers on `App` returning `Option<usize>`. Use `sheet.get_cell((r, col)).is_some()` for "non-empty" — matches how `NextNonEmpty` already does it.

- [ ] **Step 3: Wire `{` / `}`** in `normal.rs`.

- [ ] **Step 4: `cargo test -p cell-sheet-tui` — green.**

- [ ] **Step 5: Commit.** `feat: add { / } block jump in current column (#35)`

## Task 5: `*` / `#` — search current cell value (#36)

**Files:**
- Modify: `crates/cell-sheet-tui/src/action.rs` — remove `#[allow(dead_code)]` on `Search`, remove on `SearchDirection::Backward`
- Modify: `crates/cell-sheet-tui/src/app.rs` — add `SearchCellValue { backward: bool }` action handler
- Modify: `crates/cell-sheet-tui/src/mode/normal.rs`

Pragmatically: `*` reads the cell's `value.to_string()`, dispatches `Action::Search { pattern, direction }`. Dead simple.

- [ ] **Step 1: Test forward:**

```rust
#[test]
fn star_searches_for_current_cell_value() {
    let mut app = App::new();
    app.process_action(Action::EditCell((0, 0), "foo".into()));
    app.process_action(Action::EditCell((3, 2), "foo".into()));
    app.cursor = (0, 0);
    app.process_action(Action::SearchCellValue { backward: false });
    assert_eq!(app.cursor, (3, 2));
    assert_eq!(app.search_pattern.as_deref(), Some("foo"));
}
```

- [ ] **Step 2: Test empty cell is a no-op:**

```rust
#[test]
fn star_on_empty_cell_is_noop() {
    let mut app = App::new();
    app.cursor = (0, 0);
    app.process_action(Action::SearchCellValue { backward: false });
    assert_eq!(app.search_pattern, None);
}
```

- [ ] **Step 3: Implement** by reading `app.sheet.get_cell(cursor).map(|c| c.value.to_string())`, then internally dispatching `Search`.

- [ ] **Step 4: Wire `*` and `#`** in `normal.rs`.

- [ ] **Step 5: `cargo test -p cell-sheet-tui` — green.**

- [ ] **Step 6: Commit.** `feat: add * / # to search for current cell's value (#36)`

## Task 6: `gv` reselect last visual (#37)

**Files:**
- Modify: `crates/cell-sheet-tui/src/action.rs` — `ReselectLastVisual`
- Modify: `crates/cell-sheet-tui/src/app.rs` — `last_visual: Option<LastVisual>` field
- Modify: `crates/cell-sheet-tui/src/mode/visual.rs` — store on Esc / on operator-exit
- Modify: `crates/cell-sheet-tui/src/mode/normal.rs` — `gv` after `g` pending
- Modify: `crates/cell-sheet-tui/src/main.rs` — re-create `VisualState` from saved data on `ReselectLastVisual`

Tricky bit: visual mode entry/exit lives partly in `main.rs` (it owns `visual_state: Option<VisualState>`). The cleanest fix is to:

1. In `main.rs`, when leaving visual mode (exit branch), capture `(anchor, cursor, kind)` into `app.last_visual`.
2. Add a new `Action::ReselectLastVisual`. When `App::process_action` sees it, set `cursor = saved_cursor` and `mode = matching Mode::Visual* variant`. The `main.rs` handler block for `ChangeMode(Visual*)` then needs to be taught: if we already have a `last_visual`, use those endpoints to seed `visual_state`.
3. Alternative (cleaner): make `Action::ReselectLastVisual` a no-op in `App` and handle it entirely in `main.rs`'s match where it can manipulate `visual_state` directly.

Choose option 3 — cleaner separation. `App` only stores the data; `main.rs` orchestrates re-entry.

- [ ] **Step 1: Add `LastVisual` struct and field on App.**

```rust
#[derive(Debug, Clone, Copy)]
pub struct LastVisual {
    pub anchor: CellPos,
    pub cursor: CellPos,
    pub kind: crate::mode::visual::VisualKind,
}
```

- [ ] **Step 2: Test that exiting visual mode records `last_visual`.** (Test indirectly through the same machinery that `main.rs` uses — we'll factor a small helper.)

  Add to `app.rs`:
  ```rust
  pub fn record_last_visual(&mut self, anchor: CellPos, kind: VisualKind) {
      self.last_visual = Some(LastVisual { anchor, cursor: self.cursor, kind });
  }
  ```

  Test:
  ```rust
  #[test]
  fn record_last_visual_saves_anchor_cursor_kind() {
      use crate::mode::visual::VisualKind;
      let mut app = App::new();
      app.cursor = (3, 4);
      app.record_last_visual((1, 2), VisualKind::Line);
      let lv = app.last_visual.unwrap();
      assert_eq!(lv.anchor, (1, 2));
      assert_eq!(lv.cursor, (3, 4));
      assert!(matches!(lv.kind, VisualKind::Line));
  }
  ```

- [ ] **Step 3: Wire `record_last_visual` into `main.rs`** — at every visual exit point, before clearing `visual_state`, call `app.record_last_visual(vs.anchor, vs.kind)`.

- [ ] **Step 4: Test that `gv` from normal mode produces `ReselectLastVisual`.**

- [ ] **Step 5: Wire `gv` keybinding** by extending the `pending = 'g'` arm in `normal.rs` to recognize `g` then `v` → `ReselectLastVisual`.

- [ ] **Step 6: Handle `ReselectLastVisual` in `main.rs`:**

```rust
if matches!(&action, Action::ReselectLastVisual) {
    if let Some(lv) = app.last_visual {
        visual_state = Some(VisualState::new(lv.anchor, lv.kind));
        app.cursor = lv.cursor;
        app.mode = match lv.kind {
            VisualKind::Character => Mode::Visual,
            VisualKind::Line => Mode::VisualLine,
            VisualKind::Block => Mode::VisualBlock,
        };
        app.viewport.ensure_visible(app.cursor);
    }
}
```

  `App::process_action` treats `ReselectLastVisual` as `Noop`.

- [ ] **Step 7: `cargo test -p cell-sheet-tui` — green.**

- [ ] **Step 8: Commit.** `feat: add gv to reselect last visual range (#37)`

## Task 7: Final verification + CHANGELOG + PR

- [ ] **Step 1:** `cargo fmt --all` — must produce no diff after.
- [ ] **Step 2:** `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean.
- [ ] **Step 3:** `cargo test` — all pass (workspace).
- [ ] **Step 4:** Add CHANGELOG entries under `## Unreleased` → `### Added`, one bullet per issue with `(#NN)`.
- [ ] **Step 5:** Commit `docs: changelog for vim quick-wins`.
- [ ] **Step 6:** Push branch, open PR with body listing the six issues, CI plan in the test plan section.

## Self-review

- **Spec coverage:** Every task lines up to one issue. Each issue's "Scope" bullets are covered by the listed tests.
- **Type consistency:** `LastVisual` is consumed in main.rs via `last_visual: Option<LastVisual>`; `VisualKind` is re-exported from `mode::visual`. `JumpBack`/`JumpForward` (no args) match what `normal.rs` dispatches. `JumpToMark` carries `name` and `line_wise` consistently across handler and binding.
- **Placeholders:** None.
- **Dependencies between tasks:** #36 sits on top of the existing `Action::Search` plumbing, which is already in `app.rs` (just `dead_code`-flagged). Other tasks are independent.
