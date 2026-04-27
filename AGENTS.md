# AGENTS.md

Guidance for AI coding agents (Claude Code, Codex, Cursor, etc.) working in
this repository. Human contributors should read [CONTRIBUTING.md](CONTRIBUTING.md);
this file complements it with the architectural context an agent needs to
make safe, idiomatic changes.

## Project

cell is a terminal spreadsheet editor with Vim keybindings, written in Rust.
It supports CSV/TSV import/export and a native `.cell` format that preserves
formulas. It also ships a non-interactive headless mode for scripting.

## Build & test commands

```sh
cargo build                                 # build all crates
cargo build --release                       # release build (binary at target/release/cell)
cargo test                                  # all tests (unit + integration)
cargo test -p cell-sheet-core               # core library only
cargo test -p cell-sheet-tui                # TUI crate only
cargo test -p cell-sheet-core -- col_label  # single test by name
cargo fmt --all --check                     # check formatting (CI)
cargo clippy --workspace --all-targets --all-features
cargo install --path crates/cell-sheet-tui  # install the `cell` binary
```

CI runs `fmt`, `clippy`, `test`, and `build` on Linux, macOS, and Windows
with `RUSTFLAGS=-Dwarnings`. Any warning fails the build, so run clippy
locally before claiming work is complete.

## Architecture

Cargo workspace with two crates:

- **`cell-sheet-core`** — pure data library with **zero TUI dependencies**.
  Contains the data model (`Sheet`, `Cell`, `CellValue`, `CellPos`), formula
  engine (tokenizer → parser → AST → evaluator), dependency graph with
  topological recalculation, and file I/O (CSV/TSV via the `csv` crate,
  native `.cell` format).
- **`cell-sheet-tui`** — terminal UI built on `ratatui` + `crossterm`.
  Implements Vim modal editing (Normal, Insert, Visual, VisualBlock, Command),
  the main event loop, rendering, undo/redo, clipboard with formula-aware
  paste, viewport scrolling, and the headless CLI.

### Key data flow

1. User edits a cell → `Action::EditCell` dispatched to `App::process_action`.
2. If formula (`=` prefix): `deps::set_formula` parses it, registers it in
   `DepGraph`, then `mark_dirty` propagates to dependents, then `recalculate`
   does topological-sort evaluation.
3. If plain value: `Sheet::set_cell` auto-detects type (number vs text).

### Formula engine pipeline (`cell-sheet-core`)

`token.rs` (lexer) → `parser.rs` (recursive descent → `Expr` AST) →
`eval.rs` (tree-walk evaluator) → `functions.rs` (built-in function
dispatcher). The authoritative list of supported functions lives in
`functions.rs`; the user-facing list lives in
`help/entries.rs::FORMULA_ENTRIES`. These two must stay in sync — see the
Help system section below.

### Dependency graph (`deps.rs`)

`DepGraph` tracks bidirectional edges (dependencies ↔ dependents). On any
cell change, `mark_dirty` does BFS propagation, then `recalculate` does
Kahn's algorithm topological sort. Circular references produce `#CIRC!`
errors.

### Modes (`cell-sheet-tui`)

Each mode has its own input handler under `crates/cell-sheet-tui/src/mode/`:
`normal.rs` (with multi-key sequence support like `gg`, `dd`), `insert.rs`,
`visual.rs`, `command.rs` (`:` commands and `/` search), and `help.rs` (the
modal help viewer entered via `:help`). Add new key sequences in the
appropriate mode handler rather than threading them through `app.rs`.

### Help system (`cell-sheet-core::help`)

The in-app help screen and `:help <topic>` are data-driven. `HelpRegistry`
(in `crates/cell-sheet-core/src/help/mod.rs`) is built from the static
slices in `crates/cell-sheet-core/src/help/entries.rs`
(`NORMAL_ENTRIES`, `INSERT_ENTRIES`, `VISUAL_ENTRIES`, `COMMAND_ENTRIES`,
`FORMULA_ENTRIES`). The TUI's `render/help.rs` renders directly from that
registry — if an entry is missing, the help screen and `:help` will silently
omit it. Treat `entries.rs` as a first-class part of any user-visible
change, not as documentation that lags behind.

## Conventions

- `CellPos` is `(usize, usize)` = `(row, col)`, zero-indexed.
- Column labels are Excel-style (A, B, …, Z, AA, AB, …) — convert with
  `col_index_to_label` / `col_label_to_index`.
- Formulas always start with `=`. The `raw` field stores the original input;
  `value` stores the computed result.
- CSV export flattens formulas to computed values; the `.cell` format
  preserves them.
- The core crate must remain independent of any TUI dependency. New
  evaluation, parsing, or storage logic belongs in `cell-sheet-core`; new
  rendering, input, or terminal logic belongs in `cell-sheet-tui`.
- Every user-visible keybinding, `:` command, and built-in formula function
  must have a corresponding `HelpEntry` in
  `crates/cell-sheet-core/src/help/entries.rs`. The help renderer reads that
  registry as its single source of truth — if it isn't there, `:help` will
  lie to the user.

## Working in this repo as an agent

### Before changing code

- Read the relevant module in full before editing — files are small and the
  context is usually worth the tokens.
- For any change that touches behavior, identify where the existing tests
  live (`crates/<crate>/src/<module>.rs` typically has `#[cfg(test)]`
  modules; integration tests live alongside).

### While changing code

- **Add a test for any behavior change.** Bug fixes need a regression test
  that fails before the fix and passes after. Prefer tests in
  `cell-sheet-core` whenever possible — they don't need a terminal.
- **Adding a new keybinding, `:` command, or formula function is a
  three-file change**: the handler/dispatcher (in `mode/*.rs` or
  `formula/functions.rs`), the matching `HelpEntry` in `help/entries.rs`,
  and a `CHANGELOG.md` entry under `## Unreleased` if it's user-visible.
  Skipping the help entry is a regression even if the feature works.
- Don't suppress clippy warnings globally. If a lint is genuinely wrong for
  a specific case, use a local `#[allow(...)]` with a short comment.
- Don't add comments that narrate what the code does; comments should only
  explain non-obvious intent or constraints.
- Don't introduce a new dependency without a clear reason. The project
  values a small dependency tree.

### Before claiming work is done

Run, in order:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --all-features
cargo test
```

All three must pass clean. Confirm with the actual command output rather
than asserting completion from inspection alone.

### Commits and changelog

- Use [Conventional Commits](https://www.conventionalcommits.org/) prefixes:
  `feat:`, `fix:`, `docs:`, `ci:`, `chore:`, `release:` (last one is for
  maintainers only).
- Subject in the imperative mood, ≤72 chars. Body explains *why*, not
  *what*.
- For user-visible changes, add an entry to `CHANGELOG.md` under
  `## Unreleased`, grouped under `Added`, `Changed`, `Fixed`, `Removed`, or
  `Notes`. Reference the PR with `(#NN)` once it's open.
- Never commit unless the user explicitly asks. Never push to `main`
  without explicit instruction.

### File-format invariants

When changing serialization, double-check round-trip behavior:

- `.cell` round-trip must preserve formula text in `raw`.
- CSV/TSV write must flatten formulas to computed `value`.
- The `:w!` force-save flow is what allows CSV to overwrite a formula sheet.

### Things to avoid

- Adding `ratatui`, `crossterm`, or any other TUI crate to
  `cell-sheet-core`'s `Cargo.toml`. The split is load-bearing.
- Bumping the workspace version in `Cargo.toml` or editing the
  `[Releasing](README.md#releasing)` flow — that's a maintainer action.
- Force-pushing, rewriting published history, or skipping pre-commit hooks.
- Generating very long binary blobs, hashes, or auto-generated payloads.

## Further reading

- [README.md](README.md) — user-facing overview, install, keybindings,
  formula syntax, file formats, and sc-im comparison.
- [CONTRIBUTING.md](CONTRIBUTING.md) — human contributor workflow, PR
  expectations, areas that welcome help.
- [CHANGELOG.md](CHANGELOG.md) — release history; the `Unreleased` section
  is where new entries land.
