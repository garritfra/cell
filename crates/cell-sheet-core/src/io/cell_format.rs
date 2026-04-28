use crate::model::{col_index_to_label, col_label_to_index, CellValue, Sheet};
use std::io::{BufRead, BufReader, Read, Write};

fn parse_address(addr: &str) -> Option<(usize, usize)> {
    let mut col_end = 0;
    for (i, c) in addr.chars().enumerate() {
        if c.is_ascii_uppercase() {
            col_end = i + 1;
        } else {
            break;
        }
    }
    if col_end == 0 || col_end >= addr.len() {
        return None;
    }
    let col_label = &addr[..col_end];
    let row_str = &addr[col_end..];
    let col = col_label_to_index(col_label)?;
    let row: usize = row_str.parse().ok()?;
    Some((row, col))
}

fn format_address(row: usize, col: usize) -> String {
    format!("{}{}", col_index_to_label(col), row)
}

pub fn write_cell_format<W: Write>(
    sheet: &Sheet,
    mut writer: W,
) -> Result<(), Box<dyn std::error::Error>> {
    writeln!(writer, "# cell v1")?;
    writeln!(writer)?;
    writeln!(writer, "size {} {}", sheet.row_count, sheet.col_count)?;
    writeln!(writer)?;

    for (i, &width) in sheet.col_widths.iter().enumerate() {
        writeln!(writer, "col-width {} {}", i, width)?;
    }
    if !sheet.col_widths.is_empty() {
        writeln!(writer)?;
    }

    let mut positions: Vec<_> = sheet.cells.keys().cloned().collect();
    positions.sort();

    for pos in positions {
        let cell = &sheet.cells[&pos];
        let addr = format_address(pos.0, pos.1);

        if cell.raw.starts_with('=') {
            writeln!(writer, "formula {} = {}", addr, cell.raw)?;
        } else {
            match &cell.value {
                CellValue::Number(_) => {
                    writeln!(writer, "let {} = {}", addr, cell.raw)?;
                }
                CellValue::Text(s) => {
                    writeln!(writer, "label {} = \"{}\"", addr, s)?;
                }
                CellValue::Bool(b) => {
                    writeln!(
                        writer,
                        "let {} = {}",
                        addr,
                        if *b { "TRUE" } else { "FALSE" }
                    )?;
                }
                CellValue::Empty => {}
                CellValue::Error(_) => {
                    writeln!(writer, "label {} = \"{}\"", addr, cell.raw)?;
                }
            }
        }
    }

    Ok(())
}

pub fn read_cell_format<R: Read>(reader: R) -> Result<Sheet, Box<dyn std::error::Error>> {
    let mut sheet = Sheet::new();
    let buf = BufReader::new(reader);

    for line in buf.lines() {
        let line = line?;
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(rest) = line.strip_prefix("size ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() != 2 {
                return Err(format!(
                    "corrupted .cell file: `size` directive expects 2 fields, got {}: {:?}",
                    parts.len(),
                    rest
                )
                .into());
            }
            // Returning the parser error instead of silently
            // collapsing to a 0×0 sheet (issue #76 — the previous
            // `unwrap_or(0)` made a corrupted file look like an
            // empty document).
            sheet.row_count = parts[0].parse().map_err(|e| {
                format!(
                    "corrupted .cell file: `size` row_count is not numeric ({:?}): {e}",
                    parts[0]
                )
            })?;
            sheet.col_count = parts[1].parse().map_err(|e| {
                format!(
                    "corrupted .cell file: `size` col_count is not numeric ({:?}): {e}",
                    parts[1]
                )
            })?;
        } else if let Some(rest) = line.strip_prefix("col-width ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() != 2 {
                return Err(format!(
                    "corrupted .cell file: `col-width` expects 2 fields, got {}: {:?}",
                    parts.len(),
                    rest
                )
                .into());
            }
            let idx: usize = parts[0].parse().map_err(|e| {
                format!(
                    "corrupted .cell file: `col-width` index is not numeric ({:?}): {e}",
                    parts[0]
                )
            })?;
            let width: u16 = parts[1].parse().map_err(|e| {
                format!(
                    "corrupted .cell file: `col-width` width is not numeric ({:?}): {e}",
                    parts[1]
                )
            })?;
            if idx >= sheet.col_widths.len() {
                sheet.col_widths.resize(idx + 1, 10);
            }
            sheet.col_widths[idx] = width;
        } else if let Some(rest) = line.strip_prefix("let ") {
            if let Some((addr_str, value_str)) = rest.split_once(" = ") {
                if let Some(pos) = parse_address(addr_str.trim()) {
                    sheet.set_cell(pos, value_str.trim());
                }
            }
        } else if let Some(rest) = line.strip_prefix("label ") {
            if let Some((addr_str, value_str)) = rest.split_once(" = ") {
                if let Some(pos) = parse_address(addr_str.trim()) {
                    let s = value_str.trim();
                    let s = if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
                        &s[1..s.len() - 1]
                    } else {
                        s
                    };
                    sheet.set_cell(pos, s);
                }
            }
        } else if let Some(rest) = line.strip_prefix("formula ") {
            if let Some((addr_str, formula_str)) = rest.split_once(" = ") {
                if let Some(pos) = parse_address(addr_str.trim()) {
                    sheet.set_cell(pos, formula_str.trim());
                }
            }
        }
    }

    Ok(sheet)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::CellValue;

    #[test]
    fn write_and_read_roundtrip() {
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), "42");
        sheet.set_cell((0, 1), "hello");
        sheet.set_cell((1, 0), "=A1+1");
        sheet.cells.get_mut(&(1, 0)).unwrap().value = CellValue::Number(43.0);
        sheet.col_widths = vec![12, 8];

        let mut buf = Vec::new();
        write_cell_format(&sheet, &mut buf).unwrap();
        let output = String::from_utf8(buf.clone()).unwrap();

        assert!(output.contains("size 2 2"));
        assert!(output.contains("let A0 = 42"));
        assert!(output.contains("label B0 = \"hello\""));
        assert!(output.contains("formula A1 = =A1+1"));
        assert!(output.contains("col-width 0 12"));

        let sheet2 = read_cell_format(buf.as_slice()).unwrap();
        assert_eq!(sheet2.row_count, 2);
        assert_eq!(sheet2.col_count, 2);
        assert_eq!(
            sheet2.get_cell((0, 0)).unwrap().value,
            CellValue::Number(42.0)
        );
        assert_eq!(
            sheet2.get_cell((0, 1)).unwrap().value,
            CellValue::Text("hello".into())
        );
        assert_eq!(sheet2.get_cell((1, 0)).unwrap().raw, "=A1+1");
        assert_eq!(sheet2.col_widths, vec![12, 8]);
    }

    #[test]
    fn read_comments_and_blanks() {
        let data = "# comment\n\nsize 1 1\nlet A0 = 5\n";
        let sheet = read_cell_format(data.as_bytes()).unwrap();
        assert_eq!(
            sheet.get_cell((0, 0)).unwrap().value,
            CellValue::Number(5.0)
        );
    }

    #[test]
    fn read_label_with_spaces() {
        let data = "size 1 1\nlabel A0 = \"hello world\"\n";
        let sheet = read_cell_format(data.as_bytes()).unwrap();
        assert_eq!(
            sheet.get_cell((0, 0)).unwrap().value,
            CellValue::Text("hello world".into())
        );
    }

    #[test]
    fn write_empty_sheet() {
        let sheet = Sheet::new();
        let mut buf = Vec::new();
        write_cell_format(&sheet, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("size 0 0"));
    }

    #[test]
    fn read_float_value() {
        let data = "size 1 1\nlet A0 = 3.15\n";
        let sheet = read_cell_format(data.as_bytes()).unwrap();
        assert_eq!(
            sheet.get_cell((0, 0)).unwrap().value,
            CellValue::Number(3.15)
        );
    }

    // Regression for https://github.com/garritfra/cell/issues/76. A
    // corrupted `.cell` file with a non-numeric `size` header used to
    // load as a 0×0 sheet — looked like an empty document. Now the
    // parser surfaces a clear "corrupted .cell file" error so the TUI
    // can show it in the status bar instead of silently truncating
    // the user's data.
    #[test]
    fn read_size_header_with_non_numeric_row_count_returns_error() {
        let data = "size NaN 5\n";
        let err = read_cell_format(data.as_bytes())
            .expect_err("non-numeric size header must produce an error, not a 0x0 sheet");
        let msg = format!("{err}");
        assert!(msg.contains("corrupted .cell file"), "got: {msg}");
        assert!(msg.contains("row_count"), "got: {msg}");
    }

    #[test]
    fn read_size_header_with_non_numeric_col_count_returns_error() {
        let data = "size 5 NaN\n";
        let err = read_cell_format(data.as_bytes())
            .expect_err("non-numeric col_count must produce an error");
        let msg = format!("{err}");
        assert!(msg.contains("corrupted .cell file"), "got: {msg}");
        assert!(msg.contains("col_count"), "got: {msg}");
    }

    #[test]
    fn read_size_header_with_wrong_arity_returns_error() {
        let data = "size 5\n";
        let err = read_cell_format(data.as_bytes())
            .expect_err("size header with one field must produce an error");
        assert!(format!("{err}").contains("expects 2 fields"));
    }

    #[test]
    fn read_col_width_with_non_numeric_index_returns_error() {
        let data = "size 1 1\ncol-width foo 12\n";
        let err =
            read_cell_format(data.as_bytes()).expect_err("non-numeric col-width index must error");
        assert!(format!("{err}").contains("col-width"));
    }
}
