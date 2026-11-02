STATUS: DONE

## Changes
- `src/model/mod.rs`: `Item::Notice { id, level, text }` and `NoticeLevel::{Error, Warning}`.
- `src/components/turn_block/section.rs`: `SectionKind::Notice` (not foldable), `SectionContent::Notice`, derivation (blank text yields no section), `file_kind` (`error` / `notice`).
- `src/components/turn_block/renderers/text.rs` + `mod.rs`: `NoticeSection` (word-wrapped, not markdown, `with_lead` two spaces; `error_output` for Error, muted for Warning). Emphasis is the existing `emphasize`, unchanged.
- `src/transcript_search.rs`, `src/components/transcript/search.rs`: notice text is searched; not treated as markdown.
- `src/components/turn_block/export.rs`: a notice is written like the `model → …` note (`_text_`), emphasis per line so a blank line inside a notice does not break it. Copy and save go through the existing `displayed_text` / `full_text`.
- `src/harness/opencode/assemble.rs`: `AssistantData` reads `finish` and `error` tolerantly (`lenient`, `lenient_error`: object, or string as message, else missing); `ending_notice` builds the notice per §4.2; it is upserted after the row's parts with id `notice:<row id>`; a row going back to a normal ending removes it (`remove` rebuilds `Builder.index`). Stats unchanged.
- `tests/claude_injected.rs`: exhaustive match gained a `Notice` arm.

## Self-check
- AC: each OpenCode row of §4.2, in-place update giving one notice, streaming to error — `harness::opencode::notice_tests::{an_aborted_row_is_a_warning_with_the_message_when_there_is_one, another_error_type_is_an_error_with_whichever_parts_exist, another_finish_is_a_warning_naming_it, a_row_updated_in_place_has_one_notice_with_the_new_text, a_streaming_row_that_ends_in_error_gains_its_notice}`, `harness::opencode::tests::an_assistant_row_that_ended_in_error_loads_with_its_notice`
- AC: pinned texts (`aborted`; `aborted: <m>`; `<type>: <m>`; type only; message only; `stopped: <finish>`; `error`) — same tests plus `an_error_without_type_or_message_says_error`
- AC: `stop`, `tool-calls`, absent finish give no notice — `a_normal_ending_produces_no_notice`
- AC: abnormal to normal loses the notice, later in-place updates land right — `a_row_that_ends_normally_after_an_error_loses_its_notice`, `later_rows_of_the_current_turn_land_after_a_removed_notice`
- AC: `error` of unexpected JSON type leaves the row parsed — `an_error_of_an_unexpected_json_type_leaves_the_row_parsed`, `a_finish_of_an_unexpected_json_type_counts_as_absent`, `a_string_error_counts_as_its_message`
- AC: snapshots both levels, 80/120, wrapping — `components::turn_block::notice_snapshots::*` (24)
- AC: select, copy, save, export, search — `components::transcript::tests::notice::*`, `transcript_search::tests::a_notice_is_searched_by_its_text`
- AC: saved file word — `w_saves_a_notice_under_error_or_notice`, `turn_block::tests::a_notice_is_saved_as_error_or_notice`
- AC: not foldable, no marker — `a_notice_is_not_foldable_and_has_no_marker`, `turn_block::tests::a_notice_cannot_fold_and_is_never_folded`
- AC: layout-cache property tests include notices — `ItemSpec::Notice` in `transcript/tests/property.rs`
- AC: budgets — `cargo test --release -- --ignored budgets --nocapture`: within budget (full parse worst ~398 ms of 2000, frames worst 4.8 ms of 16)
- AC: snapshot changes limited to new notice snapshots — no existing `.snap` modified (the OpenCode fixture has only `finish: "stop"`).
- UI: error/warning × short/wrapping × selected / unselected dimmed / unselected undimmed @ 80 and 120 — `notice_snapshots::{error,warning}_{short,long}_{selected,dimmed,undimmed}_at_{80,120}` (styled snapshots)

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (all suites, 0 failed; lib 2281 passed)
