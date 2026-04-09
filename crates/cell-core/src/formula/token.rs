use crate::model::CellError;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Number(f64),
    StringLit(String),
    Bool(bool),
    CellRef {
        col: String,
        row: String,
        abs_col: bool,
        abs_row: bool,
    },
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Gt,
    Gte,
    Lt,
    Lte,
    Eq,
    Neq,
    LParen,
    RParen,
    Comma,
    Colon,
}

pub fn tokenize(input: &str) -> Result<Vec<Token>, CellError> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            ' ' | '\t' => {
                i += 1;
            }
            '+' => {
                tokens.push(Token::Plus);
                i += 1;
            }
            '-' => {
                tokens.push(Token::Minus);
                i += 1;
            }
            '*' => {
                tokens.push(Token::Star);
                i += 1;
            }
            '/' => {
                tokens.push(Token::Slash);
                i += 1;
            }
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            ',' => {
                tokens.push(Token::Comma);
                i += 1;
            }
            ':' => {
                tokens.push(Token::Colon);
                i += 1;
            }
            '>' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token::Gte);
                    i += 2;
                } else {
                    tokens.push(Token::Gt);
                    i += 1;
                }
            }
            '<' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token::Lte);
                    i += 2;
                } else if i + 1 < chars.len() && chars[i + 1] == '>' {
                    tokens.push(Token::Neq);
                    i += 2;
                } else {
                    tokens.push(Token::Lt);
                    i += 1;
                }
            }
            '=' => {
                tokens.push(Token::Eq);
                i += 1;
            }
            '"' => {
                i += 1;
                let start = i;
                while i < chars.len() && chars[i] != '"' {
                    i += 1;
                }
                if i >= chars.len() {
                    return Err(CellError::Parse);
                }
                let s: String = chars[start..i].iter().collect();
                tokens.push(Token::StringLit(s));
                i += 1; // skip closing quote
            }
            c if c == '$' || c.is_ascii_uppercase() => {
                let mut abs_col = false;
                let mut j = i;

                if chars[j] == '$' {
                    abs_col = true;
                    j += 1;
                }

                let col_start = j;
                while j < chars.len() && chars[j].is_ascii_uppercase() {
                    j += 1;
                }
                let col: String = chars[col_start..j].iter().collect();

                if col.is_empty() {
                    return Err(CellError::Parse);
                }

                let mut abs_row = false;
                if j < chars.len() && chars[j] == '$' {
                    abs_row = true;
                    j += 1;
                }

                let row_start = j;
                while j < chars.len() && chars[j].is_ascii_digit() {
                    j += 1;
                }
                let row: String = chars[row_start..j].iter().collect();

                if !row.is_empty() {
                    tokens.push(Token::CellRef {
                        col,
                        row,
                        abs_col,
                        abs_row,
                    });
                } else if !abs_col && !abs_row {
                    if col == "TRUE" {
                        tokens.push(Token::Bool(true));
                    } else if col == "FALSE" {
                        tokens.push(Token::Bool(false));
                    } else {
                        tokens.push(Token::Ident(col));
                    }
                } else {
                    return Err(CellError::Parse);
                }
                i = j;
            }
            c if c.is_ascii_digit() => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let s: String = chars[start..i].iter().collect();
                let n: f64 = s.parse().map_err(|_| CellError::Parse)?;
                tokens.push(Token::Number(n));
            }
            _ => return Err(CellError::Parse),
        }
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_number() {
        let tokens = tokenize("42").unwrap();
        assert_eq!(tokens, vec![Token::Number(42.0)]);
    }

    #[test]
    fn tokenize_float() {
        let tokens = tokenize("3.15").unwrap();
        assert_eq!(tokens, vec![Token::Number(3.15)]);
    }

    #[test]
    fn tokenize_string() {
        let tokens = tokenize("\"hello\"").unwrap();
        assert_eq!(tokens, vec![Token::StringLit("hello".into())]);
    }

    #[test]
    fn tokenize_cell_ref() {
        let tokens = tokenize("A1").unwrap();
        assert_eq!(
            tokens,
            vec![Token::CellRef {
                col: "A".into(),
                row: "1".into(),
                abs_col: false,
                abs_row: false,
            }]
        );
    }

    #[test]
    fn tokenize_absolute_cell_ref() {
        let tokens = tokenize("$A$1").unwrap();
        assert_eq!(
            tokens,
            vec![Token::CellRef {
                col: "A".into(),
                row: "1".into(),
                abs_col: true,
                abs_row: true,
            }]
        );
    }

    #[test]
    fn tokenize_mixed_ref() {
        let tokens = tokenize("$A1").unwrap();
        assert_eq!(
            tokens,
            vec![Token::CellRef {
                col: "A".into(),
                row: "1".into(),
                abs_col: true,
                abs_row: false,
            }]
        );
    }

    #[test]
    fn tokenize_operators() {
        let tokens = tokenize("+-*/").unwrap();
        assert_eq!(
            tokens,
            vec![Token::Plus, Token::Minus, Token::Star, Token::Slash,]
        );
    }

    #[test]
    fn tokenize_comparison_operators() {
        let tokens = tokenize(">>=<<=<>").unwrap();
        assert_eq!(
            tokens,
            vec![Token::Gt, Token::Gte, Token::Lt, Token::Lte, Token::Neq,]
        );
    }

    #[test]
    fn tokenize_parens_and_comma() {
        let tokens = tokenize("(,)").unwrap();
        assert_eq!(tokens, vec![Token::LParen, Token::Comma, Token::RParen]);
    }

    #[test]
    fn tokenize_colon() {
        let tokens = tokenize(":").unwrap();
        assert_eq!(tokens, vec![Token::Colon]);
    }

    #[test]
    fn tokenize_function_name() {
        let tokens = tokenize("SUM(").unwrap();
        assert_eq!(tokens, vec![Token::Ident("SUM".into()), Token::LParen]);
    }

    #[test]
    fn tokenize_full_formula() {
        let tokens = tokenize("SUM(A1:A3)+1").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Ident("SUM".into()),
                Token::LParen,
                Token::CellRef {
                    col: "A".into(),
                    row: "1".into(),
                    abs_col: false,
                    abs_row: false
                },
                Token::Colon,
                Token::CellRef {
                    col: "A".into(),
                    row: "3".into(),
                    abs_col: false,
                    abs_row: false
                },
                Token::RParen,
                Token::Plus,
                Token::Number(1.0),
            ]
        );
    }

    #[test]
    fn tokenize_boolean_true() {
        let tokens = tokenize("TRUE").unwrap();
        assert_eq!(tokens, vec![Token::Bool(true)]);
    }

    #[test]
    fn tokenize_boolean_false() {
        let tokens = tokenize("FALSE").unwrap();
        assert_eq!(tokens, vec![Token::Bool(false)]);
    }

    #[test]
    fn tokenize_equals() {
        let tokens = tokenize("=").unwrap();
        assert_eq!(tokens, vec![Token::Eq]);
    }

    #[test]
    fn tokenize_whitespace_ignored() {
        let tokens = tokenize(" A1 + B1 ").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::CellRef {
                    col: "A".into(),
                    row: "1".into(),
                    abs_col: false,
                    abs_row: false
                },
                Token::Plus,
                Token::CellRef {
                    col: "B".into(),
                    row: "1".into(),
                    abs_col: false,
                    abs_row: false
                },
            ]
        );
    }

    #[test]
    fn tokenize_multi_letter_col() {
        let tokens = tokenize("AA10").unwrap();
        assert_eq!(
            tokens,
            vec![Token::CellRef {
                col: "AA".into(),
                row: "10".into(),
                abs_col: false,
                abs_row: false,
            }]
        );
    }
}
