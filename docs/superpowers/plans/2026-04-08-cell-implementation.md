# cell — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `cell`, a terminal spreadsheet editor with Vim keybindings that opens CSV/TSV files, supports formulas, and saves to a native `.cell` format.

**Architecture:** Cargo workspace with two crates — `cell-core` (data model, formulas, file I/O) and `cell-tui` (ratatui rendering, Vim modes, event loop). Unidirectional data flow: key events → mode handler → action → state mutation → render.

**Tech Stack:** Rust, ratatui 0.30+, crossterm 0.29+, csv 1.3+, clap 4.6+

**Spec:** `docs/superpowers/specs/2026-04-08-cell-design.md`

---

## File Structure

```
cell/
  Cargo.toml                          # workspace root
  crates/
    cell-core/
      Cargo.toml
      src/
        lib.rs                        # re-exports
        model.rs                      # Sheet, Cell, CellValue, CellPos
        formula/
          mod.rs                      # re-exports
          token.rs                    # tokenizer
          ast.rs                      # AST types (Expr, CellRef, Op)
          parser.rs                   # token stream → Expr
          eval.rs                     # Expr → CellValue
          functions.rs                # SUM, AVERAGE, COUNT, MIN, MAX, IF
          deps.rs                     # dependency graph, topological sort, cycle detection
        io/
          mod.rs                      # re-exports
          csv.rs                      # CSV/TSV read/write
          cell_format.rs              # .cell native format read/write
    cell-tui/
      Cargo.toml
      src/
        main.rs                       # CLI parsing, terminal setup, run loop
        app.rs                        # App state struct, action processing
        action.rs                     # Action enum
        mode/
          mod.rs                      # Mode enum, re-exports
          normal.rs                   # Normal mode key handler
          insert.rs                   # Insert mode key handler
          visual.rs                   # Visual + Visual Block key handlers
          command.rs                  # Command-line mode handler (: and /)
        render/
          mod.rs                      # top-level render function
          grid.rs                     # grid widget (custom ratatui widget)
          formula_bar.rs              # formula bar widget
          status_bar.rs               # status bar widget
          command_line.rs             # command line widget
        clipboard.rs                  # Register, yank/paste logic, formula adjustment
        undo.rs                       # UndoEntry, undo/redo stacks
        viewport.rs                   # Viewport, scroll logic
```

---

## Task 1: Workspace Scaffolding

**Files:**
- Create: `Cargo.toml`
- Create: `crates/cell-core/Cargo.toml`
- Create: `crates/cell-core/src/lib.rs`
- Create: `crates/cell-tui/Cargo.toml`
- Create: `crates/cell-tui/src/main.rs`

- [ ] **Step 1: Create workspace root Cargo.toml**

```toml
[workspace]
members = ["crates/cell-core", "crates/cell-tui"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
```

- [ ] **Step 2: Create cell-core crate**

`crates/cell-core/Cargo.toml`:
```toml
[package]
name = "cell-core"
version.workspace = true
edition.workspace = true

[dependencies]
csv = "1.3"

[dev-dependencies]
pretty_assertions = "1"
```

`crates/cell-core/src/lib.rs`:
```rust
pub mod model;
```

Create `crates/cell-core/src/model.rs` as an empty file for now.

- [ ] **Step 3: Create cell-tui crate**

`crates/cell-tui/Cargo.toml`:
```toml
[package]
name = "cell-tui"
version.workspace = true
edition.workspace = true

[[bin]]
name = "cell"
path = "src/main.rs"

[dependencies]
cell-core = { path = "../cell-core" }
ratatui = "0.30"
crossterm = "0.29"
clap = { version = "4", features = ["derive"] }
```

`crates/cell-tui/src/main.rs`:
```rust
fn main() {
    println!("cell v0.1.0");
}
```

- [ ] **Step 4: Verify workspace builds**

Run: `cargo build`
Expected: Compiles with no errors, prints warnings at most.

- [ ] **Step 5: Verify binary runs**

Run: `cargo run --bin cell`
Expected: Prints `cell v0.1.0`

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/
git commit -m "feat: scaffold workspace with cell-core and cell-tui crates"
```

---

## Task 2: Data Model

**Files:**
- Create: `crates/cell-core/src/model.rs`
- Modify: `crates/cell-core/src/lib.rs`

- [ ] **Step 1: Write failing tests for CellValue and Cell**

Create `crates/cell-core/src/model.rs` with tests at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_value_display_number() {
        assert_eq!(CellValue::Number(42.0).to_string(), "42");
        assert_eq!(CellValue::Number(3.14).to_string(), "3.14");
        assert_eq!(CellValue::Number(0.0).to_string(), "0");
    }

    #[test]
    fn cell_value_display_text() {
        assert_eq!(CellValue::Text("hello".into()).to_string(), "hello");
    }

    #[test]
    fn cell_value_display_bool() {
        assert_eq!(CellValue::Bool(true).to_string(), "TRUE");
        assert_eq!(CellValue::Bool(false).to_string(), "FALSE");
    }

    #[test]
    fn cell_value_display_errors() {
        assert_eq!(CellValue::Error(CellError::DivZero).to_string(), "#DIV/0!");
        assert_eq!(CellValue::Error(CellError::Value).to_string(), "#VALUE!");
        assert_eq!(CellValue::Error(CellError::Ref).to_string(), "#REF!");
        assert_eq!(CellValue::Error(CellError::Circ).to_string(), "#CIRC!");
        assert_eq!(CellValue::Error(CellError::Name).to_string(), "#NAME?");
        assert_eq!(CellValue::Error(CellError::Parse).to_string(), "#PARSE!");
    }

    #[test]
    fn cell_value_display_empty() {
        assert_eq!(CellValue::Empty.to_string(), "");
    }

    #[test]
    fn sheet_new_is_empty() {
        let sheet = Sheet::new();
        assert_eq!(sheet.row_count, 0);
        assert_eq!(sheet.col_count, 0);
        assert!(sheet.cells.is_empty());
    }

    #[test]
    fn sheet_set_and_get_cell() {
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), "hello");
        let cell = sheet.get_cell((0, 0)).unwrap();
        assert_eq!(cell.raw, "hello");
        assert_eq!(cell.value, CellValue::Text("hello".into()));
    }

    #[test]
    fn sheet_set_cell_updates_extent() {
        let mut sheet = Sheet::new();
        sheet.set_cell((5, 3), "x");
        assert_eq!(sheet.row_count, 6);
        assert_eq!(sheet.col_count, 4);
    }

    #[test]
    fn sheet_set_cell_parses_number() {
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), "42");
        assert_eq!(sheet.get_cell((0, 0)).unwrap().value, CellValue::Number(42.0));
    }

    #[test]
    fn sheet_set_cell_parses_float() {
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), "3.14");
        assert_eq!(sheet.get_cell((0, 0)).unwrap().value, CellValue::Number(3.14));
    }

    #[test]
    fn sheet_get_cell_empty() {
        let sheet = Sheet::new();
        assert!(sheet.get_cell((0, 0)).is_none());
    }

    #[test]
    fn sheet_clear_cell() {
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), "hello");
        sheet.clear_cell((0, 0));
        assert!(sheet.get_cell((0, 0)).is_none());
    }

    #[test]
    fn col_index_to_label_single() {
        assert_eq!(col_index_to_label(0), "A");
        assert_eq!(col_index_to_label(25), "Z");
    }

    #[test]
    fn col_index_to_label_double() {
        assert_eq!(col_index_to_label(26), "AA");
        assert_eq!(col_index_to_label(27), "AB");
        assert_eq!(col_index_to_label(51), "AZ");
        assert_eq!(col_index_to_label(52), "BA");
    }

    #[test]
    fn col_label_to_index_roundtrip() {
        for i in 0..100 {
            assert_eq!(col_label_to_index(&col_index_to_label(i)).unwrap(), i);
        }
    }

    #[test]
    fn cell_value_number_display_no_trailing_zeros() {
        assert_eq!(CellValue::Number(1.0).to_string(), "1");
        assert_eq!(CellValue::Number(1.10).to_string(), "1.1");
        assert_eq!(CellValue::Number(1.123).to_string(), "1.123");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p cell-core`
Expected: Compilation errors — types not defined yet.

- [ ] **Step 3: Implement the data model**

Write the implementation in `crates/cell-core/src/model.rs` above the tests:

```rust
use std::collections::HashMap;
use std::fmt;

pub type CellPos = (usize, usize);

#[derive(Debug, Clone, PartialEq)]
pub enum CellError {
    DivZero,
    Value,
    Ref,
    Circ,
    Name,
    Parse,
}

impl fmt::Display for CellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CellError::DivZero => write!(f, "#DIV/0!"),
            CellError::Value => write!(f, "#VALUE!"),
            CellError::Ref => write!(f, "#REF!"),
            CellError::Circ => write!(f, "#CIRC!"),
            CellError::Name => write!(f, "#NAME?"),
            CellError::Parse => write!(f, "#PARSE!"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CellValue {
    Number(f64),
    Text(String),
    Bool(bool),
    Error(CellError),
    Empty,
}

impl fmt::Display for CellValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CellValue::Number(n) => {
                if n.fract() == 0.0 {
                    write!(f, "{}", *n as i64)
                } else {
                    write!(f, "{}", n)
                }
            }
            CellValue::Text(s) => write!(f, "{}", s),
            CellValue::Bool(b) => write!(f, "{}", if *b { "TRUE" } else { "FALSE" }),
            CellValue::Error(e) => write!(f, "{}", e),
            CellValue::Empty => write!(f, ""),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Cell {
    pub raw: String,
    pub value: CellValue,
    pub dirty: bool,
}

pub struct Sheet {
    pub cells: HashMap<CellPos, Cell>,
    pub col_widths: Vec<u16>,
    pub row_count: usize,
    pub col_count: usize,
}

impl Sheet {
    pub fn new() -> Self {
        Sheet {
            cells: HashMap::new(),
            col_widths: Vec::new(),
            row_count: 0,
            col_count: 0,
        }
    }

    pub fn get_cell(&self, pos: CellPos) -> Option<&Cell> {
        self.cells.get(&pos)
    }

    pub fn set_cell(&mut self, pos: CellPos, raw: &str) {
        let value = if raw.is_empty() {
            CellValue::Empty
        } else if raw.starts_with('=') {
            // Formula — will be evaluated by formula engine later.
            // For now, store as text.
            CellValue::Text(raw.to_string())
        } else if let Ok(n) = raw.parse::<f64>() {
            CellValue::Number(n)
        } else {
            CellValue::Text(raw.to_string())
        };

        self.cells.insert(pos, Cell {
            raw: raw.to_string(),
            value,
            dirty: raw.starts_with('='),
        });

        self.row_count = self.row_count.max(pos.0 + 1);
        self.col_count = self.col_count.max(pos.1 + 1);
    }

    pub fn clear_cell(&mut self, pos: CellPos) {
        self.cells.remove(&pos);
    }
}

pub fn col_index_to_label(mut col: usize) -> String {
    let mut label = String::new();
    loop {
        label.insert(0, (b'A' + (col % 26) as u8) as char);
        if col < 26 {
            break;
        }
        col = col / 26 - 1;
    }
    label
}

pub fn col_label_to_index(label: &str) -> Option<usize> {
    let mut index = 0usize;
    for (i, c) in label.chars().enumerate() {
        if !c.is_ascii_uppercase() {
            return None;
        }
        if i > 0 {
            index = (index + 1) * 26;
        }
        index += (c as usize) - ('A' as usize);
    }
    Some(index)
}
```

Update `crates/cell-core/src/lib.rs`:
```rust
pub mod model;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p cell-core`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/cell-core/src/
git commit -m "feat: implement data model — Sheet, Cell, CellValue with sparse storage"
```

---

## Task 3: Formula Tokenizer

**Files:**
- Create: `crates/cell-core/src/formula/mod.rs`
- Create: `crates/cell-core/src/formula/token.rs`
- Modify: `crates/cell-core/src/lib.rs`

- [ ] **Step 1: Write failing tests for tokenizer**

Create `crates/cell-core/src/formula/token.rs`:

```rust
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
        let tokens = tokenize("3.14").unwrap();
        assert_eq!(tokens, vec![Token::Number(3.14)]);
    }

    #[test]
    fn tokenize_string() {
        let tokens = tokenize("\"hello\"").unwrap();
        assert_eq!(tokens, vec![Token::StringLit("hello".into())]);
    }

    #[test]
    fn tokenize_cell_ref() {
        let tokens = tokenize("A1").unwrap();
        assert_eq!(tokens, vec![Token::CellRef {
            col: "A".into(),
            row: "1".into(),
            abs_col: false,
            abs_row: false,
        }]);
    }

    #[test]
    fn tokenize_absolute_cell_ref() {
        let tokens = tokenize("$A$1").unwrap();
        assert_eq!(tokens, vec![Token::CellRef {
            col: "A".into(),
            row: "1".into(),
            abs_col: true,
            abs_row: true,
        }]);
    }

    #[test]
    fn tokenize_mixed_ref() {
        let tokens = tokenize("$A1").unwrap();
        assert_eq!(tokens, vec![Token::CellRef {
            col: "A".into(),
            row: "1".into(),
            abs_col: true,
            abs_row: false,
        }]);
    }

    #[test]
    fn tokenize_operators() {
        let tokens = tokenize("+-*/").unwrap();
        assert_eq!(tokens, vec![
            Token::Plus, Token::Minus, Token::Star, Token::Slash,
        ]);
    }

    #[test]
    fn tokenize_comparison_operators() {
        let tokens = tokenize(">>=<<=<>").unwrap();
        assert_eq!(tokens, vec![
            Token::Gt, Token::Gte, Token::Lt, Token::Lte, Token::Neq,
        ]);
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
        assert_eq!(tokens, vec![
            Token::Ident("SUM".into()),
            Token::LParen,
            Token::CellRef { col: "A".into(), row: "1".into(), abs_col: false, abs_row: false },
            Token::Colon,
            Token::CellRef { col: "A".into(), row: "3".into(), abs_col: false, abs_row: false },
            Token::RParen,
            Token::Plus,
            Token::Number(1.0),
        ]);
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
        assert_eq!(tokens, vec![
            Token::CellRef { col: "A".into(), row: "1".into(), abs_col: false, abs_row: false },
            Token::Plus,
            Token::CellRef { col: "B".into(), row: "1".into(), abs_col: false, abs_row: false },
        ]);
    }

    #[test]
    fn tokenize_multi_letter_col() {
        let tokens = tokenize("AA10").unwrap();
        assert_eq!(tokens, vec![Token::CellRef {
            col: "AA".into(),
            row: "10".into(),
            abs_col: false,
            abs_row: false,
        }]);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p cell-core`
Expected: Compilation errors — Token type and tokenize function not defined.

- [ ] **Step 3: Implement the tokenizer**

Write the implementation above the tests in `crates/cell-core/src/formula/token.rs`:

```rust
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
            ' ' | '\t' => { i += 1; }
            '+' => { tokens.push(Token::Plus); i += 1; }
            '-' => { tokens.push(Token::Minus); i += 1; }
            '*' => { tokens.push(Token::Star); i += 1; }
            '/' => { tokens.push(Token::Slash); i += 1; }
            '(' => { tokens.push(Token::LParen); i += 1; }
            ')' => { tokens.push(Token::RParen); i += 1; }
            ',' => { tokens.push(Token::Comma); i += 1; }
            ':' => { tokens.push(Token::Colon); i += 1; }
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
            '=' => { tokens.push(Token::Eq); i += 1; }
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
            '$' | c if c.is_ascii_uppercase() => {
                // Could be a cell reference like $A$1, A1, $A1, A$1
                // or an identifier like SUM, TRUE, FALSE
                let mut abs_col = false;
                let mut j = i;

                if chars[j] == '$' {
                    abs_col = true;
                    j += 1;
                }

                // Read letters
                let col_start = j;
                while j < chars.len() && chars[j].is_ascii_uppercase() {
                    j += 1;
                }
                let col: String = chars[col_start..j].iter().collect();

                if col.is_empty() {
                    return Err(CellError::Parse);
                }

                // Check if followed by $ or digit (cell ref) or not (ident)
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
                    // It's a cell reference
                    tokens.push(Token::CellRef { col, row, abs_col, abs_row });
                } else if !abs_col && !abs_row {
                    // It's an identifier (function name or boolean)
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
```

Create `crates/cell-core/src/formula/mod.rs`:
```rust
pub mod token;
```

Update `crates/cell-core/src/lib.rs`:
```rust
pub mod model;
pub mod formula;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p cell-core`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/cell-core/src/
git commit -m "feat: implement formula tokenizer"
```

---

## Task 4: Formula AST and Parser

**Files:**
- Create: `crates/cell-core/src/formula/ast.rs`
- Create: `crates/cell-core/src/formula/parser.rs`
- Modify: `crates/cell-core/src/formula/mod.rs`

- [ ] **Step 1: Write failing tests for the parser**

Create `crates/cell-core/src/formula/ast.rs`:

```rust
use crate::model::col_label_to_index;

#[derive(Debug, Clone, PartialEq)]
pub struct CellRef {
    pub col: usize,
    pub row: usize,
    pub abs_col: bool,
    pub abs_row: bool,
}

impl CellRef {
    /// Create from display-style strings (col="A", row="1") where row is 1-indexed.
    pub fn from_display(col: &str, row: &str, abs_col: bool, abs_row: bool) -> Option<Self> {
        let col_idx = col_label_to_index(col)?;
        let row_idx: usize = row.parse::<usize>().ok()?.checked_sub(1)?;
        Some(CellRef { col: col_idx, row: row_idx, abs_col, abs_row })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Gt,
    Gte,
    Lt,
    Lte,
    Eq,
    Neq,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    Text(String),
    Bool(bool),
    CellRef(CellRef),
    Range { start: CellRef, end: CellRef },
    BinaryOp { op: Op, left: Box<Expr>, right: Box<Expr> },
    UnaryNeg(Box<Expr>),
    FnCall { name: String, args: Vec<Expr> },
}
```

Create `crates/cell-core/src/formula/parser.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::formula::ast::*;

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
        // 1+2*3 should parse as 1+(2*3)
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
        // SUM(A1:A3)+1
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p cell-core`
Expected: Compilation errors — parse function not defined.

- [ ] **Step 3: Implement the parser**

Write the implementation above the tests in `crates/cell-core/src/formula/parser.rs`. This is a recursive descent parser with standard operator precedence.

```rust
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

    /// expression = comparison
    fn expression(&mut self) -> Result<Expr, CellError> {
        self.comparison()
    }

    /// comparison = addition (( ">" | ">=" | "<" | "<=" | "=" | "<>" ) addition)?
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

    /// addition = multiplication (( "+" | "-" ) multiplication)*
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

    /// multiplication = unary (( "*" | "/" ) unary)*
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

    /// unary = "-" unary | primary
    fn unary(&mut self) -> Result<Expr, CellError> {
        if let Some(Token::Minus) = self.peek() {
            self.advance();
            let expr = self.unary()?;
            return Ok(Expr::UnaryNeg(Box::new(expr)));
        }
        self.primary()
    }

    /// primary = NUMBER | STRING | BOOL | cell_ref_or_range | function_call | "(" expression ")"
    fn primary(&mut self) -> Result<Expr, CellError> {
        let tok = self.advance().ok_or(CellError::Parse)?.clone();
        match tok {
            Token::Number(n) => Ok(Expr::Number(n)),
            Token::StringLit(s) => Ok(Expr::Text(s)),
            Token::Bool(b) => Ok(Expr::Bool(b)),
            Token::CellRef { col, row, abs_col, abs_row } => {
                let cell_ref = CellRef::from_display(&col, &row, abs_col, abs_row)
                    .ok_or(CellError::Ref)?;

                // Check for range (colon)
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
                // Function call: NAME "(" args ")"
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

/// Parse a formula string (without leading '=') into an AST.
pub fn parse(input: &str) -> Result<Expr, CellError> {
    let tokens = tokenize(input)?;
    let mut parser = Parser::new(tokens);
    let expr = parser.expression()?;
    if parser.pos != parser.tokens.len() {
        return Err(CellError::Parse);
    }
    Ok(expr)
}
```

Update `crates/cell-core/src/formula/mod.rs`:
```rust
pub mod token;
pub mod ast;
pub mod parser;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p cell-core`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/cell-core/src/formula/
git commit -m "feat: implement formula AST and recursive descent parser"
```

---

## Task 5: Formula Evaluator

**Files:**
- Create: `crates/cell-core/src/formula/eval.rs`
- Create: `crates/cell-core/src/formula/functions.rs`
- Modify: `crates/cell-core/src/formula/mod.rs`

- [ ] **Step 1: Write failing tests for evaluator**

Create `crates/cell-core/src/formula/eval.rs`:

```rust
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
        assert_eq!(eval_with_sheet("SUM(A1:A3)", &sheet), CellValue::Number(6.0));
    }

    #[test]
    fn eval_average() {
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), "2");
        sheet.set_cell((1, 0), "4");
        assert_eq!(eval_with_sheet("AVERAGE(A1:A2)", &sheet), CellValue::Number(3.0));
    }

    #[test]
    fn eval_count() {
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), "1");
        sheet.set_cell((1, 0), "hello");
        sheet.set_cell((2, 0), "3");
        // COUNT counts numeric values only
        assert_eq!(eval_with_sheet("COUNT(A1:A3)", &sheet), CellValue::Number(2.0));
    }

    #[test]
    fn eval_min() {
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), "5");
        sheet.set_cell((1, 0), "2");
        sheet.set_cell((2, 0), "8");
        assert_eq!(eval_with_sheet("MIN(A1:A3)", &sheet), CellValue::Number(2.0));
    }

    #[test]
    fn eval_max() {
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), "5");
        sheet.set_cell((1, 0), "2");
        sheet.set_cell((2, 0), "8");
        assert_eq!(eval_with_sheet("MAX(A1:A3)", &sheet), CellValue::Number(8.0));
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
    fn eval_error_propagation() {
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), "=1/0"); // This would be #DIV/0! after formula eval
        // For now, set the cell value directly for testing
        sheet.cells.get_mut(&(0, 0)).unwrap().value = CellValue::Error(CellError::DivZero);
        assert_eq!(eval_with_sheet("A1+1", &sheet), CellValue::Error(CellError::DivZero));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p cell-core`
Expected: Compilation errors — evaluate function not defined.

- [ ] **Step 3: Implement the functions module**

Create `crates/cell-core/src/formula/functions.rs`:

```rust
use crate::model::{CellError, CellValue};

/// Collect numeric values from a list of CellValues, skipping non-numeric.
fn collect_numbers(values: &[CellValue]) -> Result<Vec<f64>, CellError> {
    let mut nums = Vec::new();
    for v in values {
        match v {
            CellValue::Number(n) => nums.push(*n),
            CellValue::Error(e) => return Err(e.clone()),
            CellValue::Empty => {} // skip
            CellValue::Text(_) => {} // skip for aggregates
            CellValue::Bool(_) => {} // skip
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
            if *n != 0.0 { args[1].clone() } else { args[2].clone() }
        }
        CellValue::Error(e) => CellValue::Error(e.clone()),
        _ => CellValue::Error(CellError::Value),
    }
}
```

- [ ] **Step 4: Implement the evaluator**

Write the implementation above the tests in `crates/cell-core/src/formula/eval.rs`:

```rust
use crate::model::{CellError, CellValue, CellPos, Sheet};
use crate::formula::ast::*;
use crate::formula::parser;
use crate::formula::functions;

/// Expand a Range into a list of CellPos.
fn expand_range(start: &CellRef, end: &CellRef) -> Vec<CellPos> {
    let mut positions = Vec::new();
    let r1 = start.row.min(end.row);
    let r2 = start.row.max(end.row);
    let c1 = start.col.min(end.col);
    let c2 = start.col.max(end.col);
    for r in r1..=r2 {
        for c in c1..=c2 {
            positions.push((r, c));
        }
    }
    positions
}

/// Get the CellValue at a position, treating empty cells as 0 for numeric contexts.
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
        Expr::Range { .. } => {
            // A bare range outside a function is an error
            CellValue::Error(CellError::Value)
        }
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

            // Propagate errors
            if let CellValue::Error(e) = &lval { return CellValue::Error(e.clone()); }
            if let CellValue::Error(e) = &rval { return CellValue::Error(e.clone()); }

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
                Op::Gt | Op::Gte | Op::Lt | Op::Lte | Op::Eq | Op::Neq => {
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
                        Op::Eq => (ln - rn).abs() < f64::EPSILON,
                        Op::Neq => (ln - rn).abs() >= f64::EPSILON,
                        _ => unreachable!(),
                    };
                    CellValue::Bool(result)
                }
            }
        }
        Expr::FnCall { name, args } => {
            let upper = name.to_uppercase();

            // For IF, evaluate args lazily
            if upper == "IF" {
                let evaled: Vec<CellValue> = args.iter().map(|a| eval_expr(a, sheet)).collect();
                return functions::fn_if(&evaled);
            }

            // For aggregate functions, expand range args
            let mut values = Vec::new();
            for arg in args {
                match arg {
                    Expr::Range { start, end } => {
                        for pos in expand_range(start, end) {
                            values.push(resolve_cell(sheet, pos));
                        }
                    }
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

/// Evaluate a formula string (without leading '=') against a sheet.
pub fn evaluate(formula: &str, sheet: &Sheet) -> CellValue {
    match parser::parse(formula) {
        Ok(expr) => eval_expr(&expr, sheet),
        Err(e) => CellValue::Error(e),
    }
}
```

Update `crates/cell-core/src/formula/mod.rs`:
```rust
pub mod token;
pub mod ast;
pub mod parser;
pub mod eval;
pub mod functions;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p cell-core`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/cell-core/src/formula/
git commit -m "feat: implement formula evaluator with SUM, AVERAGE, COUNT, MIN, MAX, IF"
```

---

## Task 6: Dependency Graph and Recalculation

**Files:**
- Create: `crates/cell-core/src/formula/deps.rs`
- Modify: `crates/cell-core/src/model.rs`
- Modify: `crates/cell-core/src/formula/mod.rs`

- [ ] **Step 1: Write failing tests for dependency tracking and recalc**

Create `crates/cell-core/src/formula/deps.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Sheet, CellValue};

    #[test]
    fn extract_deps_cell_ref() {
        let deps = extract_deps("A1+B1");
        assert_eq!(deps, vec![(0, 0), (0, 1)]);
    }

    #[test]
    fn extract_deps_range() {
        let deps = extract_deps("SUM(A1:A3)");
        assert_eq!(deps, vec![(0, 0), (1, 0), (2, 0)]);
    }

    #[test]
    fn extract_deps_no_refs() {
        let deps = extract_deps("1+2");
        assert!(deps.is_empty());
    }

    #[test]
    fn recalc_simple() {
        let mut sheet = Sheet::new();
        let mut deps = DepGraph::new();
        sheet.set_cell((0, 0), "10");
        set_formula(&mut sheet, &mut deps, (0, 1), "=A1+5");
        recalculate(&mut sheet, &deps);
        assert_eq!(sheet.get_cell((0, 1)).unwrap().value, CellValue::Number(15.0));
    }

    #[test]
    fn recalc_chain() {
        let mut sheet = Sheet::new();
        let mut deps = DepGraph::new();
        sheet.set_cell((0, 0), "10");
        set_formula(&mut sheet, &mut deps, (0, 1), "=A1*2");
        set_formula(&mut sheet, &mut deps, (0, 2), "=B1+1");
        recalculate(&mut sheet, &deps);
        assert_eq!(sheet.get_cell((0, 1)).unwrap().value, CellValue::Number(20.0));
        assert_eq!(sheet.get_cell((0, 2)).unwrap().value, CellValue::Number(21.0));
    }

    #[test]
    fn recalc_circular_reference() {
        let mut sheet = Sheet::new();
        let mut deps = DepGraph::new();
        set_formula(&mut sheet, &mut deps, (0, 0), "=B1");
        set_formula(&mut sheet, &mut deps, (0, 1), "=A1");
        recalculate(&mut sheet, &deps);
        assert_eq!(sheet.get_cell((0, 0)).unwrap().value, CellValue::Error(CellError::Circ));
        assert_eq!(sheet.get_cell((0, 1)).unwrap().value, CellValue::Error(CellError::Circ));
    }

    #[test]
    fn recalc_after_value_change() {
        let mut sheet = Sheet::new();
        let mut deps = DepGraph::new();
        sheet.set_cell((0, 0), "10");
        set_formula(&mut sheet, &mut deps, (0, 1), "=A1+5");
        recalculate(&mut sheet, &deps);
        assert_eq!(sheet.get_cell((0, 1)).unwrap().value, CellValue::Number(15.0));

        // Change A1
        sheet.set_cell((0, 0), "20");
        mark_dirty(&mut sheet, &deps, (0, 0));
        recalculate(&mut sheet, &deps);
        assert_eq!(sheet.get_cell((0, 1)).unwrap().value, CellValue::Number(25.0));
    }

    #[test]
    fn self_reference_is_circular() {
        let mut sheet = Sheet::new();
        let mut deps = DepGraph::new();
        set_formula(&mut sheet, &mut deps, (0, 0), "=A1+1");
        recalculate(&mut sheet, &deps);
        assert_eq!(sheet.get_cell((0, 0)).unwrap().value, CellValue::Error(CellError::Circ));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p cell-core`
Expected: Compilation errors — DepGraph, set_formula, recalculate, mark_dirty not defined.

- [ ] **Step 3: Implement the dependency graph**

Write the implementation above the tests in `crates/cell-core/src/formula/deps.rs`:

```rust
use std::collections::{HashMap, HashSet, VecDeque};

use crate::model::{CellPos, CellValue, CellError, Sheet};
use crate::formula::ast::*;
use crate::formula::parser;
use crate::formula::eval;

/// Dependency graph: tracks which cells depend on which.
pub struct DepGraph {
    /// cell -> set of cells that depend on it (its dependents)
    pub dependents: HashMap<CellPos, HashSet<CellPos>>,
    /// cell -> set of cells it depends on (its dependencies)
    pub dependencies: HashMap<CellPos, HashSet<CellPos>>,
}

impl DepGraph {
    pub fn new() -> Self {
        DepGraph {
            dependents: HashMap::new(),
            dependencies: HashMap::new(),
        }
    }

    /// Register that `cell` depends on the given set of positions.
    pub fn set_dependencies(&mut self, cell: CellPos, deps: Vec<CellPos>) {
        // Remove old dependencies
        if let Some(old_deps) = self.dependencies.remove(&cell) {
            for dep in &old_deps {
                if let Some(set) = self.dependents.get_mut(dep) {
                    set.remove(&cell);
                }
            }
        }

        // Add new dependencies
        let dep_set: HashSet<CellPos> = deps.into_iter().collect();
        for dep in &dep_set {
            self.dependents.entry(*dep).or_default().insert(cell);
        }
        self.dependencies.insert(cell, dep_set);
    }
}

/// Extract cell positions referenced by a formula string (without '=').
pub fn extract_deps(formula: &str) -> Vec<CellPos> {
    let expr = match parser::parse(formula) {
        Ok(e) => e,
        Err(_) => return vec![],
    };
    let mut deps = Vec::new();
    collect_refs(&expr, &mut deps);
    deps
}

fn collect_refs(expr: &Expr, out: &mut Vec<CellPos>) {
    match expr {
        Expr::CellRef(r) => {
            out.push((r.row, r.col));
        }
        Expr::Range { start, end } => {
            let r1 = start.row.min(end.row);
            let r2 = start.row.max(end.row);
            let c1 = start.col.min(end.col);
            let c2 = start.col.max(end.col);
            for r in r1..=r2 {
                for c in c1..=c2 {
                    out.push((r, c));
                }
            }
        }
        Expr::BinaryOp { left, right, .. } => {
            collect_refs(left, out);
            collect_refs(right, out);
        }
        Expr::UnaryNeg(inner) => collect_refs(inner, out),
        Expr::FnCall { args, .. } => {
            for arg in args {
                collect_refs(arg, out);
            }
        }
        _ => {}
    }
}

/// Set a formula cell and register its dependencies.
pub fn set_formula(sheet: &mut Sheet, deps: &mut DepGraph, pos: CellPos, raw: &str) {
    sheet.set_cell(pos, raw);
    let formula = &raw[1..]; // strip '='
    let dep_list = extract_deps(formula);
    deps.set_dependencies(pos, dep_list);
}

/// Mark a cell and all its transitive dependents as dirty.
pub fn mark_dirty(sheet: &mut Sheet, deps: &DepGraph, pos: CellPos) {
    let mut queue = VecDeque::new();
    queue.push_back(pos);
    while let Some(cell) = queue.pop_front() {
        if let Some(dependents) = deps.dependents.get(&cell) {
            for &dep in dependents {
                if let Some(c) = sheet.cells.get_mut(&dep) {
                    if !c.dirty {
                        c.dirty = true;
                        queue.push_back(dep);
                    }
                }
            }
        }
    }
}

/// Recalculate all dirty formula cells in topological order.
/// Detects circular references and marks them as #CIRC!.
pub fn recalculate(sheet: &mut Sheet, deps: &DepGraph) {
    // Collect all formula cells
    let formula_cells: Vec<CellPos> = sheet.cells.iter()
        .filter(|(_, cell)| cell.raw.starts_with('='))
        .map(|(pos, _)| *pos)
        .collect();

    // Topological sort using Kahn's algorithm
    // Build in-degree counts scoped to formula cells only
    let formula_set: HashSet<CellPos> = formula_cells.iter().cloned().collect();

    let mut in_degree: HashMap<CellPos, usize> = HashMap::new();
    for &cell in &formula_cells {
        let count = deps.dependencies.get(&cell)
            .map(|d| d.iter().filter(|p| formula_set.contains(p)).count())
            .unwrap_or(0);
        in_degree.insert(cell, count);
    }

    let mut queue: VecDeque<CellPos> = in_degree.iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(&pos, _)| pos)
        .collect();

    let mut order = Vec::new();

    while let Some(cell) = queue.pop_front() {
        order.push(cell);
        if let Some(dependents) = deps.dependents.get(&cell) {
            for &dep in dependents {
                if let Some(deg) = in_degree.get_mut(&dep) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(dep);
                    }
                }
            }
        }
    }

    // Cells not in `order` are part of circular references
    let ordered_set: HashSet<CellPos> = order.iter().cloned().collect();
    for &cell in &formula_cells {
        if !ordered_set.contains(&cell) {
            if let Some(c) = sheet.cells.get_mut(&cell) {
                c.value = CellValue::Error(CellError::Circ);
                c.dirty = false;
            }
        }
    }

    // Evaluate in topological order
    for pos in order {
        let raw = match sheet.get_cell(pos) {
            Some(cell) if cell.raw.starts_with('=') => cell.raw.clone(),
            _ => continue,
        };
        let formula = &raw[1..];
        let value = eval::evaluate(formula, sheet);
        if let Some(cell) = sheet.cells.get_mut(&pos) {
            cell.value = value;
            cell.dirty = false;
        }
    }
}
```

Update `crates/cell-core/src/formula/mod.rs`:
```rust
pub mod token;
pub mod ast;
pub mod parser;
pub mod eval;
pub mod functions;
pub mod deps;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p cell-core`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/cell-core/src/formula/
git commit -m "feat: implement dependency graph with topological recalculation and cycle detection"
```

---

## Task 7: CSV/TSV I/O

**Files:**
- Create: `crates/cell-core/src/io/mod.rs`
- Create: `crates/cell-core/src/io/csv.rs`
- Modify: `crates/cell-core/src/lib.rs`

- [ ] **Step 1: Write failing tests for CSV I/O**

Create `crates/cell-core/src/io/csv.rs`:

```rust
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
        assert!(sheet.get_cell((0, 1)).is_none()); // empty cell not stored
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
        // Formulas in CSV should be treated as literal text
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
        // Manually set the computed value
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
        // col_widths should be at least as wide as the longest content
        assert!(sheet.col_widths[0] >= 5); // "Alice" = 5 chars
        assert!(sheet.col_widths[1] >= 5); // "Score" = 5 chars
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p cell-core`
Expected: Compilation errors — read_csv, write_csv not defined.

- [ ] **Step 3: Implement CSV I/O**

Write the implementation above the tests in `crates/cell-core/src/io/csv.rs`:

```rust
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

    // Auto-size column widths
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
```

Create `crates/cell-core/src/io/mod.rs`:
```rust
pub mod csv;
```

Update `crates/cell-core/src/lib.rs`:
```rust
pub mod model;
pub mod formula;
pub mod io;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p cell-core`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/cell-core/src/
git commit -m "feat: implement CSV/TSV import and export"
```

---

## Task 8: Native .cell Format I/O

**Files:**
- Create: `crates/cell-core/src/io/cell_format.rs`
- Modify: `crates/cell-core/src/io/mod.rs`

- [ ] **Step 1: Write failing tests for .cell format**

Create `crates/cell-core/src/io/cell_format.rs`:

```rust
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

        // Verify it contains expected directives
        assert!(output.contains("size 2 2"));
        assert!(output.contains("let A0 = 42"));
        assert!(output.contains("label B0 = \"hello\""));
        assert!(output.contains("formula A1 = =A1+1"));
        assert!(output.contains("col-width 0 12"));

        // Read it back
        let sheet2 = read_cell_format(buf.as_slice()).unwrap();
        assert_eq!(sheet2.row_count, 2);
        assert_eq!(sheet2.col_count, 2);
        assert_eq!(sheet2.get_cell((0, 0)).unwrap().value, CellValue::Number(42.0));
        assert_eq!(sheet2.get_cell((0, 1)).unwrap().value, CellValue::Text("hello".into()));
        assert_eq!(sheet2.get_cell((1, 0)).unwrap().raw, "=A1+1");
        assert_eq!(sheet2.col_widths, vec![12, 8]);
    }

    #[test]
    fn read_comments_and_blanks() {
        let data = "# comment\n\nsize 1 1\nlet A0 = 5\n";
        let sheet = read_cell_format(data.as_bytes()).unwrap();
        assert_eq!(sheet.get_cell((0, 0)).unwrap().value, CellValue::Number(5.0));
    }

    #[test]
    fn read_label_with_spaces() {
        let data = "size 1 1\nlabel A0 = \"hello world\"\n";
        let sheet = read_cell_format(data.as_bytes()).unwrap();
        assert_eq!(sheet.get_cell((0, 0)).unwrap().value, CellValue::Text("hello world".into()));
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
        let data = "size 1 1\nlet A0 = 3.14\n";
        let sheet = read_cell_format(data.as_bytes()).unwrap();
        assert_eq!(sheet.get_cell((0, 0)).unwrap().value, CellValue::Number(3.14));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p cell-core`
Expected: Compilation errors — write_cell_format, read_cell_format not defined.

- [ ] **Step 3: Implement .cell format I/O**

Write the implementation above the tests in `crates/cell-core/src/io/cell_format.rs`:

```rust
use std::io::{Read, Write, BufRead, BufReader};
use crate::model::{Sheet, CellValue, CellError, col_index_to_label, col_label_to_index};

/// Parse a cell address like "A0" into (row, col). Row is zero-indexed in .cell format.
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

/// Format a cell address from (row, col) to "A0" style.
fn format_address(row: usize, col: usize) -> String {
    format!("{}{}", col_index_to_label(col), row)
}

pub fn write_cell_format<W: Write>(sheet: &Sheet, mut writer: W) -> Result<(), Box<dyn std::error::Error>> {
    writeln!(writer, "# cell v1")?;
    writeln!(writer)?;
    writeln!(writer, "size {} {}", sheet.row_count, sheet.col_count)?;
    writeln!(writer)?;

    // Write column widths
    for (i, &width) in sheet.col_widths.iter().enumerate() {
        writeln!(writer, "col-width {} {}", i, width)?;
    }
    if !sheet.col_widths.is_empty() {
        writeln!(writer)?;
    }

    // Write cells sorted by position for deterministic output
    let mut positions: Vec<_> = sheet.cells.keys().cloned().collect();
    positions.sort();

    for pos in positions {
        let cell = &sheet.cells[&pos];
        let addr = format_address(pos.0, pos.1);

        if cell.raw.starts_with('=') {
            writeln!(writer, "formula {} = {}", addr, cell.raw)?;
        } else {
            match &cell.value {
                CellValue::Number(n) => {
                    writeln!(writer, "let {} = {}", addr, cell.raw)?;
                }
                CellValue::Text(s) => {
                    writeln!(writer, "label {} = \"{}\"", addr, s)?;
                }
                CellValue::Bool(b) => {
                    writeln!(writer, "let {} = {}", addr, if *b { "TRUE" } else { "FALSE" })?;
                }
                CellValue::Empty => {}
                CellValue::Error(_) => {
                    // Store the raw value so it can be re-evaluated on load
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

        // Skip comments and empty lines
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(rest) = line.strip_prefix("size ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() == 2 {
                sheet.row_count = parts[0].parse().unwrap_or(0);
                sheet.col_count = parts[1].parse().unwrap_or(0);
            }
        } else if let Some(rest) = line.strip_prefix("col-width ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() == 2 {
                let idx: usize = parts[0].parse().unwrap_or(0);
                let width: u16 = parts[1].parse().unwrap_or(10);
                if idx >= sheet.col_widths.len() {
                    sheet.col_widths.resize(idx + 1, 10);
                }
                sheet.col_widths[idx] = width;
            }
        } else if let Some(rest) = line.strip_prefix("let ") {
            // let ADDR = VALUE
            if let Some((addr_str, value_str)) = rest.split_once(" = ") {
                if let Some(pos) = parse_address(addr_str.trim()) {
                    sheet.set_cell(pos, value_str.trim());
                }
            }
        } else if let Some(rest) = line.strip_prefix("label ") {
            // label ADDR = "VALUE"
            if let Some((addr_str, value_str)) = rest.split_once(" = ") {
                if let Some(pos) = parse_address(addr_str.trim()) {
                    // Strip quotes
                    let s = value_str.trim();
                    let s = if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
                        &s[1..s.len()-1]
                    } else {
                        s
                    };
                    sheet.set_cell(pos, s);
                }
            }
        } else if let Some(rest) = line.strip_prefix("formula ") {
            // formula ADDR = =FORMULA
            if let Some((addr_str, formula_str)) = rest.split_once(" = ") {
                if let Some(pos) = parse_address(addr_str.trim()) {
                    sheet.set_cell(pos, formula_str.trim());
                }
            }
        }
    }

    Ok(sheet)
}
```

Update `crates/cell-core/src/io/mod.rs`:
```rust
pub mod csv;
pub mod cell_format;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p cell-core`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/cell-core/src/io/
git commit -m "feat: implement native .cell format read/write"
```

---

## Task 9: TUI Scaffolding — App State, Actions, Basic Event Loop

**Files:**
- Create: `crates/cell-tui/src/app.rs`
- Create: `crates/cell-tui/src/action.rs`
- Create: `crates/cell-tui/src/viewport.rs`
- Modify: `crates/cell-tui/src/main.rs`

- [ ] **Step 1: Define the Action enum**

Create `crates/cell-tui/src/action.rs`:

```rust
use std::path::PathBuf;
use cell_core::model::CellPos;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SearchDirection {
    Forward,
    Backward,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Noop,
    MoveCursor(Direction),
    MoveCursorTo(CellPos),
    EditCell(CellPos, String),
    ClearCell(CellPos),
    ClearRange { start: CellPos, end: CellPos },
    YankCell(CellPos),
    YankRange { start: CellPos, end: CellPos },
    Paste(CellPos),
    PasteBefore(CellPos),
    Undo,
    Redo,
    ChangeMode(Mode),
    Save(Option<PathBuf>),
    ForceSave(Option<PathBuf>),
    Open(PathBuf),
    Quit { force: bool },
    Sort { col: usize, ascending: bool },
    Search { pattern: String, direction: SearchDirection },
    SearchNext,
    SearchPrev,
    Resize,
    PageDown,
    PageUp,
    HalfPageDown,
    HalfPageUp,
    GotoFirstRow,
    GotoLastRow,
    GotoFirstCol,
    GotoLastCol,
    NextNonEmpty,
    PrevNonEmpty,
    DeleteRow(usize),
    YankRow(usize),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
    VisualBlock,
    Command,
}
```

- [ ] **Step 2: Define the Viewport**

Create `crates/cell-tui/src/viewport.rs`:

```rust
use cell_core::model::CellPos;

pub struct Viewport {
    pub row_offset: usize,
    pub col_offset: usize,
    pub visible_rows: usize,
    pub visible_cols: usize,
}

impl Viewport {
    pub fn new() -> Self {
        Viewport {
            row_offset: 0,
            col_offset: 0,
            visible_rows: 20,
            visible_cols: 10,
        }
    }

    /// Ensure the cursor is visible, adjusting the offset if needed.
    pub fn ensure_visible(&mut self, cursor: CellPos) {
        let (row, col) = cursor;

        if row < self.row_offset {
            self.row_offset = row;
        } else if row >= self.row_offset + self.visible_rows {
            self.row_offset = row - self.visible_rows + 1;
        }

        if col < self.col_offset {
            self.col_offset = col;
        } else if col >= self.col_offset + self.visible_cols {
            self.col_offset = col - self.visible_cols + 1;
        }
    }
}
```

- [ ] **Step 3: Define the App state**

Create `crates/cell-tui/src/app.rs`:

```rust
use std::path::PathBuf;
use cell_core::model::{Sheet, CellPos, CellValue};
use cell_core::formula::deps::{DepGraph, set_formula, mark_dirty, recalculate};
use crate::action::{Action, Mode, Direction};
use crate::viewport::Viewport;
use crate::undo::{UndoEntry, UndoStack};
use crate::clipboard::Register;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileFormat {
    Csv,
    Tsv,
    Cell,
}

pub struct App {
    pub sheet: Sheet,
    pub deps: DepGraph,
    pub viewport: Viewport,
    pub cursor: CellPos,
    pub mode: Mode,
    pub register: Option<Register>,
    pub undo_stack: UndoStack,
    pub command_line: String,
    pub status_message: Option<String>,
    pub search_pattern: Option<String>,
    pub file_path: Option<PathBuf>,
    pub file_format: FileFormat,
    pub dirty: bool,
    pub should_quit: bool,
    pub insert_buffer: String,
}

impl App {
    pub fn new() -> Self {
        App {
            sheet: Sheet::new(),
            deps: DepGraph::new(),
            viewport: Viewport::new(),
            cursor: (0, 0),
            mode: Mode::Normal,
            register: None,
            undo_stack: UndoStack::new(),
            command_line: String::new(),
            status_message: None,
            search_pattern: None,
            file_path: None,
            file_format: FileFormat::Csv,
            dirty: false,
            should_quit: false,
            insert_buffer: String::new(),
        }
    }

    pub fn has_formulas(&self) -> bool {
        self.sheet.cells.values().any(|c| c.raw.starts_with('='))
    }

    /// Process an action and mutate state.
    pub fn process_action(&mut self, action: Action) {
        match action {
            Action::Noop => {}
            Action::MoveCursor(dir) => {
                let (row, col) = self.cursor;
                self.cursor = match dir {
                    Direction::Up => (row.saturating_sub(1), col),
                    Direction::Down => (row + 1, col),
                    Direction::Left => (row, col.saturating_sub(1)),
                    Direction::Right => (row, col + 1),
                };
                self.viewport.ensure_visible(self.cursor);
            }
            Action::MoveCursorTo(pos) => {
                self.cursor = pos;
                self.viewport.ensure_visible(self.cursor);
            }
            Action::EditCell(pos, raw) => {
                let old_raw = self.sheet.get_cell(pos)
                    .map(|c| c.raw.clone())
                    .unwrap_or_default();
                self.undo_stack.push(UndoEntry::CellEdit {
                    pos,
                    old_raw,
                    new_raw: raw.clone(),
                });

                if raw.starts_with('=') {
                    set_formula(&mut self.sheet, &mut self.deps, pos, &raw);
                } else {
                    self.sheet.set_cell(pos, &raw);
                }
                mark_dirty(&mut self.sheet, &self.deps, pos);
                recalculate(&mut self.sheet, &self.deps);
                self.dirty = true;
            }
            Action::ChangeMode(mode) => {
                if mode == Mode::Insert {
                    // Load current cell content into insert buffer
                    self.insert_buffer = self.sheet.get_cell(self.cursor)
                        .map(|c| c.raw.clone())
                        .unwrap_or_default();
                }
                self.mode = mode;
            }
            Action::Quit { force } => {
                if !force && self.dirty {
                    self.status_message = Some(
                        "No write since last change (use :q! to override)".into()
                    );
                } else {
                    self.should_quit = true;
                }
            }
            Action::ClearCell(pos) => {
                let old_raw = self.sheet.get_cell(pos)
                    .map(|c| c.raw.clone())
                    .unwrap_or_default();
                if !old_raw.is_empty() {
                    self.undo_stack.push(UndoEntry::CellEdit {
                        pos,
                        old_raw,
                        new_raw: String::new(),
                    });
                    self.sheet.clear_cell(pos);
                    self.dirty = true;
                }
            }
            Action::GotoFirstRow => {
                self.cursor = (0, self.cursor.1);
                self.viewport.ensure_visible(self.cursor);
            }
            Action::GotoLastRow => {
                let last = if self.sheet.row_count > 0 { self.sheet.row_count - 1 } else { 0 };
                self.cursor = (last, self.cursor.1);
                self.viewport.ensure_visible(self.cursor);
            }
            Action::GotoFirstCol => {
                self.cursor = (self.cursor.0, 0);
                self.viewport.ensure_visible(self.cursor);
            }
            Action::GotoLastCol => {
                let last = if self.sheet.col_count > 0 { self.sheet.col_count - 1 } else { 0 };
                self.cursor = (self.cursor.0, last);
                self.viewport.ensure_visible(self.cursor);
            }
            Action::HalfPageDown => {
                let half = self.viewport.visible_rows / 2;
                self.cursor.0 += half;
                self.viewport.ensure_visible(self.cursor);
            }
            Action::HalfPageUp => {
                let half = self.viewport.visible_rows / 2;
                self.cursor.0 = self.cursor.0.saturating_sub(half);
                self.viewport.ensure_visible(self.cursor);
            }
            Action::PageDown => {
                self.cursor.0 += self.viewport.visible_rows;
                self.viewport.ensure_visible(self.cursor);
            }
            Action::PageUp => {
                self.cursor.0 = self.cursor.0.saturating_sub(self.viewport.visible_rows);
                self.viewport.ensure_visible(self.cursor);
            }
            Action::Undo => {
                if let Some(entry) = self.undo_stack.undo() {
                    self.apply_undo_entry(&entry, false);
                    self.dirty = true;
                }
            }
            Action::Redo => {
                if let Some(entry) = self.undo_stack.redo() {
                    self.apply_undo_entry(&entry, true);
                    self.dirty = true;
                }
            }
            _ => {
                // Remaining actions (Save, Search, Sort, etc.) will be
                // implemented in subsequent tasks
            }
        }
    }

    fn apply_undo_entry(&mut self, entry: &UndoEntry, redo: bool) {
        match entry {
            UndoEntry::CellEdit { pos, old_raw, new_raw } => {
                let raw = if redo { new_raw } else { old_raw };
                if raw.is_empty() {
                    self.sheet.clear_cell(*pos);
                } else if raw.starts_with('=') {
                    set_formula(&mut self.sheet, &mut self.deps, *pos, raw);
                } else {
                    self.sheet.set_cell(*pos, raw);
                }
                recalculate(&mut self.sheet, &self.deps);
            }
        }
    }
}
```

- [ ] **Step 4: Create placeholder modules for clipboard and undo**

Create `crates/cell-tui/src/clipboard.rs`:

```rust
use cell_core::model::CellPos;

#[derive(Debug, Clone)]
pub enum Register {
    Cell(String),
    Row(Vec<String>),
    Block(Vec<Vec<String>>),
}
```

Create `crates/cell-tui/src/undo.rs`:

```rust
use cell_core::model::CellPos;

#[derive(Debug, Clone)]
pub enum UndoEntry {
    CellEdit {
        pos: CellPos,
        old_raw: String,
        new_raw: String,
    },
}

pub struct UndoStack {
    undo: Vec<UndoEntry>,
    redo: Vec<UndoEntry>,
}

impl UndoStack {
    pub fn new() -> Self {
        UndoStack { undo: Vec::new(), redo: Vec::new() }
    }

    pub fn push(&mut self, entry: UndoEntry) {
        self.undo.push(entry);
        self.redo.clear();
    }

    pub fn undo(&mut self) -> Option<UndoEntry> {
        let entry = self.undo.pop()?;
        self.redo.push(entry.clone());
        Some(entry)
    }

    pub fn redo(&mut self) -> Option<UndoEntry> {
        let entry = self.redo.pop()?;
        self.undo.push(entry.clone());
        Some(entry)
    }
}
```

- [ ] **Step 5: Update main.rs with CLI parsing and basic terminal setup**

Replace `crates/cell-tui/src/main.rs`:

```rust
mod app;
mod action;
mod viewport;
mod clipboard;
mod undo;

use std::io;
use std::path::PathBuf;
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    execute,
};
use ratatui::{Terminal, backend::CrosstermBackend};
use app::{App, FileFormat};
use action::{Action, Mode, Direction};

#[derive(Parser)]
#[command(name = "cell", version, about = "A terminal spreadsheet editor")]
struct Cli {
    /// File to open (CSV, TSV, or .cell)
    file: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let mut app = App::new();

    // Load file if specified
    if let Some(path) = &cli.file {
        load_file(&mut app, path)?;
    }

    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Main loop
    let result = run_loop(&mut terminal, &mut app);

    // Cleanup
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    result
}

fn load_file(app: &mut App, path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "csv" => {
            let file = std::fs::File::open(path)?;
            app.sheet = cell_core::io::csv::read_csv(file, b',')?;
            app.file_format = FileFormat::Csv;
        }
        "tsv" => {
            let file = std::fs::File::open(path)?;
            app.sheet = cell_core::io::csv::read_csv(file, b'\t')?;
            app.file_format = FileFormat::Tsv;
        }
        "cell" => {
            let file = std::fs::File::open(path)?;
            app.sheet = cell_core::io::cell_format::read_cell_format(file)?;
            app.file_format = FileFormat::Cell;
        }
        _ => {
            // Default to CSV
            let file = std::fs::File::open(path)?;
            app.sheet = cell_core::io::csv::read_csv(file, b',')?;
            app.file_format = FileFormat::Csv;
        }
    }
    app.file_path = Some(path.clone());
    Ok(())
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        // Render
        terminal.draw(|frame| {
            // Placeholder: just clear the screen for now
            let area = frame.area();
            let block = ratatui::widgets::Block::default()
                .title(format!(" cell — {:?} ", app.mode));
            frame.render_widget(block, area);
        })?;

        // Handle input
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                let action = match app.mode {
                    Mode::Normal => match key.code {
                        KeyCode::Char('q') => Action::Quit { force: false },
                        KeyCode::Char('h') => Action::MoveCursor(Direction::Left),
                        KeyCode::Char('j') => Action::MoveCursor(Direction::Down),
                        KeyCode::Char('k') => Action::MoveCursor(Direction::Up),
                        KeyCode::Char('l') => Action::MoveCursor(Direction::Right),
                        _ => Action::Noop,
                    },
                    _ => Action::Noop,
                };
                app.process_action(action);
            }
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}
```

- [ ] **Step 6: Verify it compiles and runs**

Run: `cargo build`
Expected: Compiles.

Run: `cargo run --bin cell`
Expected: Opens alternate screen, shows blank window with "cell — Normal", press `q` to exit cleanly.

- [ ] **Step 7: Commit**

```bash
git add crates/cell-tui/
git commit -m "feat: scaffold TUI app with state, actions, basic event loop"
```

---

## Task 10: Normal Mode Key Handling

**Files:**
- Create: `crates/cell-tui/src/mode/mod.rs`
- Create: `crates/cell-tui/src/mode/normal.rs`
- Modify: `crates/cell-tui/src/main.rs`

- [ ] **Step 1: Write failing tests for normal mode handler**

Create `crates/cell-tui/src/mode/normal.rs`:

```rust
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::action::{Action, Direction, Mode};
use crate::app::App;

/// Buffer for multi-key sequences like "gg", "dd", "yy".
pub struct NormalState {
    pub pending: Option<char>,
}

impl NormalState {
    pub fn new() -> Self {
        NormalState { pending: None }
    }

    pub fn handle_key(&mut self, key: KeyEvent, app: &App) -> Action {
        // Handle Ctrl combinations first (before pending sequences)
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return match key.code {
                KeyCode::Char('d') => Action::HalfPageDown,
                KeyCode::Char('u') => Action::HalfPageUp,
                KeyCode::Char('f') => Action::PageDown,
                KeyCode::Char('b') => Action::PageUp,
                KeyCode::Char('r') => Action::Redo,
                KeyCode::Char('v') => Action::ChangeMode(Mode::VisualBlock),
                _ => Action::Noop,
            };
        }

        // Handle pending sequences
        if let Some(prev) = self.pending.take() {
            return match (prev, key.code) {
                ('g', KeyCode::Char('g')) => Action::GotoFirstRow,
                ('d', KeyCode::Char('d')) => Action::DeleteRow(app.cursor.0),
                ('y', KeyCode::Char('y')) => Action::YankRow(app.cursor.0),
                _ => Action::Noop,
            };
        }

        match key.code {
            // Navigation
            KeyCode::Char('h') | KeyCode::Left => Action::MoveCursor(Direction::Left),
            KeyCode::Char('j') | KeyCode::Down => Action::MoveCursor(Direction::Down),
            KeyCode::Char('k') | KeyCode::Up => Action::MoveCursor(Direction::Up),
            KeyCode::Char('l') | KeyCode::Right => Action::MoveCursor(Direction::Right),

            // Multi-key starters
            KeyCode::Char('g') => { self.pending = Some('g'); Action::Noop }
            KeyCode::Char('d') => { self.pending = Some('d'); Action::Noop }
            KeyCode::Char('y') => { self.pending = Some('y'); Action::Noop }

            KeyCode::Char('G') => Action::GotoLastRow,
            KeyCode::Char('0') => Action::GotoFirstCol,
            KeyCode::Char('$') => Action::GotoLastCol,

            // Jump to next/prev non-empty cell
            KeyCode::Char('w') => Action::NextNonEmpty,
            KeyCode::Char('b') => Action::PrevNonEmpty,

            // Mode changes
            KeyCode::Char('i') | KeyCode::Char('a') => Action::ChangeMode(Mode::Insert),
            KeyCode::Char('o') => Action::ChangeMode(Mode::Insert), // edit cell below — app handles moving cursor down
            KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::ChangeMode(Mode::VisualBlock),
            KeyCode::Char('v') => Action::ChangeMode(Mode::Visual),
            KeyCode::Char(':') => Action::ChangeMode(Mode::Command),

            // Cell operations
            KeyCode::Char('x') => Action::ClearCell(app.cursor),
            KeyCode::Char('p') => Action::Paste(app.cursor),
            KeyCode::Char('P') => Action::PasteBefore(app.cursor),
            KeyCode::Char('u') => Action::Undo,

            // Search
            KeyCode::Char('/') => Action::ChangeMode(Mode::Command), // will use command mode for search
            KeyCode::Char('n') => Action::SearchNext,
            KeyCode::Char('N') => Action::SearchPrev,

            // Enter edits the cell (common spreadsheet convention)
            KeyCode::Enter => Action::ChangeMode(Mode::Insert),

            _ => Action::Noop,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyEventKind, KeyEventState};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl_key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    #[test]
    fn hjkl_navigation() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(state.handle_key(key(KeyCode::Char('h')), &app), Action::MoveCursor(Direction::Left));
        assert_eq!(state.handle_key(key(KeyCode::Char('j')), &app), Action::MoveCursor(Direction::Down));
        assert_eq!(state.handle_key(key(KeyCode::Char('k')), &app), Action::MoveCursor(Direction::Up));
        assert_eq!(state.handle_key(key(KeyCode::Char('l')), &app), Action::MoveCursor(Direction::Right));
    }

    #[test]
    fn gg_goes_to_first_row() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(state.handle_key(key(KeyCode::Char('g')), &app), Action::Noop);
        assert_eq!(state.handle_key(key(KeyCode::Char('g')), &app), Action::GotoFirstRow);
    }

    #[test]
    fn shift_g_goes_to_last_row() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(state.handle_key(key(KeyCode::Char('G')), &app), Action::GotoLastRow);
    }

    #[test]
    fn dd_deletes_row() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(state.handle_key(key(KeyCode::Char('d')), &app), Action::Noop);
        assert_eq!(state.handle_key(key(KeyCode::Char('d')), &app), Action::DeleteRow(0));
    }

    #[test]
    fn yy_yanks_row() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(state.handle_key(key(KeyCode::Char('y')), &app), Action::Noop);
        assert_eq!(state.handle_key(key(KeyCode::Char('y')), &app), Action::YankRow(0));
    }

    #[test]
    fn i_enters_insert_mode() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(state.handle_key(key(KeyCode::Char('i')), &app), Action::ChangeMode(Mode::Insert));
    }

    #[test]
    fn colon_enters_command_mode() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(state.handle_key(key(KeyCode::Char(':')), &app), Action::ChangeMode(Mode::Command));
    }

    #[test]
    fn x_clears_cell() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(state.handle_key(key(KeyCode::Char('x')), &app), Action::ClearCell((0, 0)));
    }

    #[test]
    fn ctrl_d_half_page_down() {
        let app = App::new();
        let mut state = NormalState::new();
        assert_eq!(state.handle_key(ctrl_key('d'), &app), Action::HalfPageDown);
    }
}
```

Create `crates/cell-tui/src/mode/mod.rs`:
```rust
pub mod normal;
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p cell-tui`
Expected: Compilation errors until we wire up the modules.

- [ ] **Step 3: Wire up the normal mode handler in main.rs**

Update `main.rs` to import and use the mode module. Replace the inline key handling in `run_loop` with:

```rust
mod mode;

// In run_loop, replace the input handling block:
use mode::normal::NormalState;

// Add NormalState to the function:
fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut normal_state = NormalState::new();

    loop {
        terminal.draw(|frame| {
            let area = frame.area();
            let block = ratatui::widgets::Block::default()
                .title(format!(" cell — {:?} ", app.mode));
            frame.render_widget(block, area);
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                let action = match app.mode {
                    Mode::Normal => normal_state.handle_key(key, app),
                    _ => Action::Noop,
                };
                app.process_action(action);
            }
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p cell-tui`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/cell-tui/src/
git commit -m "feat: implement normal mode key handler with hjkl, gg, dd, yy"
```

---

## Task 11: Insert and Command Mode Handlers

**Files:**
- Create: `crates/cell-tui/src/mode/insert.rs`
- Create: `crates/cell-tui/src/mode/command.rs`
- Modify: `crates/cell-tui/src/mode/mod.rs`

- [ ] **Step 1: Write failing tests for insert mode**

Create `crates/cell-tui/src/mode/insert.rs`:

```rust
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::action::{Action, Mode};
use crate::app::App;

pub fn handle_insert_key(key: KeyEvent, app: &App) -> Action {
    match key.code {
        KeyCode::Esc => {
            // Confirm the edit and return to normal mode
            let new_raw = app.insert_buffer.clone();
            Action::EditCell(app.cursor, new_raw)
            // The caller should also send ChangeMode(Normal) after
        }
        KeyCode::Enter => {
            let new_raw = app.insert_buffer.clone();
            Action::EditCell(app.cursor, new_raw)
        }
        _ => Action::Noop, // Character input handled by App directly
    }
}

/// Returns the character to insert, or None for non-character keys.
pub fn handle_insert_char(key: KeyEvent) -> Option<InsertAction> {
    match key.code {
        KeyCode::Char(c) => Some(InsertAction::InsertChar(c)),
        KeyCode::Backspace => Some(InsertAction::Backspace),
        KeyCode::Delete => Some(InsertAction::Delete),
        KeyCode::Left => Some(InsertAction::CursorLeft),
        KeyCode::Right => Some(InsertAction::CursorRight),
        KeyCode::Home => Some(InsertAction::CursorHome),
        KeyCode::End => Some(InsertAction::CursorEnd),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum InsertAction {
    InsertChar(char),
    Backspace,
    Delete,
    CursorLeft,
    CursorRight,
    CursorHome,
    CursorEnd,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn esc_confirms_edit() {
        let mut app = App::new();
        app.insert_buffer = "hello".into();
        let action = handle_insert_key(key(KeyCode::Esc), &app);
        assert_eq!(action, Action::EditCell((0, 0), "hello".into()));
    }

    #[test]
    fn enter_confirms_edit() {
        let mut app = App::new();
        app.insert_buffer = "42".into();
        let action = handle_insert_key(key(KeyCode::Enter), &app);
        assert_eq!(action, Action::EditCell((0, 0), "42".into()));
    }

    #[test]
    fn char_input() {
        assert_eq!(handle_insert_char(key(KeyCode::Char('a'))), Some(InsertAction::InsertChar('a')));
    }

    #[test]
    fn backspace() {
        assert_eq!(handle_insert_char(key(KeyCode::Backspace)), Some(InsertAction::Backspace));
    }
}
```

- [ ] **Step 2: Write failing tests for command mode**

Create `crates/cell-tui/src/mode/command.rs`:

```rust
use std::path::PathBuf;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::action::{Action, Mode, SearchDirection};

pub fn parse_command(input: &str) -> Action {
    let input = input.trim();

    if input == "q" {
        Action::Quit { force: false }
    } else if input == "q!" {
        Action::Quit { force: true }
    } else if input == "w" {
        Action::Save(None)
    } else if input == "wq" {
        // Save then quit — handled by app as two actions
        Action::Save(None) // App will also quit after successful save
    } else if input.starts_with("w ") {
        let path = input[2..].trim();
        Action::Save(Some(PathBuf::from(path)))
    } else if input.starts_with("w! ") {
        let path = input[3..].trim();
        Action::ForceSave(Some(PathBuf::from(path)))
    } else if input == "w!" {
        Action::ForceSave(None)
    } else if input.starts_with("e ") {
        let path = input[2..].trim();
        Action::Open(PathBuf::from(path))
    } else if input.starts_with("sort ") {
        parse_sort_command(&input[5..])
    } else {
        Action::Noop
    }
}

fn parse_sort_command(args: &str) -> Action {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.is_empty() {
        return Action::Noop;
    }
    let col = cell_core::model::col_label_to_index(parts[0]).unwrap_or(0);
    let ascending = parts.get(1).map(|&s| s != "desc").unwrap_or(true);
    Action::Sort { col, ascending }
}

pub fn handle_command_key(key: KeyEvent, command_buffer: &str) -> CommandAction {
    match key.code {
        KeyCode::Esc => CommandAction::Cancel,
        KeyCode::Enter => CommandAction::Execute(command_buffer.to_string()),
        KeyCode::Backspace => CommandAction::Backspace,
        KeyCode::Char(c) => CommandAction::InsertChar(c),
        _ => CommandAction::Noop,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommandAction {
    Noop,
    InsertChar(char),
    Backspace,
    Execute(String),
    Cancel,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_quit() {
        assert_eq!(parse_command("q"), Action::Quit { force: false });
    }

    #[test]
    fn parse_force_quit() {
        assert_eq!(parse_command("q!"), Action::Quit { force: true });
    }

    #[test]
    fn parse_write() {
        assert_eq!(parse_command("w"), Action::Save(None));
    }

    #[test]
    fn parse_write_path() {
        assert_eq!(parse_command("w foo.csv"), Action::Save(Some(PathBuf::from("foo.csv"))));
    }

    #[test]
    fn parse_force_write() {
        assert_eq!(parse_command("w!"), Action::ForceSave(None));
    }

    #[test]
    fn parse_edit() {
        assert_eq!(parse_command("e data.csv"), Action::Open(PathBuf::from("data.csv")));
    }

    #[test]
    fn parse_sort_asc() {
        assert_eq!(parse_command("sort A asc"), Action::Sort { col: 0, ascending: true });
    }

    #[test]
    fn parse_sort_desc() {
        assert_eq!(parse_command("sort B desc"), Action::Sort { col: 1, ascending: false });
    }

    #[test]
    fn parse_wq() {
        assert_eq!(parse_command("wq"), Action::Save(None));
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn command_esc_cancels() {
        assert_eq!(handle_command_key(key(KeyCode::Esc), ""), CommandAction::Cancel);
    }

    #[test]
    fn command_enter_executes() {
        assert_eq!(handle_command_key(key(KeyCode::Enter), "w"), CommandAction::Execute("w".into()));
    }

    #[test]
    fn command_char_inserts() {
        assert_eq!(handle_command_key(key(KeyCode::Char('q')), ""), CommandAction::InsertChar('q'));
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p cell-tui`
Expected: Compilation errors.

- [ ] **Step 4: Update mode/mod.rs**

```rust
pub mod normal;
pub mod insert;
pub mod command;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p cell-tui`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/cell-tui/src/mode/
git commit -m "feat: implement insert and command mode handlers"
```

---

## Task 12: Visual Mode Handler

**Files:**
- Create: `crates/cell-tui/src/mode/visual.rs`
- Modify: `crates/cell-tui/src/mode/mod.rs`

- [ ] **Step 1: Write failing tests for visual mode**

Create `crates/cell-tui/src/mode/visual.rs`:

```rust
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use cell_core::model::CellPos;
use crate::action::{Action, Direction, Mode};
use crate::app::App;

pub struct VisualState {
    pub anchor: CellPos,
    pub is_block: bool,
}

impl VisualState {
    pub fn new(anchor: CellPos, is_block: bool) -> Self {
        VisualState { anchor, is_block }
    }

    /// Returns the normalized selection range (top-left, bottom-right).
    pub fn selection(&self, cursor: CellPos) -> (CellPos, CellPos) {
        let r1 = self.anchor.0.min(cursor.0);
        let r2 = self.anchor.0.max(cursor.0);
        let c1 = self.anchor.1.min(cursor.1);
        let c2 = self.anchor.1.max(cursor.1);

        if self.is_block {
            // Block selection: rectangle from anchor to cursor
            ((r1, c1), (r2, c2))
        } else {
            // Linear selection: all columns from start row to end row
            // For simplicity in v1, treat as rectangular too
            ((r1, c1), (r2, c2))
        }
    }

    pub fn handle_key(&self, key: KeyEvent, app: &App) -> Action {
        let (start, end) = self.selection(app.cursor);

        match key.code {
            // Navigation
            KeyCode::Char('h') | KeyCode::Left => Action::MoveCursor(Direction::Left),
            KeyCode::Char('j') | KeyCode::Down => Action::MoveCursor(Direction::Down),
            KeyCode::Char('k') | KeyCode::Up => Action::MoveCursor(Direction::Up),
            KeyCode::Char('l') | KeyCode::Right => Action::MoveCursor(Direction::Right),

            // Operations on selection
            KeyCode::Char('d') => Action::ClearRange { start, end },
            KeyCode::Char('y') => Action::YankRange { start, end },

            // Exit visual mode
            KeyCode::Esc => Action::ChangeMode(Mode::Normal),

            _ => Action::Noop,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn selection_normalized() {
        let state = VisualState::new((2, 3), false);
        let (start, end) = state.selection((0, 1));
        assert_eq!(start, (0, 1));
        assert_eq!(end, (2, 3));
    }

    #[test]
    fn selection_same_cell() {
        let state = VisualState::new((1, 1), false);
        let (start, end) = state.selection((1, 1));
        assert_eq!(start, (1, 1));
        assert_eq!(end, (1, 1));
    }

    #[test]
    fn hjkl_in_visual() {
        let app = App::new();
        let state = VisualState::new((0, 0), false);
        assert_eq!(state.handle_key(key(KeyCode::Char('j')), &app), Action::MoveCursor(Direction::Down));
    }

    #[test]
    fn d_clears_range() {
        let mut app = App::new();
        app.cursor = (2, 2);
        let state = VisualState::new((0, 0), false);
        let action = state.handle_key(key(KeyCode::Char('d')), &app);
        assert_eq!(action, Action::ClearRange { start: (0, 0), end: (2, 2) });
    }

    #[test]
    fn y_yanks_range() {
        let mut app = App::new();
        app.cursor = (1, 1);
        let state = VisualState::new((0, 0), false);
        let action = state.handle_key(key(KeyCode::Char('y')), &app);
        assert_eq!(action, Action::YankRange { start: (0, 0), end: (1, 1) });
    }

    #[test]
    fn esc_exits_visual() {
        let app = App::new();
        let state = VisualState::new((0, 0), false);
        assert_eq!(state.handle_key(key(KeyCode::Esc), &app), Action::ChangeMode(Mode::Normal));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p cell-tui`
Expected: Compilation errors.

- [ ] **Step 3: Update mode/mod.rs**

```rust
pub mod normal;
pub mod insert;
pub mod command;
pub mod visual;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p cell-tui`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/cell-tui/src/mode/
git commit -m "feat: implement visual mode with range selection"
```

---

## Task 13: Grid Rendering

**Files:**
- Create: `crates/cell-tui/src/render/mod.rs`
- Create: `crates/cell-tui/src/render/grid.rs`
- Create: `crates/cell-tui/src/render/formula_bar.rs`
- Create: `crates/cell-tui/src/render/status_bar.rs`
- Create: `crates/cell-tui/src/render/command_line.rs`

- [ ] **Step 1: Implement the formula bar widget**

Create `crates/cell-tui/src/render/formula_bar.rs`:

```rust
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Widget,
};
use cell_core::model::{col_index_to_label, CellPos};

pub struct FormulaBar<'a> {
    pub cursor: CellPos,
    pub content: &'a str,
    pub is_editing: bool,
    pub cursor_pos: usize,
}

impl<'a> Widget for FormulaBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        // Cell address (e.g., "A1")
        let addr = format!(" {}{} ",
            col_index_to_label(self.cursor.1),
            self.cursor.0 + 1
        );
        let addr_width = addr.len() as u16;

        // Render address with highlight
        let addr_style = Style::default().fg(Color::Black).bg(Color::White);
        buf.set_string(area.x, area.y, &addr, addr_style);

        // Separator
        let sep = " │ ";
        buf.set_string(area.x + addr_width, area.y, sep, Style::default());

        // Content
        let content_x = area.x + addr_width + sep.len() as u16;
        let content_width = area.width.saturating_sub(addr_width + sep.len() as u16);
        let content = if self.content.len() > content_width as usize {
            &self.content[..content_width as usize]
        } else {
            self.content
        };
        buf.set_string(content_x, area.y, content, Style::default());
    }
}
```

- [ ] **Step 2: Implement the status bar widget**

Create `crates/cell-tui/src/render/status_bar.rs`:

```rust
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style, Modifier},
    widgets::Widget,
};
use cell_core::model::CellPos;
use crate::action::Mode;

pub struct StatusBar<'a> {
    pub mode: Mode,
    pub row_count: usize,
    pub col_count: usize,
    pub cursor: CellPos,
    pub dirty: bool,
    pub file_name: Option<&'a str>,
    pub message: Option<&'a str>,
}

impl<'a> Widget for StatusBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let style = Style::default().fg(Color::Black).bg(Color::White);

        // Fill background
        for x in area.x..area.x + area.width {
            buf.set_string(x, area.y, " ", style);
        }

        // Mode indicator
        let mode_str = match self.mode {
            Mode::Normal => " NORMAL ",
            Mode::Insert => " INSERT ",
            Mode::Visual => " VISUAL ",
            Mode::VisualBlock => " V-BLOCK ",
            Mode::Command => " COMMAND ",
        };
        let mode_style = Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD);
        buf.set_string(area.x, area.y, mode_str, mode_style);

        // If there's a message, show it
        if let Some(msg) = self.message {
            let msg_x = area.x + mode_str.len() as u16 + 1;
            let msg_style = Style::default().fg(Color::Red).bg(Color::White);
            buf.set_string(msg_x, area.y, msg, msg_style);
            return;
        }

        // File info
        let file_info = match self.file_name {
            Some(name) => {
                if self.dirty { format!(" {} [+]", name) } else { format!(" {}", name) }
            }
            None => {
                if self.dirty { " [No Name] [+]".into() } else { " [No Name]".into() }
            }
        };
        let info_x = area.x + mode_str.len() as u16;
        buf.set_string(info_x, area.y, &file_info, style);

        // Right side: dimensions and cursor position
        let right = format!(
            "{} rows x {} cols │ {}{} ",
            self.row_count,
            self.col_count,
            cell_core::model::col_index_to_label(self.cursor.1),
            self.cursor.0 + 1,
        );
        let right_x = area.x + area.width - right.len() as u16;
        if right_x > info_x + file_info.len() as u16 {
            buf.set_string(right_x, area.y, &right, style);
        }
    }
}
```

- [ ] **Step 3: Implement the command line widget**

Create `crates/cell-tui/src/render/command_line.rs`:

```rust
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::Widget,
};

pub struct CommandLine<'a> {
    pub content: &'a str,
    pub prefix: char, // ':' or '/'
    pub active: bool,
}

impl<'a> Widget for CommandLine<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || !self.active {
            return;
        }

        let display = format!("{}{}", self.prefix, self.content);
        buf.set_string(area.x, area.y, &display, Style::default());
    }
}
```

- [ ] **Step 4: Implement the grid widget**

Create `crates/cell-tui/src/render/grid.rs`:

```rust
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style, Modifier},
    widgets::Widget,
};
use cell_core::model::{Sheet, CellPos, CellValue, col_index_to_label};
use crate::viewport::Viewport;

pub struct Grid<'a> {
    pub sheet: &'a Sheet,
    pub viewport: &'a Viewport,
    pub cursor: CellPos,
    pub selection: Option<(CellPos, CellPos)>,
}

const ROW_NUM_WIDTH: u16 = 5;
const DEFAULT_COL_WIDTH: u16 = 10;

impl<'a> Grid<'a> {
    fn col_width(&self, col: usize) -> u16 {
        self.sheet.col_widths.get(col).copied().unwrap_or(DEFAULT_COL_WIDTH)
    }
}

impl<'a> Widget for Grid<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 2 || area.width < ROW_NUM_WIDTH + 2 {
            return;
        }

        let header_style = Style::default().fg(Color::Black).bg(Color::DarkGray);
        let cursor_style = Style::default().fg(Color::Black).bg(Color::Yellow);
        let selection_style = Style::default().fg(Color::Black).bg(Color::Blue);
        let normal_style = Style::default();

        // Draw column headers
        let mut x = area.x + ROW_NUM_WIDTH + 1;
        let mut visible_cols = Vec::new();
        for col in self.viewport.col_offset.. {
            if x >= area.x + area.width {
                break;
            }
            let w = self.col_width(col);
            let label = col_index_to_label(col);
            let display = format!("{:^width$}", label, width = w as usize);
            let truncated = &display[..display.len().min((area.x + area.width - x) as usize)];
            buf.set_string(x, area.y, truncated, header_style);
            visible_cols.push((col, x, w));
            x += w + 1; // +1 for separator
        }

        // Draw rows
        for row_offset in 0..area.height.saturating_sub(1) {
            let row = self.viewport.row_offset + row_offset as usize;
            let y = area.y + 1 + row_offset;

            if y >= area.y + area.height {
                break;
            }

            // Row number
            let row_num = format!("{:>width$}", row + 1, width = ROW_NUM_WIDTH as usize);
            buf.set_string(area.x, y, &row_num, header_style);

            // Cell values
            for &(col, col_x, col_w) in &visible_cols {
                let pos = (row, col);
                let is_cursor = pos == self.cursor;
                let is_selected = self.selection.map_or(false, |(start, end)| {
                    row >= start.0 && row <= end.0 && col >= start.1 && col <= end.1
                });

                let style = if is_cursor {
                    cursor_style
                } else if is_selected {
                    selection_style
                } else {
                    normal_style
                };

                // Get cell display value
                let display_val = match self.sheet.get_cell(pos) {
                    Some(cell) => cell.value.to_string(),
                    None => String::new(),
                };

                // Align: numbers right, text left
                let is_number = matches!(
                    self.sheet.get_cell(pos).map(|c| &c.value),
                    Some(CellValue::Number(_))
                );

                let formatted = if is_number {
                    format!("{:>width$}", display_val, width = col_w as usize)
                } else {
                    format!("{:<width$}", display_val, width = col_w as usize)
                };

                // Truncate if needed
                let max_chars = (area.x + area.width).saturating_sub(col_x) as usize;
                let truncated = if formatted.len() > col_w as usize {
                    let mut s = formatted[..col_w as usize - 1].to_string();
                    s.push('…');
                    s
                } else {
                    formatted
                };
                let truncated = &truncated[..truncated.len().min(max_chars)];

                // Fill cell background
                for cx in 0..col_w.min(area.x + area.width - col_x) {
                    buf.set_string(col_x + cx, y, " ", style);
                }
                buf.set_string(col_x, y, truncated, style);
            }
        }
    }
}
```

- [ ] **Step 5: Create the top-level render function**

Create `crates/cell-tui/src/render/mod.rs`:

```rust
pub mod grid;
pub mod formula_bar;
pub mod status_bar;
pub mod command_line;

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Direction},
};
use crate::app::App;
use crate::action::Mode;
use grid::Grid;
use formula_bar::FormulaBar;
use status_bar::StatusBar;
use command_line::CommandLine;

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // formula bar
            Constraint::Fill(1),   // grid
            Constraint::Length(1),  // status bar
            Constraint::Length(1),  // command line
        ])
        .split(area);

    // Update viewport visible dimensions
    // (We can't mutate app here, so this is done before render in the loop)

    // Formula bar
    let cell_content = app.sheet.get_cell(app.cursor)
        .map(|c| c.raw.as_str())
        .unwrap_or("");
    let display_content = if app.mode == Mode::Insert {
        &app.insert_buffer
    } else {
        cell_content
    };
    frame.render_widget(
        FormulaBar {
            cursor: app.cursor,
            content: display_content,
            is_editing: app.mode == Mode::Insert,
            cursor_pos: 0,
        },
        chunks[0],
    );

    // Grid
    frame.render_widget(
        Grid {
            sheet: &app.sheet,
            viewport: &app.viewport,
            cursor: app.cursor,
            selection: None, // TODO: pass from visual state
        },
        chunks[1],
    );

    // Status bar
    let file_name = app.file_path.as_ref()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str());
    frame.render_widget(
        StatusBar {
            mode: app.mode,
            row_count: app.sheet.row_count,
            col_count: app.sheet.col_count,
            cursor: app.cursor,
            dirty: app.dirty,
            file_name,
            message: app.status_message.as_deref(),
        },
        chunks[2],
    );

    // Command line
    let is_command = app.mode == Mode::Command;
    frame.render_widget(
        CommandLine {
            content: &app.command_line,
            prefix: ':',
            active: is_command,
        },
        chunks[3],
    );
}
```

- [ ] **Step 6: Wire rendering into main.rs**

Update the `run_loop` in `main.rs` to use the render module:

```rust
mod render;

// Replace the terminal.draw closure:
terminal.draw(|frame| {
    // Update viewport dimensions from terminal size
    let grid_height = frame.area().height.saturating_sub(3) as usize; // minus formula bar, status, command
    app.viewport.visible_rows = grid_height;
    render::render(frame, app);
})?;
```

Note: Since we can't mutate `app` inside the draw closure, move the viewport update before the draw call:

```rust
let grid_height = terminal.size()?.height.saturating_sub(3) as usize;
app.viewport.visible_rows = grid_height;
terminal.draw(|frame| {
    render::render(frame, app);
})?;
```

- [ ] **Step 7: Verify it compiles and runs**

Run: `cargo build`
Expected: Compiles.

Create a test CSV and run:
```bash
echo -e "Name,Score\nAlice,95\nBob,88" > /tmp/test.csv
cargo run --bin cell -- /tmp/test.csv
```
Expected: Shows the grid with headers, data, formula bar, status bar. `hjkl` navigates, `q` quits.

- [ ] **Step 8: Commit**

```bash
git add crates/cell-tui/src/
git commit -m "feat: implement TUI rendering — grid, formula bar, status bar, command line"
```

---

## Task 14: Wire All Modes Into Event Loop

**Files:**
- Modify: `crates/cell-tui/src/main.rs`

- [ ] **Step 1: Integrate all mode handlers**

Replace the `run_loop` function in `main.rs` with a complete implementation:

```rust
fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<(), Box<dyn std::error::Error>> {
    use mode::normal::NormalState;
    use mode::insert::{handle_insert_key, handle_insert_char, InsertAction};
    use mode::command::{handle_command_key, parse_command, CommandAction};
    use mode::visual::VisualState;

    let mut normal_state = NormalState::new();
    let mut visual_state: Option<VisualState> = None;
    let mut insert_cursor: usize = 0; // cursor position within insert buffer
    let mut search_mode = false; // true if command line was entered via '/'
    let mut wq_pending = false; // true if :wq was issued and save succeeded

    loop {
        // Update viewport
        let grid_height = terminal.size()?.height.saturating_sub(3) as usize;
        app.viewport.visible_rows = grid_height;

        // Render
        terminal.draw(|frame| {
            render::render(frame, app);
        })?;

        // Handle input
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Clear status message on any key press
                app.status_message = None;

                let action = match app.mode {
                    Mode::Normal => normal_state.handle_key(key, app),
                    Mode::Insert => {
                        match key.code {
                            KeyCode::Esc | KeyCode::Enter => {
                                let edit_action = handle_insert_key(key, app);
                                app.process_action(edit_action);
                                Action::ChangeMode(Mode::Normal)
                            }
                            _ => {
                                if let Some(insert_action) = handle_insert_char(key) {
                                    match insert_action {
                                        InsertAction::InsertChar(c) => {
                                            app.insert_buffer.insert(insert_cursor, c);
                                            insert_cursor += 1;
                                        }
                                        InsertAction::Backspace => {
                                            if insert_cursor > 0 {
                                                insert_cursor -= 1;
                                                app.insert_buffer.remove(insert_cursor);
                                            }
                                        }
                                        InsertAction::Delete => {
                                            if insert_cursor < app.insert_buffer.len() {
                                                app.insert_buffer.remove(insert_cursor);
                                            }
                                        }
                                        InsertAction::CursorLeft => {
                                            insert_cursor = insert_cursor.saturating_sub(1);
                                        }
                                        InsertAction::CursorRight => {
                                            insert_cursor = (insert_cursor + 1).min(app.insert_buffer.len());
                                        }
                                        InsertAction::CursorHome => { insert_cursor = 0; }
                                        InsertAction::CursorEnd => { insert_cursor = app.insert_buffer.len(); }
                                    }
                                }
                                Action::Noop
                            }
                        }
                    }
                    Mode::Visual | Mode::VisualBlock => {
                        if let Some(ref vs) = visual_state {
                            let action = vs.handle_key(key, app);
                            if action == Action::ChangeMode(Mode::Normal) {
                                visual_state = None;
                            }
                            action
                        } else {
                            Action::ChangeMode(Mode::Normal)
                        }
                    }
                    Mode::Command => {
                        let cmd_action = handle_command_key(key, &app.command_line);
                        match cmd_action {
                            CommandAction::InsertChar(c) => {
                                app.command_line.push(c);
                                Action::Noop
                            }
                            CommandAction::Backspace => {
                                app.command_line.pop();
                                Action::Noop
                            }
                            CommandAction::Execute(cmd) => {
                                if search_mode {
                                    search_mode = false;
                                    let pattern = app.command_line.clone();
                                    app.command_line.clear();
                                    app.search_pattern = Some(pattern.clone());
                                    Action::ChangeMode(Mode::Normal)
                                } else {
                                    let is_wq = cmd.trim() == "wq";
                                    let parsed = parse_command(&cmd);
                                    app.command_line.clear();
                                    if is_wq {
                                        wq_pending = true;
                                    }
                                    parsed
                                }
                            }
                            CommandAction::Cancel => {
                                app.command_line.clear();
                                search_mode = false;
                                Action::ChangeMode(Mode::Normal)
                            }
                            CommandAction::Noop => Action::Noop,
                        }
                    }
                };

                // Handle mode transitions for visual mode
                if let Action::ChangeMode(Mode::Visual) = &action {
                    visual_state = Some(VisualState::new(app.cursor, false));
                }
                if let Action::ChangeMode(Mode::VisualBlock) = &action {
                    visual_state = Some(VisualState::new(app.cursor, true));
                }
                if let Action::ChangeMode(Mode::Insert) = &action {
                    insert_cursor = app.sheet.get_cell(app.cursor)
                        .map(|c| c.raw.len())
                        .unwrap_or(0);
                }
                if let Action::ChangeMode(Mode::Command) = &action {
                    if key.code == KeyCode::Char('/') {
                        search_mode = true;
                    }
                }

                app.process_action(action);

                // Handle :wq
                if wq_pending && !app.dirty {
                    app.should_quit = true;
                    wq_pending = false;
                }
            }
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles.

- [ ] **Step 3: Manual test**

```bash
echo -e "Name,Score\nAlice,95\nBob,88" > /tmp/test.csv
cargo run --bin cell -- /tmp/test.csv
```

Test: `hjkl` navigation, `i` to edit a cell, type something, `ESC` to confirm, `:q` to quit.

- [ ] **Step 4: Commit**

```bash
git add crates/cell-tui/src/main.rs
git commit -m "feat: wire all modes into event loop — normal, insert, visual, command"
```

---

## Task 15: File Save Implementation

**Files:**
- Modify: `crates/cell-tui/src/app.rs`

- [ ] **Step 1: Add save logic to App::process_action**

Add these match arms to the `process_action` method for `Save` and `ForceSave`:

```rust
Action::Save(path_opt) => {
    let path = path_opt.or(self.file_path.clone());
    if let Some(path) = path {
        let format = Self::format_from_path(&path);
        // Check for formula loss when saving as CSV/TSV
        if !matches!(format, FileFormat::Cell) && self.has_formulas() {
            self.status_message = Some(
                "Sheet contains formulas that will be lost. Use :w file.cell to preserve, or :w! to save as CSV anyway.".into()
            );
            return;
        }
        self.do_save(&path, format);
    } else {
        self.status_message = Some("No file name".into());
    }
}
Action::ForceSave(path_opt) => {
    let path = path_opt.or(self.file_path.clone());
    if let Some(path) = path {
        let format = Self::format_from_path(&path);
        self.do_save(&path, format);
    } else {
        self.status_message = Some("No file name".into());
    }
}
```

Add helper methods to `App`:

```rust
fn format_from_path(path: &PathBuf) -> FileFormat {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase().as_str() {
        "tsv" => FileFormat::Tsv,
        "cell" => FileFormat::Cell,
        _ => FileFormat::Csv,
    }
}

fn do_save(&mut self, path: &PathBuf, format: FileFormat) {
    let result = match format {
        FileFormat::Csv => {
            std::fs::File::create(path)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
                .and_then(|f| cell_core::io::csv::write_csv(&self.sheet, f, b','))
        }
        FileFormat::Tsv => {
            std::fs::File::create(path)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
                .and_then(|f| cell_core::io::csv::write_csv(&self.sheet, f, b'\t'))
        }
        FileFormat::Cell => {
            std::fs::File::create(path)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
                .and_then(|f| cell_core::io::cell_format::write_cell_format(&self.sheet, f))
        }
    };

    match result {
        Ok(()) => {
            self.file_path = Some(path.clone());
            self.file_format = format;
            self.dirty = false;
            self.status_message = Some(format!("Written to {}", path.display()));
        }
        Err(e) => {
            self.status_message = Some(format!("Error saving: {}", e));
        }
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`

- [ ] **Step 3: Manual test**

```bash
cargo run --bin cell -- /tmp/test.csv
```
Edit a cell, then `:w /tmp/out.csv`, verify the file is written. Try `:w /tmp/test.cell` to test native format save.

- [ ] **Step 4: Commit**

```bash
git add crates/cell-tui/src/app.rs
git commit -m "feat: implement file save with formula-loss warning for CSV"
```

---

## Task 16: Search Implementation

**Files:**
- Modify: `crates/cell-tui/src/app.rs`

- [ ] **Step 1: Add search to App::process_action**

Add these match arms:

```rust
Action::Search { pattern, direction } => {
    self.search_pattern = Some(pattern.clone());
    self.find_next(direction == SearchDirection::Forward);
}
Action::SearchNext => {
    if self.search_pattern.is_some() {
        self.find_next(true);
    }
}
Action::SearchPrev => {
    if self.search_pattern.is_some() {
        self.find_next(false);
    }
}
```

Add the `find_next` method:

```rust
fn find_next(&mut self, forward: bool) {
    let pattern = match &self.search_pattern {
        Some(p) => p.to_lowercase(),
        None => return,
    };

    let total_cells = self.sheet.row_count * self.sheet.col_count.max(1);
    if total_cells == 0 {
        return;
    }

    let (start_row, start_col) = self.cursor;
    let cols = self.sheet.col_count.max(1);

    for offset in 1..=total_cells {
        let flat = start_row * cols + start_col;
        let next_flat = if forward {
            (flat + offset) % total_cells
        } else {
            (flat + total_cells - offset) % total_cells
        };
        let row = next_flat / cols;
        let col = next_flat % cols;

        if let Some(cell) = self.sheet.get_cell((row, col)) {
            if cell.value.to_string().to_lowercase().contains(&pattern) {
                self.cursor = (row, col);
                self.viewport.ensure_visible(self.cursor);
                return;
            }
        }
    }

    self.status_message = Some(format!("Pattern not found: {}", pattern));
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`

- [ ] **Step 3: Commit**

```bash
git add crates/cell-tui/src/app.rs
git commit -m "feat: implement /pattern search with n/N navigation"
```

---

## Task 17: Sort Implementation

**Files:**
- Modify: `crates/cell-tui/src/app.rs`
- Modify: `crates/cell-core/src/model.rs`

- [ ] **Step 1: Write a failing test for sort**

Add to `crates/cell-core/src/model.rs` tests:

```rust
#[test]
fn sheet_sort_by_column_ascending() {
    let mut sheet = Sheet::new();
    sheet.set_cell((0, 0), "Charlie");
    sheet.set_cell((0, 1), "3");
    sheet.set_cell((1, 0), "Alice");
    sheet.set_cell((1, 1), "1");
    sheet.set_cell((2, 0), "Bob");
    sheet.set_cell((2, 1), "2");

    sheet.sort_by_column(0, true);

    assert_eq!(sheet.get_cell((0, 0)).unwrap().raw, "Alice");
    assert_eq!(sheet.get_cell((1, 0)).unwrap().raw, "Bob");
    assert_eq!(sheet.get_cell((2, 0)).unwrap().raw, "Charlie");
    // Corresponding values in other columns should move too
    assert_eq!(sheet.get_cell((0, 1)).unwrap().raw, "1");
    assert_eq!(sheet.get_cell((1, 1)).unwrap().raw, "2");
    assert_eq!(sheet.get_cell((2, 1)).unwrap().raw, "3");
}

#[test]
fn sheet_sort_numeric_column() {
    let mut sheet = Sheet::new();
    sheet.set_cell((0, 0), "30");
    sheet.set_cell((1, 0), "5");
    sheet.set_cell((2, 0), "100");

    sheet.sort_by_column(0, true);

    assert_eq!(sheet.get_cell((0, 0)).unwrap().value, CellValue::Number(5.0));
    assert_eq!(sheet.get_cell((1, 0)).unwrap().value, CellValue::Number(30.0));
    assert_eq!(sheet.get_cell((2, 0)).unwrap().value, CellValue::Number(100.0));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p cell-core`
Expected: `sort_by_column` method not found.

- [ ] **Step 3: Implement sort_by_column on Sheet**

Add to `Sheet` impl in `crates/cell-core/src/model.rs`:

```rust
pub fn sort_by_column(&mut self, col: usize, ascending: bool) {
    if self.row_count == 0 {
        return;
    }

    // Collect all rows as Vec of (row_index, sort_key)
    let mut row_indices: Vec<usize> = (0..self.row_count).collect();

    row_indices.sort_by(|&a, &b| {
        let cell_a = self.get_cell((a, col));
        let cell_b = self.get_cell((b, col));

        let ord = match (cell_a.map(|c| &c.value), cell_b.map(|c| &c.value)) {
            (Some(CellValue::Number(na)), Some(CellValue::Number(nb))) => {
                na.partial_cmp(nb).unwrap_or(std::cmp::Ordering::Equal)
            }
            (Some(va), Some(vb)) => {
                va.to_string().cmp(&vb.to_string())
            }
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        };

        if ascending { ord } else { ord.reverse() }
    });

    // Rebuild the cells HashMap with reordered rows
    let mut new_cells = HashMap::new();
    for (new_row, &old_row) in row_indices.iter().enumerate() {
        for c in 0..self.col_count {
            if let Some(cell) = self.cells.get(&(old_row, c)) {
                new_cells.insert((new_row, c), cell.clone());
            }
        }
    }
    self.cells = new_cells;
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p cell-core`
Expected: All tests pass.

- [ ] **Step 5: Wire sort into App::process_action**

Add the match arm in `app.rs`:

```rust
Action::Sort { col, ascending } => {
    self.sheet.sort_by_column(col, ascending);
    self.dirty = true;
    self.status_message = Some(format!(
        "Sorted by column {} {}",
        cell_core::model::col_index_to_label(col),
        if ascending { "ascending" } else { "descending" }
    ));
}
```

- [ ] **Step 6: Commit**

```bash
git add crates/
git commit -m "feat: implement column sort"
```

---

## Task 18: Clipboard — Yank, Paste, Formula Adjustment

**Files:**
- Modify: `crates/cell-tui/src/clipboard.rs`
- Modify: `crates/cell-tui/src/app.rs`

- [ ] **Step 1: Write failing tests for formula adjustment**

Add to `crates/cell-tui/src/clipboard.rs`:

```rust
/// Adjust cell references in a formula when pasting.
/// `row_delta` and `col_delta` are the offset from source to destination.
pub fn adjust_formula(raw: &str, row_delta: isize, col_delta: isize) -> String {
    if !raw.starts_with('=') {
        return raw.to_string();
    }
    // Parse and adjust references
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjust_formula_relative_refs() {
        assert_eq!(
            adjust_formula("=A1+B1", 2, 0),
            "=A3+B3"
        );
    }

    #[test]
    fn adjust_formula_absolute_refs_unchanged() {
        assert_eq!(
            adjust_formula("=$A$1+$B$1", 2, 1),
            "=$A$1+$B$1"
        );
    }

    #[test]
    fn adjust_formula_mixed_refs() {
        assert_eq!(
            adjust_formula("=$A1+A$1", 2, 1),
            "=$A3+B$1"
        );
    }

    #[test]
    fn adjust_no_formula() {
        assert_eq!(adjust_formula("hello", 1, 1), "hello");
    }

    #[test]
    fn adjust_formula_col_shift() {
        assert_eq!(
            adjust_formula("=A1", 0, 2),
            "=C1"
        );
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p cell-tui`
Expected: Tests fail (todo! panic).

- [ ] **Step 3: Implement formula adjustment**

Replace the `adjust_formula` function and expand `clipboard.rs`:

```rust
use cell_core::model::{CellPos, col_index_to_label, col_label_to_index};
use cell_core::formula::token::{Token, tokenize};

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
            Token::CellRef { col, row, abs_col, abs_row } => {
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p cell-tui`
Expected: All tests pass.

- [ ] **Step 5: Wire yank/paste into App::process_action**

Add these match arms in `app.rs`:

```rust
Action::YankCell(pos) => {
    if let Some(cell) = self.sheet.get_cell(pos) {
        self.register = Some(Register::Cell(cell.raw.clone()));
    }
}
Action::YankRow(row) => {
    let mut cells = Vec::new();
    for col in 0..self.sheet.col_count {
        let raw = self.sheet.get_cell((row, col))
            .map(|c| c.raw.clone())
            .unwrap_or_default();
        cells.push(raw);
    }
    self.register = Some(Register::Row(cells));
}
Action::YankRange { start, end } => {
    let mut block = Vec::new();
    for row in start.0..=end.0 {
        let mut row_data = Vec::new();
        for col in start.1..=end.1 {
            let raw = self.sheet.get_cell((row, col))
                .map(|c| c.raw.clone())
                .unwrap_or_default();
            row_data.push(raw);
        }
        block.push(row_data);
    }
    self.register = Some(Register::Block(block));
}
Action::ClearRange { start, end } => {
    // Yank first, then clear
    let mut block = Vec::new();
    for row in start.0..=end.0 {
        let mut row_data = Vec::new();
        for col in start.1..=end.1 {
            let raw = self.sheet.get_cell((row, col))
                .map(|c| c.raw.clone())
                .unwrap_or_default();
            row_data.push(raw);
            self.sheet.clear_cell((row, col));
        }
        block.push(row_data);
    }
    self.register = Some(Register::Block(block));
    self.dirty = true;
}
Action::DeleteRow(row) => {
    // Yank the row first
    let mut cells = Vec::new();
    for col in 0..self.sheet.col_count {
        let raw = self.sheet.get_cell((row, col))
            .map(|c| c.raw.clone())
            .unwrap_or_default();
        cells.push(raw);
        self.sheet.clear_cell((row, col));
    }
    self.register = Some(Register::Row(cells));
    self.dirty = true;
}
Action::Paste(pos) | Action::PasteBefore(pos) => {
    let dest_row = match action {
        Action::Paste(_) => pos.0 + 1,
        _ => pos.0,
    };
    if let Some(reg) = &self.register.clone() {
        match reg {
            Register::Cell(raw) => {
                let adjusted = clipboard::adjust_formula(
                    raw,
                    dest_row as isize - pos.0 as isize,
                    0,
                );
                self.process_action(Action::EditCell((dest_row, pos.1), adjusted));
            }
            Register::Row(cells) => {
                for (col, raw) in cells.iter().enumerate() {
                    if !raw.is_empty() {
                        let adjusted = clipboard::adjust_formula(
                            raw,
                            dest_row as isize - pos.0 as isize,
                            0,
                        );
                        self.sheet.set_cell((dest_row, col), &adjusted);
                    }
                }
                self.dirty = true;
            }
            Register::Block(block) => {
                for (r_off, row_data) in block.iter().enumerate() {
                    for (c_off, raw) in row_data.iter().enumerate() {
                        if !raw.is_empty() {
                            let adjusted = clipboard::adjust_formula(
                                raw,
                                r_off as isize,
                                c_off as isize,
                            );
                            self.sheet.set_cell((dest_row + r_off, pos.1 + c_off), &adjusted);
                        }
                    }
                }
                self.dirty = true;
            }
        }
    }
}
```

- [ ] **Step 6: Commit**

```bash
git add crates/cell-tui/src/
git commit -m "feat: implement yank/paste with formula reference adjustment"
```

---

## Task 19: End-to-End Integration Test

**Files:**
- Create: `crates/cell-core/tests/integration.rs`

- [ ] **Step 1: Write integration tests**

Create `crates/cell-core/tests/integration.rs`:

```rust
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

    // Save as .cell
    let mut buf = Vec::new();
    write_cell_format(&sheet, &mut buf).unwrap();

    // Reload
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

    // Change A1 and recalculate
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

    // Formula should be flattened to its computed value
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
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test -p cell-core --test integration`
Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/cell-core/tests/
git commit -m "test: add end-to-end integration tests for core"
```

---

## Task 20: Polish and Final Verification

**Files:**
- Modify: `crates/cell-tui/src/main.rs`
- Create: `.gitignore`

- [ ] **Step 1: Add .gitignore**

Create `.gitignore`:
```
/target
```

- [ ] **Step 2: Run all tests**

Run: `cargo test --workspace`
Expected: All tests pass across both crates.

- [ ] **Step 3: Run clippy**

Run: `cargo clippy --workspace -- -D warnings`
Expected: No warnings. Fix any that appear.

- [ ] **Step 4: Test manually end-to-end**

```bash
echo -e "Name,Score\nAlice,95\nBob,88\nCarol,72" > /tmp/test.csv
cargo run --bin cell -- /tmp/test.csv
```

Verify:
- Grid displays correctly with headers and data
- `hjkl` navigation works, cursor highlights
- `i` enters insert mode, editing works, `ESC` confirms
- `:w /tmp/out.cell` saves native format
- `:q` quits
- `cell /tmp/out.cell` reopens the saved file

- [ ] **Step 5: Commit**

```bash
git add .gitignore
git commit -m "chore: add .gitignore, final polish"
```

- [ ] **Step 6: Push**

```bash
git push origin master
```
