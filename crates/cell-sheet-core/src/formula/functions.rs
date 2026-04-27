use crate::model::{CellError, CellValue};

fn collect_numbers(values: &[CellValue]) -> Result<Vec<f64>, CellError> {
    let mut nums = Vec::new();
    for v in values {
        match v {
            CellValue::Number(n) => nums.push(*n),
            CellValue::Error(e) => return Err(e.clone()),
            CellValue::Empty => {}
            CellValue::Text(_) => {}
            CellValue::Bool(_) => {}
        }
    }
    Ok(nums)
}

/// Neumaier's improved Kahan compensated summation.
///
/// Unlike naive accumulation, it captures the rounding error lost when a
/// large and a small value are added, so it handles catastrophic cancellation
/// (e.g. `[1e16, 1.0, -1e16]`) and long sequences of small floats correctly.
fn neumaier_sum(nums: &[f64]) -> f64 {
    let mut sum = 0.0f64;
    let mut c = 0.0f64;
    for &x in nums {
        let t = sum + x;
        c += if sum.abs() >= x.abs() {
            (sum - t) + x
        } else {
            (x - t) + sum
        };
        sum = t;
    }
    sum + c
}

pub fn fn_sum(values: &[CellValue]) -> CellValue {
    match collect_numbers(values) {
        Ok(nums) => CellValue::Number(neumaier_sum(&nums)),
        Err(e) => CellValue::Error(e),
    }
}

pub fn fn_average(values: &[CellValue]) -> CellValue {
    match collect_numbers(values) {
        Ok(nums) if nums.is_empty() => CellValue::Error(CellError::DivZero),
        // Reuse the compensated sum so the mean inherits its accuracy;
        // a naive sum/n approach has the same catastrophic-cancellation
        // problem when individual values are very large.
        Ok(nums) => CellValue::Number(neumaier_sum(&nums) / nums.len() as f64),
        Err(e) => CellValue::Error(e),
    }
}

pub fn fn_count(values: &[CellValue]) -> CellValue {
    match collect_numbers(values) {
        Ok(nums) => CellValue::Number(nums.len() as f64),
        Err(e) => CellValue::Error(e),
    }
}

pub fn fn_min(values: &[CellValue]) -> CellValue {
    match collect_numbers(values) {
        Ok(nums) if nums.is_empty() => CellValue::Number(0.0),
        Ok(nums) => CellValue::Number(nums.iter().cloned().fold(f64::INFINITY, f64::min)),
        Err(e) => CellValue::Error(e),
    }
}

pub fn fn_max(values: &[CellValue]) -> CellValue {
    match collect_numbers(values) {
        Ok(nums) if nums.is_empty() => CellValue::Number(0.0),
        Ok(nums) => CellValue::Number(nums.iter().cloned().fold(f64::NEG_INFINITY, f64::max)),
        Err(e) => CellValue::Error(e),
    }
}

pub fn fn_if(args: &[CellValue]) -> CellValue {
    if args.len() != 3 {
        return CellValue::Error(CellError::Value);
    }
    match &args[0] {
        CellValue::Bool(true) => args[1].clone(),
        CellValue::Bool(false) => args[2].clone(),
        CellValue::Number(n) => {
            if *n != 0.0 {
                args[1].clone()
            } else {
                args[2].clone()
            }
        }
        CellValue::Error(e) => CellValue::Error(e.clone()),
        _ => CellValue::Error(CellError::Value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn num(n: f64) -> CellValue {
        CellValue::Number(n)
    }

    fn extract_number(v: CellValue) -> f64 {
        match v {
            CellValue::Number(n) => n,
            other => panic!("expected Number, got {:?}", other),
        }
    }

    // SUM: catastrophic cancellation — naive accumulation loses 1.0 entirely
    // because 1e16 + 1.0 rounds to 1e16 in f64.
    #[test]
    fn sum_catastrophic_cancellation_is_accurate() {
        let values = vec![num(1e16), num(1.0), num(-1e16)];
        assert_eq!(fn_sum(&values), CellValue::Number(1.0));
    }

    // SUM: long sequence — 10_000 × 0.1; the tight tolerance (1e-10)
    // catches naive per-step drift that Neumaier summation avoids.
    #[test]
    fn sum_long_sequence_of_small_floats_is_accurate() {
        let values: Vec<CellValue> = std::iter::repeat_n(num(0.1), 10_000).collect();
        let result = extract_number(fn_sum(&values));
        assert!(
            (result - 1000.0).abs() < 1e-10,
            "SUM error too large: |{result} - 1000| = {}",
            (result - 1000.0).abs()
        );
    }

    // AVERAGE: catastrophic cancellation — naive sum = 0.0, so naive mean = 0.0
    // instead of the correct 1/3.
    #[test]
    fn average_catastrophic_cancellation_is_accurate() {
        let values = vec![num(1e16), num(1.0), num(-1e16)];
        let result = extract_number(fn_average(&values));
        let expected = 1.0 / 3.0;
        assert!(
            (result - expected).abs() < 1e-10,
            "AVERAGE catastrophic error: |{result} - {expected}| = {}",
            (result - expected).abs()
        );
    }

    // AVERAGE: long sequence — compensated sum/n stays accurate without
    // accumulating intermediate drift.
    #[test]
    fn average_long_sequence_of_small_floats_is_accurate() {
        let values: Vec<CellValue> = std::iter::repeat_n(num(0.1), 10_000).collect();
        let result = extract_number(fn_average(&values));
        assert!(
            (result - 0.1).abs() < 1e-14,
            "AVERAGE error too large: |{result} - 0.1| = {}",
            (result - 0.1).abs()
        );
    }
}
