# Changelog

## Unreleased

### Added

- Operator-pending motion counts: a count typed *between* the operator and the
  motion now works as in vim — `d3j` clears the current row and 3 rows below,
  `d2k` clears 2 rows above plus the current row, `y3l` yanks the current cell
  and 3 cells to the right, `y2k` yanks 2 rows upward. Outer and inner counts
  multiply: `5d2j` clears 10 rows downward. All directional motions (`h j k l`)
  are supported after `d` and `y`. Count prefixes in Visual mode also work: `5j`
  in visual extends the selection 5 rows, `3l` extends it 3 cells right (#56)
- Vim-style numeric count prefix in normal mode: type digits before a motion or
  operator to repeat or scale it. `5j` moves five rows down, `10G` jumps to row
  10, `5gg` jumps to row 5 from the top, `3dd` deletes three rows (line-wise,
  pastable as a block with `p`/`P`), `4yy` yanks four rows, `2w` / `3b` hop
  multiple non-empty cells. `0` alone still goes to the first column; after a
  non-zero digit, `0` extends the count (so `10j` really moves ten rows). The
  partially-typed count and operator render in the status line as you type
  (vim's `showcmd`). `Esc` cancels a half-typed count, and counts saturate at
  one million to keep huge accidental inputs responsive
  ([#55](https://github.com/garritfra/cell/pull/55))
- Custom field delimiter support: `--delimiter '|'` CLI flag, auto-detection from
  file content, and `:set delimiter=X` ex-command. Writing with a non-standard
  delimiter to a `.csv` or `.tsv` file shows a warning; use `:w!` to override.
  Resolves #20.

## 0.3.1 (2026-04-28)

### Fixed

- Key presses on Windows no longer double-fire: crossterm emits both a key-press
  and a key-release event on Windows, so key release events are now ignored
  ([#50](https://github.com/garritfra/cell/pull/50))

## 0.3.0 (2026-04-27)

### Fixed

- Cursor disappearing when scrolling down: `visible_rows` was set to the full
  grid widget height, failing to account for the 1-row column header. Moving the
  cursor to the last rendered row no longer leaves it invisible ([#49](https://github.com/garritfra/cell/pull/49))
- `SUM` and `AVERAGE` now use Neumaier's improved Kahan compensated summation
  instead of naive accumulation, eliminating catastrophic-cancellation errors
  (e.g. `SUM(1e16, 1, -1e16)` now returns `1` instead of `0`) and keeping
  long-sequence drift well below 1e-10 ([#48](https://github.com/garritfra/cell/pull/48))

### Added

- Vim-style viewport motions in normal mode: `zz` / `zt` / `zb` recenter / scroll-to-top / scroll-to-bottom around the cursor, `H` / `M` / `L` jump the cursor to the topmost / middle / bottommost visible row, and `Ctrl-e` / `Ctrl-y` scroll the viewport one row without moving the cursor ([#45](https://github.com/garritfra/cell/pull/45))
- Marks: `m{a-z}` records the cursor position, `'{a-z}` jumps to the marked row at column 0, `` `{a-z} `` jumps to the exact marked cell. Jumping to an unset mark surfaces an `E20: Mark not set` status message ([#45](https://github.com/garritfra/cell/pull/45))
- Jump list with `Ctrl-o` (back) and `Ctrl-i` / Tab (forward), tracking cursor history across `gg`, `G`, marks, and `/` searches. Mid-stack jumps truncate the forward history; the list is capped at 100 entries ([#45](https://github.com/garritfra/cell/pull/45))
- Block-jump in the current column: `}` jumps to the next block boundary downward and `{` upward, mirroring vim's paragraph motion. From a non-empty cell they land on the first empty row past the current block; from an empty cell they land on the next non-empty row ([#45](https://github.com/garritfra/cell/pull/45))
- `*` and `#` search for the value of the cell under the cursor, forward and backward respectively, populating the search pattern so `n` and `N` keep stepping ([#45](https://github.com/garritfra/cell/pull/45))
- `gv` re-enters the previous visual selection with the same anchor, cursor, and visual kind (Character / Line / Block) ([#45](https://github.com/garritfra/cell/pull/45))
- `/<pattern>` and `?<pattern>` now open a search prompt that dispatches a
  forward or backward search; `n` and `N` step through the matches as before.
  Status line renders the corresponding `/` or `?` prefix while typing.
  Search is incremental (vim's `incsearch`): the cursor jumps to the first
  match as you type, `Esc` restores the cursor to where the prompt opened,
  and `Enter` commits the pattern ([#46](https://github.com/garritfra/cell/pull/46))
- `f<char>` / `F<char>` jump to the next / previous non-empty cell in the
  current row whose displayed value starts with `<char>` (case-insensitive).
  Triggered immediately on the target keypress without confirmation.
  `;` repeats the last find, `,` repeats it reversed ([#46](https://github.com/garritfra/cell/pull/46))

## 0.2.0

### Notes

- Re-release of `0.1.7`. The original `v0.1.7` tag was force-moved after publishing,
  which re-triggered the release workflow and caused a (harmless) failure when
  `cargo publish` refused to overwrite the already-published `0.1.7` crates. No
  code changes since `0.1.7` — version bumped to `0.2.0` to obtain a clean
  release run.

## 0.1.7

### Added

- Non-interactive CLI mode for scripting against `.cell`, CSV, and TSV files
  without launching the TUI: `--read <ref>` prints a cell or range, `--eval <expr>`
  evaluates a formula in-place without saving, and repeatable `--write <ref> <value>`
  flags batch into a single save ([#19](https://github.com/garritfra/cell/pull/19))

### Fixed

- Visual-mode `d` (clear range) is now undoable with `u` and redoable with `Ctrl-R`, including formula preservation ([#16](https://github.com/garritfra/cell/pull/16))
- `p` and `P` paste operations are now undoable and redoable for cell, row, and block registers, restoring any prior content at the destination ([#16](https://github.com/garritfra/cell/pull/16))
- Pasted formulas are now evaluated immediately instead of showing a default value until the next edit
- Undo, redo, and visual-range clears now keep the formula dependency graph consistent when overwriting or clearing formula cells

## 0.1.6

### Fixed

- `dd` (delete row) is now undoable with `u`, matching Vim's behavior ([#12](https://github.com/garritfra/cell/pull/12))

## 0.1.5

### Fixed

- Add README to crates.io package pages
- Bump GitHub Actions to Node.js 24 compatible versions
- Fix crates.io publish requiring version on workspace dependency

## 0.1.2

### Changed

- Renamed crates for crates.io publishing: `cell-sheet-core` (engine) and `cell-sheet-tui` (binary)
- Install via `cargo install cell-sheet-tui`

### Added

- Trusted publishing to crates.io via GitHub Actions on `v*` tag push
- Pre-built binaries attached to GitHub Releases (Linux x86_64/aarch64, macOS x86_64/aarch64, Windows x86_64)

## 0.1.0 — Initial Release

### Spreadsheet Core

- Sparse-storage data model with `Sheet`, `Cell`, and `CellValue` types
- Formula engine with tokenizer, recursive descent parser, and evaluator
- Built-in functions: `SUM`, `AVERAGE`, `COUNT`, `MIN`, `MAX`, `IF`
- Cell references (`A1`, `$A$1`, mixed) and range expressions (`A1:B3`)
- Dependency graph with topological recalculation and circular reference detection
- Column sorting (ascending/descending)

### File Formats

- CSV and TSV import/export with auto-sized column widths
- Native `.cell` format for lossless formula round-tripping
- Formula-loss warning when saving to CSV/TSV

### TUI

- Full terminal UI built with ratatui and crossterm
- Vim-style modal editing: Normal, Insert, Visual, Visual Line, Command
- Navigation: `hjkl`, `gg`, `G`, `0`, `$`, `w`, `b`, `Ctrl-D/U`, `Ctrl-F/B`
- Editing: `i` (insert), `x` (clear), `c` (change), `dd` (delete row)
- Visual mode with range highlighting and `d`/`y`/`c` operations
- V-LINE mode (`V`) for full-row selection
- Yank/paste (`yy`, `p`, `P`) with formula reference adjustment
- Undo/redo (`u`, `Ctrl-R`)
- Search (`/pattern`, `n`, `N`)
- Text cursor in formula bar during insert mode
- Color-coded mode indicator matching Vim conventions
- Built-in `:help` system with TOC, topic lookup, and vim-like scrolling
- Commands: `:w`, `:q`, `:wq`, `:e`, `:sort`, `:help`

### Infrastructure

- GitHub Actions CI: format check, clippy lint, cross-platform tests (Linux, macOS, Windows)
- GitHub Actions Release: automated binary builds for 5 targets on `v*` tag push
- MIT license
