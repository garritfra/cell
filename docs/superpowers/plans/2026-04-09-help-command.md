# Help Command Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Vim-inspired `:help [topic]` command with full-screen help viewer, tag-based lookup, and collocated help entries defined next to their implementations.

**Architecture:** A `HelpRegistry` in cell-core collects static `HelpEntry` arrays defined in each module. The TUI adds a `Mode::Help` with its own full-screen renderer and Vim-like scrolling. The command parser routes `:help` to a new `ShowHelp` action.

**Tech Stack:** Rust, ratatui, crossterm (existing stack — no new dependencies)

---

### Task 1: HelpEntry and HelpRegistry in cell-core

**Files:**
- Create: `crates/cell-core/src/help.rs`
- Modify: `crates/cell-core/src/lib.rs:1-3`

- [ ] **Step 1: Write the failing test for HelpRegistry**

In `crates/cell-core/src/help.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    static TEST_ENTRIES: &[HelpEntry] = &[
        HelpEntry {
            tags: &["h"],
            category: HelpCategory::Normal,
            summary: "Move cursor left",
            detail: "Move the cursor one column to the left.",
        },
        HelpEntry {
            tags: &[":w", ":write"],
            category: HelpCategory::Command,
            summary: "Save file",
            detail: "Write the current sheet to disk.",
        },
    ];

    #[test]
    fn find_by_tag() {
        let registry = HelpRegistry::from_entries(&[TEST_ENTRIES]);
        let entry = registry.find("h").unwrap();
        assert_eq!(entry.summary, "Move cursor left");
    }

    #[test]
    fn find_by_alias_tag() {
        let registry = HelpRegistry::from_entries(&[TEST_ENTRIES]);
        let entry = registry.find(":write").unwrap();
        assert_eq!(entry.summary, "Save file");
    }

    #[test]
    fn find_case_insensitive() {
        let registry = HelpRegistry::from_entries(&[TEST_ENTRIES]);
        let entry = registry.find("H").unwrap();
        assert_eq!(entry.summary, "Move cursor left");
    }

    #[test]
    fn find_not_found() {
        let registry = HelpRegistry::from_entries(&[TEST_ENTRIES]);
        assert!(registry.find("zzz").is_none());
    }

    #[test]
    fn by_category() {
        let registry = HelpRegistry::from_entries(&[TEST_ENTRIES]);
        let normals = registry.by_category(HelpCategory::Normal);
        assert_eq!(normals.len(), 1);
        assert_eq!(normals[0].tags[0], "h");
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd /Users/garrit/.superset/worktrees/cell/help-command && cargo test -p cell-core -- help`
Expected: Compilation error — `help` module does not exist yet.

- [ ] **Step 3: Implement HelpEntry, HelpCategory, and HelpRegistry**

In `crates/cell-core/src/help.rs`, above the test module:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HelpCategory {
    Normal,
    Insert,
    Visual,
    Command,
    Formula,
}

impl HelpCategory {
    pub fn label(&self) -> &'static str {
        match self {
            HelpCategory::Normal => "NORMAL MODE",
            HelpCategory::Insert => "INSERT MODE",
            HelpCategory::Visual => "VISUAL MODE",
            HelpCategory::Command => "COMMANDS",
            HelpCategory::Formula => "FORMULAS",
        }
    }
}

#[derive(Debug)]
pub struct HelpEntry {
    pub tags: &'static [&'static str],
    pub category: HelpCategory,
    pub summary: &'static str,
    pub detail: &'static str,
}

pub struct HelpRegistry {
    entries: Vec<&'static HelpEntry>,
}

impl HelpRegistry {
    /// Build a registry from multiple static entry slices (one per module).
    pub fn from_entries(slices: &[&'static [HelpEntry]]) -> Self {
        let mut entries = Vec::new();
        for slice in slices {
            for entry in *slice {
                entries.push(entry);
            }
        }
        HelpRegistry { entries }
    }

    /// Find an entry by tag (case-insensitive).
    pub fn find(&self, tag: &str) -> Option<&'static HelpEntry> {
        let tag_lower = tag.to_lowercase();
        for entry in &self.entries {
            for t in entry.tags {
                if t.to_lowercase() == tag_lower {
                    return Some(entry);
                }
            }
        }
        None
    }

    /// Return all entries in a given category, in registration order.
    pub fn by_category(&self, category: HelpCategory) -> Vec<&'static HelpEntry> {
        self.entries.iter().copied().filter(|e| e.category == category).collect()
    }

    /// Return all categories that have at least one entry, in display order.
    pub fn categories(&self) -> Vec<HelpCategory> {
        use HelpCategory::*;
        let order = [Normal, Insert, Visual, Command, Formula];
        order.iter().copied().filter(|cat| self.entries.iter().any(|e| e.category == *cat)).collect()
    }

    /// Return all entries in display order (grouped by category).
    pub fn all_entries(&self) -> &[&'static HelpEntry] {
        &self.entries
    }
}
```

- [ ] **Step 4: Export the help module from cell-core**

In `crates/cell-core/src/lib.rs`, add:

```rust
pub mod help;
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd /Users/garrit/.superset/worktrees/cell/help-command && cargo test -p cell-core -- help`
Expected: All 5 tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/cell-core/src/help.rs crates/cell-core/src/lib.rs
git commit -m "feat: add HelpRegistry and HelpEntry types in cell-core"
```

---

### Task 2: Collocated help entries for all modes and formulas

**Files:**
- Create: `crates/cell-core/src/help/entries.rs`
- Modify: `crates/cell-core/src/help.rs` (re-export entries)

- [ ] **Step 1: Write a test that the full entry set covers expected tags**

Add to `crates/cell-core/src/help.rs` tests:

```rust
#[test]
fn full_registry_has_expected_tags() {
    let registry = HelpRegistry::new();
    // Spot-check key entries exist
    assert!(registry.find("h").is_some(), "missing h");
    assert!(registry.find("dd").is_some(), "missing dd");
    assert!(registry.find(":w").is_some(), "missing :w");
    assert!(registry.find(":help").is_some(), "missing :help");
    assert!(registry.find("SUM").is_some(), "missing SUM");
    assert!(registry.find("IF").is_some(), "missing IF");
    assert!(registry.find("Esc").is_some(), "missing Esc");
    assert!(registry.find("v").is_some(), "missing v");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd /Users/garrit/.superset/worktrees/cell/help-command && cargo test -p cell-core -- full_registry`
Expected: Compilation error — `HelpRegistry::new()` does not exist yet.

- [ ] **Step 3: Create the entries module with all help content**

Create `crates/cell-core/src/help/entries.rs`:

```rust
use super::{HelpEntry, HelpCategory};

pub static NORMAL_ENTRIES: &[HelpEntry] = &[
    HelpEntry {
        tags: &["h"],
        category: HelpCategory::Normal,
        summary: "Move cursor left",
        detail: "Move the cursor one column to the left. Stops at column A.\nAlias: Left arrow",
    },
    HelpEntry {
        tags: &["j"],
        category: HelpCategory::Normal,
        summary: "Move cursor down",
        detail: "Move the cursor one row down.\nAlias: Down arrow",
    },
    HelpEntry {
        tags: &["k"],
        category: HelpCategory::Normal,
        summary: "Move cursor up",
        detail: "Move the cursor one row up. Stops at row 1.\nAlias: Up arrow",
    },
    HelpEntry {
        tags: &["l"],
        category: HelpCategory::Normal,
        summary: "Move cursor right",
        detail: "Move the cursor one column to the right.\nAlias: Right arrow",
    },
    HelpEntry {
        tags: &["gg"],
        category: HelpCategory::Normal,
        summary: "Go to first row",
        detail: "Move the cursor to row 1, keeping the current column.",
    },
    HelpEntry {
        tags: &["G"],
        category: HelpCategory::Normal,
        summary: "Go to last row",
        detail: "Move the cursor to the last row with data, keeping the current column.",
    },
    HelpEntry {
        tags: &["0"],
        category: HelpCategory::Normal,
        summary: "Go to first column",
        detail: "Move the cursor to column A, keeping the current row.",
    },
    HelpEntry {
        tags: &["$"],
        category: HelpCategory::Normal,
        summary: "Go to last column",
        detail: "Move the cursor to the last column with data, keeping the current row.",
    },
    HelpEntry {
        tags: &["w"],
        category: HelpCategory::Normal,
        summary: "Next non-empty cell",
        detail: "Jump to the next non-empty cell to the right in the current row.",
    },
    HelpEntry {
        tags: &["b"],
        category: HelpCategory::Normal,
        summary: "Previous non-empty cell",
        detail: "Jump to the previous non-empty cell to the left in the current row.",
    },
    HelpEntry {
        tags: &["dd"],
        category: HelpCategory::Normal,
        summary: "Delete current row",
        detail: "Deletes all cells in the current row. The row contents are stored\nin the register and can be pasted with p or P. Undoable with u.",
    },
    HelpEntry {
        tags: &["yy"],
        category: HelpCategory::Normal,
        summary: "Yank current row",
        detail: "Copies all cells in the current row to the register.\nPaste with p (below) or P (above).",
    },
    HelpEntry {
        tags: &["x"],
        category: HelpCategory::Normal,
        summary: "Clear current cell",
        detail: "Clears the content of the cell under the cursor. Undoable with u.",
    },
    HelpEntry {
        tags: &["p"],
        category: HelpCategory::Normal,
        summary: "Paste below",
        detail: "Paste the register contents below the current row (for row/block\nregisters) or into the cell below (for cell registers).\nFormula references are adjusted automatically.",
    },
    HelpEntry {
        tags: &["P"],
        category: HelpCategory::Normal,
        summary: "Paste above",
        detail: "Paste the register contents above the current row (for row/block\nregisters) or into the current cell (for cell registers).\nFormula references are adjusted automatically.",
    },
    HelpEntry {
        tags: &["u"],
        category: HelpCategory::Normal,
        summary: "Undo",
        detail: "Undo the last cell edit. Supports multiple levels of undo.",
    },
    HelpEntry {
        tags: &["Ctrl+R"],
        category: HelpCategory::Normal,
        summary: "Redo",
        detail: "Redo the last undone edit.",
    },
    HelpEntry {
        tags: &["Ctrl+D"],
        category: HelpCategory::Normal,
        summary: "Half page down",
        detail: "Move the cursor down by half the visible page height.",
    },
    HelpEntry {
        tags: &["Ctrl+U"],
        category: HelpCategory::Normal,
        summary: "Half page up",
        detail: "Move the cursor up by half the visible page height.",
    },
    HelpEntry {
        tags: &["Ctrl+F"],
        category: HelpCategory::Normal,
        summary: "Page down",
        detail: "Move the cursor down by one full page.",
    },
    HelpEntry {
        tags: &["Ctrl+B"],
        category: HelpCategory::Normal,
        summary: "Page up",
        detail: "Move the cursor up by one full page.",
    },
    HelpEntry {
        tags: &["i", "a"],
        category: HelpCategory::Normal,
        summary: "Enter Insert mode",
        detail: "Switch to Insert mode to edit the current cell.\ni places the cursor at the end of existing content.\na behaves the same as i in Cell.",
    },
    HelpEntry {
        tags: &["o"],
        category: HelpCategory::Normal,
        summary: "Enter Insert mode (new line)",
        detail: "Switch to Insert mode. In Cell, behaves the same as i\n(there are no multi-line cells).",
    },
    HelpEntry {
        tags: &["Enter"],
        category: HelpCategory::Normal,
        summary: "Edit cell",
        detail: "Enter Insert mode to edit the current cell. Same as i.",
    },
    HelpEntry {
        tags: &["v"],
        category: HelpCategory::Normal,
        summary: "Enter Visual mode",
        detail: "Start visual selection from the current cell. Use h/j/k/l to\nextend the selection. Press d to delete or y to yank.",
    },
    HelpEntry {
        tags: &["Ctrl+V"],
        category: HelpCategory::Normal,
        summary: "Enter Visual Block mode",
        detail: "Start block (rectangular) selection from the current cell.\nUse h/j/k/l to extend. Press d to delete or y to yank.",
    },
    HelpEntry {
        tags: &["/"],
        category: HelpCategory::Normal,
        summary: "Search",
        detail: "Open the search prompt. Type a pattern and press Enter to\nfind the next cell whose value contains the pattern.\nCase-insensitive.",
    },
    HelpEntry {
        tags: &["n"],
        category: HelpCategory::Normal,
        summary: "Next search match",
        detail: "Jump to the next cell matching the last search pattern.",
    },
    HelpEntry {
        tags: &["N"],
        category: HelpCategory::Normal,
        summary: "Previous search match",
        detail: "Jump to the previous cell matching the last search pattern.",
    },
];

pub static INSERT_ENTRIES: &[HelpEntry] = &[
    HelpEntry {
        tags: &["Esc"],
        category: HelpCategory::Insert,
        summary: "Confirm edit, return to Normal",
        detail: "Saves the current cell content and returns to Normal mode.",
    },
    HelpEntry {
        tags: &["Enter-insert"],
        category: HelpCategory::Insert,
        summary: "Confirm edit and move down",
        detail: "Saves the current cell content and returns to Normal mode.\nIn Insert mode, Enter confirms the edit (same as Esc).",
    },
    HelpEntry {
        tags: &["Backspace"],
        category: HelpCategory::Insert,
        summary: "Delete character before cursor",
        detail: "Deletes the character to the left of the cursor in the cell\nedit buffer.",
    },
    HelpEntry {
        tags: &["Delete"],
        category: HelpCategory::Insert,
        summary: "Delete character at cursor",
        detail: "Deletes the character at the cursor position in the cell\nedit buffer.",
    },
    HelpEntry {
        tags: &["Left-insert", "Right-insert"],
        category: HelpCategory::Insert,
        summary: "Move cursor within cell",
        detail: "Arrow keys move the cursor left/right within the cell edit\nbuffer during Insert mode.",
    },
    HelpEntry {
        tags: &["Home"],
        category: HelpCategory::Insert,
        summary: "Move to start of cell",
        detail: "Move the cursor to the beginning of the cell edit buffer.",
    },
    HelpEntry {
        tags: &["End"],
        category: HelpCategory::Insert,
        summary: "Move to end of cell",
        detail: "Move the cursor to the end of the cell edit buffer.",
    },
];

pub static VISUAL_ENTRIES: &[HelpEntry] = &[
    HelpEntry {
        tags: &["v-visual"],
        category: HelpCategory::Visual,
        summary: "Enter Visual mode",
        detail: "Start visual selection from Normal mode. The anchor is set at\nthe current cursor position.",
    },
    HelpEntry {
        tags: &["Ctrl+V-visual"],
        category: HelpCategory::Visual,
        summary: "Enter Visual Block mode",
        detail: "Start rectangular block selection from Normal mode.",
    },
    HelpEntry {
        tags: &["d-visual"],
        category: HelpCategory::Visual,
        summary: "Delete selection",
        detail: "Clear all cells in the visual selection. The contents are\nstored in the register. Returns to Normal mode.",
    },
    HelpEntry {
        tags: &["y-visual"],
        category: HelpCategory::Visual,
        summary: "Yank selection",
        detail: "Copy all cells in the visual selection to the register.\nReturns to Normal mode.",
    },
    HelpEntry {
        tags: &["Esc-visual"],
        category: HelpCategory::Visual,
        summary: "Cancel selection",
        detail: "Exit Visual mode and return to Normal mode without\nmodifying any cells.",
    },
];

pub static COMMAND_ENTRIES: &[HelpEntry] = &[
    HelpEntry {
        tags: &[":w", ":write"],
        category: HelpCategory::Command,
        summary: "Save file",
        detail: "Write the current sheet to disk. If no filename has been set,\nuse :w <path> to specify one.\n\nIf the sheet contains formulas and you save as CSV/TSV,\nCell will warn you. Use :w file.cell to preserve formulas,\nor :w! to force save as CSV (formulas become values).",
    },
    HelpEntry {
        tags: &[":w!"],
        category: HelpCategory::Command,
        summary: "Force save",
        detail: "Write the current sheet to disk, even if formulas would be\nlost by saving as CSV/TSV.",
    },
    HelpEntry {
        tags: &[":q", ":quit"],
        category: HelpCategory::Command,
        summary: "Quit",
        detail: "Exit Cell. Fails if there are unsaved changes.\nUse :q! to discard changes, or :wq to save and quit.",
    },
    HelpEntry {
        tags: &[":q!"],
        category: HelpCategory::Command,
        summary: "Force quit",
        detail: "Exit Cell without saving. All unsaved changes are discarded.",
    },
    HelpEntry {
        tags: &[":wq"],
        category: HelpCategory::Command,
        summary: "Save and quit",
        detail: "Write the current sheet to disk, then exit Cell.",
    },
    HelpEntry {
        tags: &[":e", ":edit"],
        category: HelpCategory::Command,
        summary: "Open file",
        detail: "Open a file for editing.\nUsage: :e <path>\n\nSupported formats: CSV, TSV, .cell (native format).",
    },
    HelpEntry {
        tags: &[":sort"],
        category: HelpCategory::Command,
        summary: "Sort by column",
        detail: "Sort all rows by the values in a column.\nUsage: :sort <column> [asc|desc]\n\nExamples:\n  :sort A        Sort by column A ascending\n  :sort B desc   Sort by column B descending",
    },
    HelpEntry {
        tags: &[":help"],
        category: HelpCategory::Command,
        summary: "Open help",
        detail: "Show this help screen.\nUsage: :help [topic]\n\n:help          Show table of contents\n:help dd       Show help for the dd command\n:help :w       Show help for the :w command\n:help SUM      Show help for the SUM formula",
    },
];

pub static FORMULA_ENTRIES: &[HelpEntry] = &[
    HelpEntry {
        tags: &["SUM"],
        category: HelpCategory::Formula,
        summary: "Sum values in a range",
        detail: "Returns the sum of all numeric values in the range.\nNon-numeric cells are ignored.\n\nUsage: =SUM(A1:A10)\n       =SUM(B2:D5)",
    },
    HelpEntry {
        tags: &["AVERAGE"],
        category: HelpCategory::Formula,
        summary: "Average of values in a range",
        detail: "Returns the arithmetic mean of all numeric values in the range.\nNon-numeric cells are ignored. Returns #DIV/0! if no numeric\nvalues are found.\n\nUsage: =AVERAGE(A1:A10)",
    },
    HelpEntry {
        tags: &["COUNT"],
        category: HelpCategory::Formula,
        summary: "Count numeric cells in a range",
        detail: "Returns the number of cells containing numeric values in\nthe range. Non-numeric cells are not counted.\n\nUsage: =COUNT(A1:A10)",
    },
    HelpEntry {
        tags: &["MIN"],
        category: HelpCategory::Formula,
        summary: "Minimum value in a range",
        detail: "Returns the smallest numeric value in the range.\nNon-numeric cells are ignored. Returns 0 if no numeric\nvalues are found.\n\nUsage: =MIN(A1:A10)",
    },
    HelpEntry {
        tags: &["MAX"],
        category: HelpCategory::Formula,
        summary: "Maximum value in a range",
        detail: "Returns the largest numeric value in the range.\nNon-numeric cells are ignored. Returns 0 if no numeric\nvalues are found.\n\nUsage: =MAX(A1:A10)",
    },
    HelpEntry {
        tags: &["IF"],
        category: HelpCategory::Formula,
        summary: "Conditional expression",
        detail: "Returns one value if a condition is true, another if false.\n\nUsage: =IF(condition, value_if_true, value_if_false)\n\nExamples:\n  =IF(A1>10, \"big\", \"small\")\n  =IF(B2, C2, D2)",
    },
];
```

- [ ] **Step 4: Convert help.rs into a directory module and add HelpRegistry::new()**

Restructure `crates/cell-core/src/help.rs` into `crates/cell-core/src/help/mod.rs` (same content), adding the entries import and `new()` constructor:

At the top of the file, add:
```rust
pub mod entries;
```

Add this method to `impl HelpRegistry`:
```rust
    /// Build the default registry with all built-in help entries.
    pub fn new() -> Self {
        use entries::*;
        Self::from_entries(&[
            NORMAL_ENTRIES,
            INSERT_ENTRIES,
            VISUAL_ENTRIES,
            COMMAND_ENTRIES,
            FORMULA_ENTRIES,
        ])
    }
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd /Users/garrit/.superset/worktrees/cell/help-command && cargo test -p cell-core -- help`
Expected: All 6 tests pass (5 original + 1 new).

- [ ] **Step 6: Commit**

```bash
git add crates/cell-core/src/help/
git commit -m "feat: add collocated help entries for all modes, commands, and formulas"
```

---

### Task 3: Add Mode::Help and Action::ShowHelp to cell-tui

**Files:**
- Modify: `crates/cell-tui/src/action.rs:17-65`
- Modify: `crates/cell-tui/src/app.rs:1-51`

- [ ] **Step 1: Add `Help` variant to Mode enum**

In `crates/cell-tui/src/action.rs`, add `Help` to the Mode enum:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
    VisualBlock,
    Command,
    Help,
}
```

- [ ] **Step 2: Add `ShowHelp` variant to Action enum**

In `crates/cell-tui/src/action.rs`, add to the Action enum:

```rust
    ShowHelp(Option<String>),
```

- [ ] **Step 3: Add help state to App and initialize the registry**

In `crates/cell-tui/src/app.rs`, add the import:

```rust
use cell_core::help::HelpRegistry;
```

Add fields to `App`:

```rust
    pub help_scroll: usize,
    pub help_topic: Option<String>,
    pub help_registry: HelpRegistry,
```

Initialize in `App::new()`:

```rust
    help_scroll: 0,
    help_topic: None,
    help_registry: HelpRegistry::new(),
```

- [ ] **Step 4: Handle ShowHelp in process_action**

In `crates/cell-tui/src/app.rs`, add to the `match action` block in `process_action`, before the `Action::Open(_) | Action::Resize` arm:

```rust
            Action::ShowHelp(topic) => {
                match topic {
                    Some(ref tag) => {
                        if self.help_registry.find(tag).is_some() {
                            self.help_topic = topic;
                            self.help_scroll = 0;
                            self.mode = Mode::Help;
                        } else {
                            self.status_message = Some(format!("No help for '{}'", tag));
                        }
                    }
                    None => {
                        self.help_topic = None;
                        self.help_scroll = 0;
                        self.mode = Mode::Help;
                    }
                }
            }
```

- [ ] **Step 5: Add Mode::Help display string to status_bar.rs**

In `crates/cell-tui/src/render/status_bar.rs`, add to the `mode_str` match:

```rust
            Mode::Help => " HELP ",
```

- [ ] **Step 6: Verify it compiles**

Run: `cd /Users/garrit/.superset/worktrees/cell/help-command && cargo check -p cell-tui`
Expected: Compiles (may have warnings about unmatched `ShowHelp` in tests — that's fine).

- [ ] **Step 7: Commit**

```bash
git add crates/cell-tui/src/action.rs crates/cell-tui/src/app.rs crates/cell-tui/src/render/status_bar.rs
git commit -m "feat: add Mode::Help and Action::ShowHelp to app state"
```

---

### Task 4: Parse `:help` command

**Files:**
- Modify: `crates/cell-tui/src/mode/command.rs:5-29`

- [ ] **Step 1: Write failing tests for help command parsing**

Add to the test module in `crates/cell-tui/src/mode/command.rs`:

```rust
    #[test]
    fn parse_help_no_topic() {
        assert_eq!(parse_command("help"), Action::ShowHelp(None));
    }

    #[test]
    fn parse_help_with_topic() {
        assert_eq!(parse_command("help dd"), Action::ShowHelp(Some("dd".into())));
    }

    #[test]
    fn parse_help_with_command_topic() {
        assert_eq!(parse_command("help :w"), Action::ShowHelp(Some(":w".into())));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd /Users/garrit/.superset/worktrees/cell/help-command && cargo test -p cell-tui -- parse_help`
Expected: FAIL — `ShowHelp` variant not matched.

- [ ] **Step 3: Add help parsing to parse_command**

In `crates/cell-tui/src/mode/command.rs`, in the `parse_command` function, add before the final `else` block:

```rust
    } else if input == "help" {
        Action::ShowHelp(None)
    } else if let Some(stripped) = input.strip_prefix("help ") {
        let topic = stripped.trim();
        if topic.is_empty() {
            Action::ShowHelp(None)
        } else {
            Action::ShowHelp(Some(topic.to_string()))
        }
    } else {
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd /Users/garrit/.superset/worktrees/cell/help-command && cargo test -p cell-tui -- parse_help`
Expected: All 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/cell-tui/src/mode/command.rs
git commit -m "feat: parse :help command with optional topic argument"
```

---

### Task 5: Help view renderer

**Files:**
- Create: `crates/cell-tui/src/render/help.rs`
- Modify: `crates/cell-tui/src/render/mod.rs:1-49`

- [ ] **Step 1: Create the help renderer**

Create `crates/cell-tui/src/render/help.rs`:

```rust
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Widget,
};
use cell_core::help::{HelpCategory, HelpRegistry};

pub struct HelpView<'a> {
    pub registry: &'a HelpRegistry,
    pub topic: Option<&'a str>,
    pub scroll: usize,
}

impl<'a> HelpView<'a> {
    /// Render the table of contents into lines.
    fn render_toc(&self) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push(String::new());
        lines.push("Cell — Terminal Spreadsheet Editor".to_string());
        lines.push(String::new());
        lines.push("Use :help <topic> for details on any entry below.".to_string());
        lines.push(String::new());

        for category in self.registry.categories() {
            lines.push(String::new());
            lines.push(category.label().to_string());
            lines.push(String::new());

            for entry in self.registry.by_category(category) {
                let tag = entry.tags[0];
                let padding = 16usize.saturating_sub(tag.len());
                lines.push(format!("  {}{}{}", tag, " ".repeat(padding), entry.summary));
            }
        }

        lines.push(String::new());
        lines
    }

    /// Render a specific topic's detail view into lines.
    fn render_topic(&self, tag: &str) -> Vec<String> {
        let mut lines = Vec::new();

        if let Some(entry) = self.registry.find(tag) {
            lines.push(String::new());

            // Title: all tags for this entry
            let all_tags: Vec<&str> = entry.tags.iter().copied().collect();
            lines.push(all_tags.join(", "));
            lines.push(String::new());

            // Summary as a heading
            lines.push(entry.summary.to_string());
            lines.push(String::new());

            // Detail text
            for line in entry.detail.lines() {
                lines.push(line.to_string());
            }

            lines.push(String::new());
            lines.push(format!("Category: {}", entry.category.label()));
            lines.push(String::new());
        }

        lines
    }

    fn content_lines(&self) -> Vec<String> {
        match self.topic {
            Some(tag) => self.render_topic(tag),
            None => self.render_toc(),
        }
    }
}

impl<'a> Widget for HelpView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 3 { return; }

        let title_area = Rect { height: 1, ..area };
        let content_area = Rect {
            y: area.y + 1,
            height: area.height.saturating_sub(2),
            ..area
        };
        let footer_area = Rect {
            y: area.y + area.height.saturating_sub(1),
            height: 1,
            ..area
        };

        // Title bar
        let title_style = Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD);
        let title_text = match self.topic {
            Some(tag) => format!(" Cell — Help: {}", tag),
            None => " Cell — Help".to_string(),
        };
        for x in title_area.x..title_area.x + title_area.width {
            buf.set_string(x, title_area.y, " ", title_style);
        }
        buf.set_string(title_area.x, title_area.y, &title_text, title_style);

        // Content
        let lines = self.content_lines();
        let visible_height = content_area.height as usize;
        for (i, line) in lines.iter().skip(self.scroll).take(visible_height).enumerate() {
            let y = content_area.y + i as u16;
            let truncated: String = line.chars().take(content_area.width as usize).collect();
            buf.set_string(content_area.x + 1, y, &truncated, Style::default());
        }

        // Footer
        let footer_style = Style::default().fg(Color::Black).bg(Color::White);
        for x in footer_area.x..footer_area.x + footer_area.width {
            buf.set_string(x, footer_area.y, " ", footer_style);
        }
        let footer_text = " Press q to return │ j/k scroll │ :help <topic>";
        buf.set_string(footer_area.x, footer_area.y, footer_text, footer_style);
    }
}
```

- [ ] **Step 2: Integrate the help renderer into the main render function**

In `crates/cell-tui/src/render/mod.rs`, add the module:

```rust
pub mod help;
```

Add the import:

```rust
use help::HelpView;
```

Replace the `render` function with:

```rust
pub fn render(frame: &mut Frame, app: &App) {
    if app.mode == Mode::Help {
        frame.render_widget(HelpView {
            registry: &app.help_registry,
            topic: app.help_topic.as_deref(),
            scroll: app.help_scroll,
        }, frame.area());
        return;
    }

    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    let cell_content = app.sheet.get_cell(app.cursor).map(|c| c.raw.as_str()).unwrap_or("");
    let display_content = if app.mode == Mode::Insert { &app.insert_buffer } else { cell_content };
    frame.render_widget(FormulaBar {
        cursor: app.cursor, content: display_content, is_editing: app.mode == Mode::Insert,
    }, chunks[0]);

    frame.render_widget(Grid {
        sheet: &app.sheet, viewport: &app.viewport, cursor: app.cursor, selection: None,
    }, chunks[1]);

    let file_name = app.file_path.as_ref().and_then(|p| p.file_name()).and_then(|n| n.to_str());
    frame.render_widget(StatusBar {
        mode: app.mode, row_count: app.sheet.row_count, col_count: app.sheet.col_count,
        cursor: app.cursor, dirty: app.dirty, file_name, message: app.status_message.as_deref(),
    }, chunks[2]);

    let is_command = app.mode == Mode::Command;
    frame.render_widget(CommandLine {
        content: &app.command_line, prefix: ':', active: is_command,
    }, chunks[3]);
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cd /Users/garrit/.superset/worktrees/cell/help-command && cargo check -p cell-tui`
Expected: Compiles successfully.

- [ ] **Step 4: Commit**

```bash
git add crates/cell-tui/src/render/help.rs crates/cell-tui/src/render/mod.rs
git commit -m "feat: add full-screen help view renderer with TOC and topic detail"
```

---

### Task 6: Help mode input handling in run_loop

**Files:**
- Modify: `crates/cell-tui/src/main.rs:78-221`

- [ ] **Step 1: Add Help mode branch to the input handler**

In `crates/cell-tui/src/main.rs`, in the `run_loop` function, add a new arm to the `match app.mode` block (after `Mode::Command` and before the closing `};`):

```rust
                    Mode::Help => {
                        if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                            match key.code {
                                KeyCode::Char('d') => {
                                    app.help_scroll += grid_height / 2;
                                    Action::Noop
                                }
                                KeyCode::Char('u') => {
                                    app.help_scroll = app.help_scroll.saturating_sub(grid_height / 2);
                                    Action::Noop
                                }
                                _ => Action::Noop,
                            }
                        } else {
                            match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => Action::ChangeMode(Mode::Normal),
                                KeyCode::Char('j') | KeyCode::Down => {
                                    app.help_scroll += 1;
                                    Action::Noop
                                }
                                KeyCode::Char('k') | KeyCode::Up => {
                                    app.help_scroll = app.help_scroll.saturating_sub(1);
                                    Action::Noop
                                }
                                KeyCode::Char('g') => {
                                    // gg: go to top. Consume the pending g.
                                    normal_state.pending = Some('g');
                                    Action::Noop
                                }
                                KeyCode::Char('G') => {
                                    app.help_scroll = usize::MAX;
                                    Action::Noop
                                }
                                KeyCode::Char(':') => Action::ChangeMode(Mode::Command),
                                _ => {
                                    // Handle pending gg sequence
                                    if normal_state.pending == Some('g') {
                                        normal_state.pending = None;
                                        app.help_scroll = 0;
                                    }
                                    Action::Noop
                                }
                            }
                        }
                    }
```

Wait — the `gg` handling needs to be cleaner. Let me revise. The `g` key sets pending, then if the next key is `g` we go to top. But in the current structure, Help mode is its own match arm. Let's handle gg directly:

```rust
                    Mode::Help => {
                        if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                            match key.code {
                                KeyCode::Char('d') => {
                                    app.help_scroll += grid_height / 2;
                                    Action::Noop
                                }
                                KeyCode::Char('u') => {
                                    app.help_scroll = app.help_scroll.saturating_sub(grid_height / 2);
                                    Action::Noop
                                }
                                _ => Action::Noop,
                            }
                        } else if help_pending_g {
                            help_pending_g = false;
                            if key.code == KeyCode::Char('g') {
                                app.help_scroll = 0;
                            }
                            Action::Noop
                        } else {
                            match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => Action::ChangeMode(Mode::Normal),
                                KeyCode::Char('j') | KeyCode::Down => {
                                    app.help_scroll += 1;
                                    Action::Noop
                                }
                                KeyCode::Char('k') | KeyCode::Up => {
                                    app.help_scroll = app.help_scroll.saturating_sub(1);
                                    Action::Noop
                                }
                                KeyCode::Char('g') => {
                                    help_pending_g = true;
                                    Action::Noop
                                }
                                KeyCode::Char('G') => {
                                    app.help_scroll = usize::MAX;
                                    Action::Noop
                                }
                                KeyCode::Char(':') => Action::ChangeMode(Mode::Command),
                                _ => Action::Noop,
                            }
                        }
                    }
```

This requires adding `let mut help_pending_g = false;` at the top of `run_loop`, alongside the other state variables (near line 91).

- [ ] **Step 2: Handle return to Help mode after :help command from within Help mode**

When the user types `:help <topic>` while already in Help mode, the command parser returns `Action::ShowHelp(...)` which `process_action` handles — it sets `mode = Help` and updates the topic. This works without extra code since the mode is already Help or gets set to Help. But if the command execution sets mode to Normal first (from the Command mode exit), we need to ensure `ShowHelp` sets mode to Help.

Check: In the current `Mode::Command` handler, when `Execute(cmd)` fires, it calls `parse_command(&cmd)` and returns the result. The `ChangeMode(Mode::Normal)` is NOT automatically added — the parsed action is returned directly. For `:help`, `parse_command` returns `ShowHelp(...)`, and `process_action` sets mode to `Help`. This is correct — no extra code needed.

However, when `ChangeMode(Mode::Command)` is processed from Help mode, we should track that we came from Help mode so we can return there on cancel. The simplest approach: on `CommandAction::Cancel` in command mode, we always go to Normal mode. This is acceptable — if the user cancels out of the command line, they go to Normal (which is what Vim does too). They can type `:help` again.

No code change needed for this step.

- [ ] **Step 3: Verify it compiles and run all existing tests**

Run: `cd /Users/garrit/.superset/worktrees/cell/help-command && cargo test`
Expected: All tests pass, no compilation errors.

- [ ] **Step 4: Commit**

```bash
git add crates/cell-tui/src/main.rs
git commit -m "feat: add Help mode input handling with vim-like scrolling"
```

---

### Task 7: Clamp help scroll and final integration test

**Files:**
- Modify: `crates/cell-tui/src/render/help.rs`
- Modify: `crates/cell-tui/src/app.rs`

- [ ] **Step 1: Write a test that ShowHelp action sets mode correctly**

Add to a new test module in `crates/cell-tui/src/app.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Action;

    #[test]
    fn show_help_toc_sets_help_mode() {
        let mut app = App::new();
        app.process_action(Action::ShowHelp(None));
        assert_eq!(app.mode, Mode::Help);
        assert_eq!(app.help_topic, None);
        assert_eq!(app.help_scroll, 0);
    }

    #[test]
    fn show_help_valid_topic() {
        let mut app = App::new();
        app.process_action(Action::ShowHelp(Some("dd".into())));
        assert_eq!(app.mode, Mode::Help);
        assert_eq!(app.help_topic, Some("dd".into()));
    }

    #[test]
    fn show_help_invalid_topic_stays_normal() {
        let mut app = App::new();
        app.mode = Mode::Normal;
        app.process_action(Action::ShowHelp(Some("nonexistent".into())));
        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(app.status_message, Some("No help for 'nonexistent'".into()));
    }

    #[test]
    fn help_mode_back_to_normal() {
        let mut app = App::new();
        app.process_action(Action::ShowHelp(None));
        assert_eq!(app.mode, Mode::Help);
        app.process_action(Action::ChangeMode(Mode::Normal));
        assert_eq!(app.mode, Mode::Normal);
    }
}
```

- [ ] **Step 2: Run the tests to verify they pass**

Run: `cd /Users/garrit/.superset/worktrees/cell/help-command && cargo test -p cell-tui -- show_help`
Expected: All 4 tests pass.

- [ ] **Step 3: Add scroll clamping to the help view**

The `help_scroll` can be set to `usize::MAX` by pressing `G`. The renderer should clamp it. In `crates/cell-tui/src/render/help.rs`, modify the `Widget::render` implementation's content section:

Replace:
```rust
        let lines = self.content_lines();
        let visible_height = content_area.height as usize;
        for (i, line) in lines.iter().skip(self.scroll).take(visible_height).enumerate() {
```

With:
```rust
        let lines = self.content_lines();
        let visible_height = content_area.height as usize;
        let max_scroll = lines.len().saturating_sub(visible_height);
        let scroll = self.scroll.min(max_scroll);
        for (i, line) in lines.iter().skip(scroll).take(visible_height).enumerate() {
```

- [ ] **Step 4: Run the full test suite**

Run: `cd /Users/garrit/.superset/worktrees/cell/help-command && cargo test`
Expected: All tests pass.

- [ ] **Step 5: Run clippy**

Run: `cd /Users/garrit/.superset/worktrees/cell/help-command && cargo clippy -- -D warnings`
Expected: No warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/cell-tui/src/app.rs crates/cell-tui/src/render/help.rs
git commit -m "feat: add help integration tests and scroll clamping"
```
