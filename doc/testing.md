# Chronoscope — Testing

This file says what is tested in this project, how, and where the tests live. It is a living document: a cycle's docs task updates it when the cycle changes a rule. Rules are binding, and a rule that starts with `Prefer:` is a preference. `product §n` is a section of [product.md](product.md). The commands that run the tests are not here: the factory's stack profile holds them, and CI runs them as product §8.3 says.

## What is tested

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios, so parsing is verified on every OS in CI.
- Component tests: every component ([architecture.md](architecture.md#components)) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the product §8.2 budgets.

## Where tests live

- Test code is test-only because of where it sits in the module tree, not because of its name.
- A file's own unit tests go in its `#[cfg(test)] mod tests`, written inline or in a file of their own (`foo.rs` → `foo/tests.rs`).
- Tests that cover several files of a module (for example rendering, copy and export) go in that module's `tests` module, one file per feature under `<module>/tests/`, as in `src/components/transcript/tests/`.
- The one exception is a `#[cfg(test)] pub(crate) mod fixtures` that the tests of other modules use. It may sit beside the code.
- Test helpers that the tests of several modules use (such as `buffer_to_text`) go in `src/test_support.rs`.

## Snapshots

- A UI change comes with snapshot tests: the view is rendered into a Ratatui `TestBackend` and snapshotted with `insta`.
- Snapshots stay where `insta` puts them by default: in a `snapshots/` directory next to the test file, under the name `insta` gives them (crate, module path and test name). No test changes the snapshot path or name. Moving a test module therefore moves and renames its snapshot files too.
- Accepted `.snap` files are committed. Pending `.snap.new` files are never committed.
- Prefer: a snapshot test's name says the state it shows and, where it matters, the width (`folded_call_with_a_duration_at_80`).
