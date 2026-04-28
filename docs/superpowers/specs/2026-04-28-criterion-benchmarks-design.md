# Design: criterion benchmarks for cell-sheet-core

**Date:** 2026-04-28  
**Issue:** [#7](https://github.com/garritfra/cell/issues/7)  
**Status:** Approved

## Problem

Without performance numbers there is no way to detect regressions or identify
bottlenecks as the sheet grows. The issue asks for a baseline benchmark suite
covering the core hotspots: CSV loading, formula recalculation, dependency-graph
BFS propagation, topological-sort throughput, and range expansion.

## Approach

Single benchmark file using `criterion` — the de-facto Rust benchmark harness.
All five benchmarks live in `crates/cell-sheet-core/benches/core.rs` registered
in one `criterion_group!`. No library source changes; the existing public API
(`read_csv`, `set_formula`, `mark_dirty`, `recalculate`) is sufficient.

CI compiles the benchmarks (`cargo bench --no-run -p cell-sheet-core`) but does
not run them — timing varies across machines and is not a reliable CI gate.

## Files changed

| File | Change |
|---|---|
| `crates/cell-sheet-core/Cargo.toml` | Add `criterion` dev-dependency; add `[[bench]]` entry |
| `crates/cell-sheet-core/benches/core.rs` | New — all five benchmarks |
| `.github/workflows/ci.yml` | Add `cargo bench --no-run -p cell-sheet-core` step |
| `BENCH.md` | New — how to run, what each bench measures, results table |
| `CHANGELOG.md` | Entry under `## Unreleased > Added` |

## Benchmarks

### `csv_load_100k`

Measures `read_csv` on a 100k-row × 26-col CSV built entirely in memory.
Setup constructs the CSV bytes once; each criterion iteration passes a
`std::io::Cursor` to `read_csv`. Isolates the CSV parsing and `Sheet`
construction path.

### `formula_recalc_10k`

Measures end-to-end `set_formula` + `mark_dirty` + `recalculate` for 10k
formula cells each referencing the cell to their left
(`B1=A1+1`, `B2=A2+1`, …). Setup pre-populates column A with plain numeric
values. Each iteration sets all formula cells and recalculates.

### `mark_dirty_chain`

Measures BFS dirty-propagation depth on a linear dependency chain 1000 cells
deep (`B1=A1+1`, `C1=B1+1`, …, 1000 nodes). Setup builds the chain once.
Each iteration touches `A1` (sets it to a new value) and calls `mark_dirty`.

### `recalculate_wide_dag`

Measures topological-sort throughput on a wide fan-in DAG: 1000 cells in row 1
all referencing `A1`. Setup builds the DAG once. Each iteration updates `A1`
and calls `recalculate`.

### `range_sum_10k`

Measures `SUM(A1:A10000)` parse and evaluation. Setup populates 10k numeric
values in column A and sets `B1` to `=SUM(A1:A10000)`. Each iteration calls
`recalculate`.

## Implementation notes

- Use `criterion::black_box` on all inputs/outputs to prevent dead-code
  elimination.
- Use `iter_batched` (or `iter_with_setup`) so setup cost is excluded from
  measurements.
- `criterion` requires a `harness = false` flag in the `[[bench]]` entry
  because it provides its own `main`.

## BENCH.md content

Short reference document at the repo root:

- How to run the full suite: `cargo bench -p cell-sheet-core`
- How to run one benchmark: `cargo bench -p cell-sheet-core -- csv_load_100k`
- One-sentence description of each benchmark
- Results table with columns: `Benchmark | Mean | Machine | Date` — left empty
  for contributors to fill in locally

## Acceptance criteria

- `cargo bench -p cell-sheet-core` runs all five benchmarks cleanly.
- `cargo bench --no-run -p cell-sheet-core` added to CI and passes.
- `BENCH.md` committed.
- `CHANGELOG.md` updated.
- `cargo fmt --all`, `cargo clippy --workspace --all-targets --all-features`,
  and `cargo test` all pass.
