# Criterion Benchmarks Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a criterion benchmark suite to `cell-sheet-core` covering CSV loading, formula recalculation, BFS dirty propagation, topological-sort throughput, and range evaluation.

**Architecture:** Single `benches/core.rs` file with five benchmarks registered in one `criterion_group!`. Setup state is isolated per iteration using `iter_batched`. CI compiles the benchmarks but does not run them. `Sheet`, `Cell`, and `DepGraph` get `#[derive(Clone)]` so benchmarks can clone pre-built state in setup closures.

**Tech Stack:** Rust, criterion 0.8, existing `cell-sheet-core` public API (`read_csv`, `set_formula`, `mark_dirty`, `recalculate`).

---

## File map

| File | Action |
|---|---|
| `crates/cell-sheet-core/src/model.rs` | Add `Clone` to `Cell` and `Sheet` |
| `crates/cell-sheet-core/src/formula/deps.rs` | Add `Clone` to `DepGraph` |
| `crates/cell-sheet-core/Cargo.toml` | Add `criterion` dev-dep + `[[bench]]` entry |
| `crates/cell-sheet-core/benches/core.rs` | New — all five benchmarks |
| `.github/workflows/ci.yml` | New `bench` job: `cargo bench --no-run -p cell-sheet-core` |
| `BENCH.md` | New — how to run, descriptions, results table |
| `CHANGELOG.md` | Entry under `## Unreleased > Added` |

---

## Task 1: Add `Clone` to `Sheet`, `Cell`, and `DepGraph`

**Files:**
- Modify: `crates/cell-sheet-core/src/model.rs`
- Modify: `crates/cell-sheet-core/src/formula/deps.rs`

- [ ] **Step 1: Add `Clone` to `Cell` in `model.rs`**

  Find the `Cell` struct (around line 56). Change:

  ```rust
  #[derive(Debug, Clone)]
  pub struct Cell {
  ```

  It currently has `#[derive(Debug, Clone)]` — confirm it already does. If `Clone` is missing, add it. Run:

  ```sh
  grep -n 'struct Cell' crates/cell-sheet-core/src/model.rs
  grep -n 'struct Sheet' crates/cell-sheet-core/src/model.rs
  grep -n 'struct DepGraph' crates/cell-sheet-core/src/formula/deps.rs
  ```

  Then check each `#[derive(...)]` line above those structs and add `Clone` wherever it is absent.

  The correct derive lines after the change:

  ```rust
  // model.rs — Cell
  #[derive(Debug, Clone)]
  pub struct Cell { ... }

  // model.rs — Sheet
  #[derive(Debug, Clone)]
  pub struct Sheet { ... }

  // deps.rs — DepGraph
  #[derive(Debug, Clone)]
  pub struct DepGraph { ... }
  ```

  `DepGraph` currently has no `#[derive]` at all — add the attribute above its `pub struct DepGraph` line:

  ```rust
  #[derive(Debug, Clone)]
  pub struct DepGraph {
      pub dependents: HashMap<CellPos, HashSet<CellPos>>,
      pub dependencies: HashMap<CellPos, HashSet<CellPos>>,
  }
  ```

- [ ] **Step 2: Verify compilation**

  ```sh
  cargo build -p cell-sheet-core
  ```

  Expected: compiles with no errors or warnings.

- [ ] **Step 3: Run tests to confirm nothing broke**

  ```sh
  cargo test -p cell-sheet-core
  ```

  Expected: all tests pass.

- [ ] **Step 4: Commit**

  ```sh
  git add crates/cell-sheet-core/src/model.rs \
          crates/cell-sheet-core/src/formula/deps.rs
  git commit -m "chore: derive Clone for Sheet, Cell, DepGraph (needed for benches)"
  ```

---

## Task 2: Add criterion dependency and create the bench skeleton

**Files:**
- Modify: `crates/cell-sheet-core/Cargo.toml`
- Create: `crates/cell-sheet-core/benches/core.rs`

- [ ] **Step 1: Add criterion to `Cargo.toml`**

  In `crates/cell-sheet-core/Cargo.toml`, add to `[dev-dependencies]`:

  ```toml
  criterion = "0.8"
  ```

  And add the bench entry (at the bottom of the file, after `[dev-dependencies]`):

  ```toml
  [[bench]]
  name = "core"
  harness = false
  ```

  The `harness = false` is required because criterion provides its own `main`.

- [ ] **Step 2: Create `benches/core.rs` with skeleton**

  Create `crates/cell-sheet-core/benches/core.rs`:

  ```rust
  use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
  use std::io::Cursor;

  use cell_sheet_core::formula::deps::{mark_dirty, recalculate, set_formula, DepGraph};
  use cell_sheet_core::io::csv::read_csv;
  use cell_sheet_core::model::{col_index_to_label, Sheet};

  fn bench_csv_load_100k(c: &mut Criterion) {
      todo!()
  }

  fn bench_formula_recalc_10k(c: &mut Criterion) {
      todo!()
  }

  fn bench_mark_dirty_chain(c: &mut Criterion) {
      todo!()
  }

  fn bench_recalculate_wide_dag(c: &mut Criterion) {
      todo!()
  }

  fn bench_range_sum_10k(c: &mut Criterion) {
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
  ```

- [ ] **Step 3: Confirm the skeleton compiles (not runs)**

  ```sh
  cargo bench --no-run -p cell-sheet-core
  ```

  Expected: compiles successfully. (It will panic at runtime because of `todo!()`, but compile is all we need to verify the wiring.)

- [ ] **Step 4: Commit**

  ```sh
  git add crates/cell-sheet-core/Cargo.toml \
          crates/cell-sheet-core/benches/core.rs
  git commit -m "chore: add criterion dependency and bench skeleton"
  ```

---

## Task 3: Implement all five benchmarks

**Files:**
- Modify: `crates/cell-sheet-core/benches/core.rs`

Replace each `todo!()` stub with the implementation below. Replace the entire file with:

```rust
use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
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
            |(mut s, d)| black_box(recalculate(&mut s, &d)),
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
            |(mut s, d)| black_box(mark_dirty(&mut s, &d, (0, 0))),
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
            |(mut s, d)| black_box(recalculate(&mut s, &d)),
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
            |(mut s, d)| black_box(recalculate(&mut s, &d)),
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
```

- [ ] **Step 1: Write the benchmark implementations** (replace the whole file as above)

- [ ] **Step 2: Check formatting and lints**

  ```sh
  cargo fmt --all
  cargo clippy --workspace --all-targets --all-features
  ```

  Expected: no errors, no warnings.

- [ ] **Step 3: Run the benchmark suite**

  ```sh
  cargo bench -p cell-sheet-core
  ```

  Expected: all five benchmarks complete and print timing output similar to:

  ```
  csv_load_100k           time:   [...]
  formula_recalc_10k      time:   [...]
  mark_dirty_chain        time:   [...]
  recalculate_wide_dag    time:   [...]
  range_sum_10k           time:   [...]
  ```

  No panics, no `#[...]` errors. If criterion reports a warming-up message that is normal.

- [ ] **Step 4: Commit**

  ```sh
  git add crates/cell-sheet-core/benches/core.rs
  git commit -m "feat: add criterion benchmark suite for cell-sheet-core"
  ```

---

## Task 4: Update CI to compile benchmarks

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Add a `bench` job**

  In `.github/workflows/ci.yml`, append the following job after the `build` job:

  ```yaml
    bench:
      name: Bench (compile-only)
      runs-on: ubuntu-latest
      steps:
        - uses: actions/checkout@v5
        - uses: dtolnay/rust-toolchain@stable
        - uses: Swatinem/rust-cache@v2
        - run: cargo bench --no-run -p cell-sheet-core
  ```

  The indentation must match the other jobs (two-space indent for the job name, four-space for `steps`, six-space for each step).

- [ ] **Step 2: Verify the YAML is valid**

  ```sh
  cat .github/workflows/ci.yml
  ```

  Confirm the file is well-formed by eye — four peer-level jobs (`fmt`, `clippy`, `test`, `build`, `bench`) under `jobs:`.

- [ ] **Step 3: Commit**

  ```sh
  git add .github/workflows/ci.yml
  git commit -m "ci: compile cell-sheet-core benchmarks in CI"
  ```

---

## Task 5: Write BENCH.md, update CHANGELOG, and verify

**Files:**
- Create: `BENCH.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Create `BENCH.md`**

  Create at repo root:

  ````markdown
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
  | `formula_recalc_10k` | `recalculate` with 10 000 dirty formula cells (each `=A{n}+1`) |
  | `mark_dirty_chain` | BFS dirty propagation on a 1 000-cell linear dependency chain |
  | `recalculate_wide_dag` | Topological sort on 1 000 cells all referencing a single source cell |
  | `range_sum_10k` | `SUM(A1:A10000)` parse and evaluation |

  ## Results

  Fill in this table locally after running the suite. Include your machine
  spec and the date so results are comparable across PRs.

  | Benchmark | Mean | Machine | Date |
  |---|---|---|---|
  | csv_load_100k | | | |
  | formula_recalc_10k | | | |
  | mark_dirty_chain | | | |
  | recalculate_wide_dag | | | |
  | range_sum_10k | | | |
  ````

- [ ] **Step 2: Update `CHANGELOG.md`**

  Under `## Unreleased`, inside the `### Added` block, append:

  ```markdown
  - Criterion benchmark suite in `cell-sheet-core` (`cargo bench -p cell-sheet-core`):
    `csv_load_100k`, `formula_recalc_10k`, `mark_dirty_chain`,
    `recalculate_wide_dag`, `range_sum_10k`. CI compiles the suite on every
    push to prevent API-breakage regressions. See `BENCH.md` for how to run
    and record results. Closes #7.
  ```

- [ ] **Step 3: Final verification**

  ```sh
  cargo fmt --all
  cargo clippy --workspace --all-targets --all-features
  cargo test
  cargo bench --no-run -p cell-sheet-core
  ```

  All four must pass clean with no warnings.

- [ ] **Step 4: Commit**

  ```sh
  git add BENCH.md CHANGELOG.md
  git commit -m "docs: add BENCH.md and changelog entry for benchmark suite (closes #7)"
  ```
