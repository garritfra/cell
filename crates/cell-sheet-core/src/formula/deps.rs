use std::collections::{HashMap, HashSet, VecDeque};

use crate::formula::ast::*;
use crate::formula::eval;
use crate::formula::parser;
use crate::model::{CellError, CellPos, CellValue, Sheet};

pub struct DepGraph {
    pub dependents: HashMap<CellPos, HashSet<CellPos>>,
    pub dependencies: HashMap<CellPos, HashSet<CellPos>>,
}

impl Default for DepGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl DepGraph {
    pub fn new() -> Self {
        DepGraph {
            dependents: HashMap::new(),
            dependencies: HashMap::new(),
        }
    }

    pub fn set_dependencies(&mut self, cell: CellPos, deps: Vec<CellPos>) {
        if let Some(old_deps) = self.dependencies.remove(&cell) {
            for dep in &old_deps {
                if let Some(set) = self.dependents.get_mut(dep) {
                    set.remove(&cell);
                }
            }
        }
        let dep_set: HashSet<CellPos> = deps.into_iter().collect();
        for dep in &dep_set {
            self.dependents.entry(*dep).or_default().insert(cell);
        }
        self.dependencies.insert(cell, dep_set);
    }
}

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

pub fn set_formula(sheet: &mut Sheet, deps: &mut DepGraph, pos: CellPos, raw: &str) {
    sheet.set_cell(pos, raw);
    let formula = &raw[1..]; // strip '='
    let dep_list = extract_deps(formula);
    deps.set_dependencies(pos, dep_list);
}

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

pub fn recalculate(sheet: &mut Sheet, deps: &DepGraph) {
    let formula_cells: Vec<CellPos> = sheet
        .cells
        .iter()
        .filter(|(_, cell)| cell.raw.starts_with('='))
        .map(|(pos, _)| *pos)
        .collect();

    let formula_set: HashSet<CellPos> = formula_cells.iter().cloned().collect();

    let mut in_degree: HashMap<CellPos, usize> = HashMap::new();
    for &cell in &formula_cells {
        let count = deps
            .dependencies
            .get(&cell)
            .map(|d| d.iter().filter(|p| formula_set.contains(p)).count())
            .unwrap_or(0);
        in_degree.insert(cell, count);
    }

    let mut queue: VecDeque<CellPos> = in_degree
        .iter()
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

    let ordered_set: HashSet<CellPos> = order.iter().cloned().collect();
    for &cell in &formula_cells {
        if !ordered_set.contains(&cell) {
            if let Some(c) = sheet.cells.get_mut(&cell) {
                c.value = CellValue::Error(CellError::Circ);
                c.dirty = false;
            }
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CellValue, Sheet};

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
        assert_eq!(
            sheet.get_cell((0, 1)).unwrap().value,
            CellValue::Number(15.0)
        );
    }

    #[test]
    fn recalc_chain() {
        let mut sheet = Sheet::new();
        let mut deps = DepGraph::new();
        sheet.set_cell((0, 0), "10");
        set_formula(&mut sheet, &mut deps, (0, 1), "=A1*2");
        set_formula(&mut sheet, &mut deps, (0, 2), "=B1+1");
        recalculate(&mut sheet, &deps);
        assert_eq!(
            sheet.get_cell((0, 1)).unwrap().value,
            CellValue::Number(20.0)
        );
        assert_eq!(
            sheet.get_cell((0, 2)).unwrap().value,
            CellValue::Number(21.0)
        );
    }

    #[test]
    fn recalc_circular_reference() {
        let mut sheet = Sheet::new();
        let mut deps = DepGraph::new();
        set_formula(&mut sheet, &mut deps, (0, 0), "=B1");
        set_formula(&mut sheet, &mut deps, (0, 1), "=A1");
        recalculate(&mut sheet, &deps);
        assert_eq!(
            sheet.get_cell((0, 0)).unwrap().value,
            CellValue::Error(CellError::Circ)
        );
        assert_eq!(
            sheet.get_cell((0, 1)).unwrap().value,
            CellValue::Error(CellError::Circ)
        );
    }

    #[test]
    fn recalc_after_value_change() {
        let mut sheet = Sheet::new();
        let mut deps = DepGraph::new();
        sheet.set_cell((0, 0), "10");
        set_formula(&mut sheet, &mut deps, (0, 1), "=A1+5");
        recalculate(&mut sheet, &deps);
        assert_eq!(
            sheet.get_cell((0, 1)).unwrap().value,
            CellValue::Number(15.0)
        );

        sheet.set_cell((0, 0), "20");
        mark_dirty(&mut sheet, &deps, (0, 0));
        recalculate(&mut sheet, &deps);
        assert_eq!(
            sheet.get_cell((0, 1)).unwrap().value,
            CellValue::Number(25.0)
        );
    }

    #[test]
    fn self_reference_is_circular() {
        let mut sheet = Sheet::new();
        let mut deps = DepGraph::new();
        set_formula(&mut sheet, &mut deps, (0, 0), "=A1+1");
        recalculate(&mut sheet, &deps);
        assert_eq!(
            sheet.get_cell((0, 0)).unwrap().value,
            CellValue::Error(CellError::Circ)
        );
    }
}
