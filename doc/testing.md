# Chronoscope — Testing

This file says what is tested in this project, how, and where the tests live. It is a living document: a cycle's docs task updates it when the cycle changes a rule. Rules are binding, and a rule that starts with `Prefer:` is a preference. `product §n` is a section of [product.md](product.md). The commands that run the tests are not here: the factory's stack profile holds them, and CI runs them as product §8.3 says.

## What is tested

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios, so parsing is verified on every OS in CI.
  - The one exception is OpenCode compaction: there is no OpenCode compaction fixture, because product §10 lists the OpenCode compaction format as unverified and the adapter produces no compaction. The Claude fixtures cover compaction.
- Component tests: every component ([architecture.md](architecture.md#components)) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the product §8.2 budgets.

## Where tests live

- Test code is test-only because of where it sits in the module tree, not because of its name.
- A file's own unit tests go in its `#[cfg(test)] mod tests`, written inline or in a file of their own (`foo.rs` → `foo/tests.rs`).
- Tests that cover several files of a module (for example rendering, copy and export) go in that module's `tests` module, one file per feature under `<module>/tests/`, as in `src/components/transcript/tests/`. The application's are laid out the same way: `src/app/tests.rs` is the `mod tests` of `src/app/mod.rs`, and declares one file per feature under `src/app/tests/` (`mouse.rs`, `search.rs`, `selector.rs`, …), with the browser's tests one level down, in `src/app/tests/browser.rs` and `src/app/tests/browser/*.rs`.
- The one exception is a `#[cfg(test)] pub(crate) mod fixtures` that the tests of other modules use. It may sit beside the code.
- Test helpers that the tests of several modules use (such as `buffer_to_text`) go in `src/test_support.rs`.

## Snapshots

- A UI change comes with snapshot tests: the view is rendered into a Ratatui `TestBackend` and snapshotted with `insta`.
- Snapshots stay where `insta` puts them by default: in a `snapshots/` directory next to the test file. No test changes the snapshot path. Moving a test module therefore moves and renames its snapshot files too.
- A test that takes one snapshot names none: it keeps the name `insta` gives it (crate, module path and test name).
- When a test needs snapshots of two different states, it is two tests, each with one snapshot. The one exception is a test that loops over cases, directly or through a helper, and takes one snapshot per case: it names each snapshot after its case, and nothing else names snapshots. A case may combine kinds, sizes, themes and states such as a scroll position, as in `every_kind_of_section_at_both_sizes_in_both_themes` and `content_taller_than_the_screen_at_80x24_scrolled_to_the_top_and_to_the_end` (`src/components/details_overlay/tests/snapshots.rs`), and in the tests of `src/components/turn_sidebar/tests/snapshots.rs` that call `snapshot_state`.
- Accepted `.snap` files are committed. Pending `.snap.new` files are never committed.
- Prefer: a snapshot test's name says the state it shows and, where it matters, the width (`folded_call_with_a_duration_at_80`).
