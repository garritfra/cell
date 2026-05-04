# Contributing to cell

Thanks for your interest in contributing! cell is a small project and changes
of all sizes are welcome — typo fixes, bug reports, new formulas, new modes,
docs, anything.

This guide covers how to get the project building locally, the conventions the
codebase follows, and what a good pull request looks like.

## Code of Conduct

Be kind. Assume good faith. Disagreements about technical direction are fine;
personal attacks are not. Maintainers reserve the right to close or remove
contributions that don't meet this bar.

## Ways to contribute

- **Report a bug** — open a [GitHub issue](https://github.com/garritfra/cell/issues)
  with a minimal reproduction (commands, sample data, expected vs. actual
  behavior, OS, and `cell --version`).
- **Suggest a feature** — for small, concrete requests, open an issue so we
  can discuss scope. For larger or uncertain ideas, start an
  [Ideas discussion](https://github.com/garritfra/cell/discussions/categories/ideas)
  first so we can explore the design before tracking implementation work.
- **Fix a bug or implement a feature** — see the workflow below.
- **Improve documentation** — README, CHANGELOG, doc comments, examples.

If you're looking for something to work on, the
[issues tab](https://github.com/garritfra/cell/issues) is the place to start.
Issues labelled `good first issue` are intentionally scoped for new
contributors.

## Development setup

You need a stable Rust toolchain. The project follows the latest stable
release; CI runs against `stable`.

```sh
git clone https://github.com/garritfra/cell.git
cd cell
cargo build
cargo test
```

To run the TUI from your checkout:

```sh
cargo run -- examples/demo.cell
```

To install the binary from your local checkout:

```sh
cargo install --path crates/cell-sheet-tui
```

### Useful commands

```sh
cargo build                                # build all crates
cargo test                                 # all tests (unit + integration)
cargo test -p cell-sheet-core              # core library only
cargo test -p cell-sheet-tui               # TUI crate only
cargo test -p cell-sheet-core -- col_label # single test by name
cargo fmt --all                            # auto-format
cargo fmt --all --check                    # check formatting (CI)
cargo clippy --workspace --all-targets --all-features
```

CI runs the same `fmt`, `clippy`, `test`, and `build` commands on Linux,
macOS, and Windows with `RUSTFLAGS=-Dwarnings`, so any warning will fail the
build. Run the commands locally before pushing.

## Project layout

cell is a Cargo workspace with two crates:

- `crates/cell-sheet-core` — pure data library: data model, formula engine
  (tokenizer → parser → AST → evaluator), dependency graph, file I/O for
  CSV/TSV and the native `.cell` format. **Must not depend on any TUI crate.**
- `crates/cell-sheet-tui` — terminal UI built on `ratatui` + `crossterm`:
  Vim modal editing, event loop, rendering, undo/redo, clipboard, viewport,
  and the headless CLI mode.

See [`CLAUDE.md`](CLAUDE.md) for a deeper architecture overview, including the
data flow on cell edits and the formula-engine pipeline.

### Conventions

- `CellPos` is `(usize, usize)` = `(row, col)`, zero-indexed.
- Column labels are Excel-style (`A`, `B`, …, `Z`, `AA`, `AB`, …); convert
  with `col_index_to_label` / `col_label_to_index`.
- Formulas always start with `=`. The `raw` field stores the original input;
  `value` stores the computed result.
- CSV export flattens formulas to computed values; the `.cell` format
  preserves them.
- `cell-sheet-core` must remain free of any TUI dependency. New TUI features
  belong in `cell-sheet-tui`; new evaluation, parsing, or storage logic
  belongs in `cell-sheet-core`.
- Each Vim mode has its own input handler under `crates/cell-sheet-tui/src/mode/`.
  Add new key sequences there rather than threading them through `app.rs`.

## Making changes

### Branching

Work on a feature branch in your fork. Branch names are flexible; the
convention used in this repo is `feat/<short-name>`, `fix/<short-name>`, or
`docs/<short-name>`.

### Tests

- Add tests for any behavior change. Bug fixes should include a regression
  test that fails before the fix and passes after.
- Prefer tests in `cell-sheet-core` when possible — they don't need a
  terminal and run on every platform in CI.
- For TUI changes, test the pure logic (action handlers, undo state,
  clipboard transformations) rather than the rendered output.

### Formatting and lints

Run before every commit:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --all-features
cargo test
```

Clippy must pass with no warnings — CI sets `-Dwarnings`. If a lint is
genuinely wrong for your case, use `#[allow(...)]` with a comment explaining
why rather than disabling it globally.

### Commit messages

This repo uses [Conventional Commits](https://www.conventionalcommits.org/)
prefixes:

- `feat:` — new user-visible behavior
- `fix:` — bug fix
- `docs:` — documentation only
- `ci:` — CI configuration
- `chore:` — repo maintenance, dependency bumps, etc.
- `release:` — version bumps and release prep (maintainers only)

Keep the subject line under ~72 characters and write it in the imperative
mood (`fix: make visual-mode d undoable`, not `fixed visual-mode d`). Use the
body to explain *why*, not *what* — the diff already shows what.

### Changelog

Update [`CHANGELOG.md`](CHANGELOG.md) under the `## Unreleased` section for
any user-visible change (new feature, bug fix, breaking change). Group
entries under `Added`, `Changed`, `Fixed`, `Removed`, or `Notes`. Reference
the PR with `(#NN)` once it's open. Pure refactors and internal cleanups
don't need a changelog entry.

## Submitting a pull request

1. Open or comment on an issue describing the problem you're solving, unless
   the change is trivial (typo, obvious bug fix).
2. Push your branch and open a pull request against `main`.
3. In the PR description, include:
   - **What** the change does and **why**.
   - Linked issues (`Fixes #NN`, `Refs #NN`).
   - For UI changes: a screenshot or short asciinema recording.
   - For new formulas or commands: an example and the expected result.
4. Ensure CI is green (`fmt`, `clippy`, `test` on Linux/macOS/Windows, `build`).
5. Be responsive to review feedback. It's normal for a PR to go through one
   or two rounds of revisions.

Small, focused PRs get reviewed faster than large ones. If you're working on
something big, consider splitting it into a series of commits or PRs.

## Areas that especially welcome help

- Additional formula functions toward [ODF spec](https://docs.oasis-open.org/office/OpenDocument/v1.3/os/part4-formula/OpenDocument-v1.3-os-part4-formula.html)
  compliance (math, text, date/time, statistical).
- Cell formatting (bold, italic, color) — see the comparison table in the
  README for parity goals with sc-im.
- Configuration file support (`~/.config/cell/config.toml` or similar).
- More import/export formats (XLSX, ODS, Markdown).
- Improved test coverage of TUI input handling.

If you have an idea that isn't on this list, open an issue for a scoped
request or start an
[Ideas discussion](https://github.com/garritfra/cell/discussions/categories/ideas)
for broader proposals.

## Releasing

Releases are cut by maintainers. The process is documented in the
[Releasing section of the README](README.md#releasing): bump the workspace
version in `Cargo.toml`, move `Unreleased` entries into the new version
section in `CHANGELOG.md`, commit, tag `vX.Y.Z`, and push the tag. The
release workflow handles binaries and `crates.io` publication.

## License

By contributing, you agree that your contributions will be licensed under the
[MIT License](LICENSE) that covers the project.
