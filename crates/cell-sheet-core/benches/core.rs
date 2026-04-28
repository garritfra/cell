use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use std::hint::black_box;
use std::io::Cursor;

use cell_sheet_core::formula::deps::{mark_dirty, recalculate, set_formula, DepGraph};
use cell_sheet_core::io::csv::read_csv;
use cell_sheet_core::model::{col_index_to_label, Sheet};

fn bench_csv_load_100k(c: &mut Criterion) {
    let mut csv = String::new();
    for row in 0u64..100_000 {
        for col in 0u64..26 {
            if col > 0 {
                csv.push(',');
            }
            csv.push_str(&(row * 26 + col).to_string());
        }
        csv.push('\n');
    }
    let csv_bytes = csv.into_bytes();

    c.bench_function("csv_load_100k", |b| {
        b.iter(|| {
            let reader = Cursor::new(black_box(&csv_bytes));
            black_box(read_csv(reader, b',').unwrap())
        })
    });
}

fn bench_formula_recalc_10k(c: &mut Criterion) {
    let mut sheet = Sheet::new();
    let mut deps = DepGraph::new();
    for row in 0..10_000usize {
        sheet.set_cell((row, 0), &row.to_string());
        let formula = format!("=A{}+1", row + 1);
        set_formula(&mut sheet, &mut deps, (row, 1), &formula);
    }

    c.bench_function("formula_recalc_10k", |b| {
        b.iter_batched(
            || (sheet.clone(), deps.clone()),
            |(mut s, d)| {
                recalculate(&mut s, &d);
                black_box(())
            },
            BatchSize::PerIteration,
        )
    });
}

fn bench_mark_dirty_chain(c: &mut Criterion) {
    let mut sheet = Sheet::new();
    let mut deps = DepGraph::new();
    sheet.set_cell((0, 0), "1");
    for col in 1..1000usize {
        let prev_label = col_index_to_label(col - 1);
        let formula = format!("={}1+1", prev_label);
        set_formula(&mut sheet, &mut deps, (0, col), &formula);
    }
    // Clear dirty flags so mark_dirty has real BFS work to do on every iteration.
    // Without this, cells are already dirty=true from set_formula and mark_dirty
    // would short-circuit immediately.
    recalculate(&mut sheet, &deps);

    c.bench_function("mark_dirty_chain", |b| {
        b.iter_batched(
            || (sheet.clone(), deps.clone()),
            |(mut s, d)| {
                mark_dirty(&mut s, &d, (0, 0));
                black_box(())
            },
            BatchSize::PerIteration,
        )
    });
}

fn bench_recalculate_wide_dag(c: &mut Criterion) {
    let mut sheet = Sheet::new();
    let mut deps = DepGraph::new();
    sheet.set_cell((0, 0), "1");
    for col in 1..=1000usize {
        set_formula(&mut sheet, &mut deps, (0, col), "=A1+1");
    }

    c.bench_function("recalculate_wide_dag", |b| {
        b.iter_batched(
            || (sheet.clone(), deps.clone()),
            |(mut s, d)| {
                recalculate(&mut s, &d);
                black_box(())
            },
            BatchSize::PerIteration,
        )
    });
}

fn bench_range_sum_10k(c: &mut Criterion) {
    let mut sheet = Sheet::new();
    let mut deps = DepGraph::new();
    for row in 0..10_000usize {
        sheet.set_cell((row, 0), &row.to_string());
    }
    set_formula(&mut sheet, &mut deps, (0, 1), "=SUM(A1:A10000)");

    c.bench_function("range_sum_10k", |b| {
        b.iter_batched(
            || (sheet.clone(), deps.clone()),
            |(mut s, d)| {
                recalculate(&mut s, &d);
                black_box(())
            },
            BatchSize::PerIteration,
        )
    });
}

criterion_group!(
    benches,
    bench_csv_load_100k,
    bench_formula_recalc_10k,
    bench_mark_dirty_chain,
    bench_recalculate_wide_dag,
    bench_range_sum_10k,
);
criterion_main!(benches);
