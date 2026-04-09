use cell_core::formula::token::{tokenize, Token};
use cell_core::model::{col_index_to_label, col_label_to_index};

#[derive(Debug, Clone)]
pub enum Register {
    Cell(String),
    Row(Vec<String>),
    Block(Vec<Vec<String>>),
}

pub fn adjust_formula(raw: &str, row_delta: isize, col_delta: isize) -> String {
    if !raw.starts_with('=') {
        return raw.to_string();
    }
    let formula = &raw[1..];
    let tokens = match tokenize(formula) {
        Ok(t) => t,
        Err(_) => return raw.to_string(),
    };
    let mut result = String::from("=");
    for token in &tokens {
        match token {
            Token::CellRef {
                col,
                row,
                abs_col,
                abs_row,
            } => {
                let new_col = if *abs_col {
                    format!("${}", col)
                } else {
                    let col_idx = col_label_to_index(col).unwrap_or(0);
                    let new_idx = (col_idx as isize + col_delta).max(0) as usize;
                    col_index_to_label(new_idx)
                };
                let new_row = if *abs_row {
                    format!("${}", row)
                } else {
                    let row_num: isize = row.parse().unwrap_or(1);
                    let new_num = (row_num + row_delta).max(1);
                    format!("{}", new_num)
                };
                result.push_str(&format!("{}{}", new_col, new_row));
            }
            Token::Number(n) => {
                if n.fract() == 0.0 {
                    result.push_str(&format!("{}", *n as i64));
                } else {
                    result.push_str(&format!("{}", n));
                }
            }
            Token::StringLit(s) => result.push_str(&format!("\"{}\"", s)),
            Token::Bool(b) => result.push_str(if *b { "TRUE" } else { "FALSE" }),
            Token::Ident(name) => result.push_str(name),
            Token::Plus => result.push('+'),
            Token::Minus => result.push('-'),
            Token::Star => result.push('*'),
            Token::Slash => result.push('/'),
            Token::Gt => result.push('>'),
            Token::Gte => result.push_str(">="),
            Token::Lt => result.push('<'),
            Token::Lte => result.push_str("<="),
            Token::Eq => result.push('='),
            Token::Neq => result.push_str("<>"),
            Token::LParen => result.push('('),
            Token::RParen => result.push(')'),
            Token::Comma => result.push(','),
            Token::Colon => result.push(':'),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjust_formula_relative_refs() {
        assert_eq!(adjust_formula("=A1+B1", 2, 0), "=A3+B3");
    }

    #[test]
    fn adjust_formula_absolute_refs_unchanged() {
        assert_eq!(adjust_formula("=$A$1+$B$1", 2, 1), "=$A$1+$B$1");
    }

    #[test]
    fn adjust_formula_mixed_refs() {
        assert_eq!(adjust_formula("=$A1+A$1", 2, 1), "=$A3+B$1");
    }

    #[test]
    fn adjust_no_formula() {
        assert_eq!(adjust_formula("hello", 1, 1), "hello");
    }

    #[test]
    fn adjust_formula_col_shift() {
        assert_eq!(adjust_formula("=A1", 0, 2), "=C1");
    }
}
