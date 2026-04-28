//! Non-interactive CLI surface (issue #6).
//!
//! Lets users run `cell` headlessly to read/eval/write cells from shell
//! pipelines, Makefiles, and CI without launching the TUI. Operations apply in
//! a fixed order: writes → save → reads → evals. Results print to stdout, one
//! per `--read`/`--eval` flag, and ranges render as TSV (rows separated by
//! `\n`, columns by `\t`).

use std::io::Write;
use std::path::{Path, PathBuf};

use cell_sheet_core::formula::ast::{CellRef, Expr};
use cell_sheet_core::formula::deps::{mark_dirty, recalculate, set_formula, DepGraph};
use cell_sheet_core::formula::{eval, parser};
use cell_sheet_core::io::{cell_format, csv as csv_io};
use cell_sheet_core::model::{CellPos, CellValue, Sheet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Csv,
    Tsv,
    Cell,
}

impl Format {
    fn from_path(path: &Path) -> Self {
        match path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase()
            .as_str()
        {
            "tsv" => Format::Tsv,
            "cell" => Format::Cell,
            _ => Format::Csv,
        }
    }
}

#[derive(Debug)]
pub struct Options {
    pub file: PathBuf,
    pub reads: Vec<String>,
    pub evals: Vec<String>,
    pub writes: Vec<(String, String)>,
    pub delimiter: Option<u8>,
}

impl Options {
    pub fn is_active(&self) -> bool {
        !self.reads.is_empty() || !self.evals.is_empty() || !self.writes.is_empty()
    }
}

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
            use std::io::Read as _;
            let mut buf = vec![0u8; 4096];
            let mut file = std::fs::File::open(path)?;
            let n = file.read(&mut buf)?;
            Ok(csv_io::sniff_delimiter(&buf[..n]))
        }
    }
}

/// Run the requested headless operations and write their textual results to
/// `out`. Errors are returned with a human-readable message; callers are
/// responsible for emitting them to stderr and choosing an exit code.
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

fn apply_write(sheet: &mut Sheet, deps: &mut DepGraph, pos: CellPos, value: &str) {
    if value.starts_with('=') {
        set_formula(sheet, deps, pos, value);
    } else {
        // Drop any prior formula edges so dependents don't keep tracking this cell.
        deps.remove(pos);
        sheet.set_cell(pos, value);
    }
    mark_dirty(sheet, deps, pos);
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
}
