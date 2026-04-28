# Undo/Redo Coalescing for Bulk Operations (Issue #9)

**Date:** 2026-04-28  
**Status:** Approved  
**Scope:** `crates/cell-sheet-tui/src/app.rs`, `CHANGELOG.md`

---

## Problem

A single bulk operation — visual `c` on a range — pushes one `UndoEntry::CellEdit`
per affected cell. A 100-cell `ChangeRange` therefore requires 100 `u` presses to
fully undo. The other bulk operations (`dd`, visual `d`, `p`/`P`) already coalesce
correctly into a single `UndoEntry::MultiCellEdit`.

## Non-problems (already correct)

| Operation | Action | Undo entry |
|-----------|--------|------------|
| `dd` / `[N]dd` | `DeleteRow` | `MultiCellEdit` ✅ |
| visual `d` | `ClearRange` | `MultiCellEdit` ✅ |
| `p` / `P` (any register) | `Paste` / `PasteBefore` | `MultiCellEdit` ✅ |
| File open at startup | `load_file` (outside action loop) | none — intentional ✅ |

File open does not produce an undo entry. This is intentional: opening a file
starts a fresh editing session, consistent with how vim handles `:e`. No README
or help entry is needed — this is standard editor behaviour.

## Fix

### `Action::ChangeRange` in `app.rs`

Replace the per-cell `undo_stack.push(UndoEntry::CellEdit { … })` calls with a
pattern identical to `ClearRange`:

```rust
Action::ChangeRange { start, end } => {
    let max_col = end.1.min(self.sheet.col_count.saturating_sub(1));
    let mut changes = Vec::new();
    for row in start.0..=end.0 {
        for col in start.1..=max_col {
            let old_raw = self.sheet.get_cell((row, col))
                .map(|c| c.raw.clone()).unwrap_or_default();
            if !old_raw.is_empty() {
                changes.push(((row, col), old_raw, String::new()));
                self.sheet.clear_cell((row, col));
            }
        }
    }
    if !changes.is_empty() {
        self.undo_stack.push(UndoEntry::MultiCellEdit { changes });
    }
    self.dirty = true;
    self.insert_buffer = String::new();
    self.mode = Mode::Insert;
}
```

No changes to `undo.rs`, `UndoEntry`, or any other file.

## Tests

Five new tests added to the `#[cfg(test)]` block in `app.rs`:

### ChangeRange (visual `c`) group

1. **`change_range_single_undo_restores_all_cells`** — `c` on a 2×2 block,
   single `Undo` restores all four cells.
2. **`change_range_single_undo_restores_formula`** — `c` on a formula cell,
   `Undo` restores the raw formula and re-evaluates it correctly.
3. **`change_range_can_be_redone`** — `Undo` then `Redo` re-clears the block.
4. **`change_range_of_empty_cells_no_undo_entry`** — `c` on an all-empty range
   pushes no undo entry; the prior undo slot remains the most recent one.

### Paste coalescing (explicit N-cell assertion)

5. **`paste_block_of_n_cells_is_single_undo_step`** — yanks a 3×3 block (9
   cells), pastes it, single `Undo` restores all 9 cells in one step.

## CHANGELOG

Under `## Unreleased → Fixed`:

> **Fixed:** Visual `c` (`ChangeRange`) now records a single undo step for the
> entire range, consistent with `dd`, visual `d`, and paste.

## Out of scope

- Issue #8 (batch recalculation for performance) — separate concern; the
  `begin_batch`/`commit_batch` mechanism proposed there is not needed here.
- README undo-granularity documentation — the behaviour is what users expect;
  no documentation is warranted.
