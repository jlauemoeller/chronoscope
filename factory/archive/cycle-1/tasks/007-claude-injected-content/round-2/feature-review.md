VERDICT: FAIL

## Summary
The round-1 must-fix is fixed. `flush_current` now merges entries flushed after the prompt into the User turn's trailing `Injected` item, and keeps the pre-prompt item separate. A live batch boundary therefore gives the same turns as a whole-file parse, and `a_batch_flush_between_consecutive_entries_in_a_user_turn_still_collapses_them` checks this. The round-1 nits were handled or reasonably declined, and the test gate passes. One test gap remains. The `user_flushed = false` reset in `start_user_turn`, which keeps pre-prompt and post-prompt entries apart in every User turn after the first, is not covered by any test. With the line removed, the full suite still passes, and later User turns then merge the two items.

## Acceptance criteria
- [ ] Allowlisted entries before a human prompt become an `Injected` item at the start of that User turn; mid-assistant entries are placed at that position. The behaviour at HEAD is correct (`start_user_turn`, `flush_current` in `src/harness/claude/assemble.rs`). It is covered by `entries_before_a_human_prompt_open_that_user_turn`, `entries_in_the_middle_of_assistant_activity_stay_at_that_position` and `entries_before_and_after_a_prompt_stay_in_separate_items`. The last of these only exercises the first User turn, so the separation in later turns has no test (finding 1).
- [x] Three consecutive allowlisted entries (with noise between them) collapse into one item with kind, timestamp and text. Covered by `three_entries_with_noise_between_collapse_into_one_item_with_kind_time_and_text` and `noise_does_not_break_a_run_of_injected_entries`. In User turns this now also holds across batch flushes and footers: `a_batch_flush_between_consecutive_entries_in_a_user_turn_still_collapses_them`, which compares both `asm.turns()` and the replayed updates against the whole parse, and `a_footer_between_entries_in_a_user_turn_does_not_split_them`.
- [x] A human `queued_command` starts a User turn and a new Assistant turn; a non-human one is injected. Unchanged since round 1. Covered by `human_queued_command_starts_a_user_turn_and_a_new_assistant_turn` and `queued_command_without_a_human_origin_is_injected`.
- [x] Every noise category produces no item and does not change the unparsed count. Covered by `noise_never_makes_an_item_and_is_not_counted`. The `deferred_tools_` prefix check moved into `is_known_noise_attachment`, and the table still includes `deferred_tools_delta`.
- [x] Metadata entries are skipped without counting. Covered by `metadata_entries_are_skipped_without_counting`.
- [x] A malformed line, an unknown type with a `message` and an unknown block each count once, and parsing continues. Covered by `malformed_line_unknown_type_with_message_and_unknown_block_each_count_once`.
- [x] An unknown attachment kind is skipped and logged with its count, not counted as unparsed. Covered by `unknown_attachment_kinds_are_counted_and_logged_once_each`; the type-less case by `an_attachment_without_a_type_is_counted_under_a_readable_label`.
- [x] `compact_boundary` + `isCompactSummary` give one `Turn::Compaction`, and the summary is not a prompt. Covered by `compact_boundary_and_summary_make_one_compaction_turn`.
- [x] A newer `version` warns exactly once per session and still parses. Covered by `a_newer_version_warns_exactly_once_per_session_and_still_parses`. `NEWEST_KNOWN_VERSION` = `2.1.314`, the newest version in the fixtures.
- [x] Task-006 tests pass, and random chunking gives identical results with injected content. Covered by `tests/claude_assembly.rs` and `random_chunks_give_identical_turns_with_injected_content_and_compaction`.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass. I re-ran all three; all pass.

## Findings
1. [must-fix] No test covers the `self.user_flushed = false;` reset in `start_user_turn` (`src/harness/claude/assemble.rs`, line 471).
   - In a scratch copy with that line removed, the whole suite still passes (all test binaries green). The build notes claim that "disabling ... the flag reset makes them fail"; that claim is wrong.
   - Without the reset, the flag stays `true` from an earlier User turn, so the next User turn's post-prompt entries merge into its pre-prompt item. The second User turn then shows one section where it should show two (AC 1 placement; SPEC §5.6 collapses only consecutive entries).
   - Example input: `prompt u1, hook A, reply r1, hook X, prompt u2, hook B, reply r2`. On the mutant, `user(&turns[2]).items` is `["injected"]`; at HEAD it is `["injected", "injected"]`.
   - `entries_before_and_after_a_prompt_stay_in_separate_items` only has one prompt, where the flag starts `false`, so it cannot catch this.
   - Change: add a test to `tests/claude_injected.rs` along these lines. I confirmed that it passes at HEAD and fails with the reset removed.
     ```rust
     #[test]
     fn entries_before_and_after_a_later_prompt_stay_in_separate_items() {
         let text = lines(&[
             prompt("u1", "one"),
             hook("a1", "A"),
             reply("r1", "msg_1", "hi"),
             hook("x2", "X"),
             prompt("u2", "two"),
             hook("b2", "B"),
             reply("r2", "msg_2", "ho"),
         ]);
         let turns = assemble(&text).into_turns();
         let items = &user(&turns[2]).items;
         assert_eq!(item_names(items), ["injected", "injected"]);
         assert_eq!(injected(&items[0])[0].text, "X");
         assert_eq!(injected(&items[1])[0].text, "B");
     }
     ```
     A live variant with `flush_held()` after `hook B` would be a useful addition, but it is optional.
2. [nit] The doc comment on `into_turns` (`src/harness/claude/assemble.rs`, around line 147) says held entries "are placed in the current one". After a trailing Compaction turn, or when there is no turn yet, they are dropped and only logged at debug. Change: say that they go into the current User/Assistant turn if there is one, and are otherwise dropped (logged at debug).
3. [nit] The new `log::debug!` in `end_of_input` runs on every `flush_held()` while entries stay held. In live mode, a transcript that ends with a compaction followed by attachments logs the same message once per batch until the next turn arrives. Low impact at debug level. Consider logging only from `finish` / `into_turns`, or only when the held count changes.
4. [nit] Unknown content blocks on a re-sent assistant line are still counted once per re-send (round-1 feature nit 4, declined with a stated reason: no real data shows it). This is acceptable for now. It is still an open item if the parse-failure note ever looks inflated.
