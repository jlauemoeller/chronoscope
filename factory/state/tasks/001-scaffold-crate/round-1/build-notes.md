STATUS: DONE

## Changes
- `Cargo.toml`: package `chronoscope` (edition 2024), lib + bin targets, all dependencies from the planner notes (syntect with `default-fancy`, no default features), dev-deps `insta` and `criterion`. `Cargo.lock` committed (binary crate).
- `src/main.rs`: only calls `chronoscope::run()` and maps `Err` to `ExitCode::FAILURE`.
- `src/lib.rs`: `run()` parses args with clap derive (`--help`, `--version` only; clap exits 0 for these, 2 for bad args); `#[doc(hidden)] pub mod test_support`.
- `src/test_support.rs`: `render`, `buffer_to_text`, `buffer_to_styled` (text, then a `--- styles (fg/bg/modifiers) ---` section with one line per row: `y: x0..x1 fg/bg/mods | ...`, x1 exclusive), each with unit tests.
- `tests/cli.rs`: runs the binary via `CARGO_BIN_EXE_chronoscope` for `--version`, `--help`, and an unknown argument.
- `tests/smoke_snapshot.rs` + `tests/snapshots/*.snap`: rounded Block with left title (green) and right-aligned title in 30x3, text and styled snapshots.
- `.gitignore`: appended `/target`.
- `.github/workflows/ci.yml`: ubuntu/macos/windows matrix, fmt, clippy --all-targets, test.

## Self-check
- AC: Cargo.toml / cargo build — `cargo build` succeeds; all tests compile against the declared crates.
- AC: main.rs thin, --version/--help exit 0 — `version_prints_crate_version_and_exits_zero`, `help_prints_usage_and_exits_zero` (tests/cli.rs); also `unknown_argument_exits_non_zero`.
- AC: no duplicate ratatui/ratatui-core/crossterm — `cargo tree -d -e normal` output lists only foldhash, hashbrown, memchr, miniz_oxide, regex-automata, syn, winnow (all unrelated). `cargo tree -e normal -i crossterm` -> crossterm v0.29.0 (single); `-i ratatui` -> ratatui v0.30.2; `-i ratatui-core` -> ratatui-core v0.1.2.
- AC: .gitignore — `/target` appended; diff shows only that addition.
- AC: test_support helpers — `render_returns_buffer_of_requested_size`, `text_has_one_line_per_row_and_keeps_trailing_spaces`, `text_does_not_duplicate_wide_characters`, `styled_lists_runs_of_identical_style`.
- AC: smoke snapshot — `rounded_block_with_titles_text`, `rounded_block_with_titles_styled` (snapshots in `tests/snapshots/`, no `.snap.new`).
- AC: CI workflow — `.github/workflows/ci.yml`.
- UI: none.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (12 passed, 0 failed)
