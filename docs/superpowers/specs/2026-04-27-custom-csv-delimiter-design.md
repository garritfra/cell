# Custom CSV Delimiter — Design Spec

**Issue:** [#20](https://github.com/garritfra/cell/issues/20)  
**Date:** 2026-04-27  
**Status:** Approved, ready for implementation

---

## Background

The core I/O layer (`cell-sheet-core/src/io/csv.rs`) already accepts an arbitrary `delimiter: u8` in both `read_csv` and `write_csv`. The gap is purely user-facing: the delimiter is currently derived solely from the file extension (`.csv` → `,`, `.tsv` → `\t`, everything else → `,`) with no override mechanism.

HN feedback requested pipe (`|`) support for files that use `|` to avoid issues with comma-containing cells.

---

## Goals

1. **CLI flag** — `cell data.psv --delimiter '|'` (works in both TUI and headless mode)
2. **Auto-sniff** — detect `|`, `;`, `\t` automatically when the extension is ambiguous or the flag is absent
3. **`:set delimiter=|`** — ex-command to change the active delimiter for the current buffer in the TUI; persisted on next `:w`

---

## Data Model

### `App` (cell-sheet-tui)

Add one field:

```rust
pub delimiter: u8,  // default b','
```

This is orthogonal to `FileFormat` — `FileFormat` answers "does this format preserve formulas?" while `delimiter` answers "what byte separates fields?" A pipe-delimited file is still `FileFormat::Csv` as far as formula-preservation goes.

### `Action` (cell-sheet-tui)

Add one variant:

```rust
SetDelimiter(u8),
```

### `headless::Options` (cell-sheet-tui)

Add one field:

```rust
pub delimiter: Option<u8>,  // None = auto-sniff
```

### `Cli` (cell-sheet-tui `main.rs`)

Add one argument:

```rust
/// Field delimiter character (e.g. '|', ';'). Defaults to auto-detect from extension or content sniffing.
#[arg(long, value_name = "CHAR")]
delimiter: Option<char>,
```

---

## Auto-Sniff

### Location

`cell-sheet-core/src/io/csv.rs` — a standalone public function:

```rust
pub fn sniff_delimiter(sample: &[u8]) -> u8
```

### Algorithm

1. Read the first line of input (up to 4 KB).
2. Count occurrences of each candidate delimiter: `','`, `'\t'`, `'|'`, `';'`.
3. Return the one with the highest count.
4. Ties break in that order (comma wins ties — it is the most common CSV delimiter).
5. Return `b','` if the sample is empty.

### When Sniffing Is Triggered

| Condition | Behavior |
|---|---|
| `--delimiter` flag provided | Use flag value; skip sniffing |
| Extension is `.tsv` | Use `b'\t'`; skip sniffing |
| Extension is `.cell` | Not applicable (non-CSV format) |
| Extension is `.csv`, no flag | Sniff — may override comma default for pipe/semicolon files |
| Unknown extension, no flag | Sniff |

---

## CLI Surface

```
cell data.psv --delimiter '|'
cell data.csv --delimiter ';' --read A1
```

- The `char` value is validated at startup: must be a printable, non-alphanumeric ASCII byte. Invalid input exits with code 2 and a human-readable error.
- The resolved delimiter is stored in `app.delimiter` (TUI) or carried through `headless::Options` (headless).
- Headless writes use the delimiter silently (no interactive warning).

---

## TUI: `:set delimiter=X`

- Parsed in `command.rs`: `:set delimiter=X` where `X` is a single character.
- Produces `Action::SetDelimiter(b'X')`.
- `process_action` sets `app.delimiter = X` and writes `"Delimiter set to '|'"` to the status bar.
- Does **not** re-parse the currently loaded file — only affects the next save.
- Validation: if `X` is absent, multi-byte, or alphanumeric, show a status bar error and leave state unchanged.

---

## Save Behavior

### Warning on non-standard delimiter to `.csv`

In `App::do_save` (non-force path), if:
- `format == FileFormat::Csv`, **and**
- `self.delimiter != b','`

Emit:

> `"Non-standard delimiter '|' will be used. Use :w! to force, or save as .tsv / .psv."`

`:w!` / `ForceSave` bypasses the warning and writes with the active delimiter.

This mirrors the existing formula-flatten warning pattern.

### `.tsv` files

If the active delimiter is not `b'\t'` and the path ends in `.tsv`, the same warning fires (saving a comma-delimited file as `.tsv` is equally odd).

---

## Error Handling

| Scenario | Response |
|---|---|
| `--delimiter` with multi-byte or alphanumeric char | Exit code 2, error to stderr |
| `:set delimiter=` with no/invalid char | Status bar error, no state change |
| Sniff on empty file | Default to `b','` |

---

## Testing

### `cell-sheet-core`

- `sniff_delimiter` unit tests: pipe-heavy → `b'|'`, semicolon-heavy → `b';'`, tab-heavy → `b'\t'`, empty → `b','`, tie → `b','`
- `read_csv` / `write_csv` round-trip with `b'|'` and `b';'`

### `cell-sheet-tui`

- `command.rs`: `parse_command("set delimiter=|")` → `Action::SetDelimiter(b'|')`; invalid inputs → `Action::Noop`
- `app.rs`: `Action::SetDelimiter` sets the field; saving `.csv` with non-comma delimiter triggers warning; `ForceSave` bypasses it; `.tsv` with non-tab delimiter also warns
- Headless integration test: write a pipe-delimited file, read it back with `--delimiter '|'`, assert cell values

---

## Non-Goals

- Re-parsing the open buffer when `:set delimiter` changes (out of scope; close and reopen the file to change how it was read)
- Supporting multi-character delimiters
- Configuring the quote character
