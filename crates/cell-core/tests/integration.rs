use cell_core::model::{Sheet, CellValue, col_index_to_label, col_label_to_index};
use cell_core::formula::deps::{DepGraph, set_formula, recalculate, mark_dirty};
use cell_core::io::csv::{read_csv, write_csv};
use cell_core::io::cell_format::{read_cell_format, write_cell_format};

#[test]
fn csv_roundtrip() {
    let input = "Name,Score,Grade\nAlice,95,A\nBob,88,B+\n";
    let sheet = read_csv(input.as_bytes(), b',').unwrap();
    let mut buf = Vec::new();
    write_csv(&sheet, &mut buf, b',').unwrap();
    let output = String::from_utf8(buf).unwrap();
    assert_eq!(output, input);
}

#[test]
fn cell_format_roundtrip_with_formulas() {
    let mut sheet = Sheet::new();
    let mut deps = DepGraph::new();
    sheet.set_cell((0, 0), "10");
    sheet.set_cell((0, 1), "20");
    set_formula(&mut sheet, &mut deps, (0, 2), "=A1+B1");
    recalculate(&mut sheet, &deps);
    assert_eq!(sheet.get_cell((0, 2)).unwrap().value, CellValue::Number(30.0));

    let mut buf = Vec::new();
    write_cell_format(&sheet, &mut buf).unwrap();
    let sheet2 = read_cell_format(buf.as_slice()).unwrap();
    assert_eq!(sheet2.get_cell((0, 0)).unwrap().value, CellValue::Number(10.0));
    assert_eq!(sheet2.get_cell((0, 1)).unwrap().value, CellValue::Number(20.0));
    assert_eq!(sheet2.get_cell((0, 2)).unwrap().raw, "=A1+B1");
}

#[test]
fn formula_chain_recalculation() {
    let mut sheet = Sheet::new();
    let mut deps = DepGraph::new();
    sheet.set_cell((0, 0), "5");
    sheet.set_cell((1, 0), "10");
    sheet.set_cell((2, 0), "15");
    set_formula(&mut sheet, &mut deps, (3, 0), "=SUM(A1:A3)");
    set_formula(&mut sheet, &mut deps, (4, 0), "=A4*2");
    recalculate(&mut sheet, &deps);
    assert_eq!(sheet.get_cell((3, 0)).unwrap().value, CellValue::Number(30.0));
    assert_eq!(sheet.get_cell((4, 0)).unwrap().value, CellValue::Number(60.0));

    sheet.set_cell((0, 0), "100");
    mark_dirty(&mut sheet, &deps, (0, 0));
    recalculate(&mut sheet, &deps);
    assert_eq!(sheet.get_cell((3, 0)).unwrap().value, CellValue::Number(125.0));
    assert_eq!(sheet.get_cell((4, 0)).unwrap().value, CellValue::Number(250.0));
}

#[test]
fn csv_export_flattens_formulas() {
    let mut sheet = Sheet::new();
    let mut deps = DepGraph::new();
    sheet.set_cell((0, 0), "10");
    sheet.set_cell((0, 1), "20");
    set_formula(&mut sheet, &mut deps, (0, 2), "=A1+B1");
    recalculate(&mut sheet, &deps);

    let mut buf = Vec::new();
    write_csv(&sheet, &mut buf, b',').unwrap();
    let output = String::from_utf8(buf).unwrap();
    assert_eq!(output, "10,20,30\n");
}

#[test]
fn col_label_conversion() {
    assert_eq!(col_index_to_label(0), "A");
    assert_eq!(col_index_to_label(25), "Z");
    assert_eq!(col_index_to_label(26), "AA");
    assert_eq!(col_index_to_label(701), "ZZ");
    assert_eq!(col_index_to_label(702), "AAA");

    for i in 0..1000 {
        let label = col_index_to_label(i);
        assert_eq!(col_label_to_index(&label).unwrap(), i, "Roundtrip failed for {}: {}", i, label);
    }
}
