use crate::formula::deps::{mark_dirty, recalculate, set_formula, DepGraph};
use crate::model::{CellPos, Sheet};

/// Coordinates sheet mutations with dependency graph maintenance.
///
/// `Sheet` remains the storage type and `DepGraph` remains the graph type, but
/// callers that mutate raw cell contents should go through this wrapper so
/// formula edges, dirty marking, and recalculation stay in sync.
pub struct SheetEngine<'a> {
    sheet: &'a mut Sheet,
    deps: &'a mut DepGraph,
}

impl<'a> SheetEngine<'a> {
    pub fn new(sheet: &'a mut Sheet, deps: &'a mut DepGraph) -> Self {
        Self { sheet, deps }
    }

    pub fn set_cell_raw(&mut self, pos: CellPos, raw: &str) {
        if raw.starts_with('=') {
            set_formula(self.sheet, self.deps, pos, raw);
        } else {
            self.sheet.set_cell(pos, raw);
            self.deps.remove(pos);
        }
        mark_dirty(self.sheet, self.deps, pos);
    }

    pub fn clear_cell(&mut self, pos: CellPos) {
        self.sheet.clear_cell(pos);
        self.deps.remove(pos);
        mark_dirty(self.sheet, self.deps, pos);
    }

    pub fn write_cell_raw(&mut self, pos: CellPos, raw: &str) {
        if raw.is_empty() {
            self.clear_cell(pos);
        } else {
            self.set_cell_raw(pos, raw);
        }
    }

    pub fn write_cell_raw_and_recalculate(&mut self, pos: CellPos, raw: &str) {
        self.write_cell_raw(pos, raw);
        self.recalculate();
    }

    pub fn set_cell_raw_and_recalculate(&mut self, pos: CellPos, raw: &str) {
        self.set_cell_raw(pos, raw);
        self.recalculate();
    }

    pub fn recalculate(&mut self) {
        recalculate(self.sheet, self.deps);
    }

    pub fn sort_by_column_and_recalculate(&mut self, col: usize, ascending: bool) {
        self.sheet.sort_by_column(col, ascending);
        self.rebuild_formulas_and_recalculate();
    }

    pub fn rebuild_formulas_and_recalculate(&mut self) {
        *self.deps = DepGraph::new();
        let formula_cells: Vec<_> = self
            .sheet
            .cells
            .iter()
            .filter(|(_, cell)| cell.raw.starts_with('='))
            .map(|(pos, cell)| (*pos, cell.raw.clone()))
            .collect();
        for (pos, raw) in formula_cells {
            set_formula(self.sheet, self.deps, pos, &raw);
        }
        self.recalculate();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::CellValue;

    #[test]
    fn write_value_updates_dependents() {
        let mut sheet = Sheet::new();
        let mut deps = DepGraph::new();
        SheetEngine::new(&mut sheet, &mut deps).set_cell_raw((0, 0), "10");
        SheetEngine::new(&mut sheet, &mut deps).write_cell_raw_and_recalculate((0, 1), "=A1+5");

        assert_eq!(
            sheet.get_cell((0, 1)).unwrap().value,
            CellValue::Number(15.0)
        );

        SheetEngine::new(&mut sheet, &mut deps).write_cell_raw_and_recalculate((0, 0), "20");

        assert_eq!(
            sheet.get_cell((0, 1)).unwrap().value,
            CellValue::Number(25.0)
        );
    }

    #[test]
    fn rewriting_formula_as_value_drops_edges() {
        let mut sheet = Sheet::new();
        let mut deps = DepGraph::new();
        SheetEngine::new(&mut sheet, &mut deps).set_cell_raw((0, 0), "10");
        SheetEngine::new(&mut sheet, &mut deps).write_cell_raw_and_recalculate((0, 1), "=A1+5");

        assert!(deps.dependencies.contains_key(&(0, 1)));

        SheetEngine::new(&mut sheet, &mut deps).write_cell_raw_and_recalculate((0, 1), "plain");

        assert!(!deps.dependencies.contains_key(&(0, 1)));
        assert!(!deps
            .dependents
            .get(&(0, 0))
            .map(|set| set.contains(&(0, 1)))
            .unwrap_or(false));
    }

    #[test]
    fn set_empty_cell_preserves_extent() {
        let mut sheet = Sheet::new();
        let mut deps = DepGraph::new();

        SheetEngine::new(&mut sheet, &mut deps).set_cell_raw((1, 1), "");

        assert_eq!(sheet.row_count, 2);
        assert_eq!(sheet.col_count, 2);
        assert!(sheet.get_cell((1, 1)).is_some());
    }

    #[test]
    fn clear_cell_removes_cell_and_updates_dependents() {
        let mut sheet = Sheet::new();
        let mut deps = DepGraph::new();
        SheetEngine::new(&mut sheet, &mut deps).set_cell_raw((0, 0), "10");
        SheetEngine::new(&mut sheet, &mut deps).write_cell_raw_and_recalculate((0, 1), "=A1+5");

        SheetEngine::new(&mut sheet, &mut deps).write_cell_raw_and_recalculate((0, 0), "");

        assert!(sheet.get_cell((0, 0)).is_none());
        assert_eq!(
            sheet.get_cell((0, 1)).unwrap().value,
            CellValue::Number(5.0)
        );
    }

    #[test]
    fn sort_rebuilds_formula_graph() {
        let mut sheet = Sheet::new();
        let mut deps = DepGraph::new();
        SheetEngine::new(&mut sheet, &mut deps).set_cell_raw((0, 0), "2");
        SheetEngine::new(&mut sheet, &mut deps).set_cell_raw((1, 0), "1");
        SheetEngine::new(&mut sheet, &mut deps).write_cell_raw_and_recalculate((0, 1), "=A1*10");

        SheetEngine::new(&mut sheet, &mut deps).sort_by_column_and_recalculate(0, true);
        SheetEngine::new(&mut sheet, &mut deps).set_cell_raw_and_recalculate((0, 0), "3");

        assert_eq!(
            sheet.get_cell((1, 1)).unwrap().value,
            CellValue::Number(30.0)
        );
    }

    #[test]
    fn rebuilds_formula_graph_for_loaded_sheet() {
        let mut sheet = Sheet::new();
        sheet.set_cell((0, 0), "10");
        sheet.set_cell((0, 1), "=A1+5");
        let mut deps = DepGraph::new();

        SheetEngine::new(&mut sheet, &mut deps).rebuild_formulas_and_recalculate();

        assert!(deps.dependencies.contains_key(&(0, 1)));
        assert_eq!(
            sheet.get_cell((0, 1)).unwrap().value,
            CellValue::Number(15.0)
        );
    }
}
