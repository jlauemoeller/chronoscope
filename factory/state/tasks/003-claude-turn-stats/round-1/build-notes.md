STATUS: DONE

## Changes
- `src/harness/claude/assemble.rs`: the per-turn `Builder` now tracks `counted` (distinct message ids whose model is not `<synthetic>`), `start` (prompt timestamp, else the first assistant entry's own timestamp) and `duration` (set by the latest `turn_duration` entry, never summed). `Assembler::prompt_ts` carries a prompt's usable timestamp from `start_user_turn` (user prompt or human `queued_command`) to the next Assistant turn; compactions drop it. `on_turn_duration` computes end minus start, falling back to `durationMs` when either timestamp is unusable or the span is negative; with neither it sets nothing. `on_assistant` refreshes count and tokens of an existing footer and marks the turn dirty, so late messages send `Updated`. Added `usable_timestamp` (shared with `timestamp()`).
- `src/harness/claude/wire.rs`: removed `message_count` from `Entry`.
- `src/model/mod.rs`: `TurnStats.message_count` doc no longer says "as reported by the harness".
- `tests/claude_assembly.rs`: updated the fixture expectations (counts 3 and 2; durations unchanged at 30 s and 10 s) and the live footer test (6 s); added the new tests below. No snapshot changed.

## Self-check
- AC: several footers + large messageCount, wall-clock duration, per-turn count — `several_footers_give_wall_clock_time_and_the_count_is_per_turn`
- AC: fallbacks and synthetic — `a_turn_after_a_compaction_starts_at_its_first_assistant_entry`, `a_compaction_ends_the_association_with_the_prompt`, `a_transcript_starting_with_an_assistant_entry_starts_at_that_entry`, `a_prompt_without_a_usable_timestamp_falls_back_to_the_first_assistant_entry`, `a_footer_without_a_usable_timestamp_uses_its_duration_ms`, `a_start_without_a_usable_timestamp_uses_the_duration_ms`, `synthetic_messages_are_left_out_of_the_count`, `a_turn_of_only_synthetic_messages_has_a_count_of_zero`
- AC: spec example 6 m 51 s — `the_spec_example_runs_six_minutes_and_fifty_one_seconds`
- AC: non-human queued_command — `a_non_human_queued_command_neither_starts_a_turn_nor_resets_the_start`
- AC: human queued_command starts the turn — `a_human_queued_command_is_the_start_of_the_assistant_turn_after_it`
- AC: late message Updated, chunk vs line — `a_message_after_a_footer_updates_the_count`, `a_late_message_gives_the_same_turns_by_chunk_and_line_by_line`
- AC: end before start — `an_end_before_the_start_falls_back_to_the_duration_ms`
- AC: stats None before footer — `there_is_no_footer_before_a_turn_duration_entry`
- AC: messageCount not read — field removed from `wire::Entry`; `a_message_count_of_the_wrong_type_does_not_fail_the_line`
- AC: doc updated — `src/model/mod.rs`
- AC: OpenCode unchanged — no OpenCode file or test edited; its tests pass
- AC: snapshots — none changed (git status shows no `.snap` diff)
- AC: budgets — `cargo test --release --test budgets -- --ignored budgets --nocapture`: first content 2.2 ms, full parse worst 366 ms, memory 0.83x, frames under 3 ms
- UI: none

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (all suites, 0 failed)
