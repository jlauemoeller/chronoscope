VERDICT: PASS

## Summary
Round 2 closes the dangling-symlink gap in the harness-data guard (§9.5). `resolved()` in `src/app/save.rs` now follows links whose targets do not exist yet, and a new update test would fail without that fix. Every acceptance criterion is met and covered by an effective test, the round-1 nits are handled, and the gate is green.

## Acceptance criteria
- [x] `w`/`Ctrl-s` save as displayed, `W` saves full content, `e`/`Ctrl-e` export as folded, `E` exports full, and each opens the path prompt with the right default name.
  - Implemented in `Transcript::on` (`src/components/transcript/mod.rs`) and in `App::save_section` / `App::export_session` (`src/app/save.rs`).
  - Covered by `browser_tests::save::w_ctrl_s_and_capital_w_ask_for_a_path_with_the_default_save_name` (`./brio-85a86657-shell.txt`) and `e_ctrl_e_and_capital_e_ask_for_a_path_with_the_default_export_name` (`./brio-20261021-0835-85a86657.md`), plus `transcript::tests::save::*`.
  - Unchanged since round 1.
- [x] A path that does not exist is written at once. An existing path asks `Overwrite <path>? (y/n)`, `y` overwrites, and `n`/`Esc` leave the file untouched.
  - Implemented in `path_chosen`, `write_prompt_submit` and `write_prompt_cancel`.
  - Covered by `a_path_that_does_not_exist_is_written_at_once`, `a_path_that_exists_asks_before_overwriting_and_writes_nothing_yet`, `y_overwrites`, `n_leaves_the_file_untouched`, `esc_leaves_the_file_untouched` and `any_other_key_than_y_is_a_no`, all in a temp dir.
- [x] Folded export: folded sections appear as their summary lines, unfolded ones as displayed, and hidden system content is left out.
  - Implemented in `section_markdown` / `unfolded_tool` (`src/components/turn_block/export.rs`).
  - Covered by the golden test `the_folded_export_is_a_golden_file`, which was re-accepted with the escaped summary `> Read the &lt;dump> &amp; friends`, and by `hidden_system_content_is_omitted_from_the_folded_export` and the new `folded_summaries_escape_what_a_renderer_would_take_for_markup`.
- [x] Full export: every tool call is a `<details>` block with fenced input and output, outputs are untruncated (lazy ones included), and system content is included.
  - Covered by the golden test `the_full_export_is_a_golden_file`, plus `the_full_export_has_untruncated_outputs_including_lazy_ones` and `the_full_export_includes_system_content_whatever_the_toggle`.
  - Unchanged since round 1.
- [x] A fence is longer than any backtick run in its content.
  - Implemented in `fence_length` / `fenced`.
  - Covered by `a_fence_is_longer_than_the_longest_run_of_backticks_in_its_content`.
- [x] Success and failure toasts use the planner texts, and writing into the Claude data dir is refused.
  - Toast texts: `WriteKind::done` / `failed`. `done` now cuts a long path at its start with `tail_fitting`, which still gives the exact planner text for paths that fit; covered by `a_success_toast_keeps_the_end_of_a_long_path`.
  - Claude data dir: covered by `the_claude_data_directory_is_refused`.
  - Dangling-link bypass, now closed:
    - `resolved_in` reads the link with `read_link` at each component it appends and resolves again from where the link points.
    - Covered by the update test `a_dangling_symlink_into_harness_data_is_refused_without_asking`, with an absolute and a relative link. It asserts the `Refusing to write into harness data` toast, that no prompt is mounted, and that no file is created. Without the fix the overwrite prompt would open and this test would fail.
    - Also covered by the unit tests `a_dangling_symlink_is_followed_to_where_it_points` (absolute, relative, chain, link to a missing directory) and `a_loop_of_symlinks_ends`.
- [x] The default-name sanitising turns `My Project/x` into `My-Project-x`.
  - Covered by `app::save::tests::a_name_is_sanitised_character_by_character`.
- [x] Every UI state has a snapshot: `save_prompt_80x24_text`/`_styled`, `export_prompt_120x40_text`, `overwrite_confirmation_80x24_text`, and `exported_toast_80x24_text`.
  - All are unchanged since round 1 and pass with `INSTA_UPDATE=no`.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass. I re-ran all three, with 0 failures and no `.snap.new` files.

## Findings
1. [nit] `doc/save-and-export.md`, "The markdown export" paragraph: it says a folded prompt is written "as `y` copies them". Since round 2, folded summary lines escape `&` and `<`, so `<dump>` is written as `&lt;dump>` and no longer matches the copied text byte for byte. Change: add a short clause saying that `&` and `<` in folded summary lines and in bold descriptions are written as `&amp;` / `&lt;`. The module doc in `export.rs` already says this.
2. [nit] **Security** `src/app/save.rs` `resolved_in`: when the hop limit runs out (`hops == 0`), it returns the link path itself. That path is not treated as harness data, so a 40-link chain gets the overwrite prompt instead of a refusal. The write then fails with the OS's own symlink-loop error, so nothing is ever written. Change, optional: when `hops == 0`, treat the path as unresolvable and refuse it, for example by having `is_harness_data` return true.
3. [nit] `src/app/save.rs` `a_loop_of_symlinks_ends` only checks that the call returns. Change, optional: also assert that the result is one of `real.join("a")` / `real.join("b")`, so that the test documents what the function returns.
4. [nit] **Security** `src/app/save.rs` `is_harness_data` / `path_chosen`: a hard link in the working directory to an existing file under a harness data directory is not detected. `canonicalize` has no link to follow, so the user is asked to overwrite, and on `y` `std::fs::write` truncates the shared inode. This needs a planted hard link plus a confirmation, and the planner fixed the write method and the refusal rule, so it is outside this task's scope. Change, optional follow-up: on Unix, refuse an existing target whose `metadata().nlink() > 1`, or compare its `(dev, ino)` against files under the watch paths.
