# Undo Coalescing for ChangeRange — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make visual `c` (`ChangeRange`) record a single undo step for the entire range, and add tests that explicitly verify coalescing for `ChangeRange`, paste, `dd`, and visual `d`.

**Architecture:** `ChangeRange` in `app.rs` currently pushes one `UndoEntry::CellEdit` per non-empty cell. We replace that with a `Vec`-collect pattern identical to `ClearRange`/`DeleteRow`, pushing a single `UndoEntry::MultiCellEdit`. No new types or abstractions are needed — `UndoEntry::MultiCellEdit` and `apply_undo_entry` already handle the multi-cell case.

**Tech Stack:** Rust, Cargo workspace (`cell-sheet-tui` crate).

---

### Task 1: Failing tests for `ChangeRange` undo coalescing

**Files:**
- Modify: `crates/cell-sheet-tui/src/app.rs` (test block, lines 1062–2426)

- [ ] **Step 1: Add four failing tests for `ChangeRange`**

Locate the `#[cfg(test)]` block in `crates/cell-sheet-tui/src/app.rs`. Find the `// ── ClearRange (visual-mode d) ──` comment group (around line 1067). Add a new group immediately after it:

```rust
// ── ChangeRange (visual-mode c) ────────────────────────────────────────────

#[test]
fn change_range_single_undo_restores_all_cells() {
    let mut app = App::new();
    app.process_action(Action::EditCell((0, 0), "a".into()));
    app.process_action(Action::EditCell((0, 1), "b".into()));
    app.process_action(Action::EditCell((1, 0), "c".into()));
    app.process_action(Action::EditCell((1, 1), "d".into()));
    app.process_action(Action::ChangeRange {
        start: (0, 0),
        end: (1, 1),
    });
    assert!(app.sheet.get_cell((0, 0)).is_none());
    app.process_action(Action::Undo);
    assert_eq!(
        app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
        Some("a")
    );
    assert_eq!(
        app.sheet.get_cell((0, 1)).map(|c| c.raw.as_str()),
        Some("b")
    );
    assert_eq!(
        app.sheet.get_cell((1, 0)).map(|c| c.raw.as_str()),
        Some("c")
    );
    assert_eq!(
        app.sheet.get_cell((1, 1)).map(|c| c.raw.as_str()),
        Some("d")
    );
}

#[test]
fn change_range_single_undo_restores_formula() {
    let mut app = App::new();
    app.process_action(Action::EditCell((0, 0), "=1+1".into()));
    app.process_action(Action::ChangeRange {
        start: (0, 0),
        end: (0, 0),
    });
    app.process_action(Action::Undo);
    assert_eq!(
        app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
        Some("=1+1")
    );
    let val = app.sheet.get_cell((0, 0)).map(|c| c.value.to_string());
    assert_eq!(val.as_deref(), Some("2"));
}

#[test]
fn change_range_can_be_redone() {
    let mut app = App::new();
    app.process_action(Action::EditCell((0, 0), "a".into()));
    app.process_action(Action::EditCell((0, 1), "b".into()));
    app.process_action(Action::ChangeRange {
        start: (0, 0),
        end: (0, 1),
    });
    app.process_action(Action::Undo);
    assert_eq!(
        app.sheet.get_cell((0, 0)).map(|c| c.raw.as_str()),
        Some("a")
    );
    app.process_action(Action::Redo);
    assert!(app.sheet.get_cell((0, 0)).is_none());
    assert!(app.sheet.get_cell((0, 1)).is_none());
}

#[test]
fn change_range_of_empty_cells_no_undo_entry() {
    let mut app = App::new();
    // Set up one prior edit so the undo stack has exactly one entry.
    app.process_action(Action::EditCell((5, 5), "prior".into()));
    app.sheet.col_count = 2;
    app.sheet.row_count = 2;
    // ChangeRange on a range with no non-empty cells: should push nothing.
    app.process_action(Action::ChangeRange {
        start: (0, 0),
        end: (1, 1),
    });
    // The only undo step is the prior EditCell — not the ChangeRange.
    app.process_action(Action::Undo);
    assert!(
        app.sheet.get_cell((5, 5)).is_none(),
        "undo should have reverted the prior edit, not a no-op ChangeRange"
    );
}
```

- [ ] **Step 2: Run the tests to confirm they all fail**

```bash
cargo test -p cell-sheet-tui change_range
```

Expected output: four test failures along the lines of:
```
test tests::change_range_single_undo_restores_all_cells ... FAILED
test tests::change_range_single_undo_restores_formula ... FAILED
test tests::change_range_can_be_redone ... FAILED
test tests::change_range_of_empty_cells_no_undo_entry ... FAILED
```

(They fail because `ChangeRange` still pushes per-cell `CellEdit` entries.)

---

### Task 2: Fix `ChangeRange` to push a single `MultiCellEdit`

**Files:**
- Modify: `crates/cell-sheet-tui/src/app.rs` (`Action::ChangeRange` arm, lines 217–239)

- [ ] **Step 1: Replace the per-cell push with a collect-and-push**

Find the `Action::ChangeRange { start, end } =>` arm in `process_action` (currently lines 217–239). Replace it entirely with:

```rust
Action::ChangeRange { start, end } => {
    let max_col = end.1.min(self.sheet.col_count.saturating_sub(1));
    let mut changes = Vec::new();
    for row in start.0..=end.0 {
        for col in start.1..=max_col {
            let old_raw = self
                .sheet
                .get_cell((row, col))
                .map(|c| c.raw.clone())
                .unwrap_or_default();
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

- [ ] **Step 2: Run the four new tests — expect all to pass**

```bash
cargo test -p cell-sheet-tui change_range
```

Expected output:
```
test tests::change_range_single_undo_restores_all_cells ... ok
test tests::change_range_single_undo_restores_formula ... ok
test tests::change_range_can_be_redone ... ok
test tests::change_range_of_empty_cells_no_undo_entry ... ok

test result: ok. 4 passed; 0 failed
```

- [ ] **Step 3: Run the full test suite to confirm no regressions**

```bash
cargo test -p cell-sheet-tui
```

Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/cell-sheet-tui/src/app.rs
git commit -m "fix: coalesce ChangeRange into single MultiCellEdit undo entry"
```

---

### Task 3: Explicit paste-of-N-cells undo coalescing test

**Files:**
- Modify: `crates/cell-sheet-tui/src/app.rs` (test block — `// ── Paste / PasteBefore ──` group)

- [ ] **Step 1: Add the test**

Find the `// ── Paste / PasteBefore ─────────────────────────────────────────────────` comment (around line 1134). Add this test at the end of that group, before the next comment section:

```rust
#[test]
fn paste_block_of_n_cells_is_single_undo_step() {
    let mut app = App::new();
    // Fill a 3×3 source block.
    for row in 0..3_usize {
        for col in 0..3_usize {
            let val = format!("r{}c{}", row, col);
            app.process_action(Action::EditCell((row, col), val));
        }
    }
    app.process_action(Action::YankRange {
        start: (0, 0),
        end: (2, 2),
    });
    // Paste at (3, 0): fills rows 3–5, cols 0–2 (9 cells).
    app.process_action(Action::Paste((3, 0)));
    for row in 3..6_usize {
        for col in 0..3_usize {
            assert!(
                app.sheet.get_cell((row, col)).is_some(),
                "expected cell ({row},{col}) to be filled after paste"
            );
        }
    }
    // A single undo should clear all 9 pasted cells.
    app.process_action(Action::Undo);
    for row in 3..6_usize {
        for col in 0..3_usize {
            assert!(
                app.sheet.get_cell((row, col)).is_none(),
                "expected cell ({row},{col}) to be empty after single undo"
            );
        }
    }
}
```

- [ ] **Step 2: Run it to verify it passes (implementation already correct)**

```bash
cargo test -p cell-sheet-tui paste_block_of_n_cells_is_single_undo_step
```

Expected:
```
test tests::paste_block_of_n_cells_is_single_undo_step ... ok
```

- [ ] **Step 3: Run full suite**

```bash
cargo test -p cell-sheet-tui
```

Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/cell-sheet-tui/src/app.rs
git commit -m "test: explicitly verify paste of N cells is a single undo step"
```

---

### Task 4: CHANGELOG and final checks

**Files:**
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Add the fixed entry**

In `CHANGELOG.md`, find the `## Unreleased` section. Add a `### Fixed` subsection (if one doesn't exist yet) and insert:

```markdown
### Fixed

- Visual `c` (`ChangeRange`) now records a single undo step for the entire
  range, consistent with `dd`, visual `d`, and paste (#9).
```

- [ ] **Step 2: Run clippy and fmt**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features
```

Expected: no warnings, no errors.

- [ ] **Step 3: Run the full test suite one final time**

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add CHANGELOG.md
git commit -m "chore: add CHANGELOG entry for ChangeRange undo coalescing fix (#9)"
```
