# cell

A terminal spreadsheet editor with Vim keybindings, written in Rust.

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
├──────────────────────────────────────────────────┤
│ NORMAL | 4 rows x 3 cols | A1        status bar  │
└──────────────────────────────────────────────────┘
```

## Install

```sh
cargo install --path crates/cell-sheet-tui
```

Or build from source:

```sh
cargo build --release
# Binary at target/release/cell
```

## Usage

```sh
cell                    # empty sheet
cell data.csv           # open CSV
cell data.tsv           # open TSV
cell sheet.cell         # open native format
```

## Keybindings

If you know Vim, you know cell.

### Normal Mode

| Key | Action |
|-----|--------|
| `h` `j` `k` `l` | Move cursor |
| `gg` | First row |
| `G` | Last row |
| `0` | First column |
| `$` | Last column |
| `Ctrl-D` / `Ctrl-U` | Half-page down/up |
| `Ctrl-F` / `Ctrl-B` | Full page down/up |
| `w` / `b` | Next/previous non-empty cell |
| `i` / `a` / `Enter` | Edit cell (Insert mode) |
| `x` | Clear cell |
| `dd` | Delete row |
| `yy` | Yank row |
| `p` / `P` | Paste below/above |
| `u` | Undo |
| `Ctrl-R` | Redo |
| `v` | Visual selection |
| `Ctrl-V` | Visual block selection |
| `/` | Search |
| `n` / `N` | Next/previous match |
| `:` | Command mode |

### Insert Mode

Type to edit the cell. `ESC` or `Enter` confirms.

### Visual Mode

Select with `hjkl`, then `y` to yank, `d` to delete.

### Commands

| Command | Action |
|---------|--------|
| `:w` | Save |
| `:w file.csv` | Save as CSV |
| `:w file.cell` | Save as native format |
| `:w!` | Force save (flatten formulas) |
| `:q` | Quit |
| `:q!` | Quit without saving |
| `:wq` | Save and quit |
| `:e file` | Open file |
| `:sort A asc` | Sort by column A ascending |
| `:sort B desc` | Sort by column B descending |

## Formulas

Formulas start with `=` and support Excel-compatible syntax:

```
=A1+B1
=SUM(A1:A10)
=AVERAGE(B1:B5)
=IF(A1>100, "high", "low")
```

### Supported Functions (v1)

`SUM`, `AVERAGE`, `COUNT`, `MIN`, `MAX`, `IF`

Formula compliance with the ODF (OpenDocument Formula) spec is tracked and will expand over time.

## File Formats

- **CSV/TSV** -- Opens and saves standard comma/tab-separated files. Formulas are flattened to their computed values on CSV export.
- **`.cell`** -- Native format that preserves formulas. Plain text, human-readable, inspired by [sc-im](https://github.com/andmarti1992/sc-im).

When saving a CSV that contains formulas, cell warns you and suggests saving as `.cell` instead. Use `:w!` to force a CSV save.

## Architecture

```
cell/
  crates/
    cell-sheet-core/    # Data model, formula engine, file I/O (no TUI dependency)
    cell-sheet-tui/     # Ratatui rendering, Vim modes, event loop
```

The core library is independent of the terminal UI and can be tested without a terminal.

## Releasing

1. Update the version in [`Cargo.toml`](Cargo.toml) (workspace version) and the `cell-sheet-core` dependency version in [`crates/cell-sheet-tui/Cargo.toml`](crates/cell-sheet-tui/Cargo.toml)
2. Update [`CHANGELOG.md`](CHANGELOG.md) with the new version's changes
3. Commit: `git commit -am "release: bump to vX.Y.Z"`
4. Tag and push:
   ```sh
   git tag vX.Y.Z
   git push origin main --tags
   ```

Pushing a `v*` tag triggers the [release workflow](.github/workflows/release.yml), which:
- Builds binaries for Linux (x86_64, aarch64), macOS (x86_64, aarch64), and Windows (x86_64)
- Creates a GitHub Release with the binaries attached
- Publishes `cell-sheet-core` and `cell-sheet-tui` to [crates.io](https://crates.io) via trusted publishing

## License

[MIT](LICENSE)
