VERDICT: PASS

## Summary
The Claude footer now counts the turn's own distinct non-synthetic messages and measures wall-clock time from the prompt (or first assistant entry) to the latest `turn_duration`, with the specified `durationMs` fallbacks, and every acceptance criterion is met. No snapshot or OpenCode file changed, no `.snap.new` file exists, and the new tests separate old behaviour from new. The release budgets also pass (`cargo test --release -- --ignored budgets --nocapture`): first content worst 1.8 ms, full parse worst 555.6 ms, memory 0.83x, frames worst 1.01 ms.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass

## Findings
1. [nit] `tests/claude_assembly.rs` (`messages_without_an_id_are_counted_by_uuid_then_line`) — every line in this test has a uuid, so the line-offset fallback named in the test and in the planner notes is never exercised. The `push` helper also gives every line the range `0..len`, so a no-uuid line fed through `assembled()` would always be `L0`. Change: add a case with no `message.id` and no uuid, fed through `push_line` with distinct ranges or through `push_chunk`, asserting that each line counts separately; or rename the test to cover only the uuid fallback.
2. [nit] `src/harness/claude/assemble.rs:659` — no test covers `prompt_ts` being dropped when an `isCompactSummary` entry arrives without a boundary. `a_compaction_ends_the_association_with_the_prompt` only goes through `on_compact_boundary` (line 632), so removing line 659 would fail no test. Change: add a variant of that test without `COMPACT_BOUNDARY`, asserting that the duration starts at the first assistant entry.
3. [nit] `tests/claude_assembly.rs` (`turn_duration_becomes_the_stats_footer`) — in `tests/fixtures/claude/session.jsonl` the footers' `durationMs` (30000 and 10000) equal the wall-clock spans, so the duration assertions and the comment about the span cannot tell the two methods apart. The constructed tests cover the behaviour. Change: none required; to make the fixture discriminate, set a `durationMs` that differs from the span on one footer, provided no snapshot changes.
4. [nit] `src/harness/claude/assemble.rs:707` — `usable_timestamp(&entry)` parses the timestamp again on every assistant line, straight after `Assembler::timestamp()` parsed it, but the value is only used when a new `Builder` is created. The cost is small and the budgets hold. Change: compute it only when needed, e.g. `let usable = self.cur.is_none().then(|| usable_timestamp(&entry)).flatten();`.
5. [nit] `src/harness/claude/wire.rs:48,60` — `timestamp: Option<String>` and `duration_ms: Option<u64>` still fail the whole line on a wrong JSON type, so a `turn_duration` entry with, say, a fractional `durationMs` loses a footer that its timestamps could have produced. This behaviour predates the task and the acceptance criteria only require it for `messageCount`. Change: none required here; consider a lenient deserializer for these two fields if such data is ever observed.
