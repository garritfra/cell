use criterion::{criterion_group, criterion_main, Criterion};

fn bench_csv_load_100k(_c: &mut Criterion) {
    todo!()
}

fn bench_formula_recalc_10k(_c: &mut Criterion) {
    todo!()
}

fn bench_mark_dirty_chain(_c: &mut Criterion) {
    todo!()
}

fn bench_recalculate_wide_dag(_c: &mut Criterion) {
    todo!()
}

fn bench_range_sum_10k(_c: &mut Criterion) {
    todo!()
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
