# Benchmarks

Benchmarks live in `crates/cell-sheet-core/benches/core.rs` and use
[criterion](https://github.com/bheisler/criterion.rs).

## Running

```sh
# Full suite
cargo bench -p cell-sheet-core

# Single benchmark by name
cargo bench -p cell-sheet-core -- csv_load_100k
```

HTML reports are written to `target/criterion/` after each run.

## Suite

| Benchmark | What it measures |
|---|---|
| `csv_load_100k` | `read_csv` on a 100 000-row × 26-col in-memory CSV |
| `formula_recalc_10k` | `recalculate` on 10 000 independent formula cells (each `=A{n}+1`); stresses topological sort and eval throughput |
| `mark_dirty_chain` | BFS dirty propagation on a 1 000-cell linear dependency chain |
| `recalculate_wide_dag` | Topological sort on 1 000 cells all referencing a single source cell |
| `range_sum_10k` | `SUM(A1:A10000)` parse and evaluation |

## Results

Fill in this table locally after running the suite. Include your machine
spec and the date so results are comparable across PRs.

| Benchmark | Mean | Machine | Date |
|---|---|---|---|
| csv_load_100k | 444 ms | Apple M4 Pro / macOS 26.4.1 | 2026-04-28 |
| formula_recalc_10k | 5.78 ms | Apple M4 Pro / macOS 26.4.1 | 2026-04-28 |
| mark_dirty_chain | 74.9 µs | Apple M4 Pro / macOS 26.4.1 | 2026-04-28 |
| recalculate_wide_dag | 438 µs | Apple M4 Pro / macOS 26.4.1 | 2026-04-28 |
| range_sum_10k | 594 µs | Apple M4 Pro / macOS 26.4.1 | 2026-04-28 |
