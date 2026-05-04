# cell

A terminal spreadsheet editor with Vim keybindings, written in Rust.

![cell screenshot](assets/cell.png)

## Install

From [crates.io](https://crates.io/crates/cell-sheet-tui):

```sh
cargo install cell-sheet-tui
```

Pre-built binaries for Linux, macOS, and Windows are available on the [GitHub Releases](https://github.com/garritfra/cell/releases) page.

### Build from source

```sh
git clone https://github.com/garritfra/cell.git
cd cell
cargo build --release
# Binary at target/release/cell
```

## Usage

```sh
cell                          # empty sheet
cell data.csv                 # open CSV
cell data.tsv                 # open TSV
cell sheet.cell               # open native format
cell data.psv --delimiter '|' # open with a custom field delimiter
cat data.csv | cell           # read from stdin (pipe input)
```

To explore an example sheet with formulas, ranges, and IF logic:

```sh
cell examples/demo.cell
```

The CSV/TSV delimiter is auto-detected from file content; pass
`--delimiter` to override. The native `.cell` format is auto-detected
via its `# cell v` magic header.

## Headless mode

For shell pipelines, Makefiles, and CI, `cell` can read from and write to a
file without launching the TUI:

```sh
cell sales.cell --read A1                 # print one cell's computed value
cell sales.cell --read B1:B10             # print a range as TSV
cell sales.cell --eval '=SUM(B1:B10)'     # evaluate a formula (no save)
cell sales.cell --write A1 42             # set a cell, recalc, save in place
cell sales.cell --write A1 42 --write B1 7 # batch multiple writes into one save
cell sales.cell --write Total '=SUM(B:B)' --read Total  # write a formula, then print it
cat data.csv | cell --read A1             # read from stdin
cell data.psv --delimiter '|' --read A1   # custom delimiter
```

- Cell references are 1-indexed and Excel-style (`A1`, `AA10`, `A1:B3`).
- The `=` prefix on `--eval` is optional.
- Writes whose value starts with `=` are stored as formulas; others are auto-typed (number vs text).
- Operations apply in a fixed order per invocation: writes → save → reads → evals.
- Errors print to stderr; the process exits non-zero on bad refs, parse errors, or missing files.
- Stdin input supports CSV, TSV, and the native `.cell` format. The delimiter
  is auto-detected; pass `--delimiter` to override (CSV/TSV only). `--write`
  requires a file argument when reading from stdin.

## Keybindings

If you know Vim, you know cell.

All motions and operators accept a `[count]` prefix (`5j`, `10G`, `3dd`,
`4yy`, `2w`). Counts also work between an operator and its motion (`d3j`,
`y2k`); outer and inner counts multiply (`5d2j` clears 10 rows). The
in-progress count and operator render in the status line as you type.

### Normal Mode

#### Motion

| Key                  | Action                                 |
| -------------------- | -------------------------------------- |
| `h` `j` `k` `l`      | Move cursor (one cell)                 |
| `gg`                 | First row (or row N with `[count]gg`)  |
| `G`                  | Last row (or row N with `[count]G`)    |
| `0`                  | First column                           |
| `$`                  | Last column                            |
| `w` / `b`            | Next / previous non-empty cell in row  |
| `Ctrl-D` / `Ctrl-U`  | Half-page down / up                    |
| `Ctrl-F` / `Ctrl-B`  | Full page down / up                    |
| `{` / `}`            | Previous / next block boundary in column |
| `H` / `M` / `L`      | Cursor to top / middle / bottom of viewport |
| `zz` / `zt` / `zb`   | Recenter / scroll-to-top / scroll-to-bottom around cursor |
| `Ctrl-e` / `Ctrl-y`  | Scroll viewport one row without moving cursor |
| `Ctrl-o` / `Ctrl-i` (or Tab) | Jump back / forward in jump list |

#### Marks

| Key            | Action                              |
| -------------- | ----------------------------------- |
| `m{a-z}`       | Set mark at cursor                  |
| `'{a-z}`       | Jump to marked row (column 0)       |
| `` `{a-z} ``   | Jump to exact marked cell           |

#### Editing

| Key                  | Action                                 |
| -------------------- | -------------------------------------- |
| `i` / `a` / `Enter`  | Edit cell (Insert mode)                |
| `x`                  | Clear cell                             |
| `dd`                 | Delete row (`[count]dd` for N rows)    |
| `d{motion}`          | Clear cells along motion (`dj`, `d3l`, `dh`, `dk`) |
| `yy`                 | Yank row (`[count]yy` for N rows)      |
| `y{motion}`          | Yank cells along motion (`yj`, `y3l`, `yh`, `yk`) |
| `p` / `P`            | Paste below / above                    |
| `Ctrl-A` / `Ctrl-X`  | Increment / decrement number in cell (`[count]` accepted) |
| `~`                  | Toggle case of first character, advance cursor |
| `guu` / `gUU`        | Lowercase / uppercase entire cell      |
| `g~~`                | Toggle case of every character in cell |
| `.`                  | Repeat last change                     |
| `u`                  | Undo                                   |
| `Ctrl-R`             | Redo                                   |

#### Selection & search

| Key                   | Action                                              |
| --------------------- | --------------------------------------------------- |
| `v`                   | Visual selection                                    |
| `V`                   | Visual line (full-row) selection                    |
| `Ctrl-V`              | Visual block selection                              |
| `gv`                  | Re-enter previous visual selection                  |
| `/` / `?`             | Search forward / backward (incremental)             |
| `n` / `N`             | Next / previous match                               |
| `*` / `#`             | Search for current cell's value forward / backward  |
| `f<char>` / `F<char>` | Jump to next / prev cell in row starting with `<char>` |
| `;` / `,`             | Repeat last `f`/`F` (same / reversed direction)     |
| `:`                   | Command mode                                        |

### Insert Mode

Type to edit the cell. `ESC` or `Enter` confirms. Arrow keys, `Home`,
`End`, `Backspace`, and `Delete` work as expected within the cell.

### Visual Mode

Extend the selection with `hjkl` (or `[count]j` etc.), then:

| Key      | Action                                                   |
| -------- | -------------------------------------------------------- |
| `y`      | Yank selection                                           |
| `d`      | Delete selection                                         |
| `c`      | Change selection (clear and enter Insert mode)           |
| `u` / `U` / `~` | Lowercase / uppercase / toggle case of selection (formula cells skipped) |
| `Esc`    | Cancel selection                                         |

### Commands


| Command              | Action                                         |
| -------------------- | ---------------------------------------------- |
| `:w`                 | Save                                           |
| `:w file.csv`        | Save as CSV                                    |
| `:w file.cell`       | Save as native format                          |
| `:w!`                | Force save (flatten formulas, override warnings) |
| `:q`                 | Quit                                           |
| `:q!`                | Quit without saving                            |
| `:wq`                | Save and quit                                  |
| `:e file`            | Open file                                      |
| `:sort A asc`        | Sort by column A ascending                     |
| `:sort B desc`       | Sort by column B descending                    |
| `:set delimiter=\|`   | Change CSV/TSV delimiter for the next save     |
| `:help` / `:help <topic>` | Open the in-app help screen / jump to a topic |

In the `:` prompt, `↑` / `↓` cycle through previously executed commands.


## Mouse support

Mouse support is **off by default** so the terminal's native text
selection keeps working. Enable it at runtime with `:set mouse on`,
disable it with `:set mouse off`, or flip the current state with
`:set mouse toggle`.

When enabled:

- **Left-click** on a cell moves the cursor.
- **Click + drag** inside the grid selects a Visual range.
- **Click + drag** on a column header selects whole columns.
- **Click + drag** on a row header selects whole rows.
- **Scroll wheel** scrolls the viewport (cursor stays put). Horizontal
  scroll works when the terminal emits `ScrollLeft` / `ScrollRight`
  (commonly bound to Shift + wheel).
- **Double-click** a cell to enter Insert mode on it.
- **Drag past the visible edge** auto-scrolls the viewport.

To copy a cell value out to your system clipboard while mouse mode is
on, hold your terminal's bypass modifier when clicking and dragging:

| Terminal | Bypass |
| --- | --- |
| Linux terminals (gnome-terminal, alacritty, kitty, …) | Shift |
| Windows Terminal | Shift |
| macOS Terminal.app, iTerm2 | Option/Alt |
| tmux / screen | configure per their docs |

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
- `**.cell`** -- Native format that preserves formulas. Plain text, human-readable, inspired by [sc-im](https://github.com/andmarti1424/sc-im).

When saving a CSV that contains formulas, cell warns you and suggests saving as `.cell` instead. Use `:w!` to force a CSV save.

## Comparison with sc-im

[sc-im](https://github.com/andmarti1424/sc-im) is a battle-tested terminal spreadsheet built on the classic `sc` (Spreadsheet Calculator, 1981). It inspired cell's native `.cell` format. Here's how the two tools compare:

| | cell | sc-im |
| --- | --- | --- |
| **Language / TUI** | Rust + ratatui | C + ncurses |
| **Editing model** | True Vim modal editing (`i` → Insert, `ESC` → Normal) | Vim-inspired navigation; `=` to enter a value, `e`/`E` to edit |
| **Formula syntax** | Excel-compatible (`=SUM(A1:A10)`, `=IF(...)`) | `@`-prefix style (`@sum(A1:A10)`, `@avg(...)`) |
| **Built-in functions** | SUM, AVERAGE, COUNT, MIN, MAX, IF | Extensive (@sum, @avg, @min, @max, @abs, @sqrt, ...) |
| **File formats** | CSV, TSV, `.cell` | CSV, TSV, XLSX/XLS/ODS import, Markdown export, `.sc` |
| **Cell formatting** | not yet | Bold, italic, underline, RGB colors |
| **Scripting** | Headless CLI mode (`--read` / `--write` / `--eval`, stdin pipe) | Lua scripting, external C modules, non-interactive mode |
| **Charting** | not yet | GNUPlot integration |
| **Windows support** | ✓ (pre-built binaries) | Limited |
| **Clipboard** | Built-in | Requires tmux / xclip / pbpaste |
| **Config file** | not yet | `~/.config/sc-im/scimrc` |

### Choose cell if…

- You want editing that works exactly like Vim (`i` to insert, `ESC` to return, `/` to search)
- You prefer Excel-compatible formula syntax (`=SUM`, `=IF`, `=AVERAGE`)
- You need a working binary on Windows without extra setup
- You value a modern, memory-safe codebase with minimal dependencies

### Choose sc-im if…

- You need XLSX, ODS, or Markdown support right now
- You need Lua scripting or GNUPlot charting right now
- You need cell-level formatting (colors, bold, italic) right now
- You want a highly configurable, feature-rich tool with decades of history behind it

## Architecture

```
cell/
  crates/
    cell-sheet-core/    # Data model, formula engine, file I/O (no TUI dependency)
    cell-sheet-tui/     # Ratatui rendering, Vim modes, event loop
```

The core library is independent of the terminal UI and can be tested without a terminal.

## Releasing

Releases are automated with [release-plz](https://release-plz.dev/) from the
[release workflow](.github/workflows/release.yml):

- Pushes to `main` open or update a release PR with the next version and
  changelog updates.
- Merging the release PR publishes `cell-sheet-core` and `cell-sheet-tui` to
  [crates.io](https://crates.io) via trusted publishing, creates the `vX.Y.Z`
  tag, and creates a draft GitHub Release.
- The `vX.Y.Z` tag then builds binaries for Linux (x86_64, aarch64), macOS
  (x86_64, aarch64), and Windows (x86_64), uploads archives and SHA256
  checksums, and publishes the GitHub Release.

See [RELEASE.md](RELEASE.md) for maintainer instructions and failure handling.

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for the
development setup, project conventions, and pull request workflow.

## License

[MIT](LICENSE)