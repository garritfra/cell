# Custom CSV Delimiter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users open, edit, and save delimiter-separated files using any single-byte ASCII delimiter (pipe, semicolon, etc.) via a CLI flag, auto-sniff, and a `:set delimiter=X` ex-command.

**Architecture:** Add a `delimiter: u8` field to `App` (default `b','`) that is resolved from the CLI flag, auto-sniff, or `:set delimiter=X` and is used by `do_save` for all CSV/TSV writes. A `sniff_delimiter` utility in `cell-sheet-core` reads the first line to pick the most frequent candidate delimiter. A save warning fires when the delimiter doesn't match the file extension's convention (mirrors the existing formula-flatten warning).

**Tech Stack:** Rust, `csv` crate (already used), `clap` (already used), `tempfile` (already a dev-dependency).

---

## File Map

| File | Change |
|---|---|
| `crates/cell-sheet-core/src/io/csv.rs` | Add `pub fn sniff_delimiter(sample: &[u8]) -> u8`; add non-standard-delimiter round-trip tests |
| `crates/cell-sheet-tui/src/action.rs` | Add `SetDelimiter(u8)` variant |
| `crates/cell-sheet-tui/src/app.rs` | Add `delimiter: u8` field; handle `SetDelimiter`; update `Action::Save` warning; update `do_save` |
| `crates/cell-sheet-tui/src/mode/command.rs` | Parse `:set delimiter=X` |
| `crates/cell-sheet-tui/src/main.rs` | Add `--delimiter` CLI flag; `parse_delimiter` validator; pass delimiter to `load_file` and `run_tui` |
| `crates/cell-sheet-tui/src/headless.rs` | Add `delimiter: Option<u8>` to `Options`; `resolve_delimiter`; pass delimiter through `load`/`save` |
| `crates/cell-sheet-tui/tests/headless.rs` | Integration tests for `--delimiter` and auto-sniff |
| `CHANGELOG.md` | Entry under `## Unreleased → Added` |

---

## Task 1: `sniff_delimiter` in `cell-sheet-core`

**Files:**
- Modify: `crates/cell-sheet-core/src/io/csv.rs`

- [ ] **Step 1: Write failing tests**

Add inside the existing `#[cfg(test)] mod tests` block at the bottom of `crates/cell-sheet-core/src/io/csv.rs`:

```rust
#[test]
fn sniff_pipe_delimiter() {
    let sample = b"name|score|grade\nalice|95|A\n";
    assert_eq!(sniff_delimiter(sample), b'|');
}

#[test]
fn sniff_semicolon_delimiter() {
    let sample = b"a;b;c\n1;2;3\n";
    assert_eq!(sniff_delimiter(sample), b';');
}

#[test]
fn sniff_tab_delimiter() {
    let sample = b"a\tb\tc\n1\t2\t3\n";
    assert_eq!(sniff_delimiter(sample), b'\t');
}

#[test]
fn sniff_empty_defaults_to_comma() {
    assert_eq!(sniff_delimiter(b""), b',');
}

#[test]
fn sniff_tie_prefers_comma() {
    // one comma, one pipe — comma wins ties
    assert_eq!(sniff_delimiter(b"a,b|c\n"), b',');
}

#[test]
fn sniff_only_reads_first_line() {
    // First line has pipes; second line has many commas — sniff ignores line 2
    let sample = b"a|b|c\n1,2,3,4,5,6,7,8,9\n";
    assert_eq!(sniff_delimiter(sample), b'|');
}

#[test]
fn read_pipe_delimited() {
    let data = "a|b|c\n1|2|3\n";
    let sheet = read_csv(data.as_bytes(), b'|').unwrap();
    assert_eq!(sheet.row_count, 2);
    assert_eq!(sheet.col_count, 3);
    assert_eq!(
        sheet.get_cell((0, 1)).unwrap().value,
        CellValue::Text("b".into())
    );
    assert_eq!(
        sheet.get_cell((1, 2)).unwrap().value,
        CellValue::Number(3.0)
    );
}

#[test]
fn write_pipe_delimited() {
    let mut sheet = Sheet::new();
    sheet.set_cell((0, 0), "a");
    sheet.set_cell((0, 1), "b");
    let mut buf = Vec::new();
    write_csv(&sheet, &mut buf, b'|').unwrap();
    assert_eq!(String::from_utf8(buf).unwrap(), "a|b\n");
}

#[test]
fn sniff_then_read_round_trip() {
    let data = b"x|y|z\n1|2|3\n";
    let delim = sniff_delimiter(data);
    assert_eq!(delim, b'|');
    let sheet = read_csv(data.as_ref(), delim).unwrap();
    assert_eq!(sheet.col_count, 3);
    assert_eq!(
        sheet.get_cell((1, 1)).unwrap().value,
        CellValue::Number(2.0)
    );
}
```

- [ ] **Step 2: Run to confirm they fail**

```bash
cargo test -p cell-sheet-core -- sniff 2>&1 | head -20
```

Expected: errors like `cannot find function sniff_delimiter`.

- [ ] **Step 3: Implement `sniff_delimiter`**

Add this function **before** `read_csv` in `crates/cell-sheet-core/src/io/csv.rs`:

```rust
/// Inspect the first line of `sample` (up to 4 KiB) and return the most
/// frequent delimiter among `,`, `\t`, `|`, `;`. Ties are broken in that
/// order (comma wins ties). Returns `b','` for empty input.
pub fn sniff_delimiter(sample: &[u8]) -> u8 {
    let line_end = sample
        .iter()
        .position(|&b| b == b'\n')
        .unwrap_or(sample.len());
    let line = &sample[..line_end.min(4096)];

    // Iterate in preference order so the first candidate wins ties.
    let candidates = [b',', b'\t', b'|', b';'];
    let mut best_delim = b',';
    let mut best_count = 0usize;
    for &d in &candidates {
        let count = line.iter().filter(|&&b| b == d).count();
        if count > best_count {
            best_count = count;
            best_delim = d;
        }
    }
    best_delim
}
```

- [ ] **Step 4: Run tests and confirm they pass**

```bash
cargo test -p cell-sheet-core
```

Expected: all tests pass (including the nine new ones).

- [ ] **Step 5: Commit**

```bash
git add crates/cell-sheet-core/src/io/csv.rs
git commit -m "feat: add sniff_delimiter and non-standard delimiter round-trip tests"
```

---

## Task 2: `Action::SetDelimiter` and `App.delimiter`

**Files:**
- Modify: `crates/cell-sheet-tui/src/action.rs`
- Modify: `crates/cell-sheet-tui/src/app.rs`

- [ ] **Step 1: Write failing test**

Add inside the `#[cfg(test)] mod tests` block at the bottom of `crates/cell-sheet-tui/src/app.rs`:

```rust
#[test]
fn set_delimiter_updates_field_and_status() {
    let mut app = App::new();
    assert_eq!(app.delimiter, b',', "default delimiter should be comma");
    app.process_action(Action::SetDelimiter(b'|'));
    assert_eq!(app.delimiter, b'|');
    assert_eq!(
        app.status_message.as_deref(),
        Some("Delimiter set to '|'")
    );
}

#[test]
fn set_delimiter_tab() {
    let mut app = App::new();
    app.process_action(Action::SetDelimiter(b'\t'));
    assert_eq!(app.delimiter, b'\t');
}
```

- [ ] **Step 2: Run to confirm they fail**

```bash
cargo test -p cell-sheet-tui -- set_delimiter 2>&1 | head -20
```

Expected: compile error — `SetDelimiter` not yet defined.

- [ ] **Step 3: Add `SetDelimiter(u8)` to `Action`**

In `crates/cell-sheet-tui/src/action.rs`, add after the `ShowHelp` line:

```rust
SetDelimiter(u8),
```

The full enum tail should look like:

```rust
    YankRow(usize),
    ShowHelp(Option<String>),
    SetDelimiter(u8),
}
```

- [ ] **Step 4: Add `delimiter` field to `App`**

In `crates/cell-sheet-tui/src/app.rs`, add to the `App` struct after `insert_buffer`:

```rust
pub delimiter: u8,
```

In `App::new()`, add after `insert_buffer: String::new(),`:

```rust
delimiter: b',',
```

- [ ] **Step 5: Handle `SetDelimiter` in `process_action`**

In the `match action` block in `process_action`, add before the closing brace (before `Action::Open(_) | Action::Resize => {}`):

```rust
Action::SetDelimiter(d) => {
    self.delimiter = d;
    self.status_message = Some(format!("Delimiter set to '{}'", d as char));
}
```

- [ ] **Step 6: Run tests and confirm they pass**

```bash
cargo test -p cell-sheet-tui
```

Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/cell-sheet-tui/src/action.rs crates/cell-sheet-tui/src/app.rs
git commit -m "feat: add App.delimiter field and Action::SetDelimiter"
```

---

## Task 3: `:set delimiter=X` command parsing

**Files:**
- Modify: `crates/cell-sheet-tui/src/mode/command.rs`

- [ ] **Step 1: Write failing tests**

Add inside the `#[cfg(test)] mod tests` block in `crates/cell-sheet-tui/src/mode/command.rs`:

```rust
#[test]
fn parse_set_delimiter_pipe() {
    assert_eq!(parse_command("set delimiter=|"), Action::SetDelimiter(b'|'));
}

#[test]
fn parse_set_delimiter_semicolon() {
    assert_eq!(parse_command("set delimiter=;"), Action::SetDelimiter(b';'));
}

#[test]
fn parse_set_delimiter_tab() {
    // Users can type :set delimiter=<Tab> — less likely but valid
    assert_eq!(parse_command("set delimiter=\t"), Action::SetDelimiter(b'\t'));
}

#[test]
fn parse_set_delimiter_empty_is_noop() {
    assert_eq!(parse_command("set delimiter="), Action::Noop);
}

#[test]
fn parse_set_delimiter_alphanumeric_is_noop() {
    assert_eq!(parse_command("set delimiter=a"), Action::Noop);
    assert_eq!(parse_command("set delimiter=1"), Action::Noop);
}

#[test]
fn parse_set_delimiter_multi_char_is_noop() {
    assert_eq!(parse_command("set delimiter=||"), Action::Noop);
}

#[test]
fn parse_set_delimiter_non_ascii_is_noop() {
    assert_eq!(parse_command("set delimiter=€"), Action::Noop);
}
```

- [ ] **Step 2: Run to confirm they fail**

```bash
cargo test -p cell-sheet-tui -- parse_set_delimiter 2>&1 | head -20
```

Expected: tests fail because `parse_command("set delimiter=|")` currently returns `Action::Noop`.

- [ ] **Step 3: Add parsing branch in `parse_command`**

In `crates/cell-sheet-tui/src/mode/command.rs`, inside `parse_command`, add a new `else if` branch **before** the final `else { Action::Noop }`:

```rust
} else if let Some(stripped) = input.strip_prefix("set delimiter=") {
    let mut chars = stripped.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii() && !c.is_alphanumeric() => {
            Action::SetDelimiter(c as u8)
        }
        _ => Action::Noop,
    }
} else {
    Action::Noop
}
```

You also need to add the import at the top of the file (it should already be there since `Action` is imported, but make sure `SetDelimiter` is accessible through it):

The existing `use crate::action::Action;` already covers this since `SetDelimiter` is a variant.

- [ ] **Step 4: Run tests and confirm they pass**

```bash
cargo test -p cell-sheet-tui -- command
```

Expected: all command tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/cell-sheet-tui/src/mode/command.rs
git commit -m "feat: parse :set delimiter=X ex-command"
```

---

## Task 4: Save warning + `do_save` uses `self.delimiter`

**Files:**
- Modify: `crates/cell-sheet-tui/src/app.rs`

- [ ] **Step 1: Write failing tests**

Add to the `#[cfg(test)] mod tests` block in `crates/cell-sheet-tui/src/app.rs`:

```rust
#[test]
fn save_csv_with_non_comma_delimiter_warns() {
    let mut app = App::new();
    // No file_path — triggers "No file name". Set one.
    // We deliberately choose a non-existent path so the actual write fails,
    // but the delimiter warning must fire *before* the write attempt.
    app.file_path = Some(std::path::PathBuf::from("data.csv"));
    app.file_format = FileFormat::Csv;
    app.delimiter = b'|';
    app.process_action(Action::Save(None));
    let msg = app.status_message.as_deref().unwrap_or("");
    assert!(
        msg.contains("Non-standard delimiter"),
        "expected delimiter warning, got: {msg:?}"
    );
}

#[test]
fn save_tsv_with_non_tab_delimiter_warns() {
    let mut app = App::new();
    app.file_path = Some(std::path::PathBuf::from("data.tsv"));
    app.file_format = FileFormat::Tsv;
    app.delimiter = b'|';
    app.process_action(Action::Save(None));
    let msg = app.status_message.as_deref().unwrap_or("");
    assert!(
        msg.contains("Non-standard delimiter"),
        "expected delimiter warning, got: {msg:?}"
    );
}

#[test]
fn save_csv_with_comma_delimiter_no_delimiter_warning() {
    let mut app = App::new();
    // delimiter is b',' (default) — no warning should fire.
    // Using a path that would fail the write; the error message should NOT
    // mention "Non-standard delimiter".
    app.file_path = Some(std::path::PathBuf::from("data.csv"));
    app.file_format = FileFormat::Csv;
    app.delimiter = b',';
    app.process_action(Action::Save(None));
    let msg = app.status_message.as_deref().unwrap_or("");
    assert!(
        !msg.contains("Non-standard delimiter"),
        "unexpected delimiter warning: {msg:?}"
    );
}

#[test]
fn force_save_csv_with_non_comma_delimiter_skips_warning() {
    let mut app = App::new();
    app.file_path = Some(std::path::PathBuf::from("data.csv"));
    app.file_format = FileFormat::Csv;
    app.delimiter = b'|';
    // ForceSave must not produce the delimiter warning.
    // (It may produce an I/O error since the path may not be writable, which is fine.)
    app.process_action(Action::ForceSave(None));
    let msg = app.status_message.as_deref().unwrap_or("");
    assert!(
        !msg.contains("Non-standard delimiter"),
        "ForceSave should bypass delimiter warning, got: {msg:?}"
    );
}
```

- [ ] **Step 2: Run to confirm they fail**

```bash
cargo test -p cell-sheet-tui -- save_csv_with_non_comma 2>&1 | head -30
```

Expected: tests fail — no warning fires yet.

- [ ] **Step 3: Add delimiter warning in `Action::Save`**

In `crates/cell-sheet-tui/src/app.rs`, inside `process_action`, find the `Action::Save(path_opt)` arm. After the existing formula warning block and **before** the `self.do_save(...)` call, insert:

```rust
// Warn when the active delimiter doesn't match the file extension's convention.
let expected_delim = match format {
    FileFormat::Csv => b',',
    FileFormat::Tsv => b'\t',
    FileFormat::Cell => 0, // irrelevant — guarded by the match below
};
if !matches!(format, FileFormat::Cell) && self.delimiter != expected_delim {
    self.status_message = Some(format!(
        "Non-standard delimiter '{}' will be used. Use :w! to force, or save as .tsv / .psv.",
        self.delimiter as char
    ));
    return;
}
```

The updated `Action::Save` arm should look like:

```rust
Action::Save(path_opt) => {
    let path = path_opt.or(self.file_path.clone());
    if let Some(path) = path {
        let format = Self::format_from_path(&path);
        if !matches!(format, FileFormat::Cell) && self.has_formulas() {
            self.status_message = Some(
                "Sheet contains formulas that will be lost. Use :w file.cell to preserve, or :w! to save as CSV anyway.".into()
            );
            return;
        }
        let expected_delim = match format {
            FileFormat::Csv => b',',
            FileFormat::Tsv => b'\t',
            FileFormat::Cell => 0,
        };
        if !matches!(format, FileFormat::Cell) && self.delimiter != expected_delim {
            self.status_message = Some(format!(
                "Non-standard delimiter '{}' will be used. Use :w! to force, or save as .tsv / .psv.",
                self.delimiter as char
            ));
            return;
        }
        self.do_save(&path, format);
    } else {
        self.status_message = Some("No file name".into());
    }
}
```

- [ ] **Step 4: Update `do_save` to use `self.delimiter`**

Find `fn do_save` in `crates/cell-sheet-tui/src/app.rs`. Replace the hardcoded `b','` and `b'\t'` with `self.delimiter`:

```rust
fn do_save(&mut self, path: &PathBuf, format: FileFormat) {
    let delimiter = self.delimiter;
    let result = match format {
        FileFormat::Csv | FileFormat::Tsv => std::fs::File::create(path)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
            .and_then(|f| cell_sheet_core::io::csv::write_csv(&self.sheet, f, delimiter)),
        FileFormat::Cell => std::fs::File::create(path)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
            .and_then(|f| cell_sheet_core::io::cell_format::write_cell_format(&self.sheet, f)),
    };

    match result {
        Ok(()) => {
            self.file_path = Some(path.clone());
            self.file_format = format;
            self.dirty = false;
            self.status_message = Some(format!("Written to {}", path.display()));
        }
        Err(e) => {
            self.status_message = Some(format!("Error saving: {}", e));
        }
    }
}
```

- [ ] **Step 5: Run tests and confirm they pass**

```bash
cargo test -p cell-sheet-tui
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/cell-sheet-tui/src/app.rs
git commit -m "feat: warn on non-standard delimiter when saving; do_save uses self.delimiter"
```

---

## Task 5: CLI `--delimiter` flag, auto-sniff integration, and headless support

**Files:**
- Modify: `crates/cell-sheet-tui/src/main.rs`
- Modify: `crates/cell-sheet-tui/src/headless.rs`
- Modify: `crates/cell-sheet-tui/tests/headless.rs`

- [ ] **Step 1: Write failing integration tests**

Add to the bottom of `crates/cell-sheet-tui/tests/headless.rs`:

```rust
#[test]
fn read_pipe_delimited_with_delimiter_flag() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_csv(&dir, "data.psv", "a|b|c\n1|2|3\n");

    let out = run(&[path.to_str().unwrap(), "--delimiter", "|", "--read", "B2"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(stdout(&out), "2\n");
}

#[test]
fn read_pipe_delimited_auto_sniff_csv_extension() {
    // .csv extension but pipe-separated content — sniff should detect the pipe
    let dir = tempfile::tempdir().unwrap();
    let path = write_csv(&dir, "data.csv", "a|b|c\n1|2|3\n");

    let out = run(&[path.to_str().unwrap(), "--read", "B2"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(stdout(&out), "2\n");
}

#[test]
fn read_semicolon_delimited_with_flag() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_csv(&dir, "data.csv", "a;b;c\n10;20;30\n");

    let out = run(&[path.to_str().unwrap(), "--delimiter", ";", "--read", "C2"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(stdout(&out), "30\n");
}

#[test]
fn write_pipe_delimited_with_flag() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_csv(&dir, "data.psv", "a|b\n1|2\n");

    // Write a new value and check the file stays pipe-delimited
    let out = run(&[
        path.to_str().unwrap(),
        "--delimiter",
        "|",
        "--write",
        "A1",
        "99",
    ]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));

    let after = std::fs::read_to_string(&path).unwrap();
    assert!(after.starts_with("99|b\n"), "got: {after:?}");
}

#[test]
fn invalid_delimiter_exits_with_code_2() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_csv(&dir, "data.csv", "a,b\n");

    // Alphanumeric delimiter is not valid
    let out = run(&[path.to_str().unwrap(), "--delimiter", "a", "--read", "A1"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(!stderr(&out).is_empty());
}
```

- [ ] **Step 2: Run to confirm they fail**

```bash
cargo test -p cell-sheet-tui --test headless -- delimiter 2>&1 | head -30
```

Expected: fail — `--delimiter` flag doesn't exist yet.

- [ ] **Step 3: Add `--delimiter` to `Cli` and add `parse_delimiter` helper in `main.rs`**

In `crates/cell-sheet-tui/src/main.rs`, update the `Cli` struct to add the delimiter field:

```rust
#[derive(Parser)]
#[command(name = "cell", version, about = "A terminal spreadsheet editor")]
struct Cli {
    /// File to open (CSV, TSV, or .cell)
    file: Option<PathBuf>,

    /// Print the computed value of a cell or range (e.g. A1, A1:B3).
    /// Repeat to read multiple refs. Ranges render as TSV.
    #[arg(long, value_name = "REF")]
    read: Vec<String>,

    /// Evaluate a formula against the loaded sheet without persisting.
    /// The leading `=` is optional. Repeat to evaluate multiple expressions.
    #[arg(long, value_name = "EXPR")]
    eval: Vec<String>,

    /// Set a cell to a value (auto-detects formula if it starts with `=`).
    /// Repeat to batch multiple writes into a single save.
    #[arg(long, value_names = ["REF", "VALUE"], num_args = 2)]
    write: Vec<String>,

    /// Field delimiter character (e.g. '|', ';'). Auto-detected from file
    /// content when omitted; .tsv files always default to tab.
    #[arg(long, value_name = "CHAR")]
    delimiter: Option<char>,
}
```

Add this function **before** `main()`:

```rust
fn parse_delimiter(c: char) -> Result<u8, String> {
    if !c.is_ascii() {
        return Err(format!("delimiter must be a single ASCII character, got {c:?}"));
    }
    if c.is_alphanumeric() || c == '"' || c == '\n' || c == '\r' {
        return Err(format!("'{c}' is not a valid field delimiter"));
    }
    Ok(c as u8)
}
```

- [ ] **Step 4: Resolve delimiter in `main()` and thread it through**

Replace the body of `main()` in `crates/cell-sheet-tui/src/main.rs` with:

```rust
fn main() -> ExitCode {
    let cli = Cli::parse();

    let explicit_delimiter = match cli.delimiter.map(parse_delimiter) {
        Some(Err(msg)) => {
            eprintln!("error: {msg}");
            return ExitCode::from(2);
        }
        Some(Ok(b)) => Some(b),
        None => None,
    };

    let opts = headless::Options {
        file: cli.file.clone().unwrap_or_default(),
        reads: cli.read,
        evals: cli.eval,
        writes: cli
            .write
            .chunks_exact(2)
            .map(|c| (c[0].clone(), c[1].clone()))
            .collect(),
        delimiter: explicit_delimiter,
    };

    if opts.is_active() {
        if cli.file.is_none() {
            eprintln!("error: a FILE argument is required for --read/--eval/--write");
            return ExitCode::from(2);
        }
        let mut stdout = io::stdout().lock();
        return match headless::run(&opts, &mut stdout) {
            Ok(()) => ExitCode::SUCCESS,
            Err(msg) => {
                eprintln!("error: {msg}");
                ExitCode::FAILURE
            }
        };
    }

    match run_tui(cli.file.as_deref(), explicit_delimiter) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
```

- [ ] **Step 5: Update `run_tui` and `load_file` signatures in `main.rs`**

Replace `run_tui` with:

```rust
fn run_tui(
    file: Option<&std::path::Path>,
    explicit_delimiter: Option<u8>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new();

    if let Some(path) = file {
        load_file(&mut app, path, explicit_delimiter)?;
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    result
}
```

Replace `load_file` with:

```rust
fn load_file(
    app: &mut App,
    path: &std::path::Path,
    explicit_delimiter: Option<u8>,
) -> Result<(), Box<dyn std::error::Error>> {
    use cell_sheet_core::formula::deps::{recalculate, set_formula};

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "cell" => {
            let file = std::fs::File::open(path)?;
            app.sheet = cell_sheet_core::io::cell_format::read_cell_format(file)?;
            app.file_format = FileFormat::Cell;
            // delimiter is irrelevant for .cell; leave app.delimiter at its default
        }
        _ => {
            // Read entire file once; use bytes both for sniffing and parsing.
            let data = std::fs::read(path)?;
            let delimiter = if let Some(d) = explicit_delimiter {
                d
            } else if ext == "tsv" {
                b'\t'
            } else {
                cell_sheet_core::io::csv::sniff_delimiter(&data)
            };
            app.sheet = cell_sheet_core::io::csv::read_csv(data.as_slice(), delimiter)?;
            app.file_format = if ext == "tsv" {
                FileFormat::Tsv
            } else {
                FileFormat::Csv
            };
            app.delimiter = delimiter;
        }
    }

    app.file_path = Some(path.to_path_buf());

    // Register formulas in the dependency graph and evaluate them.
    let formula_cells: Vec<_> = app
        .sheet
        .cells
        .iter()
        .filter(|(_, cell)| cell.raw.starts_with('='))
        .map(|(pos, cell)| (*pos, cell.raw.clone()))
        .collect();
    for (pos, raw) in formula_cells {
        set_formula(&mut app.sheet, &mut app.deps, pos, &raw);
    }
    recalculate(&mut app.sheet, &app.deps);

    Ok(())
}
```

- [ ] **Step 6: Update `headless::Options` and `headless::run`**

In `crates/cell-sheet-tui/src/headless.rs`:

Add `delimiter` to `Options`:

```rust
#[derive(Debug)]
pub struct Options {
    pub file: PathBuf,
    pub reads: Vec<String>,
    pub evals: Vec<String>,
    pub writes: Vec<(String, String)>,
    pub delimiter: Option<u8>,
}
```

Add a `resolve_delimiter` helper after the `Format` impl block:

```rust
fn resolve_delimiter(
    path: &Path,
    format: Format,
    explicit: Option<u8>,
) -> Result<u8, Box<dyn std::error::Error>> {
    if let Some(d) = explicit {
        return Ok(d);
    }
    match format {
        Format::Tsv => Ok(b'\t'),
        Format::Cell => Ok(b','), // unused — .cell files don't use a delimiter
        Format::Csv => {
            // Read only a small sample for sniffing — avoid a full second read.
            use std::io::Read as _;
            let mut buf = vec![0u8; 4096];
            let mut file = std::fs::File::open(path)?;
            let n = file.read(&mut buf)?;
            Ok(csv_io::sniff_delimiter(&buf[..n]))
        }
    }
}
```

Replace the `load` function with:

```rust
fn load(
    path: &Path,
    format: Format,
    delimiter: u8,
) -> Result<(Sheet, DepGraph), Box<dyn std::error::Error>> {
    let mut sheet = match format {
        Format::Csv => {
            let data = std::fs::read(path)?;
            csv_io::read_csv(data.as_slice(), delimiter)?
        }
        Format::Tsv => {
            let file = std::fs::File::open(path)?;
            csv_io::read_csv(file, delimiter)?
        }
        Format::Cell => {
            let file = std::fs::File::open(path)?;
            cell_format::read_cell_format(file)?
        }
    };
    let mut deps = DepGraph::new();

    let formula_cells: Vec<_> = sheet
        .cells
        .iter()
        .filter(|(_, cell)| cell.raw.starts_with('='))
        .map(|(pos, cell)| (*pos, cell.raw.clone()))
        .collect();
    for (pos, raw) in formula_cells {
        set_formula(&mut sheet, &mut deps, pos, &raw);
    }
    recalculate(&mut sheet, &deps);

    Ok((sheet, deps))
}
```

Replace the `save` function with:

```rust
fn save(
    path: &Path,
    format: Format,
    sheet: &Sheet,
    delimiter: u8,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = std::fs::File::create(path)?;
    match format {
        Format::Csv | Format::Tsv => csv_io::write_csv(sheet, file, delimiter)?,
        Format::Cell => cell_format::write_cell_format(sheet, file)?,
    }
    Ok(())
}
```

Replace the `run` function with:

```rust
pub fn run<W: Write>(opts: &Options, out: &mut W) -> Result<(), String> {
    let format = Format::from_path(&opts.file);
    let delimiter = resolve_delimiter(&opts.file, format, opts.delimiter)
        .map_err(|e| format!("failed to read {}: {e}", opts.file.display()))?;

    let (mut sheet, mut deps) = load(&opts.file, format, delimiter)
        .map_err(|e| format!("failed to read {}: {e}", opts.file.display()))?;

    if !opts.writes.is_empty() {
        for (idx, (ref_str, value)) in opts.writes.iter().enumerate() {
            let pos = parse_single_ref(ref_str).ok_or_else(|| {
                format!(
                    "invalid cell reference for --write #{}: {ref_str:?}",
                    idx + 1
                )
            })?;
            apply_write(&mut sheet, &mut deps, pos, value);
        }
        recalculate(&mut sheet, &deps);
        save(&opts.file, format, &sheet, delimiter)
            .map_err(|e| format!("failed to write {}: {e}", opts.file.display()))?;
    }

    for ref_str in &opts.reads {
        let rendered = render_read(&sheet, ref_str)?;
        writeln!(out, "{rendered}").map_err(|e| format!("write error: {e}"))?;
    }

    for expr in &opts.evals {
        let formula = expr.strip_prefix('=').unwrap_or(expr);
        let value = eval::evaluate(formula, &sheet);
        if let CellValue::Error(err) = &value {
            return Err(format!("evaluation error in {expr:?}: {err}"));
        }
        writeln!(out, "{value}").map_err(|e| format!("write error: {e}"))?;
    }

    Ok(())
}
```

- [ ] **Step 7: Run all tests**

```bash
cargo test
```

Expected: all tests pass, including the five new integration tests.

- [ ] **Step 8: Commit**

```bash
git add crates/cell-sheet-tui/src/main.rs \
        crates/cell-sheet-tui/src/headless.rs \
        crates/cell-sheet-tui/tests/headless.rs
git commit -m "feat: add --delimiter flag, auto-sniff, and headless delimiter support"
```

---

## Task 6: Final verification and CHANGELOG

**Files:**
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Format and lint**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features
```

Expected: no warnings, no errors. Fix any clippy lints that appear before moving on. Common ones to watch for:
- Unused `use std::io::Read as _;` if the import wasn't needed
- `match format { Format::Csv | Format::Tsv => ... }` — ensure the wildcard arm doesn't produce a dead-code warning

- [ ] **Step 2: Run full test suite**

```bash
cargo test
```

Expected: all tests pass across both crates.

- [ ] **Step 3: Add CHANGELOG entry**

Open `CHANGELOG.md` and add under `## Unreleased` → `### Added`:

```markdown
### Added
- Custom field delimiter support: `--delimiter '|'` CLI flag, auto-detection from
  file content, and `:set delimiter=X` ex-command. Writing with a non-standard
  delimiter to a `.csv` or `.tsv` file shows a warning; use `:w!` to override.
  Resolves #20.
```

- [ ] **Step 4: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs: add CHANGELOG entry for custom delimiter support (#20)"
```

---

## Spec Coverage Checklist

| Spec requirement | Task |
|---|---|
| `sniff_delimiter` function in `csv.rs` | Task 1 |
| Round-trip tests with `b'|'` and `b';'` | Task 1 |
| `App.delimiter: u8` field (default `b','`) | Task 2 |
| `Action::SetDelimiter(u8)` | Task 2 |
| `process_action` handles `SetDelimiter` | Task 2 |
| `:set delimiter=X` command parsing | Task 3 |
| Validation: empty/multi-char/alphanumeric → `Noop` | Task 3 |
| Save warning on non-standard delimiter | Task 4 |
| `ForceSave` bypasses warning | Task 4 |
| `do_save` uses `self.delimiter` | Task 4 |
| `--delimiter <CHAR>` CLI flag | Task 5 |
| `parse_delimiter` validator (non-ASCII, alphanumeric rejected) | Task 5 |
| `load_file` auto-sniffs `.csv` and unknown extensions | Task 5 |
| `.tsv` always uses `b'\t'` (no sniff) | Task 5 |
| `headless::Options.delimiter` | Task 5 |
| Headless `load`/`save` use resolved delimiter | Task 5 |
| Integration tests for `--delimiter` flag | Task 5 |
| Integration test for auto-sniff | Task 5 |
| CHANGELOG entry | Task 6 |
