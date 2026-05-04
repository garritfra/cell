//! Integration tests for the non-interactive (headless) CLI surface added in #6.
//!
//! These spawn the `cell` binary directly and assert on stdout/stderr/exit
//! status, so they exercise the same code path real users hit from the shell.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_cell");

fn write_csv(dir: &TempDir, name: &str, contents: &str) -> PathBuf {
    let path = dir.path().join(name);
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(contents.as_bytes()).unwrap();
    path
}

fn run(args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .output()
        .expect("failed to spawn cell binary")
}

fn stdout(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).unwrap()
}

fn stderr(out: &Output) -> String {
    String::from_utf8(out.stderr.clone()).unwrap()
}

fn cell_path(dir: &TempDir, name: &str) -> PathBuf {
    dir.path().join(name)
}

fn run_with_stdin(args: &[&str], stdin_data: &[u8]) -> Output {
    use std::io::Write as _;
    use std::process::Stdio;
    let mut child = Command::new(BIN)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn cell binary");
    child.stdin.as_mut().unwrap().write_all(stdin_data).unwrap();
    drop(child.stdin.take());
    child.wait_with_output().expect("failed to wait on cell")
}

#[test]
fn read_single_cell_prints_value() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_csv(&dir, "data.csv", "10,20\n30,40\n");

    let out = run(&[path.to_str().unwrap(), "--read", "A1"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(stdout(&out), "10\n");
}

#[test]
fn read_range_prints_tsv_grid() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_csv(&dir, "data.csv", "10,20\n30,40\n");

    let out = run(&[path.to_str().unwrap(), "--read", "A1:B2"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(stdout(&out), "10\t20\n30\t40\n");
}

#[test]
fn read_evaluates_formula_cells() {
    let dir = tempfile::tempdir().unwrap();
    // .cell format preserves formulas; reading A2 must return the computed value.
    let cell_file = "# cell v1\nsize 2 1\nlet A0 = 5\nformula A1 = =A1+3\n";
    let path = cell_path(&dir, "data.cell");
    std::fs::write(&path, cell_file).unwrap();

    let out = run(&[path.to_str().unwrap(), "--read", "A2"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(stdout(&out), "8\n");
}

#[test]
fn eval_runs_against_loaded_sheet() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_csv(&dir, "data.csv", "1\n2\n3\n4\n");

    let out = run(&[path.to_str().unwrap(), "--eval", "=SUM(A1:A4)"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(stdout(&out), "10\n");
}

#[test]
fn eval_accepts_expr_without_leading_equals() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_csv(&dir, "data.csv", "1\n2\n3\n");

    let out = run(&[path.to_str().unwrap(), "--eval", "AVERAGE(A1:A3)"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(stdout(&out), "2\n");
}

#[test]
fn eval_does_not_persist() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_csv(&dir, "data.csv", "1,2\n");
    let before = std::fs::read_to_string(&path).unwrap();

    let out = run(&[path.to_str().unwrap(), "--eval", "=A1+B1"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let after = std::fs::read_to_string(&path).unwrap();
    assert_eq!(before, after, "--eval must not modify the file");
}

#[test]
fn single_write_persists_value() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_csv(&dir, "data.csv", "1,2\n3,4\n");

    let out = run(&[path.to_str().unwrap(), "--write", "A1", "42"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));

    let after = std::fs::read_to_string(&path).unwrap();
    assert!(after.starts_with("42,2\n"), "got: {after:?}");
}

#[test]
fn write_then_read_sees_new_value() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_csv(&dir, "data.csv", "1\n");

    let out = run(&[
        path.to_str().unwrap(),
        "--write",
        "A1",
        "99",
        "--read",
        "A1",
    ]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(stdout(&out), "99\n");
}

#[test]
fn write_formula_recalculates_dependents() {
    let dir = tempfile::tempdir().unwrap();
    let cell_file = "# cell v1\nsize 2 1\nlet A0 = 1\nformula A1 = =A1*10\n";
    let path = cell_path(&dir, "data.cell");
    std::fs::write(&path, cell_file).unwrap();

    let out = run(&[path.to_str().unwrap(), "--write", "A1", "7", "--read", "A2"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(stdout(&out), "70\n");
}

#[test]
fn write_empty_value_recalculates_dependents() {
    let dir = tempfile::tempdir().unwrap();
    let cell_file = "# cell v1\nsize 2 1\nlet A0 = 99\nformula A1 = =A1\n";
    let path = cell_path(&dir, "data.cell");
    std::fs::write(&path, cell_file).unwrap();

    let out = run(&[path.to_str().unwrap(), "--write", "A1", "", "--read", "A2"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(stdout(&out), "0\n");
}

#[test]
fn write_empty_value_preserves_requested_extent() {
    let dir = tempfile::tempdir().unwrap();
    let path = cell_path(&dir, "data.cell");
    std::fs::write(&path, "# cell v1\nsize 1 1\n").unwrap();

    let out = run(&[path.to_str().unwrap(), "--write", "B2", ""]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));

    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("size 2 2"), "got: {saved:?}");
}

#[test]
fn batched_writes_apply_in_order_and_save_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_csv(&dir, "data.csv", ",,\n");

    let out = run(&[
        path.to_str().unwrap(),
        "--write",
        "A1",
        "10",
        "--write",
        "B1",
        "20",
        "--write",
        "C1",
        "30",
    ]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));

    let after = std::fs::read_to_string(&path).unwrap();
    assert!(after.starts_with("10,20,30\n"), "got: {after:?}");
}

#[test]
fn write_auto_detects_formula() {
    let dir = tempfile::tempdir().unwrap();
    let path = cell_path(&dir, "data.cell");
    std::fs::write(&path, "# cell v1\nsize 1 2\nlet A0 = 4\n").unwrap();

    let out = run(&[
        path.to_str().unwrap(),
        "--write",
        "B1",
        "=A1*5",
        "--read",
        "B1",
    ]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(stdout(&out), "20\n");

    // Round-trip: re-open with no flags should still see the stored formula.
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("formula B0 = =A1*5"), "got: {saved:?}");
}

#[test]
fn bad_ref_errors_to_stderr() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_csv(&dir, "data.csv", "1,2\n");

    let out = run(&[path.to_str().unwrap(), "--read", "not-a-ref"]);
    assert!(!out.status.success());
    assert!(
        !stderr(&out).is_empty(),
        "expected an error message on stderr"
    );
}

#[test]
fn eval_parse_error_exits_nonzero() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_csv(&dir, "data.csv", "1\n");

    let out = run(&[path.to_str().unwrap(), "--eval", "1++"]);
    assert!(!out.status.success(), "stdout: {}", stdout(&out));
    assert!(!stderr(&out).is_empty());
}

#[test]
fn missing_file_exits_nonzero() {
    let missing = Path::new("/definitely/not/a/real/path/cell-test-missing.csv");
    let out = run(&[missing.to_str().unwrap(), "--read", "A1"]);
    assert!(!out.status.success());
    assert!(!stderr(&out).is_empty());
}

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

#[test]
fn read_tsv_without_delimiter_flag() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_csv(&dir, "data.tsv", "a\tb\tc\n10\t20\t30\n");

    let out = run(&[path.to_str().unwrap(), "--read", "C2"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(stdout(&out), "30\n");
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
fn stdin_cell_format_read_single_cell() {
    // .cell format uses the row number directly in addresses (A0 == row 0),
    // while --read takes A1-style 1-indexed refs (A1 == row 0).
    let cell_blob = b"# cell v1\nsize 1 1\nlet A0 = 42\n";
    let out = run_with_stdin(&["--read", "A1"], cell_blob);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(stdout(&out), "42\n");
}

#[test]
fn stdin_cell_format_read_evaluates_formula() {
    // Cell-format address syntax (`let A0 ...`) is 0-indexed by row number,
    // but formula refs (`=A1+B1`) are 1-indexed — so A1/B1 inside the formula
    // resolve to row 0 cols 0/1, where the values 5 and 7 live. The formula
    // is stored at storage-address B1 (row 1, col 1), and --read B2 (1-indexed
    // row 1, col 1) should print the computed value, proving the dep graph
    // and recalc fired during stdin load.
    let cell_blob = b"# cell v1\nsize 2 2\nlet A0 = 5\nlet B0 = 7\nformula B1 = =A1+B1\n";
    let out = run_with_stdin(&["--read", "B2"], cell_blob);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(stdout(&out), "12\n");
}

#[test]
fn stdin_cell_format_eval_sum() {
    let cell_blob = b"# cell v1\nsize 4 1\nlet A0 = 1\nlet A1 = 2\nlet A2 = 3\nlet A3 = 4\n";
    let out = run_with_stdin(&["--eval", "=SUM(A1:A4)"], cell_blob);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(stdout(&out), "10\n");
}

#[test]
fn stdin_cell_format_rejects_explicit_delimiter() {
    // Pipe a .cell-format blob with --delimiter set: the flag is meaningless
    // for the native format, so we surface a clear error instead of silently
    // ignoring it.
    let cell_blob = b"# cell v1\nsize 1 1\nlet A0 = 1\n";
    let out = run_with_stdin(&["--delimiter", "|", "--read", "A1"], cell_blob);
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(
        err.contains("--delimiter"),
        "expected --delimiter mention in error: {err}"
    );
}
