use crate::model::CellError;
use crate::formula::token::{Token, tokenize};
use crate::formula::ast::*;

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Token> {
        let tok = self.tokens.get(self.pos);
        self.pos += 1;
        tok
    }

    fn expect(&mut self, expected: &Token) -> Result<(), CellError> {
        match self.advance() {
            Some(tok) if tok == expected => Ok(()),
            _ => Err(CellError::Parse),
        }
    }

    fn expression(&mut self) -> Result<Expr, CellError> {
        self.comparison()
    }

    fn comparison(&mut self) -> Result<Expr, CellError> {
        let mut left = self.addition()?;
        if let Some(op) = self.peek().and_then(|t| match t {
            Token::Gt => Some(Op::Gt),
            Token::Gte => Some(Op::Gte),
            Token::Lt => Some(Op::Lt),
            Token::Lte => Some(Op::Lte),
            Token::Eq => Some(Op::Eq),
            Token::Neq => Some(Op::Neq),
            _ => None,
        }) {
            self.advance();
            let right = self.addition()?;
            left = Expr::BinaryOp { op, left: Box::new(left), right: Box::new(right) };
        }
        Ok(left)
    }

    fn addition(&mut self) -> Result<Expr, CellError> {
        let mut left = self.multiplication()?;
        loop {
            let op = match self.peek() {
                Some(Token::Plus) => Op::Add,
                Some(Token::Minus) => Op::Sub,
                _ => break,
            };
            self.advance();
            let right = self.multiplication()?;
            left = Expr::BinaryOp { op, left: Box::new(left), right: Box::new(right) };
        }
        Ok(left)
    }

    fn multiplication(&mut self) -> Result<Expr, CellError> {
        let mut left = self.unary()?;
        loop {
            let op = match self.peek() {
                Some(Token::Star) => Op::Mul,
                Some(Token::Slash) => Op::Div,
                _ => break,
            };
            self.advance();
            let right = self.unary()?;
            left = Expr::BinaryOp { op, left: Box::new(left), right: Box::new(right) };
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Expr, CellError> {
        if let Some(Token::Minus) = self.peek() {
            self.advance();
            let expr = self.unary()?;
            return Ok(Expr::UnaryNeg(Box::new(expr)));
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Expr, CellError> {
        let tok = self.advance().ok_or(CellError::Parse)?.clone();
        match tok {
            Token::Number(n) => Ok(Expr::Number(n)),
            Token::StringLit(s) => Ok(Expr::Text(s)),
            Token::Bool(b) => Ok(Expr::Bool(b)),
            Token::CellRef { col, row, abs_col, abs_row } => {
                let cell_ref = CellRef::from_display(&col, &row, abs_col, abs_row)
                    .ok_or(CellError::Ref)?;

                if let Some(Token::Colon) = self.peek() {
                    self.advance();
                    let end_tok = self.advance().ok_or(CellError::Parse)?.clone();
                    if let Token::CellRef { col: col2, row: row2, abs_col: ac2, abs_row: ar2 } = end_tok {
                        let end_ref = CellRef::from_display(&col2, &row2, ac2, ar2)
                            .ok_or(CellError::Ref)?;
                        Ok(Expr::Range { start: cell_ref, end: end_ref })
                    } else {
                        Err(CellError::Parse)
                    }
                } else {
                    Ok(Expr::CellRef(cell_ref))
                }
            }
            Token::Ident(name) => {
                self.expect(&Token::LParen)?;
                let mut args = Vec::new();
                if self.peek() != Some(&Token::RParen) {
                    args.push(self.expression()?);
                    while let Some(Token::Comma) = self.peek() {
                        self.advance();
                        args.push(self.expression()?);
                    }
                }
                self.expect(&Token::RParen)?;
                Ok(Expr::FnCall { name, args })
            }
            Token::LParen => {
                let expr = self.expression()?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }
            _ => Err(CellError::Parse),
        }
    }
}

pub fn parse(input: &str) -> Result<Expr, CellError> {
    let tokens = tokenize(input)?;
    let mut parser = Parser::new(tokens);
    let expr = parser.expression()?;
    if parser.pos != parser.tokens.len() {
        return Err(CellError::Parse);
    }
    Ok(expr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_number() {
        let expr = parse("42").unwrap();
        assert_eq!(expr, Expr::Number(42.0));
    }

    #[test]
    fn parse_string() {
        let expr = parse("\"hello\"").unwrap();
        assert_eq!(expr, Expr::Text("hello".into()));
    }

    #[test]
    fn parse_cell_ref() {
        let expr = parse("A1").unwrap();
        assert_eq!(expr, Expr::CellRef(CellRef { col: 0, row: 0, abs_col: false, abs_row: false }));
    }

    #[test]
    fn parse_addition() {
        let expr = parse("1+2").unwrap();
        assert_eq!(expr, Expr::BinaryOp {
            op: Op::Add,
            left: Box::new(Expr::Number(1.0)),
            right: Box::new(Expr::Number(2.0)),
        });
    }

    #[test]
    fn parse_precedence_mul_before_add() {
        let expr = parse("1+2*3").unwrap();
        assert_eq!(expr, Expr::BinaryOp {
            op: Op::Add,
            left: Box::new(Expr::Number(1.0)),
            right: Box::new(Expr::BinaryOp {
                op: Op::Mul,
                left: Box::new(Expr::Number(2.0)),
                right: Box::new(Expr::Number(3.0)),
            }),
        });
    }

    #[test]
    fn parse_parentheses() {
        let expr = parse("(1+2)*3").unwrap();
        assert_eq!(expr, Expr::BinaryOp {
            op: Op::Mul,
            left: Box::new(Expr::BinaryOp {
                op: Op::Add,
                left: Box::new(Expr::Number(1.0)),
                right: Box::new(Expr::Number(2.0)),
            }),
            right: Box::new(Expr::Number(3.0)),
        });
    }

    #[test]
    fn parse_function_call() {
        let expr = parse("SUM(A1:A3)").unwrap();
        assert_eq!(expr, Expr::FnCall {
            name: "SUM".into(),
            args: vec![Expr::Range {
                start: CellRef { col: 0, row: 0, abs_col: false, abs_row: false },
                end: CellRef { col: 0, row: 2, abs_col: false, abs_row: false },
            }],
        });
    }

    #[test]
    fn parse_function_multiple_args() {
        let expr = parse("IF(A1>0,A1,0)").unwrap();
        assert_eq!(expr, Expr::FnCall {
            name: "IF".into(),
            args: vec![
                Expr::BinaryOp {
                    op: Op::Gt,
                    left: Box::new(Expr::CellRef(CellRef { col: 0, row: 0, abs_col: false, abs_row: false })),
                    right: Box::new(Expr::Number(0.0)),
                },
                Expr::CellRef(CellRef { col: 0, row: 0, abs_col: false, abs_row: false }),
                Expr::Number(0.0),
            ],
        });
    }

    #[test]
    fn parse_unary_negation() {
        let expr = parse("-A1").unwrap();
        assert_eq!(expr, Expr::UnaryNeg(Box::new(
            Expr::CellRef(CellRef { col: 0, row: 0, abs_col: false, abs_row: false })
        )));
    }

    #[test]
    fn parse_comparison() {
        let expr = parse("A1>=10").unwrap();
        assert_eq!(expr, Expr::BinaryOp {
            op: Op::Gte,
            left: Box::new(Expr::CellRef(CellRef { col: 0, row: 0, abs_col: false, abs_row: false })),
            right: Box::new(Expr::Number(10.0)),
        });
    }

    #[test]
    fn parse_range() {
        let expr = parse("A1:B3").unwrap();
        assert_eq!(expr, Expr::Range {
            start: CellRef { col: 0, row: 0, abs_col: false, abs_row: false },
            end: CellRef { col: 1, row: 2, abs_col: false, abs_row: false },
        });
    }

    #[test]
    fn parse_bool() {
        assert_eq!(parse("TRUE").unwrap(), Expr::Bool(true));
        assert_eq!(parse("FALSE").unwrap(), Expr::Bool(false));
    }

    #[test]
    fn parse_complex_formula() {
        let expr = parse("SUM(A1:A3)+1").unwrap();
        assert_eq!(expr, Expr::BinaryOp {
            op: Op::Add,
            left: Box::new(Expr::FnCall {
                name: "SUM".into(),
                args: vec![Expr::Range {
                    start: CellRef { col: 0, row: 0, abs_col: false, abs_row: false },
                    end: CellRef { col: 0, row: 2, abs_col: false, abs_row: false },
                }],
            }),
            right: Box::new(Expr::Number(1.0)),
        });
    }
}
