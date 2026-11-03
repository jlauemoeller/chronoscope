VERDICT: PASS

## Summary
The `[keys.turns]` context, the focus rules (including rewriting remembered focus when the sidebar is hidden), the shared `scroll_to_block` path, the hints and the help group all match the spec and planner notes, with effective tests for every acceptance criterion. The 12 new sidebar snapshots and the 2 changed help snapshots show only the intended changes. I also ran the release budgets: the slowest sidebar key press was 2.53 ms against the 16 ms budget, on both 100 MB fixtures. There are no must-fix findings, only small nits.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass

## Findings
1. [nit] `README.md:260` — the key syntax note still says chords work "in the `browser` and `selector` contexts", but `Context::supports_chords` now includes `turns` and the warning in `src/keymap/mod.rs` says "only in browser, selector and turns". Change: make the sentence read "in the `browser`, `selector` and `turns` contexts".
2. [nit] `src/components/transcript/paging.rs:41-66` — `]` / `[` now call `turn_stop` twice for the block they land on: once in `jump_turn`'s `find` and again in `scroll_to_block`. `turn_stop` renders the block, so this repeats work the old `find_map` did once. Change: split out a private helper that takes the block index and the already-found `Option<SectionId>`, keeping `scroll_to_block` as the one entry point for callers that have no stop yet.
3. [nit] `src/app/browser_tests/sidebar_keys.rs:562` — `a_turn_that_arrives_while_the_sidebar_has_focus_adds_a_row_and_moves_nothing` only covers a transcript away from the bottom. At the bottom, a scratch-copy probe showed the transcript following with the sidebar focused: the offset went from 87 to 92, the transcript's selection moved to the new turn, the row stayed on entry 19, and it went to entry 20 after `Tab`. That agrees with §6.4 and base §7.2, but the acceptance criterion's "without moving ... the transcript" doesn't state the exception and no test pins it. Change: add a test for the at-bottom case, asserting that the transcript follows and the sidebar's row stays.
4. [nit] `tests/budgets.rs:160` — the sidebar key press line prints `SIZE`, but the scenario lays out `bench_support::FRAME_SCREEN`. Both are 200x50 today, so the output is correct, but nothing ties them together. Change: print `FRAME_SCREEN`, or pass the size to `SidebarKeyScenario::new`.
5. [nit] `src/app/browser_tests/sidebar_keys.rs:879` — `the_selector_hidden_and_back_wide_does_not_give_the_sidebar_the_focus_back` opens the help overlay, not the selector. Change: rename it to say "overlay", or open the selector as the name says.
6. [nit] `src/components/transcript/tests/go_to_turn.rs:24` — the helper `comment_only` builds a link reference definition, not a comment. The build notes are right that this is needed: `html_block` in `src/markdown/blocks.rs:226` draws an HTML comment as a paragraph, so the spec's HTML comment example does have a row. Change: rename it, for example to `renders_no_rows`.
7. [nit] `src/components/turn_sidebar/mod.rs` — `selected_msg` sits in its own `impl TurnSidebar` block between the `Component` and `AppComponent` impls. Change: move it into the main inherent `impl` block next to `step`.
