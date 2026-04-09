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

pub fn fn_sum(values: &[CellValue]) -> CellValue {
    match collect_numbers(values) {
        Ok(nums) => CellValue::Number(nums.iter().sum()),
        Err(e) => CellValue::Error(e),
    }
}

pub fn fn_average(values: &[CellValue]) -> CellValue {
    match collect_numbers(values) {
        Ok(nums) if nums.is_empty() => CellValue::Error(CellError::DivZero),
        Ok(nums) => {
            let count = nums.len() as f64;
            CellValue::Number(nums.iter().sum::<f64>() / count)
        }
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
