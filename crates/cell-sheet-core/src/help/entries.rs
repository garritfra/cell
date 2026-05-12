use super::{HelpCategory, HelpEntry};

pub static NORMAL_ENTRIES: &[HelpEntry] = &[
    HelpEntry {
        tags: &["count", "[count]", "N"],
        category: HelpCategory::Normal,
        summary: "Numeric count prefix",
        detail: "Type digits before a motion or operator to repeat or scale it,\n\
                 vim-style. Examples: 5j moves 5 rows down, 10G jumps to row 10,\n\
                 3dd deletes 3 rows, 4yy yanks 4 rows, 2w hops 2 non-empty cells.\n\
                 The count is shown in the status line as you type.\n\n\
                 0 alone goes to the first column; only after a non-zero digit\n\
                 does 0 extend the count (so 10j really moves 10 rows).\n\n\
                 Esc cancels a partially-typed count. Counts apply to:\n\
                 h j k l, Up/Down/Left/Right, w b, gg G, dd yy.\n\n\
                 Operator-pending counts: you can also place a count between the\n\
                 operator and the motion (e.g. d3j, y2k). If both an outer count\n\
                 and an inner motion count are given they multiply: 5d2j clears\n\
                 10 rows downward from the cursor.",
    },
    HelpEntry {
        tags: &["h"],
        category: HelpCategory::Normal,
        summary: "Move cursor left ([count]h)",
        detail: "Move the cursor one column to the left. Stops at column A.\n\
                 With a count prefix, moves [count] columns.\n\
                 Alias: Left arrow",
    },
    HelpEntry {
        tags: &["j"],
        category: HelpCategory::Normal,
        summary: "Move cursor down ([count]j)",
        detail: "Move the cursor one row down. With a count prefix, moves\n\
                 [count] rows.\nAlias: Down arrow",
    },
    HelpEntry {
        tags: &["k"],
        category: HelpCategory::Normal,
        summary: "Move cursor up ([count]k)",
        detail: "Move the cursor one row up. Stops at row 1. With a count\n\
                 prefix, moves [count] rows.\nAlias: Up arrow",
    },
    HelpEntry {
        tags: &["l"],
        category: HelpCategory::Normal,
        summary: "Move cursor right ([count]l)",
        detail: "Move the cursor one column to the right. With a count prefix,\n\
                 moves [count] columns.\nAlias: Right arrow",
    },
    HelpEntry {
        tags: &["gg"],
        category: HelpCategory::Normal,
        summary: "Go to first row / row N ([count]gg)",
        detail: "Move the cursor to row 1, keeping the current column.\n\
                 With a count prefix, jumps to row [count] (1-indexed).",
    },
    HelpEntry {
        tags: &["G"],
        category: HelpCategory::Normal,
        summary: "Go to last row / row N ([count]G)",
        detail: "Move the cursor to the last row with data, keeping the\n\
                 current column. With a count prefix, jumps to row [count]\n\
                 (1-indexed) instead.",
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
        summary: "Next non-empty cell ([count]w)",
        detail: "Jump to the next non-empty cell to the right in the current\n\
                 row. With a count prefix, hops [count] non-empty cells.",
    },
    HelpEntry {
        tags: &["b"],
        category: HelpCategory::Normal,
        summary: "Previous non-empty cell ([count]b)",
        detail: "Jump to the previous non-empty cell to the left in the\n\
                 current row. With a count prefix, hops [count] non-empty\n\
                 cells back.",
    },
    HelpEntry {
        tags: &["dd"],
        category: HelpCategory::Normal,
        summary: "Delete current row ([count]dd)",
        detail: "Deletes all cells in the current row. With a count prefix,\n\
                 deletes [count] rows starting at the cursor; the deleted\n\
                 rows are stored line-wise in the register and can be\n\
                 pasted as a block with p (below) or P (above). Undoable\n\
                 with u.\n\n\
                 To clear a range using a motion instead of repeating the\n\
                 operator, use d{motion}: see dj, dk, dl, dh.",
    },
    HelpEntry {
        tags: &["dj", "d3j"],
        category: HelpCategory::Normal,
        summary: "Clear rows downward (d[count]j)",
        detail: "Clear the current row and [count] rows below it (content only;\n\
                 rows are not removed). Without a count, clears the current row\n\
                 and the one below. An outer count before d multiplies the motion\n\
                 count: 5d2j clears 10 rows. Undoable with u.",
    },
    HelpEntry {
        tags: &["dk", "d2k"],
        category: HelpCategory::Normal,
        summary: "Clear rows upward (d[count]k)",
        detail: "Clear [count] rows above the cursor and the current row\n\
                 (content only; rows are not removed). Without a count, clears\n\
                 the current row and the one above. Undoable with u.",
    },
    HelpEntry {
        tags: &["dl"],
        category: HelpCategory::Normal,
        summary: "Clear cells rightward (d[count]l)",
        detail: "Clear the current cell and [count] cells to the right.\n\
                 Without a count, clears the current cell and the one to its\n\
                 right. Undoable with u.",
    },
    HelpEntry {
        tags: &["dh"],
        category: HelpCategory::Normal,
        summary: "Clear cells leftward (d[count]h)",
        detail: "Clear [count] cells to the left and the current cell.\n\
                 Without a count, clears the cell to the left and the current\n\
                 cell. Undoable with u.",
    },
    HelpEntry {
        tags: &["yy"],
        category: HelpCategory::Normal,
        summary: "Yank current row ([count]yy)",
        detail: "Copies all cells in the current row to the register. With a\n\
                 count prefix, yanks [count] rows starting at the cursor.\n\
                 Paste with p (below) or P (above).\n\n\
                 To yank a range using a motion, use y{motion}: see yj, yk,\n\
                 yl, yh.",
    },
    HelpEntry {
        tags: &["yj"],
        category: HelpCategory::Normal,
        summary: "Yank rows downward (y[count]j)",
        detail: "Yank the current row and [count] rows below it into the register.\n\
                 Paste with p (below) or P (above). An outer count multiplies\n\
                 the motion count: 5y2j yanks 10 rows.",
    },
    HelpEntry {
        tags: &["yk"],
        category: HelpCategory::Normal,
        summary: "Yank rows upward (y[count]k)",
        detail: "Yank [count] rows above the cursor and the current row into the\n\
                 register. Paste with p (below) or P (above).",
    },
    HelpEntry {
        tags: &["yl", "y3l", "y4l"],
        category: HelpCategory::Normal,
        summary: "Yank cells rightward (y[count]l)",
        detail: "Yank the current cell and [count] cells to the right into the\n\
                 register. Without a count, yanks the current cell and the one\n\
                 to its right. Paste with p.",
    },
    HelpEntry {
        tags: &["yh"],
        category: HelpCategory::Normal,
        summary: "Yank cells leftward (y[count]h)",
        detail: "Yank [count] cells to the left and the current cell into the\n\
                 register. Without a count, yanks the cell to the left and the\n\
                 current cell. Paste with p.",
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
        tags: &["."],
        category: HelpCategory::Normal,
        summary: "Repeat last change",
        detail: "Re-apply the last cell-mutating operation at the current cursor \
                 position. Works after x, dd, d (visual), p, P, and any edit committed from \
                 Insert mode (i/a/c + Esc or Enter). u and Ctrl-r do not affect \
                 the repeat register.",
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
        tags: &["Ctrl+E"],
        category: HelpCategory::Normal,
        summary: "Scroll viewport down one row",
        detail: "Scroll the viewport down one row without moving the cursor.\n\
                 If the cursor would scroll off the top, it stays pinned to\n\
                 the top visible row.",
    },
    HelpEntry {
        tags: &["Ctrl+Y"],
        category: HelpCategory::Normal,
        summary: "Scroll viewport up one row",
        detail: "Scroll the viewport up one row without moving the cursor.\n\
                 If the cursor would scroll off the bottom, it stays pinned\n\
                 to the bottom visible row.",
    },
    HelpEntry {
        tags: &["zz"],
        category: HelpCategory::Normal,
        summary: "Recenter viewport on cursor",
        detail: "Scroll the viewport so the cursor sits at the vertical\n\
                 center of the visible rows. Cursor position is unchanged.",
    },
    HelpEntry {
        tags: &["zt"],
        category: HelpCategory::Normal,
        summary: "Scroll cursor to top of viewport",
        detail: "Scroll the viewport so the cursor row becomes the top\n\
                 visible row. Cursor position is unchanged.",
    },
    HelpEntry {
        tags: &["zb"],
        category: HelpCategory::Normal,
        summary: "Scroll cursor to bottom of viewport",
        detail: "Scroll the viewport so the cursor row becomes the bottom\n\
                 visible row. Cursor position is unchanged.",
    },
    HelpEntry {
        tags: &["H"],
        category: HelpCategory::Normal,
        summary: "Cursor to top of viewport",
        detail: "Move the cursor to the topmost visible row, keeping the\n\
                 current column. Viewport is unchanged.",
    },
    HelpEntry {
        tags: &["M"],
        category: HelpCategory::Normal,
        summary: "Cursor to middle of viewport",
        detail: "Move the cursor to the middle visible row, keeping the\n\
                 current column. Viewport is unchanged.",
    },
    HelpEntry {
        tags: &["L"],
        category: HelpCategory::Normal,
        summary: "Cursor to bottom of viewport",
        detail: "Move the cursor to the bottommost visible row, keeping the\n\
                 current column. Viewport is unchanged.",
    },
    HelpEntry {
        tags: &["m", "mark"],
        category: HelpCategory::Normal,
        summary: "Set mark (m{a-z})",
        detail: "After pressing m, the next lowercase letter records the\n\
                 current cursor position as a named mark. Marks are\n\
                 session-only. Jump back with '{a-z} (line-wise) or\n\
                 `{a-z} (exact cell).",
    },
    HelpEntry {
        tags: &["'"],
        category: HelpCategory::Normal,
        summary: "Jump to mark (line-wise)",
        detail: "After pressing ', the next lowercase letter jumps the\n\
                 cursor to the row of the matching mark, at column A.\n\
                 If the mark is unset, status reports `E20: Mark not set`.",
    },
    HelpEntry {
        tags: &["`", "backtick"],
        category: HelpCategory::Normal,
        summary: "Jump to mark (exact cell)",
        detail: "After pressing `, the next lowercase letter jumps the\n\
                 cursor to the exact cell of the matching mark.\n\
                 If the mark is unset, status reports `E20: Mark not set`.",
    },
    HelpEntry {
        tags: &["Ctrl+O"],
        category: HelpCategory::Normal,
        summary: "Jump back in jump list",
        detail: "Move backward through the jump list, which records cursor\n\
                 positions across long-distance motions (gg, G, marks,\n\
                 search). Pairs with Ctrl+I / Tab to jump forward. The\n\
                 jump list is capped at 100 entries.",
    },
    HelpEntry {
        tags: &["Ctrl+I", "Tab"],
        category: HelpCategory::Normal,
        summary: "Jump forward in jump list",
        detail: "Move forward through the jump list. Pairs with Ctrl+O\n\
                 to jump back. Mid-stack jumps truncate the forward history.",
    },
    HelpEntry {
        tags: &["{"],
        category: HelpCategory::Normal,
        summary: "Block jump up in column",
        detail: "Jump to the previous block boundary in the current column,\n\
                 mirroring vim's paragraph motion. From a non-empty cell,\n\
                 lands on the first empty row above the current block;\n\
                 from an empty cell, lands on the next non-empty row above.",
    },
    HelpEntry {
        tags: &["}"],
        category: HelpCategory::Normal,
        summary: "Block jump down in column",
        detail: "Jump to the next block boundary in the current column,\n\
                 mirroring vim's paragraph motion. From a non-empty cell,\n\
                 lands on the first empty row below the current block;\n\
                 from an empty cell, lands on the next non-empty row below.",
    },
    HelpEntry {
        tags: &["*"],
        category: HelpCategory::Normal,
        summary: "Search current cell value forward",
        detail: "Treat the current cell's displayed value as the search\n\
                 pattern and jump to the next matching cell. The pattern\n\
                 is stored, so n / N continue stepping through matches.",
    },
    HelpEntry {
        tags: &["#"],
        category: HelpCategory::Normal,
        summary: "Search current cell value backward",
        detail: "Treat the current cell's displayed value as the search\n\
                 pattern and jump to the previous matching cell. The pattern\n\
                 is stored, so n / N continue stepping through matches.",
    },
    HelpEntry {
        tags: &["gv"],
        category: HelpCategory::Normal,
        summary: "Re-enter previous visual selection",
        detail: "Re-enter Visual mode with the same anchor, cursor, and\n\
                 visual kind (Character / Line / Block) as the last\n\
                 selection. No-op if no previous selection exists.",
    },
    HelpEntry {
        tags: &["c"],
        category: HelpCategory::Normal,
        summary: "Change cell",
        detail: "Clear the current cell and enter Insert mode to type its new\n\
                 content. Equivalent to x followed by i.",
    },
    HelpEntry {
        tags: &["V"],
        category: HelpCategory::Normal,
        summary: "Enter Visual Line mode",
        detail: "Start full-row (line-wise) visual selection from the\n\
                 current row. Use j/k to extend; press d/y/c to act on the\n\
                 selected rows.",
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
        summary: "Search forward",
        detail: "Open the forward-search prompt. Type a pattern and press Enter to\nfind the next cell whose value contains the pattern.\nCase-insensitive. Use n / N to step through matches.",
    },
    HelpEntry {
        tags: &["?"],
        category: HelpCategory::Normal,
        summary: "Search backward",
        detail: "Open the backward-search prompt. Type a pattern and press Enter\nto find the previous cell whose value contains the pattern.\nCase-insensitive. Use n / N to step through matches.",
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
    HelpEntry {
        tags: &["f"],
        category: HelpCategory::Normal,
        summary: "Find char in row (forward)",
        detail: "After f, the next keypress is the target character. The cursor\njumps to the next non-empty cell in the current row whose displayed\nvalue starts with that character (case-insensitive).\nUse ; to repeat and , to repeat reversed.",
    },
    HelpEntry {
        tags: &["F"],
        category: HelpCategory::Normal,
        summary: "Find char in row (backward)",
        detail: "Like f, but searches the current row to the left of the cursor.",
    },
    HelpEntry {
        tags: &[";"],
        category: HelpCategory::Normal,
        summary: "Repeat last find",
        detail: "Repeat the last f / F find in the same direction.",
    },
    HelpEntry {
        tags: &[","],
        category: HelpCategory::Normal,
        summary: "Repeat last find reversed",
        detail: "Repeat the last f / F find in the opposite direction.",
    },
    HelpEntry {
        tags: &["~", "tilde"],
        category: HelpCategory::Normal,
        summary: "Toggle case of first character",
        detail: "Toggle the case of the first character of the current cell's value\n\
                 and advance the cursor one column to the right (vim `~` semantics\n\
                 scoped to one cell). No-op on formula cells.",
    },
    HelpEntry {
        tags: &["guu"],
        category: HelpCategory::Normal,
        summary: "Lowercase entire cell",
        detail: "Convert every character in the current cell's value to lowercase.\n\
                 No-op on formula cells. Undoable with u.",
    },
    HelpEntry {
        tags: &["gUU"],
        category: HelpCategory::Normal,
        summary: "Uppercase entire cell",
        detail: "Convert every character in the current cell's value to uppercase.\n\
                 No-op on formula cells. Undoable with u.",
    },
    HelpEntry {
        tags: &["g~~"],
        category: HelpCategory::Normal,
        summary: "Toggle case of entire cell",
        detail: "Toggle the case of every character in the current cell's value\n\
                 (uppercase ↔ lowercase). No-op on formula cells. Undoable with u.",
    },
    HelpEntry {
        tags: &["Ctrl+A"],
        category: HelpCategory::Normal,
        summary: "Increment number under cursor ([count]Ctrl+A)",
        detail: "Add 1 (or [count]) to the number stored in the current cell.\n\
                 Dependent formula cells are re-evaluated automatically.\n\
                 No-op if the cell contains a formula (shows an error message)\n\
                 or non-numeric text. Repeatable with '.'.",
    },
    HelpEntry {
        tags: &["Ctrl+X"],
        category: HelpCategory::Normal,
        summary: "Decrement number under cursor ([count]Ctrl+X)",
        detail: "Subtract 1 (or [count]) from the number stored in the current cell.\n\
                 Dependent formula cells are re-evaluated automatically.\n\
                 No-op if the cell contains a formula (shows an error message)\n\
                 or non-numeric text. Repeatable with '.'.",
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
        tags: &["count-visual", "[count]-visual"],
        category: HelpCategory::Visual,
        summary: "Count prefix in Visual mode ([count]motion)",
        detail: "Type digits before a motion key (h j k l) to extend the\n\
                 selection by that many cells. For example, v then 3l selects\n\
                 the current cell plus 3 more to the right; 5j extends the\n\
                 selection 5 rows down.",
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
    HelpEntry {
        tags: &["u-visual"],
        category: HelpCategory::Visual,
        summary: "Lowercase selection",
        detail: "Convert every character in every selected cell to lowercase.\n\
                 Formula cells in the selection are skipped. Undoable with u\n\
                 after returning to Normal mode.",
    },
    HelpEntry {
        tags: &["U-visual"],
        category: HelpCategory::Visual,
        summary: "Uppercase selection",
        detail: "Convert every character in every selected cell to uppercase.\n\
                 Formula cells in the selection are skipped. Undoable with u\n\
                 after returning to Normal mode.",
    },
    HelpEntry {
        tags: &["~-visual"],
        category: HelpCategory::Visual,
        summary: "Toggle case of selection",
        detail: "Toggle the case of every character in every selected cell\n\
                 (uppercase ↔ lowercase). Formula cells in the selection are\n\
                 skipped. Undoable with u after returning to Normal mode.",
    },
];

pub static COMMAND_ENTRIES: &[HelpEntry] = &[
    HelpEntry {
        tags: &[":w", ":write"],
        category: HelpCategory::Command,
        summary: "Save file",
        detail: "Write the current sheet to disk. If no filename has been set,\nuse :w <path> to specify one.\n\nIf the sheet contains formulas and you save as CSV/TSV,\nCell will warn you. Use :w file.cell to preserve formulas,\nor :w! to force save as CSV (formulas become values).\n\nIf the active delimiter (see :set delimiter) does not match\nthe file extension convention, Cell will also warn you.\nUse :w! to override.",
    },
    HelpEntry {
        tags: &[":w!"],
        category: HelpCategory::Command,
        summary: "Force save",
        detail: "Write the current sheet to disk, bypassing both the\nformula-flatten warning and the non-standard-delimiter warning.",
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
        tags: &[":set delimiter", "--delimiter", "delimiter"],
        category: HelpCategory::Command,
        summary: "Set the field delimiter",
        detail: "Set the delimiter character used when reading or saving\nCSV/TSV files.\n\nUsage (ex-command):  :set delimiter=|\n                     :set delimiter=;\nUsage (CLI flag):    cell data.psv --delimiter '|'\n\nValid delimiters: any single printable ASCII character that\nis not a letter, digit, or double-quote (e.g. | ; , \\t).\n\nThe delimiter is auto-detected from file content on open\nwhen the --delimiter flag is not provided and the extension\nis not .tsv. Use --delimiter to override detection.\n\n:set delimiter only affects the next save — it does not\nre-parse the currently loaded file. To reload with a new\ndelimiter, close and reopen the file with --delimiter.",
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
    HelpEntry {
        tags: &["=", "<>", "equality", "comparison"],
        category: HelpCategory::Formula,
        summary: "Equality operators (`=`, `<>`)",
        detail: "Compare two values for equality (`=`) or inequality (`<>`).\n\nValues are compared within their own type:\n  - Number vs. Number: numeric comparison.\n  - Text vs. Text:    case-INSENSITIVE string comparison.\n  - Bool vs. Bool:    boolean comparison.\n  - Empty vs. Empty:  equal.\n  - Mixed types:      `=` is FALSE, `<>` is TRUE (no error).\n\nOrdering operators (`<`, `<=`, `>`, `>=`) are numeric-only.\n\nExamples:\n  =IF(A1=C3, A2, 0)         compare two text cells\n  =IF(\"foo\"=\"FOO\", 1, 0)    -> 1 (case-insensitive)\n  =1<>\"1\"                   -> TRUE (mixed types)",
    },
];

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
                 Dragging a cell selection past the visible edge auto-\n\
                 scrolls the viewport one row/column per drag event.",
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
