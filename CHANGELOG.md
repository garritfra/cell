# Changelog

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
