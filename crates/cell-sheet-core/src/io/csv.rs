use crate::model::Sheet;
use std::io::{Read, Write};

const MAX_COL_WIDTH: u16 = 40;
const DEFAULT_COL_WIDTH: u16 = 10;

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

pub fn read_csv<R: Read>(reader: R, delimiter: u8) -> Result<Sheet, Box<dyn std::error::Error>> {
    let mut sheet = Sheet::new();
    let mut csv_reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .delimiter(delimiter)
        .from_reader(reader);

    let mut max_col = 0usize;
    let mut col_content_widths: Vec<usize> = Vec::new();

    for (row_idx, result) in csv_reader.records().enumerate() {
        let record = result?;
        if record.len() > max_col {
            max_col = record.len();
            col_content_widths.resize(max_col, 0);
        }
        for (col_idx, field) in record.iter().enumerate() {
            if !field.is_empty() {
                sheet.set_cell((row_idx, col_idx), field);
                col_content_widths[col_idx] = col_content_widths[col_idx].max(field.len());
            }
        }
        sheet.row_count = row_idx + 1;
    }
    sheet.col_count = max_col;

    sheet.col_widths = col_content_widths
        .iter()
        .map(|&w| {
            let width = (w as u16).max(DEFAULT_COL_WIDTH);
            width.min(MAX_COL_WIDTH)
        })
        .collect();

    Ok(sheet)
}

pub fn write_csv<W: Write>(
    sheet: &Sheet,
    writer: W,
    delimiter: u8,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut csv_writer = csv::WriterBuilder::new()
        .delimiter(delimiter)
        .from_writer(writer);

    for row in 0..sheet.row_count {
        let mut record = Vec::new();
        for col in 0..sheet.col_count {
            let value = match sheet.get_cell((row, col)) {
                Some(cell) => cell.value.to_string(),
                None => String::new(),
            };
            record.push(value);
        }
        csv_writer.write_record(&record)?;
    }
    csv_writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::CellValue;

    #[test]
    fn read_csv_simple() {
        let data = "Name,Score\nAlice,95\nBob,88\n";
        let sheet = read_csv(data.as_bytes(), b',').unwrap();
        assert_eq!(sheet.row_count, 3);
        assert_eq!(sheet.col_count, 2);
        assert_eq!(
            sheet.get_cell((0, 0)).unwrap().value,
            CellValue::Text("Name".into())
        );
        assert_eq!(
            sheet.get_cell((1, 1)).unwrap().value,
            CellValue::Number(95.0)
        );
    }

    #[test]
    fn read_tsv() {
        let data = "A\tB\n1\t2\n";
        let sheet = read_csv(data.as_bytes(), b'\t').unwrap();
        assert_eq!(
            sheet.get_cell((1, 0)).unwrap().value,
            CellValue::Number(1.0)
        );
        assert_eq!(
            sheet.get_cell((1, 1)).unwrap().value,
            CellValue::Number(2.0)
        );
    }

    #[test]
    fn read_csv_empty_cells() {
        let data = "a,,b\n,,\n";
        let sheet = read_csv(data.as_bytes(), b',').unwrap();
        assert_eq!(
            sheet.get_cell((0, 0)).unwrap().value,
            CellValue::Text("a".into())
        );
        assert!(sheet.get_cell((0, 1)).is_none());
        assert_eq!(
            sheet.get_cell((0, 2)).unwrap().value,
            CellValue::Text("b".into())
        );
    }

    #[test]
    fn read_csv_quoted_fields() {
        let data = "\"hello, world\",42\n";
        let sheet = read_csv(data.as_bytes(), b',').unwrap();
        assert_eq!(
            sheet.get_cell((0, 0)).unwrap().value,
            CellValue::Text("hello, world".into())
        );
    }

    #[test]
    fn read_csv_formula_as_text() {
        let data = "=SUM(A1:A3)\n";
        let sheet = read_csv(data.as_bytes(), b',').unwrap();
        assert_eq!(sheet.get_cell((0, 0)).unwrap().raw, "=SUM(A1:A3)");
        assert_eq!(
            sheet.get_cell((0, 0)).unwrap().value,
            CellValue::Text("=SUM(A1:A3)".into())
        );
    }

    #[test]
    fn write_csv_simple() {
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), "Name");
        sheet.set_cell((0, 1), "Score");
        sheet.set_cell((1, 0), "Alice");
        sheet.set_cell((1, 1), "95");
        let mut buf = Vec::new();
        write_csv(&sheet, &mut buf, b',').unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output, "Name,Score\nAlice,95\n");
    }

    #[test]
    fn write_csv_flattens_formula_values() {
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), "=1+2");
        sheet.cells.get_mut(&(0, 0)).unwrap().value = CellValue::Number(3.0);
        let mut buf = Vec::new();
        write_csv(&sheet, &mut buf, b',').unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output, "3\n");
    }

    #[test]
    fn write_csv_empty_cells() {
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), "a");
        sheet.set_cell((0, 2), "b");
        sheet.row_count = 1;
        sheet.col_count = 3;
        let mut buf = Vec::new();
        write_csv(&sheet, &mut buf, b',').unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output, "a,,b\n");
    }

    #[test]
    fn write_csv_needs_quoting() {
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), "hello, world");
        let mut buf = Vec::new();
        write_csv(&sheet, &mut buf, b',').unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output, "\"hello, world\"\n");
    }

    #[test]
    fn col_widths_auto_sized() {
        let data = "Name,Score\nAlice,95\n";
        let sheet = read_csv(data.as_bytes(), b',').unwrap();
        assert!(sheet.col_widths[0] >= 5);
        assert!(sheet.col_widths[1] >= 5);
    }

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
    fn sniff_respects_4kib_cap_on_long_first_line() {
        // First line is 4097 bytes: 4096 pipes followed by one comma.
        // Only the first 4096 bytes are inspected, so pipe wins.
        let mut line = vec![b'|'; 4096];
        line.push(b',');
        line.push(b'\n');
        assert_eq!(sniff_delimiter(&line), b'|');
    }

    #[test]
    fn sniff_crlf_line_ending() {
        // Windows-style CRLF — \r is not a candidate, so it's harmless;
        // sniff should still correctly detect the pipe on the first line.
        let sample = b"a|b|c\r\n1,2,3\n";
        assert_eq!(sniff_delimiter(sample), b'|');
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
}
