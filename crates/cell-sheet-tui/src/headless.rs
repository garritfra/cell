//! Non-interactive CLI surface (issue #6).
//!
//! Lets users run `cell` headlessly to read/eval/write cells from shell
//! pipelines, Makefiles, and CI without launching the TUI. Operations apply in
//! a fixed order: writes → save → reads → evals. Results print to stdout, one
//! per `--read`/`--eval` flag, and ranges render as TSV (rows separated by
//! `\n`, columns by `\t`).

use std::io::Write;
use std::path::{Path, PathBuf};

use cell_sheet_core::engine::SheetEngine;
use cell_sheet_core::formula::ast::{CellRef, Expr};
use cell_sheet_core::formula::deps::DepGraph;
use cell_sheet_core::formula::{eval, parser};
use cell_sheet_core::io::{cell_format, csv as csv_io};
use cell_sheet_core::model::{CellPos, CellValue, Sheet};

use crate::file_format::FileFormat;

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

/// Run the requested headless operations and write their textual results to
/// `out`. Errors are returned with a human-readable message; callers are
/// responsible for emitting them to stderr and choosing an exit code.
pub fn run<W: Write>(opts: &Options, out: &mut W) -> Result<(), String> {
    let (mut sheet, mut deps, file_ctx) = if let Some(ref data) = opts.stdin_data {
        if !opts.writes.is_empty() {
            return Err(
                "cannot use --write when reading from stdin; provide a FILE argument instead"
                    .to_string(),
            );
        }
        let format = FileFormat::from_stdin_bytes(data);
        let delimiter = format
            .resolve_stdin_delimiter(data, opts.delimiter)
            .map_err(|msg| msg.to_string())?;
        let (sheet, deps) = load_from_bytes(data, format, delimiter)
            .map_err(|e| format!("failed to parse stdin: {e}"))?;
        (sheet, deps, None::<(FileFormat, u8)>)
    } else {
        let format = FileFormat::from_path(&opts.file);
        let delimiter = format
            .resolve_path_delimiter(&opts.file, opts.delimiter)
            .map_err(|e| format!("failed to read {}: {e}", opts.file.display()))?;
        let (sheet, deps) = load(&opts.file, format, delimiter)
            .map_err(|e| format!("failed to read {}: {e}", opts.file.display()))?;
        (sheet, deps, Some((format, delimiter)))
    };

    if !opts.writes.is_empty() {
        // Only reachable when opts.stdin_data is None (stdin + writes already errored above).
        let (format, delimiter) = file_ctx
            .ok_or_else(|| "internal error: --write reached without a file path".to_string())?;
        for (idx, (ref_str, value)) in opts.writes.iter().enumerate() {
            let pos = parse_single_ref(ref_str).ok_or_else(|| {
                format!(
                    "invalid cell reference for --write #{}: {ref_str:?}",
                    idx + 1
                )
            })?;
            apply_write(&mut sheet, &mut deps, pos, value);
        }
        SheetEngine::new(&mut sheet, &mut deps).recalculate();
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

fn load(
    path: &Path,
    format: FileFormat,
    delimiter: u8,
) -> Result<(Sheet, DepGraph), Box<dyn std::error::Error>> {
    let mut sheet = match format {
        FileFormat::Csv | FileFormat::Tsv => {
            let file = std::fs::File::open(path)?;
            csv_io::read_csv(file, delimiter)?
        }
        FileFormat::Cell => {
            let file = std::fs::File::open(path)?;
            cell_format::read_cell_format(file)?
        }
    };
    let mut deps = DepGraph::new();

    SheetEngine::new(&mut sheet, &mut deps).rebuild_formulas_and_recalculate();

    Ok((sheet, deps))
}

fn load_from_bytes(
    data: &[u8],
    format: FileFormat,
    delimiter: u8,
) -> Result<(Sheet, DepGraph), Box<dyn std::error::Error>> {
    let mut sheet = match format {
        FileFormat::Cell => cell_format::read_cell_format(data)?,
        FileFormat::Csv | FileFormat::Tsv => csv_io::read_csv(data, delimiter)?,
    };
    let mut deps = DepGraph::new();

    SheetEngine::new(&mut sheet, &mut deps).rebuild_formulas_and_recalculate();

    Ok((sheet, deps))
}

fn save(
    path: &Path,
    format: FileFormat,
    sheet: &Sheet,
    delimiter: u8,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = std::fs::File::create(path)?;
    match format {
        FileFormat::Csv | FileFormat::Tsv => csv_io::write_csv(sheet, file, delimiter)?,
        FileFormat::Cell => cell_format::write_cell_format(sheet, file)?,
    }
    Ok(())
}

fn apply_write(sheet: &mut Sheet, deps: &mut DepGraph, pos: CellPos, value: &str) {
    SheetEngine::new(sheet, deps).set_cell_raw(pos, value);
}

/// Parse a single A1-style reference like "A1" or "AB12". 1-indexed rows.
/// Internally reuses the formula parser so behaviour stays in sync with
/// formulas typed inside the TUI.
fn parse_single_ref(s: &str) -> Option<CellPos> {
    let trimmed = s.trim();
    let expr = parser::parse(trimmed).ok()?;
    match expr {
        Expr::CellRef(CellRef { row, col, .. }) => Some((row, col)),
        _ => None,
    }
}

fn parse_range_ref(s: &str) -> Option<(CellPos, CellPos)> {
    let trimmed = s.trim();
    let expr = parser::parse(trimmed).ok()?;
    match expr {
        Expr::CellRef(CellRef { row, col, .. }) => Some(((row, col), (row, col))),
        Expr::Range { start, end } => {
            let r1 = start.row.min(end.row);
            let r2 = start.row.max(end.row);
            let c1 = start.col.min(end.col);
            let c2 = start.col.max(end.col);
            Some(((r1, c1), (r2, c2)))
        }
        _ => None,
    }
}

fn render_read(sheet: &Sheet, ref_str: &str) -> Result<String, String> {
    let (start, end) = parse_range_ref(ref_str)
        .ok_or_else(|| format!("invalid cell reference for --read: {ref_str:?}"))?;

    let mut rows = Vec::with_capacity(end.0 - start.0 + 1);
    for r in start.0..=end.0 {
        let mut cols = Vec::with_capacity(end.1 - start.1 + 1);
        for c in start.1..=end.1 {
            let value = match sheet.get_cell((r, c)) {
                Some(cell) => cell.value.to_string(),
                None => String::new(),
            };
            cols.push(value);
        }
        rows.push(cols.join("\t"));
    }
    Ok(rows.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_ref_basic() {
        assert_eq!(parse_single_ref("A1"), Some((0, 0)));
        assert_eq!(parse_single_ref("B3"), Some((2, 1)));
        assert_eq!(parse_single_ref("AA10"), Some((9, 26)));
    }

    #[test]
    fn parse_single_ref_rejects_garbage() {
        assert_eq!(parse_single_ref("not-a-ref"), None);
        assert_eq!(parse_single_ref(""), None);
        assert_eq!(parse_single_ref("A1:B2"), None);
    }

    #[test]
    fn parse_range_ref_normalises_corners() {
        assert_eq!(
            parse_range_ref("B3:A1"),
            Some(((0, 0), (2, 1))),
            "range corners should be normalised so iteration is forward"
        );
    }

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
    fn run_stdin_empty_produces_empty_sheet() {
        let opts = Options {
            file: PathBuf::new(),
            stdin_data: Some(vec![]),
            reads: vec!["A1".to_string()],
            evals: vec![],
            writes: vec![],
            delimiter: None,
        };
        let mut out = Vec::new();
        run(&opts, &mut out).unwrap();
        // Empty stdin yields an empty sheet; A1 is blank → empty string
        assert_eq!(String::from_utf8(out).unwrap(), "\n");
    }

    #[test]
    fn run_stdin_respects_explicit_delimiter() {
        let opts = Options {
            file: PathBuf::new(),
            stdin_data: Some(b"a|b|c\n".to_vec()),
            reads: vec!["B1".to_string()],
            evals: vec![],
            writes: vec![],
            delimiter: Some(b'|'),
        };
        let mut out = Vec::new();
        run(&opts, &mut out).unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), "b\n");
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
}
