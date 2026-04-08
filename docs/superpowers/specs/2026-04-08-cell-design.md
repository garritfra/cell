# cell — Terminal Spreadsheet

A terminal-based spreadsheet editor with Vim keybindings, written in Rust. Aims to sit alongside tools like Vim, sc-im, and other solid Unix software. Intuitive if you know Vim, minimal dependencies, fast.

## Goals

- Edit CSV/TSV files with a real spreadsheet interface in the terminal
- Excel-compatible formulas (ODF subset, compliance tracked over time)
- Full Vim modal editing (Normal, Insert, Visual, Visual Block, Command-line)
- Native `.cell` format to preserve formulas; CSV/TSV for interchange
- Minimal dependencies, single-threaded, synchronous

## Non-Goals (v1)

- Multi-sheet support
- Named registers
- Charts or graphical output
- Async I/O
- Plugin system
- Undo branching (linear undo only)

---

## Architecture

Cargo workspace with two crates:

```
cell/
  Cargo.toml              # workspace root
  crates/
    cell-core/
      Cargo.toml
      src/lib.rs
    cell-tui/
      Cargo.toml
      src/main.rs
```

- **cell-core**: Data model, formula engine, file I/O. No TUI dependency. Independently testable.
- **cell-tui**: Ratatui-based rendering, Vim modal editing, event loop. Depends on cell-core.

---

## Data Model (`cell-core`)

The grid uses sparse storage. Only cells with content are stored.

```rust
struct Sheet {
    cells: HashMap<CellPos, Cell>,
    col_widths: Vec<u16>,
    row_count: usize,
    col_count: usize,
}

struct Cell {
    raw: String,        // what the user typed ("=SUM(A1:A3)" or "hello")
    value: CellValue,   // computed result
    dirty: bool,        // needs recalculation
}

enum CellValue {
    Number(f64),
    Text(String),
    Bool(bool),
    Error(CellError),   // #REF!, #VALUE!, #DIV/0!, etc.
    Empty,
}

type CellPos = (usize, usize); // (row, col), zero-indexed
```

**Key decisions:**
- `HashMap<CellPos, Cell>` — sparse. Large CSVs with mostly empty columns don't blow up memory.
- `raw` vs `value` — the cell always stores what the user typed. If it starts with `=`, it's a formula and `value` holds the computed result. Otherwise `raw` is parsed directly into `value`.
- `Bool` values come from formula evaluation only (e.g., `=A1>5`, `=IF(...)`). Text typed as "TRUE" is stored as `Text`, not `Bool`.
- Single sheet for v1.
- Coordinates are zero-indexed internally, displayed as A1-style to the user (A=0, B=1, ... Z=25, AA=26, etc.).

---

## Formula Engine (`cell-core`)

### Pipeline

```
"=SUM(A1:A3)+1"  →  Tokenize  →  Parse (AST)  →  Evaluate  →  CellValue
```

### Tokenizer

Breaks input into tokens: numbers, strings, cell references (`A1`), ranges (`A1:A3`), operators (`+`, `-`, `*`, `/`, `>`, `<`, `=`, `<>`, `>=`, `<=`), function names, parentheses, commas.

### AST

```rust
struct CellRef {
    col: usize,
    row: usize,
    abs_col: bool,  // true if $A
    abs_row: bool,  // true if $1
}

enum Expr {
    Number(f64),
    Text(String),
    Bool(bool),
    CellRef(CellRef),
    Range { start: CellRef, end: CellRef },
    BinaryOp { op: Op, left: Box<Expr>, right: Box<Expr> },
    UnaryOp { op: Op, expr: Box<Expr> },
    FnCall { name: String, args: Vec<Expr> },
}
```

### Evaluator

Walks the AST, resolves cell references by reading from the Sheet, computes the result. Functions are registered in a dispatch table (`HashMap<&str, fn>`) so new functions are easy to add.

### v1 Functions

`SUM`, `AVERAGE`, `COUNT`, `MIN`, `MAX`, `IF`

### Recalculation

- Dependency graph: `HashMap<CellPos, HashSet<CellPos>>` — maps a cell to all cells that reference it.
- On cell edit, mark dependents as dirty.
- Recalc dirty cells in topological order.
- Circular reference detection: if a cycle is found during topological sort, all cells in the cycle are set to `#CIRC!`.

### ODF Compliance Tracking

A test suite maps each ODF-defined function and operator to a test case. Unimplemented functions are tagged `#[ignore]`, so `cargo test` shows the coverage gap.

---

## File I/O (`cell-core`)

### CSV/TSV Import

- Uses the `csv` crate (RFC 4180 compliant).
- Delimiter auto-detected from file extension (`.csv` → comma, `.tsv` → tab), overridable via flag.
- All values imported as-is into `cell.raw`. No formula interpretation on CSV import — a cell containing `=SUM(A1:A3)` in a CSV is treated as literal text.
- Auto-sizes `col_widths` to content, capped at a maximum.

### CSV/TSV Export

- Formulas flattened — writes `cell.value`, not `cell.raw`.
- Numbers written without trailing zeros. Errors written as their display string (`#DIV/0!`).

### Native `.cell` Format

Plain text, line-oriented, SC-IM inspired. Human-readable and diffable.

```
# cell v1
# Auto-generated — do not edit manually unless you know the format

size 100 26

col-width 0 12
col-width 1 8

let A0 = 42
let B0 = 3.14
label A1 = "Name"
label B1 = "Score"
formula C0 = =A0+B0
formula C1 = =SUM(C0:C0)
```

Format rules:
- `let` — numeric value
- `label` — string value
- `formula` — stores the raw formula text (re-parsed and evaluated on load)
- `col-width` — column display widths
- `size` — row and column extent
- Lines starting with `#` are comments
- Coordinates use zero-indexed rows (A0 = display A1, A1 = display A2). This differs from user-facing display which is 1-indexed.

### Save Behavior

- `:w` / `:wq` on a CSV/TSV with **no formulas** → saves as CSV/TSV.
- `:w` / `:wq` on a CSV/TSV with **formulas** → warns: `Sheet contains formulas that will be lost. Use :w file.cell to preserve, or :w! to save as CSV anyway.`
- `:w!` forces save in the original format regardless.
- `:w file.cell` saves as native. `:w file.csv` saves as CSV (flattened).
- `:w` preserves the format the file was opened with.

---

## Vim Modal Editing (`cell-tui`)

### Modes

```
                 i/a/o          ESC
  ┌─────────┐ ──────────▶ ┌──────────┐
  │  Normal  │             │  Insert  │
  │  (grid)  │ ◀────────── │  (cell)  │
  └─────────┘              └──────────┘
    │     ▲
    │v/V  │ESC
    ▼     │
  ┌──────────┐
  │  Visual  │
  │ / V-Block│
  └──────────┘

  Any mode ── : ──▶ Command-line ── ESC ──▶ back
```

### Normal Mode (grid navigation)

- `h/j/k/l` — move one cell left/down/up/right
- `gg` — first row, `G` — last row
- `0` — first column, `$` — last column
- `Ctrl-D/U` — half-page down/up
- `Ctrl-F/B` — full page down/up
- `w/b` — jump to next/previous non-empty cell in row
- `dd` — delete row (yanks first), `yy` — yank row, `p` — paste below, `P` — paste above
- `x` — delete cell (yanks first)
- `u` — undo, `Ctrl-R` — redo

### Insert Mode (cell editing)

- `i` — edit cell (cursor at end of existing content)
- `a` — same as `i` for cells
- `o` — edit cell below (inserts row if at bottom)
- `ESC` — confirm edit, return to Normal
- Full text editing: arrow keys, backspace, delete, Home/End

### Visual Mode

- `v` — visual (select range by moving with `hjkl`)
- `Ctrl-V` — visual block (rectangular selection)
- `d` — yank then clear selected cells
- `y` — yank selection
- `p` — paste over selection
- Selected range displayed in status bar (e.g., `A1:C5`)

### Command-line Mode

- `:w [file]` — save
- `:q` / `:q!` — quit / force quit
- `:wq` — save and quit
- `:e file` — open file
- `:sort [col] [asc|desc]` — sort by column
- `/pattern` — search cell contents, `n`/`N` — next/prev match

### Yank/Paste System

Single register holds the last yanked content:

```rust
enum Register {
    Cell(CellContent),
    Row(Vec<CellContent>),
    Block(Vec<Vec<CellContent>>),
}
```

`CellContent` clones `raw` — formulas are yanked as formulas, not computed values.

**Formula adjustment on paste:**
- Relative references shift. Yank `=A1+B1` from C1, paste to C3 → `=A3+B3`.
- Absolute references (`$A$1`) don't shift.
- Mixed references (`$A1`, `A$1`) shift only the non-absolute part.

### Undo/Redo

Operation-based, not character-based. Each cell edit, row delete, paste, sort is one undo step. Stored as a stack of `(inverse_operation, forward_operation)` pairs.

---

## TUI Layout (`cell-tui`)

```
┌──────────────────────────────────────────────────┐
│ A1 │ =SUM(B1:B10)                    formula bar │
├──────────────────────────────────────────────────┤
│     │  A       │  B       │  C       │  D        │
├─────┼──────────┼──────────┼──────────┼───────────┤
│  1  │ Name     │ Score    │ Total    │           │
│  2  │ Alice    │ 95       │ 287      │           │
│  3  │ Bob      │ 88       │          │           │
│  4  │ Carol    │ 104      │          │           │
│  ·  │          │          │          │           │
│  ·  │          │          │          │           │
├──────────────────────────────────────────────────┤
│ NORMAL | 4 rows x 3 cols | C1        status bar  │
├──────────────────────────────────────────────────┤
│ :                                    command line │
└──────────────────────────────────────────────────┘
```

**Sections (top to bottom):**
1. **Formula bar** (1 row) — current cell address and `raw` content. During Insert mode, becomes the edit area with a cursor.
2. **Grid** (fills remaining space) — column headers, row numbers, cell values. Only computed `value` displayed, not formulas.
3. **Status bar** (1 row) — current mode, sheet dimensions, cursor position. In Visual mode, shows selected range.
4. **Command line** (1 row) — active when `:` or `/` is typed, otherwise blank.

**Rendering details:**
- Active cell highlighted with distinct background.
- Visual selection highlighted with a different background.
- Column widths independently resizable (`:colwidth A 20` or similar).
- Viewport scrolling — row/column headers stay pinned. Cursor near edges scrolls the viewport.
- Cell content exceeding column width truncated with `…`. Formula bar always shows full content.
- Numbers right-aligned, text left-aligned, errors centered.
- Uses terminal's own colors (no hardcoded RGB) for light/dark terminal compatibility.

---

## Application Architecture (`cell-tui`)

### Event Loop

```
Key Event  →  Mode Handler  →  Action  →  Mutate State  →  Render
```

1. Poll for terminal event (key press, resize).
2. Dispatch to current mode handler.
3. Mode handler returns an `Action`.
4. Application processes the action, mutates state, pushes undo history.
5. Render current state.

### Action Enum

```rust
enum Action {
    Noop,
    MoveCursor(Direction),
    EditCell(CellPos, String),
    DeleteCells(Range),
    YankCells(Range),
    Paste(CellPos),
    Undo,
    Redo,
    ChangeMode(Mode),
    Save(Option<PathBuf>),
    Open(PathBuf),
    Quit { force: bool },
    Sort { col: usize, ascending: bool },
    Search { pattern: String, direction: SearchDirection },
    Resize,
}
```

### Application State

```rust
struct App {
    sheet: Sheet,
    viewport: Viewport,
    cursor: CellPos,
    mode: Mode,
    register: Register,
    undo_stack: Vec<UndoEntry>,
    redo_stack: Vec<UndoEntry>,
    command_line: String,
    search: Option<SearchState>,
    file_path: Option<PathBuf>,
    file_format: FileFormat,
    dirty: bool,
}
```

**Principles:**
- Unidirectional data flow — events produce actions, actions mutate state, state drives rendering.
- Mode handlers are pure-ish — take `(key_event, &App)`, return `Action`. No direct state mutation.
- Rendering is stateless — given `App`, produce the frame.

---

## Error Handling

### Cell Errors

- `#DIV/0!` — division by zero
- `#VALUE!` — type mismatch (e.g., `"hello" + 1`)
- `#REF!` — reference to out-of-bounds or deleted cell
- `#CIRC!` — circular reference
- `#NAME?` — unrecognized function
- `#PARSE!` — malformed formula syntax

Error propagation: a formula referencing an error cell also produces that error.

### File Handling

- `:q` with unsaved changes → `No write since last change (use :q! to override)`
- `:w` to read-only file → error in command line
- Malformed CSV → best-effort parse, warn in status bar with count of skipped rows
- `:e` on non-existent file → new empty sheet with path set for `:w`

### Grid Boundaries

- Navigation stops at edges.
- Pasting beyond sheet dimensions expands the sheet.
- Deleting a row referenced by a formula → `#REF!`.

### Performance Guardrails

- Max sheet size: 1,000,000 rows x 18,278 columns (A–ZZZ).
- Lazy recalculation — only dirty cells and dependents.
- Stream-parse CSV on open — don't load entire file then parse.

---

## Dependencies

### Production

| Crate | Used in | Purpose |
|---|---|---|
| `ratatui` | cell-tui | TUI rendering and layout |
| `crossterm` | cell-tui | Terminal backend, raw mode, events |
| `csv` | cell-core | RFC 4180 CSV/TSV parsing |
| `clap` | cell-tui | CLI argument parsing |

### Dev

| Crate | Purpose |
|---|---|
| `pretty_assertions` | Readable test diffs |

No serde, no regex, no async runtime.

---

## CLI Interface

```
cell                    # open empty sheet
cell data.csv           # open CSV
cell data.tsv           # open TSV
cell sheet.cell         # open native format
cell --version          # print version
cell --help             # usage
```
