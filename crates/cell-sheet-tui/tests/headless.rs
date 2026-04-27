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
