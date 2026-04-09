# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Cell is a terminal spreadsheet editor with Vim keybindings, written in Rust. It supports CSV/TSV import/export and a native `.cell` format that preserves formulas.

## Build & Test Commands

```sh
cargo build                              # build all crates
cargo build --release                    # release build (binary at target/release/cell)
cargo test                               # run all tests (unit + integration)
cargo test -p cell-sheet-core                  # test only the core library
cargo test -p cell-sheet-tui                   # test only the TUI crate
cargo test -p cell-sheet-core -- col_label     # run a single test by name
cargo fmt --check                        # check formatting
cargo clippy                             # lint
cargo install --path crates/cell-sheet-tui     # install the `cell` binary
```

## Architecture

Cargo workspace with two crates:

- **`cell-sheet-core`** — Pure data library with zero TUI dependencies. Contains the data model (`Sheet`, `Cell`, `CellValue`, `CellPos`), formula engine (tokenizer → parser → AST → evaluator), dependency graph with topological recalculation, and file I/O (CSV/TSV via the `csv` crate, native `.cell` format).
- **`cell-sheet-tui`** — Terminal UI built on `ratatui`/`crossterm`. Implements Vim modal editing (Normal, Insert, Visual, VisualBlock, Command modes), the main event loop, rendering, undo/redo, clipboard with formula-aware paste, and viewport scrolling.

### Key data flow

1. User edits a cell → `Action::EditCell` dispatched to `App::process_action`
2. If formula (`=` prefix): `deps::set_formula` parses it, registers in `DepGraph`, then `mark_dirty` propagates to dependents, then `recalculate` does topological-sort evaluation
3. If plain value: `Sheet::set_cell` auto-detects type (number vs text)

### Formula engine pipeline (cell-sheet-core)

`token.rs` (lexer) → `parser.rs` (recursive descent → `Expr` AST) → `eval.rs` (tree-walk evaluator) → `functions.rs` (built-in functions: SUM, AVERAGE, COUNT, MIN, MAX, IF)

### Dependency graph (`deps.rs`)

`DepGraph` tracks bidirectional edges (dependencies ↔ dependents). On any cell change, `mark_dirty` does BFS propagation, then `recalculate` does Kahn's algorithm topological sort. Circular references produce `#CIRC!` errors.

### Modes (cell-sheet-tui)

Each mode has its own input handler in `mode/`: `normal.rs` (with multi-key sequence support like `gg`, `dd`), `insert.rs`, `visual.rs`, `command.rs` (`:` commands and `/` search).

## Conventions

- `CellPos` is `(usize, usize)` = `(row, col)`, zero-indexed
- Column labels are Excel-style (A, B, ..., Z, AA, AB, ...) — convert with `col_index_to_label` / `col_label_to_index`
- Formulas always start with `=`; the `raw` field stores the original input, `value` stores the computed result
- CSV export flattens formulas to computed values; `.cell` format preserves them
- The core crate must remain independent of any TUI dependency
