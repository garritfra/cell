# Design: `:help` Command for Cell

## Overview

A full-screen help viewer activated by `:help [topic]`, modeled on Vim's help
UX but backed by a collocated registration system where help entries live next
to their implementations. Each mode, command, and formula function is documented
with entries defined as static data alongside the code they describe.

## Goals

- Every mode, keybinding, command, and formula function has a help entry
- Help text lives next to the implementation it documents (stays in sync)
- `:help` shows a browsable table of contents; `:help <topic>` jumps to a
  specific entry
- Vim-like navigation within the help view (j/k, gg/G, Ctrl+D/Ctrl+U, q)
- Clean integration into Cell's existing mode/action/render architecture

## Non-Goals

- Cross-references or clickable links between help entries
- Search within help text (beyond `:help <topic>` tag lookup)
- Plugin or user-defined help entries
- External help file generation
- Fuzzy matching for unknown topics

## Help Registry (cell-core)

### Data Structures

```rust
pub struct HelpEntry {
    pub tags: &'static [&'static str],
    pub category: HelpCategory,
    pub summary: &'static str,
    pub detail: &'static str,
}

pub enum HelpCategory {
    Normal,
    Insert,
    Visual,
    Command,
    Formula,
}

pub struct HelpRegistry {
    entries: Vec<&'static HelpEntry>,
}
```

`HelpEntry` uses all `&'static` fields. Entries are defined as `const` or
`static` arrays next to the code they document.

### Registry Construction

`HelpRegistry::new()` collects entries from all modules:

- `NORMAL_HELP_ENTRIES` from `mode/normal.rs`
- `INSERT_HELP_ENTRIES` from `mode/insert.rs`
- `VISUAL_HELP_ENTRIES` from `mode/visual.rs`
- `COMMAND_HELP_ENTRIES` from `mode/command.rs`
- `FORMULA_HELP_ENTRIES` from `formula/functions.rs`

### Tag Lookup

`HelpRegistry::find(tag: &str) -> Option<&HelpEntry>` performs a
case-insensitive linear scan over all entries, checking each entry's `tags`
array. With ~50 entries, this is fast enough without indexing.

### Category Listing

`HelpRegistry::by_category(cat: HelpCategory) -> Vec<&HelpEntry>` returns all
entries in a category, used for rendering the table of contents.

### Collocation Strategy

Each module defines its help entries as a static array next to the
implementation. Example from `mode/normal.rs`:

```rust
pub static NORMAL_HELP_ENTRIES: &[HelpEntry] = &[
    HelpEntry {
        tags: &["h"],
        category: HelpCategory::Normal,
        summary: "Move cursor left",
        detail: "Move the cursor one column to the left. Stops at column A.",
    },
    HelpEntry {
        tags: &["dd"],
        category: HelpCategory::Normal,
        summary: "Delete current row",
        detail: "Deletes the entire row under the cursor. The row is stored\n\
                 in the register and can be pasted with p or P.",
    },
    // ...
];
```

This pattern ensures a developer editing a keybinding sees the help text right
next to it.

## Help View (cell-tui)

### New Mode

Add `Help` to the `Mode` enum in `action.rs`:

```rust
pub enum Mode {
    Normal,
    Insert,
    Visual,
    VisualBlock,
    Command,
    Help,
}
```

### New Action

Add to the `Action` enum:

```rust
Action::ShowHelp(Option<String>)  // None = TOC, Some(tag) = specific topic
```

### App State

Add to the `App` struct:

```rust
pub help_scroll: usize,           // Current scroll offset in help view
pub help_topic: Option<String>,   // None = TOC, Some = specific topic
pub help_registry: HelpRegistry,  // Initialized at startup
```

`help_registry` is constructed once in `App::new()`.

### Command Parsing

In `mode/command.rs`, `parse_command` gains a new branch:

- `"help"` -> `Action::ShowHelp(None)`
- `"help <topic>"` -> `Action::ShowHelp(Some(topic.to_string()))`

### Action Processing

In `app.rs`, `process_action` handles `ShowHelp`:

1. If `Some(tag)`: look up tag in `help_registry.find(tag)`
   - Found: set `mode = Help`, `help_topic = Some(tag)`, `help_scroll = 0`
   - Not found: set `status_message = Some("No help for '<tag>'")`
2. If `None`: set `mode = Help`, `help_topic = None`, `help_scroll = 0`

### Input Handling in Help Mode

In `main.rs` run_loop, when `mode == Help`:

| Key | Action |
|-----|--------|
| `q`, `Esc` | Return to `Mode::Normal` |
| `j`, `Down` | Scroll down one line |
| `k`, `Up` | Scroll up one line |
| `Ctrl+D` | Scroll down half page |
| `Ctrl+U` | Scroll up half page |
| `gg` | Scroll to top |
| `G` | Scroll to bottom |
| `:` | Enter Command mode (allows `:help <other>` or `:q`) |

This reuses `NormalState` for the `gg` pending-char pattern.

### Rendering

New file `render/help.rs`. The main `render()` in `render/mod.rs` delegates to
`render_help()` when `mode == Help`.

Layout (3 regions):

1. **Title bar** (1 line) — "Cell - Help" (or "Cell - Help: <tag>" for a topic)
2. **Content area** (terminal height - 2 lines) — scrollable text
3. **Footer** (1 line) — "Press q to return | j/k scroll | :help <topic>"

#### Table of Contents (`:help`, no args)

Rendered from the registry, grouped by category:

```
Cell — Terminal Spreadsheet Editor

NORMAL MODE                                                          *normal*

  h            Move cursor left
  j            Move cursor down
  k            Move cursor up
  l            Move cursor right
  gg           Go to first row
  G            Go to last row
  0            Go to first column
  $            Go to last column
  dd           Delete current row
  yy           Yank current row
  p            Paste below
  P            Paste above
  u            Undo
  Ctrl+R       Redo
  ...

INSERT MODE                                                          *insert*

  Esc          Return to Normal mode
  Enter        Confirm edit and move down
  ...

VISUAL MODE                                                          *visual*

  v            Enter Visual mode (from Normal)
  Ctrl+V       Enter Visual Block mode (from Normal)
  ...

COMMANDS                                                            *command*

  :w [path]    Save file
  :q           Quit (fails if unsaved changes)
  :q!          Force quit
  :wq          Save and quit
  :e <path>    Open file
  :sort <col>  Sort by column
  :help [topic] Open help

FORMULAS                                                            *formula*

  SUM          SUM(range) — add values in range
  AVERAGE      AVERAGE(range) — mean of values
  COUNT        COUNT(range) — count non-empty cells
  MIN          MIN(range) — minimum value
  MAX          MAX(range) — maximum value
  IF           IF(cond, then, else) — conditional
  ...
```

#### Topic Detail View (`:help <topic>`)

Shows the full `detail` text for the matched entry, with its tags and category
as a header:

```
:help dd

DD                                                                       *dd*

Delete current row

Deletes the entire row under the cursor. The row is stored in the register
and can be pasted with p or P. This operation is undoable with u.

Category: Normal Mode
See also: :help yy, :help p, :help u
```

The "See also" line is not a clickable link — just a hint the user can type
`:help <tag>` to navigate.

#### Scroll State

The content is pre-rendered into a `Vec<String>` (one per line). The scroll
offset determines which slice of lines is displayed in the content area. This
avoids re-computing the layout on every frame.

## Integration Summary

| File | Change |
|------|--------|
| `cell-core/src/help.rs` | New: `HelpRegistry`, `HelpEntry`, `HelpCategory` |
| `cell-core/src/lib.rs` | Export `help` module |
| `cell-tui/src/action.rs` | Add `Mode::Help`, `Action::ShowHelp(Option<String>)` |
| `cell-tui/src/app.rs` | Add help state fields, handle `ShowHelp` action, init registry |
| `cell-tui/src/mode/command.rs` | Parse `help` command |
| `cell-tui/src/mode/normal.rs` | Add `NORMAL_HELP_ENTRIES` |
| `cell-tui/src/mode/insert.rs` | Add `INSERT_HELP_ENTRIES` |
| `cell-tui/src/mode/visual.rs` | Add `VISUAL_HELP_ENTRIES` |
| `cell-tui/src/mode/command.rs` | Add `COMMAND_HELP_ENTRIES` |
| `cell-core/src/formula/functions.rs` | Add `FORMULA_HELP_ENTRIES` |
| `cell-tui/src/render/help.rs` | New: help view rendering |
| `cell-tui/src/render/mod.rs` | Delegate to `render_help` when in Help mode |
| `cell-tui/src/main.rs` | Help mode input handling in run_loop |

## Content Completeness

Every keybinding, command, and formula function currently implemented must have
a corresponding `HelpEntry`. The initial set covers:

- **Normal mode**: h, j, k, l, gg, G, 0, $, w, b, dd, yy, x, p, P, u,
  Ctrl+R, Ctrl+D, Ctrl+U, Ctrl+F, Ctrl+B, Ctrl+V, v, n, N, /, :, i, a, o,
  Enter
- **Insert mode**: Esc, Enter, arrow keys, Home, End, Backspace, Delete
- **Visual mode**: v, Ctrl+V, h, j, k, l, d, y, Esc
- **Commands**: :w, :w!, :q, :q!, :wq, :e, :sort, :help
- **Formulas**: SUM, AVERAGE, COUNT, MIN, MAX, IF
