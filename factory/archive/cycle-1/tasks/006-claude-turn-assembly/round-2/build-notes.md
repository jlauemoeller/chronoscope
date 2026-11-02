STATUS: DONE

## Changes
Rework of round 1 (code-review FAIL on finding 1, plus nits). All changes are in `src/harness/claude/{assemble,wire,mod}.rs`, `src/model/output.rs` and `tests/claude_assembly.rs`.
- Block ids without `apiBlockIndex` (code-review 1, must-fix): text/thinking blocks of a message now get a per-message running number (`<message.id>:0`, `:1`, ...), so distinct blocks never collide. A block replaces the message's previous block only if that previous block is still the message's most recent block of any kind and it is a re-send: same kind and the text is identical or extended; for redacted thinking, the same `signature`. With `apiBlockIndex` the id is unchanged (`<message.id>:<apiBlockIndex + position>`). Checked against the real session cited in the review (`-focus/40f5827e-*.jsonl`): 95 of 95 thinking items now survive (scratch run, not committed).
- Synthetic/empty first model (code-review 2, feature-review 2): the turn's model and the initial `last_model` come from the first real model; a first real message after a placeholder fills `turn.model` without a `ModelChange`.
- `wire.rs` (code-review 3, 8): `isMeta` is `Option<bool>` (null tolerated), module doc now states precisely what is tolerated; `Block` derives `Default` and the text-content literal uses `..Block::default()`.
- `pub(crate)` for `extract_result_text` and `count_lines` (code-review 5).
- `push_chunk` searches for the newline only in newly added bytes (code-review 6).
- `push_line` doc: use `push_line` or `push_chunk`/`finish`, not both (code-review 7).
- Server tool results (feature-review 1): an object content of type `*_redacted_result` (real `advisor_tool_result`) yields empty output instead of the encrypted JSON; array items without `text` but with a `url` render as `title — url` (web search results).

Not changed (deliberately):
- code-review 4 (model depends on `harness::claude` via `LineFormat`): the review says "later, at the latest with the OpenCode adapter"; left as is, to be replaced by an injected extractor then.
- code-review 9: noted for task 036 (measure `Arc::make_mut` clone cost against the §9.2 budgets).
- feature-review 3 (origin-less `user` entries become User turns per SPEC rule 1) and 4 (real scrubbed fixture excerpts): for tasks 007/009; I cannot edit the task files.

## Self-check
- Review: code-review 1 — `without_api_block_index_distinct_blocks_of_a_message_all_survive_in_order` (thinking, text, tool_use, text, re-sent redacted thinking, text; all survive with distinct ids), `without_api_block_index_a_readable_thinking_then_text_keeps_both`, `without_api_block_index_a_grown_text_replaces_the_earlier_one` (replaces the old colliding test), `older_format_assembles_identically_by_line_and_in_random_chunks` (line-by-line, random chunks and update replay)
- Review: code-review 2 / feature-review 2 — `turn_starting_with_a_synthetic_message_takes_the_first_real_model_without_a_note`
- Review: code-review 3 — `null_meta_flag_does_not_fail_the_line`
- Review: feature-review 1 — `server_tool_result_in_an_assistant_line_becomes_the_result` (now the real redacted shape), `server_text_result_object_uses_its_text`, `web_search_results_render_as_title_and_url_lines`
- Review: code-review 5, 6, 7, 8 — refactors; covered by the existing chunking tests (`one_call_line_by_line_and_random_chunks_give_identical_turns`, `replaying_emitted_updates_reproduces_the_final_turns` with chunk size 1)
- All round 1 ACs unchanged and still covered (see round-1 build notes).
- UI: none

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (174 unit, 50 in `claude_assembly`, 0 failed)
