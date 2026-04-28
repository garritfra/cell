# Stdin Pipe Input Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow `cell` to read spreadsheet data from stdin so users can pipe CSV/TSV directly into the editor or headless mode.

**Architecture:** Detect stdin-is-not-a-TTY before the TUI starts; buffer all bytes; pass them through the same CSV parsing pipeline as file loading. Headless mode gets `stdin_data: Option<Vec<u8>>` on `Options`; TUI gets an additional `stdin_data` parameter on `run_tui`. The `--write` flag errors when used with stdin since there is no file path to save back to.

**Tech Stack:** Rust, `std::io::IsTerminal` (stable since 1.70), existing `cell_sheet_core::io::csv::{read_csv, sniff_delimiter}`, crossterm 0.29 (already opens `/dev/tty` for keyboard input when stdin is a pipe).

---

## File map

| File | Change |
|------|--------|
| `crates/cell-sheet-tui/src/main.rs` | Detect stdin, read bytes, route to TUI or headless |
| `crates/cell-sheet-tui/src/headless.rs` | Add `stdin_data` to `Options`, load from bytes, reject `--write` + stdin |
| `crates/cell-sheet-tui/tests/headless.rs` | Integration tests for headless stdin path |
| `CHANGELOG.md` | Add `Added` entry under `## Unreleased` |

---

## Task 1: Add `stdin_data` to headless `Options` and update `headless::run`

**Files:**
- Modify: `crates/cell-sheet-tui/src/headless.rs`

- [ ] **Step 1: Add `stdin_data` field to `Options` and a `load_from_bytes` helper**

Replace the `Options` struct and add `load_from_bytes` in `headless.rs`:

```rust
#[derive(Debug)]
pub struct Options {
    pub file: PathBuf,
    /// Raw bytes read from stdin when no FILE was given and stdin is not a TTY.
    pub stdin_data: Option<Vec<u8>>,
    pub reads: Vec<String>,
    pub evals: Vec<String>,
    pub writes: Vec<(String, String)>,
    pub delimiter: Option<u8>,
}
```

Add after the existing `load` function:

```rust
fn load_from_bytes(
    data: &[u8],
    delimiter: u8,
) -> Result<(Sheet, DepGraph), Box<dyn std::error::Error>> {
    let mut sheet = csv_io::read_csv(data, delimiter)?;
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

- [ ] **Step 2: Update `headless::run` to use stdin_data when present**

Replace the first section of `run()` (the load block) with:

```rust
pub fn run<W: Write>(opts: &Options, out: &mut W) -> Result<(), String> {
    let (mut sheet, mut deps) = if let Some(ref data) = opts.stdin_data {
        if !opts.writes.is_empty() {
            return Err(
                "cannot use --write when reading from stdin; provide a FILE argument instead"
                    .to_string(),
            );
        }
        let delimiter = opts
            .delimiter
            .unwrap_or_else(|| csv_io::sniff_delimiter(data));
        load_from_bytes(data, delimiter)
            .map_err(|e| format!("failed to parse stdin: {e}"))?
    } else {
        let format = Format::from_path(&opts.file);
        let delimiter = resolve_delimiter(&opts.file, format, opts.delimiter)
            .map_err(|e| format!("failed to read {}: {e}", opts.file.display()))?;
        load(&opts.file, format, delimiter)
            .map_err(|e| format!("failed to read {}: {e}", opts.file.display()))?
    };

    if !opts.writes.is_empty() {
        // (stdin_data case already returned an error above)
        let format = Format::from_path(&opts.file);
        let delimiter = resolve_delimiter(&opts.file, format, opts.delimiter)
            .map_err(|e| format!("failed to read {}: {e}", opts.file.display()))?;
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

- [ ] **Step 3: Add unit tests for headless stdin path in headless.rs**

Add to the `#[cfg(test)]` block in `headless.rs`:

```rust
#[test]
fn run_reads_from_stdin_data() {
    let opts = Options {
        file: PathBuf::new(),
        stdin_data: Some(b"10,20\n30,40\n".to_vec()),
        reads: vec!["A1".to_string()],
        evals: vec![],
        writes: vec![],
        delimiter: None,
    };
    let mut out = Vec::new();
    run(&opts, &mut out).unwrap();
    assert_eq!(String::from_utf8(out).unwrap(), "10\n");
}

#[test]
fn run_evals_from_stdin_data() {
    let opts = Options {
        file: PathBuf::new(),
        stdin_data: Some(b"1\n2\n3\n4\n".to_vec()),
        reads: vec![],
        evals: vec!["=SUM(A1:A4)".to_string()],
        writes: vec![],
        delimiter: None,
    };
    let mut out = Vec::new();
    run(&opts, &mut out).unwrap();
    assert_eq!(String::from_utf8(out).unwrap(), "10\n");
}

#[test]
fn run_rejects_write_with_stdin_data() {
    let opts = Options {
        file: PathBuf::new(),
        stdin_data: Some(b"10,20\n".to_vec()),
        reads: vec![],
        evals: vec![],
        writes: vec![("A1".to_string(), "99".to_string())],
        delimiter: None,
    };
    let mut out = Vec::new();
    let err = run(&opts, &mut out).unwrap_err();
    assert!(err.contains("--write"), "expected --write in error: {err}");
}

#[test]
fn run_sniffs_tsv_from_stdin_data() {
    let opts = Options {
        file: PathBuf::new(),
        stdin_data: Some(b"hello\tworld\n".to_vec()),
        reads: vec!["B1".to_string()],
        evals: vec![],
        writes: vec![],
        delimiter: None,
    };
    let mut out = Vec::new();
    run(&opts, &mut out).unwrap();
    assert_eq!(String::from_utf8(out).unwrap(), "world\n");
}
```

- [ ] **Step 4: Run unit tests to verify they pass**

```
cargo test -p cell-sheet-tui -- headless
```

Expected: all headless tests pass (new ones + existing).

- [ ] **Step 5: Commit**

```bash
git add crates/cell-sheet-tui/src/headless.rs
git commit -m "feat: add stdin_data to headless Options and load_from_bytes helper"
```

---

## Task 2: Update `main.rs` to detect stdin and route it

**Files:**
- Modify: `crates/cell-sheet-tui/src/main.rs`

- [ ] **Step 1: Add stdin detection and reading in `main()`**

Add `use std::io::{IsTerminal, Read};` to the imports and add the stdin-reading block in `main()` before the headless opts are built:

The full updated `main()` function:

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

    // Read stdin before enabling raw mode. We only do this when no FILE was
    // given and stdin is not an interactive terminal (i.e. the user is piping
    // data in). Crossterm 0.29 opens /dev/tty for keyboard events when stdin
    // is redirected, so the TUI still receives input afterwards.
    let stdin_data: Option<Vec<u8>> = if cli.file.is_none() && !io::stdin().is_terminal() {
        let mut buf = Vec::new();
        if let Err(e) = io::stdin().lock().read_to_end(&mut buf) {
            eprintln!("error: failed to read stdin: {e}");
            return ExitCode::FAILURE;
        }
        Some(buf)
    } else {
        None
    };

    let has_headless_ops =
        !cli.read.is_empty() || !cli.eval.is_empty() || !cli.write.is_empty();

    if has_headless_ops {
        if cli.file.is_none() && stdin_data.is_none() {
            eprintln!("error: a FILE argument is required for --read/--eval/--write");
            return ExitCode::from(2);
        }
        let opts = headless::Options {
            file: cli.file.clone().unwrap_or_default(),
            stdin_data,
            reads: cli.read,
            evals: cli.eval,
            writes: cli
                .write
                .chunks_exact(2)
                .map(|c| (c[0].clone(), c[1].clone()))
                .collect(),
            delimiter: explicit_delimiter,
        };
        let mut stdout = io::stdout().lock();
        return match headless::run(&opts, &mut stdout) {
            Ok(()) => ExitCode::SUCCESS,
            Err(msg) => {
                eprintln!("error: {msg}");
                ExitCode::FAILURE
            }
        };
    }

    match run_tui(cli.file.as_deref(), explicit_delimiter, stdin_data) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
```

- [ ] **Step 2: Update `run_tui` to accept and load stdin data**

Replace the existing `run_tui` and add a `load_stdin_data` helper:

```rust
fn run_tui(
    file: Option<&std::path::Path>,
    explicit_delimiter: Option<u8>,
    stdin_data: Option<Vec<u8>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new();

    if let Some(path) = file {
        load_file(&mut app, path, explicit_delimiter)?;
    } else if let Some(data) = stdin_data {
        load_stdin_data(&mut app, data, explicit_delimiter)?;
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

fn load_stdin_data(
    app: &mut App,
    data: Vec<u8>,
    explicit_delimiter: Option<u8>,
) -> Result<(), Box<dyn std::error::Error>> {
    use cell_sheet_core::formula::deps::{recalculate, set_formula};

    let delimiter = explicit_delimiter
        .unwrap_or_else(|| cell_sheet_core::io::csv::sniff_delimiter(&data));
    app.sheet = cell_sheet_core::io::csv::read_csv(data.as_slice(), delimiter)?;
    app.file_format = FileFormat::Csv;
    app.delimiter = delimiter;
    // file_path stays None — unnamed buffer, :w <path> still works

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

- [ ] **Step 3: Verify it compiles and all existing tests still pass**

```
cargo fmt --all && cargo clippy --workspace --all-targets --all-features && cargo test
```

Expected: clean compile, no warnings, all tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/cell-sheet-tui/src/main.rs
git commit -m "feat: read stdin when no FILE given and stdin is not a TTY"
```

---

## Task 3: Integration tests for headless stdin

**Files:**
- Modify: `crates/cell-sheet-tui/tests/headless.rs`

- [ ] **Step 1: Add `run_with_stdin` helper and stdin integration tests**

Add to the test file (after the existing helpers):

```rust
fn run_with_stdin(args: &[&str], stdin_data: &[u8]) -> Output {
    use std::process::Stdio;
    let mut child = Command::new(BIN)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn cell binary");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(stdin_data)
        .unwrap();
    drop(child.stdin.take());
    child.wait_with_output().expect("failed to wait on cell")
}

#[test]
fn stdin_read_single_cell() {
    let out = run_with_stdin(&["--read", "A1"], b"10,20\n30,40\n");
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(stdout(&out), "10\n");
}

#[test]
fn stdin_read_range() {
    let out = run_with_stdin(&["--read", "A1:B2"], b"10,20\n30,40\n");
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(stdout(&out), "10\t20\n30\t40\n");
}

#[test]
fn stdin_eval_sum() {
    let out = run_with_stdin(&["--eval", "=SUM(A1:A4)"], b"1\n2\n3\n4\n");
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(stdout(&out), "10\n");
}

#[test]
fn stdin_tsv_auto_detected() {
    let out = run_with_stdin(&["--read", "B1"], b"hello\tworld\n");
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(stdout(&out), "world\n");
}

#[test]
fn stdin_with_explicit_delimiter() {
    let out = run_with_stdin(&["--delimiter", "|", "--read", "B1"], b"a|b|c\n");
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(stdout(&out), "b\n");
}

#[test]
fn stdin_write_errors_without_file() {
    let out = run_with_stdin(&["--write", "A1", "99"], b"10,20\n");
    assert!(!out.status.success());
    assert!(!stderr(&out).is_empty());
}

#[test]
fn no_file_no_stdin_headless_errors() {
    // When stdin IS a TTY (the test runner's stdin), no piped data, no file: should error.
    // We simulate by just not passing stdin_data (run(), not run_with_stdin()).
    let out = run(&["--read", "A1"]);
    assert!(!out.status.success());
    assert!(!stderr(&out).is_empty());
}
```

- [ ] **Step 2: Run the integration tests**

```
cargo test -p cell-sheet-tui --test headless
```

Expected: all tests pass including the new stdin_* tests.

- [ ] **Step 3: Commit**

```bash
git add crates/cell-sheet-tui/tests/headless.rs
git commit -m "test: integration tests for headless stdin pipe input"
```

---

## Task 4: Update CHANGELOG

**Files:**
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Add entry under `## Unreleased` → `### Added`**

Insert at the top of the `### Added` list in the `## Unreleased` section:

```markdown
- Pipe support: `cell` now reads CSV/TSV from stdin when no file argument is
  given and stdin is not a terminal. Both interactive mode (`cat data.csv | cell`)
  and headless mode (`cat data.csv | cell --read A1`) are supported. Delimiter
  is auto-detected from the piped content; `--delimiter` overrides it. Using
  `--write` without a file argument when reading from stdin is an error
  ([#47](https://github.com/garritfra/cell/pull/PRNUM))
```

(Replace `PRNUM` with the actual PR number after opening the PR.)

- [ ] **Step 2: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs: add stdin pipe support to changelog"
```

---

## Task 5: Final verification

- [ ] **Step 1: Run full CI check**

```
cargo fmt --all --check && cargo clippy --workspace --all-targets --all-features && cargo test
```

Expected: all pass, no warnings.

- [ ] **Step 2: Smoke test with real binary**

Build and manually verify the pipe works end-to-end:

```bash
cargo build --release
echo "name,age\nAlice,30\nBob,25" | ./target/release/cell --read A2
# Expected output: Alice
echo "1\n2\n3\n4" | ./target/release/cell --eval "=SUM(A1:A4)"
# Expected output: 10
```

Expected: correct values printed.
