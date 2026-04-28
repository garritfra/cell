# Stdin Pipe Input — Design Spec

**Issue:** [#47](https://github.com/garritfra/cell/issues/47)
**PR:** [#67](https://github.com/garritfra/cell/pull/67)
**Date:** 2026-04-28
**Status:** Approved, implemented

---

## Background

`cell` currently requires a `FILE` argument for both interactive and headless
operation. Users coming from Unix workflows expect to pipe data in:

```bash
cat data.csv | cell                       # open the TUI on piped data
cat data.csv | cell --read A1             # one-shot lookup
psql -At -c '...' | cell --eval '=SUM(A1:A100)'
```

The core IO layer already supports parsing CSV/TSV from any `Read`, and the
native `.cell` format reader (`cell_format::read_cell_format`) is generic
over `Read` as well. The blocker is purely at the binary entry point: it
opens a file unconditionally and the TUI's terminal backend reads keyboard
input from stdin, which a pipe steals.

---

## Goals

1. **Auto-detect piped input.** When no `FILE` argument is given and stdin
   is not an interactive terminal, slurp stdin into a buffer before the
   TUI starts.
2. **Same buffer feeds both surfaces.** Bytes read from stdin route to the
   headless pipeline (`--read` / `--eval`) when those flags are present,
   otherwise to the TUI loader.
3. **Format auto-detection.** Distinguish CSV/TSV from the native `.cell`
   format on a byte stream, since extension-based detection isn't available.
4. **TUI keeps working with a piped stdin.** Crossterm's keyboard reader
   must read from `/dev/tty`, not the now-consumed stdin.
5. **Clear errors for nonsensical combinations** (e.g., `--write` on stdin
   with no save target; `--delimiter` on a `.cell`-format stream).

---

## CLI Surface

No new flags. Behavior changes when `FILE` is omitted:

| `FILE` | stdin is TTY | Headless flags? | Behavior |
|---|---|---|---|
| present | – | – | Existing: open file, route by flags |
| absent | yes | none | Existing: open empty TUI |
| absent | yes | any | Error (existing): "a FILE argument is required" |
| absent | **no** | none | **New:** read stdin → load → open TUI |
| absent | **no** | any | **New:** read stdin → load → run headless ops |

`--delimiter` continues to apply to the loaded data when it's CSV/TSV.
On a `.cell`-format stdin stream it is rejected (delimiters are meaningless
for the native format) rather than silently ignored.

`--write` requires a `FILE`. With stdin and no `FILE`, `--write` errors
because there is no save target.

---

## Format Auto-Detection on Stdin

Extension-based dispatch isn't available, so we detect on content.

### `.cell` format

The native format always opens with a fixed magic header:

```
# cell v1
```

`write_cell_format` writes this on line 1 unconditionally; `read_cell_format`
expects it. Detection is a simple byte-prefix check on `# cell v` (the
trailing version digit is consumed by the parser, so we tolerate `v1`,
`v2`, etc. at the detect step).

### CSV vs. TSV

Same as today on the file path: the existing `csv_io::sniff_delimiter`
helper counts `,`, `\t`, `|`, `;` occurrences in a sample and picks the
winner (comma breaks ties). The sample for stdin is the entire piped
buffer rather than the first 4 KB of a file — slightly more accurate at
no cost since we already have the bytes in memory.

### Dispatch table

| First bytes | Format | Delimiter |
|---|---|---|
| starts with `# cell v` | `.cell` | n/a — `--delimiter` rejected |
| anything else | CSV | sniffed from buffer, overridable by `--delimiter` |

Anything else parses as CSV. This is consistent with file-path behavior:
unknown extensions also default to CSV.

---

## TUI Input After Stdin Is Consumed

By default, `crossterm`'s event source reads from stdin via `mio`. When
stdin is a pipe, mio fails to register it as a TTY and the first call to
`event::poll` returns `"Failed to initialize input reader"`.

Crossterm exposes a `use-dev-tty` Cargo feature that swaps the event
source to one that opens `/dev/tty` directly, leaving stdin untouched.
We **must** enable this feature for the piped TUI path to work.

```toml
crossterm = { version = "0.29", features = ["use-dev-tty"] }
```

This adds `filedescriptor` and `rustix/process` as transitive deps. Both
are pure Rust, no system deps. macOS, Linux, and BSD all expose `/dev/tty`;
Windows uses a separate event source already and is unaffected by this
flag.

**Pre-existing assumption corrected:** an earlier draft of the plan
claimed crossterm 0.29 reads `/dev/tty` "automatically when stdin is
redirected." It does not — that behavior is gated on the feature flag.

---

## Data Flow

### `main.rs`

1. Parse CLI args.
2. If `FILE` is `None` and `stdin().is_terminal()` is false:
   - Lock stdin, `read_to_end` into a `Vec<u8>`.
3. Dispatch:
   - **Headless flags present:** build `headless::Options { stdin_data: Some(buf), ... }` and call `headless::run`.
   - **No headless flags:** call `run_tui(file=None, explicit_delimiter, stdin_data=Some(buf))`.

`run_tui` and the headless path each have their own loader that branches
on `stdin_data` before doing format-specific work.

### `headless::run`

```text
if stdin_data.is_some():
    if writes.is_empty():
        format = detect_stdin_format(data)         # Cell vs Csv
        if format == Cell and explicit delimiter:  # error
            "--delimiter has no effect on .cell-format input piped to stdin"
        delimiter = explicit ?? sniff(data)        # only used for Csv
        load_from_bytes(data, format, delimiter)
    else:
        "cannot use --write when reading from stdin"
else:
    existing file path
```

`load_from_bytes` dispatches: `Format::Cell` → `read_cell_format(data)`,
`Format::Csv | Format::Tsv` → `read_csv(data, delimiter)`. Both then run
the existing dep-graph + recalc bootstrap.

### TUI `load_stdin_data`

Mirror of the headless dispatch, sets `app.file_format` accordingly and
leaves `app.file_path = None` so the buffer is "unnamed" (a subsequent
`:w <path>` saves it under that path with the appropriate writer).

---

## Error Handling

| Scenario | Response |
|---|---|
| Reading stdin fails (e.g. broken pipe mid-read) | Exit 1, error to stderr |
| `--write` + stdin (no FILE) | Exit 1, "cannot use --write when reading from stdin; provide a FILE argument instead" |
| `--delimiter` + `.cell`-format stdin | Exit 1, "--delimiter has no effect on .cell-format input piped to stdin" |
| CSV parse error on stdin bytes | Exit 1, "failed to parse stdin: {csv error}" |
| Empty stdin (`echo -n \| cell`) | Empty sheet, headless ops succeed; TUI opens empty |

---

## Testing

### `cell-sheet-tui` integration (`tests/headless.rs`)

- `stdin_read_single_cell`: pipe `10,20\n30,40\n`, `--read A1` → `10`
- `stdin_read_range`: same input, `--read A1:B2` → TSV grid
- `stdin_eval_sum`: pipe a single column, `--eval =SUM(A1:A4)` → `10`
- `stdin_tsv_auto_detected`: pipe `hello\tworld\n`, `--read B1` → `world`
- `stdin_with_explicit_delimiter`: `--delimiter '|'` overrides sniff
- `stdin_write_errors_without_file`: `--write A1 99` on piped stdin fails
- `stdin_cell_format_read_single_cell`: pipe `# cell v1` blob, value reads back
- `stdin_cell_format_read_evaluates_formula`: piped cell-format formula recomputes
- `stdin_cell_format_eval_sum`: `--eval` against piped cell-format
- `stdin_cell_format_rejects_explicit_delimiter`: clear error, exit nonzero

### `headless.rs` in-module unit tests

- `run_reads_from_stdin_data` / `run_evals_from_stdin_data` / `run_rejects_write_with_stdin_data`
- `run_stdin_empty_produces_empty_sheet`
- `run_stdin_respects_explicit_delimiter`
- `run_sniffs_tsv_from_stdin_data`

### Manual smoke test (TUI path)

```bash
cat examples/demo.cell | cell           # opens TUI with budget sheet
cat data.csv          | cell --read A1
```

Verifies that crossterm + `use-dev-tty` actually reads `/dev/tty` for
keys after stdin is consumed.

---

## Non-Goals

- **Reading from stdin while a `FILE` is also given.** If both are
  present, the file wins and stdin is not read.
- **Streaming / chunked load.** We slurp the entire buffer before parsing.
  Sheets large enough to matter are bounded by the existing in-memory
  `Sheet` representation, not the slurp.
- **Saving back to stdout.** `--write` requires a file path; piping to a
  followup `cell` invocation is a future possibility.
- **Auto-detecting `.cell` format on file paths without the `.cell`
  extension.** File-path dispatch stays extension-driven.
- **Detecting whether `/dev/tty` is available.** Headless containers
  without a controlling terminal won't be able to launch the TUI even
  with stdin support; that's an existing limitation, not introduced here.
