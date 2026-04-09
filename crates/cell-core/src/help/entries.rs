use super::{HelpCategory, HelpEntry};

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
