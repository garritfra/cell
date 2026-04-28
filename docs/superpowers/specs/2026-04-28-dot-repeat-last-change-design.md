# Design: Normal mode `.` — repeat last change (issue #33)

**Date:** 2026-04-28  
**Issue:** [#33](https://github.com/garritfra/cell/issues/33)

---

## Goal

Implement Vim's `.` (dot) command in Normal mode: re-apply the most recently recorded cell-mutating operation at the current cursor position. `xj.j.j.` walks down a column clearing each cell. `dd` then `j.` deletes the next row.

## Out of scope

- `[count].` to repeat N times (not mentioned in issue)
- `ChangeRange` (complex multi-cell insert-mode operation; not listed in issue scope)
- Macro-style multi-step recording (separate RFC per the issue)

---

## Data model

### New field in `App`

```rust
pub last_change: Option<Action>,
```

Initialised to `None` in `App::new()`. Holds the most recent cell-mutating action in its original form (original cursor position included). The cursor position is rebound at replay time, not at record time, so pressing `.` multiple times at different positions all work correctly.

### Actions that record `last_change`

Recorded inside `process_action` at the end of each matching arm:

| Action | Trigger |
|---|---|
| `EditCell(pos, raw)` | Esc / Enter out of Insert mode (covers `i`, `a`, `c`, `Enter`) |
| `ClearCell(pos)` | `x` |
| `ClearRange { start, end }` | Visual-mode `d` |
| `DeleteRow { start, count }` | `dd` / `[count]dd` |
| `Paste(pos)` | `p` |
| `PasteBefore(pos)` | `P` |

### Actions that do NOT update `last_change`

- `Undo`, `Redo` — per the issue specification
- `RepeatLastChange` (`.`) — the saved value is restored after the recursive dispatch, so consecutive `.` all repeat the same original operation
- `ChangeCell` — not recorded directly; the `EditCell` that insert-mode exit emits is what gets stored

---

## New `Action` variant

```rust
/// Replay the last recorded cell-mutating change at the current cursor.
RepeatLastChange,
```

Added to `action.rs`.

---

## Rebinding logic

`rebind_change_to_cursor` is a private method on `App` that substitutes the current cursor before replay:

| Stored action | Rebound to cursor |
|---|---|
| `EditCell(_, raw)` | `EditCell(cursor, raw)` |
| `ClearCell(_)` | `ClearCell(cursor)` |
| `ClearRange{start, end}` | `ClearRange{start: cursor, end: cursor + (end − start)}` |
| `DeleteRow{_, count}` | `DeleteRow{start: cursor.row, count}` |
| `Paste(_)` | `Paste(cursor)` |
| `PasteBefore(_)` | `PasteBefore(cursor)` |

For anything else (should not happen), the action is passed through unchanged.

---

## Replay guard

The `RepeatLastChange` arm in `process_action`:

```rust
Action::RepeatLastChange => {
    if let Some(change) = self.last_change.clone() {
        let saved = self.last_change.clone();
        let rebound = self.rebind_change_to_cursor(change);
        self.process_action(rebound);  // may overwrite last_change
        self.last_change = saved;      // restore so next . repeats same op
    }
}
```

---

## Key binding

In `mode/normal.rs`, `handle_key`:

```rust
KeyCode::Char('.') => {
    self.discard_count();
    Action::RepeatLastChange
}
```

---

## Help entry

Added to `NORMAL_ENTRIES` in `crates/cell-sheet-core/src/help/entries.rs`:

```rust
HelpEntry {
    tags: &["."],
    category: HelpCategory::Normal,
    summary: "Repeat last change",
    detail: "Re-apply the last cell-mutating operation at the current cursor \
             position. Works with x, dd, p, P, and any edit committed from \
             Insert mode. u and Ctrl-r do not affect the repeat register.",
},
```

---

## Files changed

| File | Change |
|---|---|
| `crates/cell-sheet-tui/src/action.rs` | Add `RepeatLastChange` variant |
| `crates/cell-sheet-tui/src/app.rs` | Add `last_change` field; `rebind_change_to_cursor`; `RepeatLastChange` arm; record in 6 mutating arms |
| `crates/cell-sheet-tui/src/mode/normal.rs` | Map `'.'` → `Action::RepeatLastChange` |
| `crates/cell-sheet-core/src/help/entries.rs` | Add `HelpEntry` for `.` |
| `CHANGELOG.md` | Add entry under `## Unreleased → Added` |

---

## Tests

All tests live in `crates/cell-sheet-tui/src/app.rs` (unit) and `crates/cell-sheet-tui/src/mode/normal.rs` (key binding).

### `app.rs` tests

1. **`dot_repeats_edit_cell_at_new_cursor`** — Edit `(0,0)` to `"x"`, move cursor to `(0,1)`, dispatch `RepeatLastChange` → `(0,1)` contains `"x"`.
2. **`dot_repeats_dd_at_new_row`** — `DeleteRow{start:0, count:1}` at row 0 with data, move cursor to row 1 with data, dispatch `RepeatLastChange` → row 1 is now empty.
3. **`dot_after_undo_does_not_undo_again`** — Edit `(0,0)` to `"x"`, undo, dispatch `RepeatLastChange` → cell becomes `"x"` again (not undone a second time).
4. **`dot_with_no_last_change_is_noop`** — Fresh `App`, dispatch `RepeatLastChange` → no panic, cursor unchanged.
5. **`dot_preserves_last_change_for_next_dot`** — Edit, move, `.`, move, `.` → all three positions contain `"x"`.

### `normal.rs` test

6. **`dot_emits_repeat_last_change`** — `'.'` key → `Action::RepeatLastChange`.
