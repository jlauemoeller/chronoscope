# Scaffold the chronoscope crate

## Goal
Create the Rust project every later task builds on. It is a `chronoscope` package with a library target (`src/lib.rs`) that will hold all code, and a thin binary (`src/main.rs`) that only calls into the library. It declares the dependencies the spec names, pinned so the whole tree shares one `ratatui` (0.30) and one `crossterm` (0.29). It also sets up snapshot testing (`insta` + Ratatui `TestBackend`) with reusable render helpers, and the CI workflow of §9.3. Reviewers will render the library's widgets from a separate scratch crate through a path dependency, so the library must build cleanly on its own.

## Spec excerpts
From SPEC.md §2:

**Decision:** Rust + [tui-realm](https://github.com/veeso/tui-realm) 4.x (`tuirealm`, built on Ratatui 0.30 + crossterm) as the application framework, with custom components for every UI element (§1.3). ratcn is not a runtime dependency; its theme role model is adopted (see §8).

Candidate crates: `tuirealm` (with the `crossterm` feature), `ratatui`, `crossterm`, `serde`, `serde_json`, `toml`, `rusqlite` (bundled), `notify`, `textwrap`, `unicode-width`, `tui-input`, `arboard` (+ OSC52 fallback), `nucleo`, `clap`, `etcetera`.

From SPEC.md §1.1:

- The UI is a tui-realm `Application` (§2) driven by a single-threaded loop: the loop calls `Application::tick`, passes every returned `Msg` to `App::update`, and redraws only when something changed (at most every 50 ms, §7.1).
- Parsing, watching, stats computation and search indexing run on worker threads. Their results come back over channels, which sync Ports (`Poll`) read and turn into `Event::User` events. No async runtime is required; tui-realm's async ports (tokio) are not used.

From SPEC.md §1.3:

- Components are `pub` types that can be built from plain data (props and the theme) and rendered into a `TestBackend` frame on their own, without an `Application` or a terminal (§9.6).
- All colours and modifiers come from theme tokens (§8.1), passed in as a prop. Components never hard-code colours.
- `tui-realm-stdlib` is not used. The look in §4 and §8 is specific enough that wrapping stdlib components would mean overriding their rendering. Prompt text editing uses `tui-input` inside the `Prompt` component.

From SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

From SPEC.md §9.3:

- Hosted on GitHub.
- CI: a GitHub Actions matrix of `ubuntu-latest`, `macos-latest` and `windows-latest` runs `cargo fmt --check`, `cargo clippy -- -D warnings` and `cargo test`.
- Releases: tagged builds produce binaries for x86_64/aarch64 Linux, aarch64/x86_64 macOS and x86_64 Windows (e.g. via `cargo-dist`).

From SPEC.md §9.1:

Rules that keep tiers 2–3 cheap:
- No direct OS APIs; only cross-platform crates (crossterm, notify, arboard, rusqlite bundled, etcetera).
- All filesystem paths are `PathBuf`; no hard-coded `/` separators or `~` expansion by string concatenation.
- Harness data locations are resolved from a per-platform candidate list and can be overridden in config. The Claude location is `~/.claude/projects` on all platforms. The OpenCode location on macOS and Windows is **unverified** and must be confirmed before tier-2/3 release.
- Every harness adapter is tested against recorded fixture sessions (trimmed copies of real sessions), so parsing is verified on every OS in CI.

Planner notes (decisions not in the spec; binding for this task):
- Package name `chronoscope`, edition 2024. `src/lib.rs` is the library. `src/main.rs` calls one library function (e.g. `chronoscope::run()`) and maps its error to a non-zero exit code. For now that function parses the command line with `clap` (derive) and supports only `--help` and `--version` (name, version, one-line description). The full CLI is a later task.
- Dependencies to declare now:
  - Runtime, from the spec: `tuirealm` 4.1 with its `crossterm` feature (default features are fine), `ratatui` 0.30, `crossterm` 0.29, `serde` (derive), `serde_json`, `toml`, `toml_edit`, `rusqlite` (feature `bundled`), `notify` 8, `textwrap`, `unicode-width` 0.2, `tui-input` 0.15, `arboard`, `nucleo`, `clap` 4 (derive), `etcetera` 0.11, `pulldown-cmark`, `syntect`, `similar`.
  - Use `syntect` with `default-features = false` and the `default-fancy` feature (pure-Rust regex, no C dependency, keeps Windows CI simple).
  - Dev-dependencies: `insta`, `criterion`.
  - Crates the spec does not name are added by the task that first needs them.
- Run `cargo tree -d -e normal` and check that it lists no second version of `ratatui`, `ratatui-core` or `crossterm`. The UI reviewer's scratch crate must be able to depend on exactly `ratatui = "0.30"`.
- `.gitignore` already exists with other entries. Append `/target` and keep the existing lines.
- Test helpers live in a public library module `chronoscope::test_support` (marked `#[doc(hidden)]`), so unit tests, integration tests and the reviewers' scratch crate can all use them:
  - `render(width, height, |frame| ...) -> Buffer` draws with a `Terminal<TestBackend>`.
  - `buffer_to_text(&Buffer) -> String` returns one line per row, trailing spaces kept.
  - `buffer_to_styled(&Buffer) -> String` lists the text plus, for each row, runs of cells with identical style written as `fg/bg/modifiers`, so colour and modifier assertions are readable in a snapshot.
- CI: `.github/workflows/ci.yml` runs on push and pull_request. It uses a matrix of `ubuntu-latest`, `macos-latest` and `windows-latest`, and runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test`.
- Do not add placeholder modules for later features (no dead code).

## Acceptance criteria
- [ ] `Cargo.toml` defines package `chronoscope` (edition 2024) with a library target (`src/lib.rs`) and a binary target (`src/main.rs`), and declares every dependency listed in the planner notes. `cargo build` succeeds.
- [ ] `src/main.rs` contains no logic beyond calling the library entry point and turning an error into a non-zero exit code. `chronoscope --version` prints the crate version and `chronoscope --help` prints usage, both exiting with code 0. An integration test (e.g. `assert_cmd` or `std::process::Command` on `env!("CARGO_BIN_EXE_chronoscope")`) covers both.
- [ ] `cargo tree -d -e normal` shows no duplicate `ratatui`, `ratatui-core` or `crossterm` versions. Record the command output in the build notes.
- [ ] `.gitignore` contains `/target` and still contains every line it had before.
- [ ] `chronoscope::test_support` provides `render`, `buffer_to_text` and `buffer_to_styled` as described in the planner notes, each with a unit test.
- [ ] A smoke snapshot test renders a Ratatui `Block` with `BorderType::Rounded`, a left title and a right-aligned title into a 30x3 `TestBackend`. It has an accepted text snapshot and an accepted styled snapshot (the title in a non-default colour) under `snapshots/`, and no `.snap.new` files are committed.
- [ ] `.github/workflows/ci.yml` exists with the three-OS matrix and the three cargo commands from the planner notes.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
UI: none

## Depends on
none
