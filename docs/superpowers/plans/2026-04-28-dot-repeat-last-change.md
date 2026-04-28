# Dot Repeat Last Change Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Vim's `.` (dot) command — re-apply the last cell-mutating operation at the current cursor position.

**Architecture:** A new `RepeatLastChange` `Action` variant is added to the action enum. `App` gains a `last_change: Option<Action>` field that is updated inside `process_action` for each cell-mutating arm. On replay, a `rebind_change_to_cursor` helper substitutes the current cursor into the stored action before re-dispatching; the saved value is restored afterward so consecutive `.` presses all repeat the same original operation.

**Tech Stack:** Rust, no new dependencies. All tests are unit tests inside the existing `#[cfg(test)]` modules.

---

## Background: how the codebase is organised

- `crates/cell-sheet-tui/src/action.rs` — the `Action` enum; every user-visible operation is a variant. Add new variants here.
- `crates/cell-sheet-tui/src/app.rs` — `App` struct + `process_action`. All sheet mutations happen here. Tests live in a `#[cfg(test)]` module at the bottom of this file.
- `crates/cell-sheet-tui/src/mode/normal.rs` — `NormalState::handle_key` returns an `Action`. Tests live in `#[cfg(test)]` at the bottom.
- `crates/cell-sheet-core/src/help/entries.rs` — static slices of `HelpEntry` values that power `:help`. `NORMAL_ENTRIES` is the slice for normal-mode bindings.
- `CHANGELOG.md` — add to `## Unreleased → Added`.

---

## File map

| File | What changes |
|---|---|
| `crates/cell-sheet-tui/src/action.rs` | Add `RepeatLastChange` variant |
| `crates/cell-sheet-tui/src/app.rs` | Add `last_change` field; `rebind_change_to_cursor`; `RepeatLastChange` arm; record in 6 mutating arms |
| `crates/cell-sheet-tui/src/mode/normal.rs` | Map `'.'` → `Action::RepeatLastChange` |
| `crates/cell-sheet-core/src/help/entries.rs` | Add `HelpEntry` for `.` |
| `CHANGELOG.md` | Unreleased entry |

---

## Task 1: Add `RepeatLastChange` to `Action` and map `'.'` in Normal mode

**Files:**
- Modify: `crates/cell-sheet-tui/src/action.rs`
- Modify: `crates/cell-sheet-tui/src/mode/normal.rs`

- [ ] **Step 1: Write the failing test in `normal.rs`**

  Open `crates/cell-sheet-tui/src/mode/normal.rs`. Inside the `#[cfg(test)]` module, after the existing `tab_jumps_forward` test, add:

  ```rust
  #[test]
  fn dot_emits_repeat_last_change() {
      let app = App::new();
      let mut state = NormalState::new();
      assert_eq!(
          state.handle_key(key(KeyCode::Char('.')), &app),
          Action::RepeatLastChange
      );
  }
  ```

- [ ] **Step 2: Run the test to confirm it fails**

  ```sh
  cargo test -p cell-sheet-tui dot_emits_repeat_last_change
  ```

  Expected: compile error — `Action::RepeatLastChange` does not exist yet.

- [ ] **Step 3: Add `RepeatLastChange` to the `Action` enum**

  Open `crates/cell-sheet-tui/src/action.rs`. After the `SetStatus` variant (last line before the closing `}`), add:

  ```rust
  /// Re-apply the last recorded cell-mutating operation at the current cursor.
  RepeatLastChange,
  ```

  The enum already has `#[derive(Debug, Clone, PartialEq)]` so no derive changes are needed.

- [ ] **Step 4: Map `'.'` in `NormalState::handle_key`**

  Open `crates/cell-sheet-tui/src/mode/normal.rs`. In the big `match key.code { ... }` block inside `handle_key`, add a new arm just before the final `_ =>` catch-all:

  ```rust
  KeyCode::Char('.') => {
      self.discard_count();
      Action::RepeatLastChange
  }
  ```

- [ ] **Step 5: Handle `RepeatLastChange` in `process_action` (stub)**

  Open `crates/cell-sheet-tui/src/app.rs`. In `process_action`, the final arm is `Action::Open(_) | Action::Resize => {}`. Add a new arm just before it:

  ```rust
  Action::RepeatLastChange => {}
  ```

  This silences the "non-exhaustive patterns" compiler error. The real implementation comes in Task 2.

- [ ] **Step 6: Run the test to confirm it passes**

  ```sh
  cargo test -p cell-sheet-tui dot_emits_repeat_last_change
  ```

  Expected: PASS.

- [ ] **Step 7: Commit**

  ```sh
  git add crates/cell-sheet-tui/src/action.rs \
          crates/cell-sheet-tui/src/mode/normal.rs \
          crates/cell-sheet-tui/src/app.rs
  git commit -m "feat: add RepeatLastChange action and bind '.' in normal mode"
  ```

---

## Task 2: `last_change` field, `rebind_change_to_cursor`, and `EditCell` recording

**Files:**
- Modify: `crates/cell-sheet-tui/src/app.rs`

- [ ] **Step 1: Write the failing tests in `app.rs`**

  Open `crates/cell-sheet-tui/src/app.rs`. Inside the `#[cfg(test)]` module, after the last existing test, add:

  ```rust
  // ── dot-repeat (#33) ────────────────────────────────────────────────────

  #[test]
  fn dot_with_no_last_change_is_noop() {
      let mut app = App::new();
      app.cursor = (2, 3);
      app.process_action(Action::RepeatLastChange);
      assert_eq!(app.cursor, (2, 3));
      assert!(app.sheet.get_cell((2, 3)).is_none());
  }

  #[test]
  fn dot_repeats_edit_cell_at_new_cursor() {
      let mut app = App::new();
      app.process_action(Action::EditCell((0, 0), "x".into()));
      app.cursor = (0, 1);
      app.process_action(Action::RepeatLastChange);
      assert_eq!(
          app.sheet.get_cell((0, 1)).map(|c| c.raw.as_str()),
          Some("x")
      );
  }

  #[test]
  fn dot_preserves_last_change_for_next_dot() {
      let mut app = App::new();
      app.process_action(Action::EditCell((0, 0), "x".into()));
      app.cursor = (0, 1);
      app.process_action(Action::RepeatLastChange);
      app.cursor = (0, 2);
      app.process_action(Action::RepeatLastChange);
      assert_eq!(
          app.sheet.get_cell((0, 2)).map(|c| c.raw.as_str()),
          Some("x")
      );
  }
  ```

- [ ] **Step 2: Run the tests to confirm they fail**

  ```sh
  cargo test -p cell-sheet-tui dot_repeats_edit_cell_at_new_cursor
  ```

  Expected: FAIL — `dot_repeats_edit_cell_at_new_cursor` asserts `Some("x")` but gets `None`.

- [ ] **Step 3: Add `last_change` field to `App`**

  In the `App` struct definition, after `pub last_visual: Option<LastVisual>,`, add:

  ```rust
  pub last_change: Option<Action>,
  ```

  In `App::new()`, after `last_visual: None,`, add:

  ```rust
  last_change: None,
  ```

- [ ] **Step 4: Add `rebind_change_to_cursor`**

  In the `impl App` block, just before the `fn format_from_path` function, add:

  ```rust
  fn rebind_change_to_cursor(&self, action: Action) -> Action {
      let cursor = self.cursor;
      match action {
          Action::EditCell(_, raw) => Action::EditCell(cursor, raw),
          Action::ClearCell(_) => Action::ClearCell(cursor),
          Action::ClearRange { start, end } => {
              let dr = end.0.saturating_sub(start.0);
              let dc = end.1.saturating_sub(start.1);
              Action::ClearRange {
                  start: cursor,
                  end: (cursor.0 + dr, cursor.1 + dc),
              }
          }
          Action::DeleteRow { count, .. } => Action::DeleteRow {
              start: cursor.0,
              count,
          },
          Action::Paste(_) => Action::Paste(cursor),
          Action::PasteBefore(_) => Action::PasteBefore(cursor),
          _ => action,
      }
  }
  ```

- [ ] **Step 5: Implement the `RepeatLastChange` arm**

  Replace the stub `Action::RepeatLastChange => {}` added in Task 1 with:

  ```rust
  Action::RepeatLastChange => {
      if let Some(change) = self.last_change.clone() {
          let saved = self.last_change.clone();
          let rebound = self.rebind_change_to_cursor(change);
          self.process_action(rebound);
          self.last_change = saved;
      }
  }
  ```

- [ ] **Step 6: Record `last_change` in the `EditCell` arm**

  The `EditCell` arm ends with `self.dirty = true;`. Directly after that line, add:

  ```rust
  self.last_change = Some(Action::EditCell(pos, raw));
  ```

  `pos` is `Copy` (`(usize, usize)`). `raw` is a `String` that was only borrowed (via `&raw`) throughout the arm body, so it is still owned here.

- [ ] **Step 7: Run the tests to confirm they pass**

  ```sh
  cargo test -p cell-sheet-tui dot_with_no_last_change_is_noop dot_repeats_edit_cell_at_new_cursor dot_preserves_last_change_for_next_dot
  ```

  Expected: all three PASS.

- [ ] **Step 8: Commit**

  ```sh
  git add crates/cell-sheet-tui/src/app.rs
  git commit -m "feat: add last_change field and implement dot-repeat for EditCell"
  ```

---

## Task 3: Record `last_change` for `ClearCell` and `DeleteRow`

**Files:**
- Modify: `crates/cell-sheet-tui/src/app.rs`

- [ ] **Step 1: Write the failing tests**

  In the `#[cfg(test)]` module in `app.rs`, after the tests from Task 2, add:

  ```rust
  #[test]
  fn dot_repeats_clear_cell() {
      let mut app = App::new();
      app.process_action(Action::EditCell((0, 0), "a".into()));
      app.process_action(Action::EditCell((0, 1), "b".into()));
      app.process_action(Action::ClearCell((0, 0)));
      app.cursor = (0, 1);
      app.process_action(Action::RepeatLastChange);
      assert!(app.sheet.get_cell((0, 1)).is_none());
  }

  #[test]
  fn dot_repeats_dd_at_new_row() {
      let mut app = App::new();
      app.process_action(Action::EditCell((0, 0), "a".into()));
      app.process_action(Action::EditCell((1, 0), "b".into()));
      app.process_action(Action::DeleteRow { start: 0, count: 1 });
      app.cursor = (1, 0);
      app.process_action(Action::RepeatLastChange);
      assert!(app.sheet.get_cell((1, 0)).is_none());
  }

  #[test]
  fn dot_after_undo_does_not_undo_again() {
      let mut app = App::new();
      app.process_action(Action::EditCell((0, 0), "x".into()));
      app.process_action(Action::Undo);
      assert!(app.sheet.get_cell((0, 0)).is_none());
      app.cursor = (0, 0);
      app.process_action(Action::RepeatLastChange);
      assert_eq!(
          app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
          Some("x")
      );
  }
  ```

- [ ] **Step 2: Run the tests to confirm they fail**

  ```sh
  cargo test -p cell-sheet-tui dot_repeats_clear_cell dot_repeats_dd_at_new_row dot_after_undo_does_not_undo_again
  ```

  Expected: `dot_repeats_clear_cell` and `dot_repeats_dd_at_new_row` FAIL (no last_change recorded). `dot_after_undo_does_not_undo_again` FAIL.

- [ ] **Step 3: Record `last_change` in the `ClearCell` arm**

  The `ClearCell` arm ends just after the `if !old_raw.is_empty() { ... }` block. After the closing `}` of that block, add:

  ```rust
  self.last_change = Some(Action::ClearCell(pos));
  ```

  `pos` is `Copy`. It is recorded unconditionally (even if the cell was already empty) to match Vim's behaviour.

- [ ] **Step 4: Record `last_change` in the `DeleteRow` arm**

  The `DeleteRow` arm ends with the `if !changes.is_empty() { ... }` block. After that block's closing `}`, add:

  ```rust
  self.last_change = Some(Action::DeleteRow { start, count });
  ```

  Both `start` and `count` are `usize` (`Copy`).

- [ ] **Step 5: Run the tests to confirm they pass**

  ```sh
  cargo test -p cell-sheet-tui dot_repeats_clear_cell dot_repeats_dd_at_new_row dot_after_undo_does_not_undo_again
  ```

  Expected: all three PASS.

- [ ] **Step 6: Commit**

  ```sh
  git add crates/cell-sheet-tui/src/app.rs
  git commit -m "feat: record last_change for ClearCell and DeleteRow"
  ```

---

## Task 4: Record `last_change` for `ClearRange`, `Paste`, `PasteBefore`

**Files:**
- Modify: `crates/cell-sheet-tui/src/app.rs`

- [ ] **Step 1: Write the failing tests**

  In the `#[cfg(test)]` module in `app.rs`, after the tests from Task 3, add:

  ```rust
  #[test]
  fn dot_repeats_paste() {
      let mut app = App::new();
      app.process_action(Action::EditCell((0, 0), "hello".into()));
      app.process_action(Action::YankCell((0, 0)));
      app.process_action(Action::Paste((0, 1)));
      app.cursor = (0, 2);
      app.process_action(Action::RepeatLastChange);
      assert_eq!(
          app.sheet.get_cell((0, 2)).map(|c| c.raw.as_str()),
          Some("hello")
      );
  }

  #[test]
  fn dot_repeats_clear_range_with_same_shape() {
      let mut app = App::new();
      app.process_action(Action::EditCell((0, 0), "a".into()));
      app.process_action(Action::EditCell((0, 1), "b".into()));
      app.process_action(Action::EditCell((1, 0), "c".into()));
      app.process_action(Action::EditCell((1, 1), "d".into()));
      // Clear 1×2 range at row 0
      app.process_action(Action::ClearRange { start: (0, 0), end: (0, 1) });
      // Dot at (1, 0) should clear (1,0)–(1,1)
      app.cursor = (1, 0);
      app.process_action(Action::RepeatLastChange);
      assert!(app.sheet.get_cell((1, 0)).is_none());
      assert!(app.sheet.get_cell((1, 1)).is_none());
  }
  ```

- [ ] **Step 2: Run the tests to confirm they fail**

  ```sh
  cargo test -p cell-sheet-tui dot_repeats_paste dot_repeats_clear_range_with_same_shape
  ```

  Expected: both FAIL.

- [ ] **Step 3: Record `last_change` in the `ClearRange` arm**

  The `ClearRange` arm ends with the `if !changes.is_empty() { ... }` block. After that block's closing `}`, add:

  ```rust
  self.last_change = Some(Action::ClearRange { start, end });
  ```

  Both `start` and `end` are `(usize, usize)` (`Copy`).

- [ ] **Step 4: Record `last_change` in the `Paste` / `PasteBefore` arm**

  The combined `Action::Paste(pos) | Action::PasteBefore(pos)` arm already computes `let is_after = ...` at the top and ends after the `}` that closes the `if let Some(reg) = ...` block. After that closing `}`, add:

  ```rust
  if is_after {
      self.last_change = Some(Action::Paste(pos));
  } else {
      self.last_change = Some(Action::PasteBefore(pos));
  }
  ```

  `pos` is `Copy`; `is_after` is already in scope.

- [ ] **Step 5: Run the tests to confirm they pass**

  ```sh
  cargo test -p cell-sheet-tui dot_repeats_paste dot_repeats_clear_range_with_same_shape
  ```

  Expected: both PASS.

- [ ] **Step 6: Run the full test suite**

  ```sh
  cargo test
  ```

  Expected: all tests pass (no regressions).

- [ ] **Step 7: Commit**

  ```sh
  git add crates/cell-sheet-tui/src/app.rs
  git commit -m "feat: record last_change for ClearRange, Paste, PasteBefore"
  ```

---

## Task 5: Help entry and CHANGELOG

**Files:**
- Modify: `crates/cell-sheet-core/src/help/entries.rs`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Add `HelpEntry` for `.` to `NORMAL_ENTRIES`**

  Open `crates/cell-sheet-core/src/help/entries.rs`. In `NORMAL_ENTRIES`, find the `HelpEntry` for `"u"` (Undo). Add the following entry **before** the `"u"` entry (keeps the logical flow: edits → dot → undo):

  ```rust
  HelpEntry {
      tags: &["."],
      category: HelpCategory::Normal,
      summary: "Repeat last change",
      detail: "Re-apply the last cell-mutating operation at the current cursor \
               position. Works after x, dd, p, P, and any edit committed from \
               Insert mode (i/a/c + Esc or Enter). u and Ctrl-r do not affect \
               the repeat register.",
  },
  ```

- [ ] **Step 2: Add CHANGELOG entry**

  Open `CHANGELOG.md`. Under `## Unreleased → ### Added`, add:

  ```markdown
  - Normal mode `.` repeats the last cell-mutating change at the current cursor
    position, vim-style. Works after `x`, `dd`, `p`/`P`, and any edit committed
    from Insert mode. `u` and `Ctrl-r` do not overwrite the repeat register
    ([#33](https://github.com/garritfra/cell/issues/33))
  ```

- [ ] **Step 3: Verify `:help .` resolves correctly**

  The `HelpRegistry` is built from the static slices at startup. A quick compile-and-test is enough to confirm registration:

  ```sh
  cargo test -p cell-sheet-tui show_help_valid_topic
  ```

  Expected: PASS (this test was already passing; it exercises the registry lookup path).

- [ ] **Step 4: Commit**

  ```sh
  git add crates/cell-sheet-core/src/help/entries.rs CHANGELOG.md
  git commit -m "docs: add help entry and changelog for dot-repeat (#33)"
  ```

---

## Task 6: Final verification

- [ ] **Step 1: Format**

  ```sh
  cargo fmt --all
  ```

  Expected: no output (already formatted, or formats cleanly).

- [ ] **Step 2: Clippy**

  ```sh
  cargo clippy --workspace --all-targets --all-features
  ```

  Expected: no warnings (CI runs with `RUSTFLAGS=-Dwarnings`).

- [ ] **Step 3: Full test suite**

  ```sh
  cargo test
  ```

  Expected: all tests pass.

- [ ] **Step 4: Commit formatting fixes if any**

  If `cargo fmt` produced changes:

  ```sh
  git add -u
  git commit -m "chore: fmt"
  ```
