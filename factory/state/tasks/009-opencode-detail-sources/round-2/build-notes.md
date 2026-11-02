STATUS: DONE

## Changes
- `src/harness/opencode/assemble.rs`: `upsert` now also updates the source entry of an item it replaces (`note_item(..., again)` -> `TurnSources::set_row_item`), so a tool part sent again by a later row is read from that row (start, duration and snapshot from one row). `RowError::has_text` is new. Comments state that held entries are always `Injected` items, which name their own row.
- `src/model/sources.rs`: `set_row_item`; the `SourceGroupKind` doc says a harness can add to the Turn group; `a_table_of_lines_has_no_row_references` now builds lines and a call; new test `an_item_sent_again_by_another_row_follows_that_row`.
- `src/harness/opencode/details.rs`: `content` is read per element (`RawValue`), parts that are not objects are skipped but keep their index, so a tool part with an id or found by `<row id>:<index>` is still found and nothing becomes `unavailable`. The `error` row is shown when the error has a usable `type` or `message`, or when `finish` is `error` (then it is the notice's text, including the `error` fallback). Each distinct row is read once (the turn's first row is shared with the section).
- `src/components/details_overlay/source.rs`: group table without the useless `Option`.
- `src/components/mod.rs`: `section_details` re-exported `pub(crate)` for tests only.
- `src/harness/opencode/details_tests.rs`: `a_tool_part_sent_again_by_a_later_row_is_read_from_that_row`, `parts_that_are_not_objects_do_not_hide_a_tool_part_or_move_it`, `an_error_that_says_nothing_is_no_row_unless_the_notice_says_error`; the share-one-reference test also asserts on the tool call's reference.

## Self-check
- Review: feature 1 (repeated part, two rows) / code none — `a_tool_part_sent_again_by_a_later_row_is_read_from_that_row` (fails with the `upsert` change removed)
- Review: feature 2, code 3, ui 1 (`error  error` placeholder) — `an_error_that_says_nothing_is_no_row_unless_the_notice_says_error`; `the_error_row_says_what_the_notice_of_the_same_row_says` still passes (finish `error`)
- Review: code 1, feature 3 (non-object sibling part) — `parts_that_are_not_objects_do_not_hide_a_tool_part_or_move_it`
- Review: code 2, feature 7 (row read three times) — rows read once; `find_part` still parses `data` for a tool part (not folded into one pass)
- Review: code 4, 5, 6, 7, 8, feature 5, 6 — doc, group table, test, `pub(crate)` re-export, comments, assertion on the call's reference
- Not done (nits): `SourceRow.replaces` stays an `Option<&'static str>` (code 9); WAL variant of the read-only test (feature 4); extra 80x24 scrolled and styled snapshots (ui 2, 3).
- UI: the two snapshots of round 1 are unchanged (`the_details_of_an_opencode_text_section_80x24_text`, `..._120x40_text`).

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (2897 passed, 0 failed; `INSTA_UPDATE=no`)
- cargo test --release -- --ignored budgets --nocapture: pass
