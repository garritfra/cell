# Changelog

## Unreleased

### Fixed

- Cursor disappearing when scrolling down: `visible_rows` was set to the full
  grid widget height, failing to account for the 1-row column header. Moving the
  cursor to the last rendered row no longer leaves it invisible (#29)

### Added

- Vim-style viewport motions in normal mode: `zz` / `zt` / `zb` recenter / scroll-to-top / scroll-to-bottom around the cursor, `H` / `M` / `L` jump the cursor to the topmost / middle / bottommost visible row, and `Ctrl-e` / `Ctrl-y` scroll the viewport one row without moving the cursor (#30)
- Marks: `m{a-z}` records the cursor position, `'{a-z}` jumps to the marked row at column 0, `` `{a-z} `` jumps to the exact marked cell. Jumping to an unset mark surfaces an `E20: Mark not set` status message (#31)
- Jump list with `Ctrl-o` (back) and `Ctrl-i` / Tab (forward), tracking cursor history across `gg`, `G`, marks, and `/` searches. Mid-stack jumps truncate the forward history; the list is capped at 100 entries (#32)
- Block-jump in the current column: `}` jumps to the next block boundary downward and `{` upward, mirroring vim's paragraph motion. From a non-empty cell they land on the first empty row past the current block; from an empty cell they land on the next non-empty row (#35)
- `*` and `#` search for the value of the cell under the cursor, forward and backward respectively, populating the search pattern so `n` and `N` keep stepping (#36)
- `gv` re-enters the previous visual selection with the same anchor, cursor, and visual kind (Character / Line / Block) (#37)
- `/<pattern>` and `?<pattern>` now open a search prompt that dispatches a
  forward or backward search; `n` and `N` step through the matches as before.
  Status line renders the corresponding `/` or `?` prefix while typing.
  Search is incremental (vim's `incsearch`): the cursor jumps to the first
  match as you type, `Esc` restores the cursor to where the prompt opened,
  and `Enter` commits the pattern (#28)
- `f<char>` / `F<char>` jump to the next / previous non-empty cell in the
  current row whose displayed value starts with `<char>` (case-insensitive).
  Triggered immediately on the target keypress without confirmation.
  `;` repeats the last find, `,` repeats it reversed (#28)

### Fixed

- `SUM` and `AVERAGE` now use Neumaier's improved Kahan compensated summation
  instead of naive accumulation, eliminating catastrophic-cancellation errors
  (e.g. `SUM(1e16, 1, -1e16)` now returns `1` instead of `0`) and keeping
  long-sequence drift well below 1e-10 (#43)

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
  flags batch into a single save (#6)

### Fixed

- Visual-mode `d` (clear range) is now undoable with `u` and redoable with `Ctrl-R`, including formula preservation (#13)
- `p` and `P` paste operations are now undoable and redoable for cell, row, and block registers, restoring any prior content at the destination (#13)
- Pasted formulas are now evaluated immediately instead of showing a default value until the next edit
- Undo, redo, and visual-range clears now keep the formula dependency graph consistent when overwriting or clearing formula cells

## 0.1.6

### Fixed

- `dd` (delete row) is now undoable with `u`, matching Vim's behavior (#5)

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
