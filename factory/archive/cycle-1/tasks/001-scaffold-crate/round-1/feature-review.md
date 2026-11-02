VERDICT: FAIL

## Summary
The scaffold works. I re-ran the gates myself: `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings` and `INSTA_UPDATE=no cargo test --locked` all pass (12 tests). `cargo tree -d -e normal` shows no second `ratatui`, `ratatui-core` or `crossterm`. One criterion is not met as written: the build notes describe the `cargo tree` output in prose instead of recording it. That is the only must-fix finding.

## Acceptance criteria
- [x] `Cargo.toml` defines package `chronoscope` (edition 2024) with lib (`src/lib.rs`) and bin (`src/main.rs`) targets and every planner-listed dependency. `tuirealm` 4.1 has the `crossterm` feature, `rusqlite` has `bundled`, and `syntect` has `default-features = false` plus `default-fancy`. Dev-deps are `insta` and `criterion`. `cargo build` succeeds, and so does the `--locked` clippy/test build.
- [x] `src/main.rs` only calls `chronoscope::run()` and maps `Err` to `ExitCode::FAILURE`. `--version` and `--help` exit 0. The tests in `tests/cli.rs` (`version_prints_crate_version_and_exits_zero`, `help_prints_usage_and_exits_zero`) run `CARGO_BIN_EXE_chronoscope` and check the exit code and stdout: the exact version string, plus usage and description for help.
- [ ] `cargo tree -d -e normal` shows no duplicate `ratatui`, `ratatui-core` or `crossterm`, which I confirmed independently: `-i crossterm` gives only v0.29.0, and `-i ratatui-core` gives only v0.1.2 (through ratatui v0.30.2). The criterion also says to record the command output in the build notes. `build-notes.md` only gives a prose list of crate names, with no versions and no raw output (see finding 1).
- [x] `.gitignore` keeps both original lines and adds `/target`.
- [x] `chronoscope::test_support` (`#[doc(hidden)] pub mod`) provides `render`, `buffer_to_text` and `buffer_to_styled`. Each has a unit test with an exact assertion that fails if the behaviour changes. Trailing spaces are kept, wide characters are not duplicated, and style runs are written as `fg/bg/modifiers`.
- [x] `tests/smoke_snapshot.rs` renders a rounded `Block` into 30x3 with a green left title and a right-aligned title. The accepted text and styled snapshots are in `tests/snapshots/`; the styled one shows `1..7 Green/Reset/NONE` for the title. No `.snap.new` or `.pending-snap` files are tracked.
- [x] `.github/workflows/ci.yml` runs on push and pull_request, with an ubuntu/macos/windows matrix running `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test`.
- [x] fmt, clippy (`--all-targets`, `-D warnings`) and test all pass locally.

## Findings
1. [must-fix] The acceptance criterion says "Record the command output in the build notes", but `build-notes.md` "Self-check" only paraphrases the `cargo tree -d -e normal` result. Change: in the next round's `build-notes.md`, add the raw output of `cargo tree -d -e normal` in a fenced code block. At minimum include the top-level lines with versions, which my run gave as:
   ```
   foldhash v0.1.5
   foldhash v0.2.0
   hashbrown v0.15.5 (*)
   hashbrown v0.17.1 (*)
   memchr v2.8.3
   memchr v2.8.3
   miniz_oxide v0.8.9
   miniz_oxide v0.9.1
   regex-automata v0.4.18 (*)
   regex-automata v0.4.18 (*)
   syn v2.0.119
   syn v3.0.9
   winnow v0.7.15
   winnow v1.0.4
   ```
   Add a line saying that none of these are `ratatui`, `ratatui-core` or `crossterm`. No code change is needed.
2. [nit] `src/lib.rs` `run()`: `Err(err) => err.exit()` calls `std::process::exit` from inside the library. The `Result` return type is never `Err` today, so the error-to-exit-code mapping in `main.rs` never runs. That is acceptable for this scaffold. Change: when the full CLI task lands, consider having `run` return `Result<ExitCode, _>` (or print with `err.print()` and return the clap exit code) so the library never exits the process itself and stays testable.
3. [nit] `Cargo.toml` has no `rust-version`, even though edition 2024 needs Rust 1.85 or later. Change: add `rust-version = "1.85"` (or the actual MSRV) to `[package]` so older toolchains get a clear error.
