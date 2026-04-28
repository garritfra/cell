use cell_sheet_core::formula::deps::{mark_dirty, recalculate, set_formula, DepGraph};
use cell_sheet_core::model::Sheet;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

/// Build a sheet with `n` value cells in column A and `n` formula cells in
/// column B where B_i = =A_i + 1.  Returns a pair ready for timing.
fn setup_independent_formulas(n: usize) -> (Sheet, DepGraph) {
    let mut sheet = Sheet::new();
    let mut deps = DepGraph::new();
    for i in 0..n {
        sheet.set_cell((i, 0), &i.to_string());
        let formula = format!("=A{}", i + 1);
        set_formula(&mut sheet, &mut deps, (i, 1), &formula);
    }
    recalculate(&mut sheet, &deps);
    (sheet, deps)
}

/// Simulate N sequential single-cell edits where each write is immediately
/// followed by mark_dirty + recalculate — the unbatched O(n × graph) path
/// that occurs when N independent EditCell actions fire without a batch.
fn bench_unbatched_n_edits(c: &mut Criterion) {
    let mut group = c.benchmark_group("edit_cells/unbatched");
    for n in [10usize, 50, 100] {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter(|| {
                let (mut sheet, deps) = setup_independent_formulas(n);
                for i in 0..n {
                    sheet.set_cell((i, 0), &(i + 100).to_string());
                    mark_dirty(&mut sheet, &deps, (i, 0));
                    recalculate(black_box(&mut sheet), black_box(&deps));
                }
                black_box(sheet)
            });
        });
    }
    group.finish();
}

/// Same N edits batched: mark_dirty N times, then a single recalculate.
/// Demonstrates O(graph) cost vs O(n × graph) for the unbatched path.
fn bench_batched_n_edits(c: &mut Criterion) {
    let mut group = c.benchmark_group("edit_cells/batched");
    for n in [10usize, 50, 100] {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter(|| {
                let (mut sheet, deps) = setup_independent_formulas(n);
                for i in 0..n {
                    sheet.set_cell((i, 0), &(i + 100).to_string());
                    mark_dirty(&mut sheet, &deps, (i, 0));
                }
                recalculate(black_box(&mut sheet), black_box(&deps));
                black_box(sheet)
            });
        });
    }
    group.finish();
}

/// Benchmark a single recalculate call on a sheet with F independent
/// formula cells — measures the base cost of one topological pass.
fn bench_recalculate_fanout(c: &mut Criterion) {
    let mut group = c.benchmark_group("recalculate/fanout");
    for n in [100usize, 500, 1000] {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            let (mut sheet, deps) = setup_independent_formulas(n);
            for i in 0..n {
                mark_dirty(&mut sheet, &deps, (i, 0));
            }
            b.iter(|| {
                recalculate(black_box(&mut sheet), black_box(&deps));
            });
        });
    }
    group.finish();
}

/// Benchmark mark_dirty BFS on a deep chain: B1=A1+1, C1=B1+1, …
/// Measures propagation cost for a max-depth dependency chain.
fn bench_mark_dirty_deep_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("mark_dirty/deep_chain");
    for depth in [10usize, 50, 100] {
        group.bench_with_input(BenchmarkId::from_parameter(depth), &depth, |b, &depth| {
            let mut sheet = Sheet::new();
            let mut deps = DepGraph::new();
            sheet.set_cell((0, 0), "1");
            for col in 1..depth {
                let prev_label = cell_sheet_core::model::col_index_to_label(col - 1);
                let formula = format!("={prev_label}1+1");
                set_formula(&mut sheet, &mut deps, (0, col), &formula);
            }
            recalculate(&mut sheet, &deps);
            b.iter(|| {
                mark_dirty(black_box(&mut sheet), black_box(&deps), (0, 0));
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_unbatched_n_edits,
    bench_batched_n_edits,
    bench_recalculate_fanout,
    bench_mark_dirty_deep_chain,
);
criterion_main!(benches);
