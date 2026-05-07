use crate::formula::ast::*;
use crate::formula::functions;
use crate::formula::parser;
use crate::model::{CellError, CellPos, CellValue, Sheet};

/// Maximum number of cells a single range expression may expand to. Ranges
/// larger than this (e.g. `A1:XFD1048576`) are rejected as `#VALUE!` to
/// prevent the evaluator from allocating gigabytes of `CellPos` and OOM-killing
/// the editor.
const MAX_RANGE_CELLS: usize = 1_000_000;

fn expand_range(start: &CellRef, end: &CellRef) -> Result<Vec<CellPos>, CellError> {
    let r1 = start.row.min(end.row);
    let r2 = start.row.max(end.row);
    let c1 = start.col.min(end.col);
    let c2 = start.col.max(end.col);
    let rows = r2.saturating_sub(r1).saturating_add(1);
    let cols = c2.saturating_sub(c1).saturating_add(1);
    let total = rows.checked_mul(cols).ok_or(CellError::Value)?;
    if total > MAX_RANGE_CELLS {
        return Err(CellError::Value);
    }
    let mut positions = Vec::with_capacity(total);
    for r in r1..=r2 {
        for c in c1..=c2 {
            positions.push((r, c));
        }
    }
    Ok(positions)
}

fn resolve_cell(sheet: &Sheet, pos: CellPos) -> CellValue {
    match sheet.get_cell(pos) {
        Some(cell) => cell.value.clone(),
        None => CellValue::Empty,
    }
}

fn cell_value_to_number(v: &CellValue) -> Result<f64, CellError> {
    match v {
        CellValue::Number(n) => Ok(*n),
        CellValue::Empty => Ok(0.0),
        CellValue::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
        CellValue::Error(e) => Err(e.clone()),
        CellValue::Text(_) => Err(CellError::Value),
    }
}

fn eval_expr(expr: &Expr, sheet: &Sheet) -> CellValue {
    match expr {
        Expr::Number(n) => CellValue::Number(*n),
        Expr::Text(s) => CellValue::Text(s.clone()),
        Expr::Bool(b) => CellValue::Bool(*b),
        Expr::CellRef(cell_ref) => {
            let val = resolve_cell(sheet, (cell_ref.row, cell_ref.col));
            if val == CellValue::Empty {
                CellValue::Number(0.0)
            } else {
                val
            }
        }
        Expr::Range { .. } => CellValue::Error(CellError::Value),
        Expr::UnaryNeg(inner) => {
            let val = eval_expr(inner, sheet);
            match cell_value_to_number(&val) {
                Ok(n) => CellValue::Number(-n),
                Err(e) => CellValue::Error(e),
            }
        }
        Expr::BinaryOp { op, left, right } => {
            let lval = eval_expr(left, sheet);
            let rval = eval_expr(right, sheet);

            if let CellValue::Error(e) = &lval {
                return CellValue::Error(e.clone());
            }
            if let CellValue::Error(e) = &rval {
                return CellValue::Error(e.clone());
            }

            match op {
                Op::Add | Op::Sub | Op::Mul | Op::Div => {
                    let ln = match cell_value_to_number(&lval) {
                        Ok(n) => n,
                        Err(e) => return CellValue::Error(e),
                    };
                    let rn = match cell_value_to_number(&rval) {
                        Ok(n) => n,
                        Err(e) => return CellValue::Error(e),
                    };
                    match op {
                        Op::Add => CellValue::Number(ln + rn),
                        Op::Sub => CellValue::Number(ln - rn),
                        Op::Mul => CellValue::Number(ln * rn),
                        Op::Div => {
                            if rn == 0.0 {
                                CellValue::Error(CellError::DivZero)
                            } else {
                                CellValue::Number(ln / rn)
                            }
                        }
                        _ => unreachable!(),
                    }
                }
                Op::Eq | Op::Neq => {
                    // Typed equality: compare values within the same type.
                    // Mixed types are not equal (matches Excel/Google Sheets),
                    // rather than producing a #VALUE! error.
                    //
                    // Text comparison is case-insensitive (Unicode-aware via
                    // `to_lowercase`), matching Excel/Google Sheets `=` on text.
                    let eq = match (&lval, &rval) {
                        (CellValue::Number(a), CellValue::Number(b)) => {
                            (a - b).abs() < f64::EPSILON
                        }
                        (CellValue::Text(a), CellValue::Text(b)) => {
                            a.to_lowercase() == b.to_lowercase()
                        }
                        (CellValue::Bool(a), CellValue::Bool(b)) => a == b,
                        (CellValue::Empty, CellValue::Empty) => true,
                        // Mixed types are unequal rather than an error.
                        _ => false,
                    };
                    let result = match op {
                        Op::Eq => eq,
                        Op::Neq => !eq,
                        _ => unreachable!(),
                    };
                    CellValue::Bool(result)
                }
                Op::Gt | Op::Gte | Op::Lt | Op::Lte => {
                    // Ordering operators remain numeric-only for now.
                    let ln = match cell_value_to_number(&lval) {
                        Ok(n) => n,
                        Err(e) => return CellValue::Error(e),
                    };
                    let rn = match cell_value_to_number(&rval) {
                        Ok(n) => n,
                        Err(e) => return CellValue::Error(e),
                    };
                    let result = match op {
                        Op::Gt => ln > rn,
                        Op::Gte => ln >= rn,
                        Op::Lt => ln < rn,
                        Op::Lte => ln <= rn,
                        _ => unreachable!(),
                    };
                    CellValue::Bool(result)
                }
            }
        }
        Expr::FnCall { name, args } => {
            let upper = name.to_uppercase();

            if upper == "IF" {
                let evaled: Vec<CellValue> = args.iter().map(|a| eval_expr(a, sheet)).collect();
                return functions::fn_if(&evaled);
            }

            let mut values = Vec::new();
            for arg in args {
                match arg {
                    Expr::Range { start, end } => match expand_range(start, end) {
                        Ok(positions) => {
                            for pos in positions {
                                values.push(resolve_cell(sheet, pos));
                            }
                        }
                        Err(e) => return CellValue::Error(e),
                    },
                    other => {
                        values.push(eval_expr(other, sheet));
                    }
                }
            }

            match upper.as_str() {
                "SUM" => functions::fn_sum(&values),
                "AVERAGE" => functions::fn_average(&values),
                "COUNT" => functions::fn_count(&values),
                "MIN" => functions::fn_min(&values),
                "MAX" => functions::fn_max(&values),
                _ => CellValue::Error(CellError::Name),
            }
        }
    }
}

pub fn evaluate(formula: &str, sheet: &Sheet) -> CellValue {
    match parser::parse(formula) {
        Ok(expr) => eval_expr(&expr, sheet),
        Err(e) => CellValue::Error(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Sheet;

    fn eval_with_sheet(formula: &str, sheet: &Sheet) -> CellValue {
        evaluate(formula, sheet)
    }

    fn eval(formula: &str) -> CellValue {
        let sheet = Sheet::new();
        eval_with_sheet(formula, &sheet)
    }

    #[test]
    fn eval_number() {
        assert_eq!(eval("42"), CellValue::Number(42.0));
    }

    #[test]
    fn eval_addition() {
        assert_eq!(eval("1+2"), CellValue::Number(3.0));
    }

    #[test]
    fn eval_subtraction() {
        assert_eq!(eval("5-3"), CellValue::Number(2.0));
    }

    #[test]
    fn eval_multiplication() {
        assert_eq!(eval("3*4"), CellValue::Number(12.0));
    }

    #[test]
    fn eval_division() {
        assert_eq!(eval("10/4"), CellValue::Number(2.5));
    }

    #[test]
    fn eval_division_by_zero() {
        assert_eq!(eval("1/0"), CellValue::Error(CellError::DivZero));
    }

    #[test]
    fn eval_precedence() {
        assert_eq!(eval("1+2*3"), CellValue::Number(7.0));
    }

    #[test]
    fn eval_parentheses() {
        assert_eq!(eval("(1+2)*3"), CellValue::Number(9.0));
    }

    #[test]
    fn eval_negation() {
        assert_eq!(eval("-5"), CellValue::Number(-5.0));
    }

    #[test]
    fn eval_cell_ref() {
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), "10");
        assert_eq!(eval_with_sheet("A1", &sheet), CellValue::Number(10.0));
    }

    #[test]
    fn eval_cell_ref_empty() {
        let sheet = Sheet::new();
        assert_eq!(eval_with_sheet("A1", &sheet), CellValue::Number(0.0));
    }

    #[test]
    fn eval_comparison_gt() {
        assert_eq!(eval("3>2"), CellValue::Bool(true));
        assert_eq!(eval("2>3"), CellValue::Bool(false));
    }

    #[test]
    fn eval_comparison_eq() {
        assert_eq!(eval("3=3"), CellValue::Bool(true));
        assert_eq!(eval("3=4"), CellValue::Bool(false));
    }

    #[test]
    fn eval_string() {
        assert_eq!(eval("\"hello\""), CellValue::Text("hello".into()));
    }

    #[test]
    fn eval_string_add_error() {
        assert_eq!(eval("\"hello\"+1"), CellValue::Error(CellError::Value));
    }

    #[test]
    fn eval_bool() {
        assert_eq!(eval("TRUE"), CellValue::Bool(true));
    }

    #[test]
    fn eval_sum() {
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), "1");
        sheet.set_cell((1, 0), "2");
        sheet.set_cell((2, 0), "3");
        assert_eq!(
            eval_with_sheet("SUM(A1:A3)", &sheet),
            CellValue::Number(6.0)
        );
    }

    #[test]
    fn eval_average() {
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), "2");
        sheet.set_cell((1, 0), "4");
        assert_eq!(
            eval_with_sheet("AVERAGE(A1:A2)", &sheet),
            CellValue::Number(3.0)
        );
    }

    #[test]
    fn eval_count() {
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), "1");
        sheet.set_cell((1, 0), "hello");
        sheet.set_cell((2, 0), "3");
        assert_eq!(
            eval_with_sheet("COUNT(A1:A3)", &sheet),
            CellValue::Number(2.0)
        );
    }

    #[test]
    fn eval_min() {
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), "5");
        sheet.set_cell((1, 0), "2");
        sheet.set_cell((2, 0), "8");
        assert_eq!(
            eval_with_sheet("MIN(A1:A3)", &sheet),
            CellValue::Number(2.0)
        );
    }

    #[test]
    fn eval_max() {
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), "5");
        sheet.set_cell((1, 0), "2");
        sheet.set_cell((2, 0), "8");
        assert_eq!(
            eval_with_sheet("MAX(A1:A3)", &sheet),
            CellValue::Number(8.0)
        );
    }

    #[test]
    fn eval_if_true() {
        assert_eq!(eval("IF(TRUE,1,2)"), CellValue::Number(1.0));
    }

    #[test]
    fn eval_if_false() {
        assert_eq!(eval("IF(FALSE,1,2)"), CellValue::Number(2.0));
    }

    #[test]
    fn eval_unknown_function() {
        assert_eq!(eval("FOO(1)"), CellValue::Error(CellError::Name));
    }

    #[test]
    fn eval_eq_text_text_equal() {
        assert_eq!(eval("\"foo\"=\"foo\""), CellValue::Bool(true));
    }

    #[test]
    fn eval_eq_text_text_not_equal() {
        assert_eq!(eval("\"foo\"=\"bar\""), CellValue::Bool(false));
    }

    #[test]
    fn eval_eq_text_case_insensitive() {
        // Match Excel/Google Sheets behavior for `=` on strings.
        assert_eq!(eval("\"foo\"=\"FOO\""), CellValue::Bool(true));
        assert_eq!(eval("\"Hello\"=\"hello\""), CellValue::Bool(true));
    }

    #[test]
    fn eval_neq_text_text() {
        assert_eq!(eval("\"foo\"<>\"bar\""), CellValue::Bool(true));
        assert_eq!(eval("\"foo\"<>\"foo\""), CellValue::Bool(false));
    }

    #[test]
    fn eval_eq_bool_bool() {
        assert_eq!(eval("TRUE=TRUE"), CellValue::Bool(true));
        assert_eq!(eval("TRUE=FALSE"), CellValue::Bool(false));
    }

    #[test]
    fn eval_neq_bool_bool() {
        assert_eq!(eval("TRUE<>FALSE"), CellValue::Bool(true));
        assert_eq!(eval("TRUE<>TRUE"), CellValue::Bool(false));
    }

    #[test]
    fn eval_eq_mixed_types_is_false() {
        // Number vs. Text — Excel returns FALSE rather than #VALUE!.
        assert_eq!(eval("\"1\"=1"), CellValue::Bool(false));
        assert_eq!(eval("1=\"1\""), CellValue::Bool(false));
    }

    #[test]
    fn eval_neq_mixed_types_is_true() {
        assert_eq!(eval("\"1\"<>1"), CellValue::Bool(true));
        assert_eq!(eval("1<>\"1\""), CellValue::Bool(true));
    }

    #[test]
    fn eval_if_with_text_cell_equality() {
        // Acceptance-criteria example from the issue: IF(A1=C3, A2, 0)
        // with both A1 and C3 containing text.
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), "hello"); // A1
        sheet.set_cell((1, 0), "42"); // A2 -> Number(42)
        sheet.set_cell((2, 2), "hello"); // C3
        assert_eq!(
            eval_with_sheet("IF(A1=C3,A2,0)", &sheet),
            CellValue::Number(42.0)
        );
    }

    #[test]
    fn eval_if_with_text_cell_inequality() {
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), "hello");
        sheet.set_cell((1, 0), "42");
        sheet.set_cell((2, 2), "world");
        assert_eq!(
            eval_with_sheet("IF(A1=C3,A2,0)", &sheet),
            CellValue::Number(0.0)
        );
    }

    #[test]
    fn eval_ordering_on_text_still_errors() {
        // Per the issue, ordering operators on strings remain numeric-only
        // (out of scope for this fix). Lock that in so we notice if it changes.
        assert_eq!(eval("\"a\"<\"b\""), CellValue::Error(CellError::Value));
        assert_eq!(eval("\"a\">\"b\""), CellValue::Error(CellError::Value));
    }

    #[test]
    fn eval_error_propagation() {
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), "=1/0");
        sheet.cells.get_mut(&(0, 0)).unwrap().value = CellValue::Error(CellError::DivZero);
        assert_eq!(
            eval_with_sheet("A1+1", &sheet),
            CellValue::Error(CellError::DivZero)
        );
    }

    #[test]
    fn eval_huge_range_returns_error_instead_of_oom() {
        let sheet = Sheet::new();
        assert_eq!(
            eval_with_sheet("SUM(A1:XFD1048576)", &sheet),
            CellValue::Error(CellError::Value)
        );
    }
}
