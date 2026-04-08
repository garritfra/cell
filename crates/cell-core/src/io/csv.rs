use std::io::{Read, Write};
use crate::model::{Sheet, CellValue};

const MAX_COL_WIDTH: u16 = 40;
const DEFAULT_COL_WIDTH: u16 = 10;

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

    sheet.col_widths = col_content_widths.iter()
        .map(|&w| {
            let width = (w as u16).max(DEFAULT_COL_WIDTH);
            width.min(MAX_COL_WIDTH)
        })
        .collect();

    Ok(sheet)
}

pub fn write_csv<W: Write>(sheet: &Sheet, writer: W, delimiter: u8) -> Result<(), Box<dyn std::error::Error>> {
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
        assert_eq!(sheet.get_cell((0, 0)).unwrap().value, CellValue::Text("Name".into()));
        assert_eq!(sheet.get_cell((1, 1)).unwrap().value, CellValue::Number(95.0));
    }

    #[test]
    fn read_tsv() {
        let data = "A\tB\n1\t2\n";
        let sheet = read_csv(data.as_bytes(), b'\t').unwrap();
        assert_eq!(sheet.get_cell((1, 0)).unwrap().value, CellValue::Number(1.0));
        assert_eq!(sheet.get_cell((1, 1)).unwrap().value, CellValue::Number(2.0));
    }

    #[test]
    fn read_csv_empty_cells() {
        let data = "a,,b\n,,\n";
        let sheet = read_csv(data.as_bytes(), b',').unwrap();
        assert_eq!(sheet.get_cell((0, 0)).unwrap().value, CellValue::Text("a".into()));
        assert!(sheet.get_cell((0, 1)).is_none());
        assert_eq!(sheet.get_cell((0, 2)).unwrap().value, CellValue::Text("b".into()));
    }

    #[test]
    fn read_csv_quoted_fields() {
        let data = "\"hello, world\",42\n";
        let sheet = read_csv(data.as_bytes(), b',').unwrap();
        assert_eq!(sheet.get_cell((0, 0)).unwrap().value, CellValue::Text("hello, world".into()));
    }

    #[test]
    fn read_csv_formula_as_text() {
        let data = "=SUM(A1:A3)\n";
        let sheet = read_csv(data.as_bytes(), b',').unwrap();
        assert_eq!(sheet.get_cell((0, 0)).unwrap().raw, "=SUM(A1:A3)");
        assert_eq!(sheet.get_cell((0, 0)).unwrap().value, CellValue::Text("=SUM(A1:A3)".into()));
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
}
