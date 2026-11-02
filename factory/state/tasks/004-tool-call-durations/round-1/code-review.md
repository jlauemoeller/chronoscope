VERDICT: FAIL

## Summary
The rendering, search, copy, save, export and generator work is correct and well tested, and the test gate and budgets pass. One regression blocks: the derived `#[serde(untagged)]` head on `toolUseResult` validates the whole value while buffering it, so result lines that parse on `main` now fail, and their calls stay `Pending` with no result. The other findings are nits.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass

Also re-run: `cargo test --release -- --ignored budgets --nocapture` passes on the regenerated v2 fixtures. Mixed: first content 2.3 ms, full parse 124.5 ms, slowest frame 3.15 ms, peak heap 0.22x. Dense: first content 1.7 ms, full parse 299.5 ms, slowest frame 1.03 ms, peak heap 0.58x.

## Findings
1. [must-fix] `src/harness/claude/wire.rs:64,76-80` (`Entry.tool_use_result`, `ToolUseResult`) — a `toolUseResult` that `main` skipped now fails the whole line.

   **Cause.** On `main` the field is unknown, so serde_json skips it with `ignore_value`, which does not check string contents or number ranges. Derived `untagged` first buffers the value into serde's `Content`, using the validating parser. A failure there comes from the deserializer itself, before either arm is tried, so the `IgnoredAny` arm never gets a chance.

   **Evidence.** I ran the same probe through `Assembler::from_memory` on exports of `main` and `a96db80`, with a clean `message.content` of `done`:

   | `toolUseResult` | `main` | branch |
   |---|---|---|
   | `{"stdout":"x"}` | Ok, result, 3s | Ok, result, 3s |
   | `{"stdout":"cut \ud83d here"}` | Ok, result, 3s | **Pending, no result** |
   | `"cut \ud83d here"` | Ok, result, 3s | **Pending, no result** |
   | `{"stdout":"bad \xff byte"}` (a raw invalid byte) | Ok, result, 3s | **Pending, no result** |
   | `{"n":1e999}` | Ok, result, 3s | **Pending, no result** |
   | `{"isAsync":true,"status":"async_launched"}` | Ok, result, 3s | Ok, result, no duration (correct) |
   | `{"isAsync":true,"prompt":"\ud83d"}` | Ok, result, 3s | **Pending, no result** |

   **Effect.** A feature that only adds a duration loses the tool's result. From reading `Assembler::parse_entry`, the failed line also goes through `skip_unparsed`, so it adds to the header's `N entries could not be parsed` note. This breaks §3's rule that the head must not fail the line, the planner note on `toolUseResult`, and base §9.5 (changed harness data degrades, it does not fail).

   **Realism.** A lone surrogate is plausible: a JavaScript string cut in the middle of a surrogate pair is written by `JSON.stringify` as a lone `\ud83d` escape, and `toolUseResult` holds fields that `message.content` does not repeat, such as `file.content` and `originalFile`. I did not measure how often this occurs in real transcripts.

   **Test gap.** No committed test gives `toolUseResult` anything but clean values, so the gate cannot see this. The budget run cannot either, because the generator writes only clean text.

   **Change, required behaviour.** Every line that parses on `main` must still parse and attach its result. Reading `toolUseResult` may fall back to "absent", and must never fail the `Entry`.

   **Change, tests.** Add every row of the table above to `tests/claude_assembly.rs`. Each must give `status == Ok` with the result attached. The non-async rows must keep the 3s duration, and both `isAsync: true` rows must have no duration. The raw byte needs a byte-level input, because `assemble_str` takes `&str`: use `Assembler::from_memory`.

   **Change, suggested fix (verify each case against `main`).**
   - Replace the derive with a hand-written `Deserialize` for `ToolUseResult` using `deserialize_any`.
   - `visit_map` reads `isAsync` through a lenient value (a bool gives `Some`, any other type is ignored and is not an error) and skips every other value with `IgnoredAny`. Nothing is buffered, which also removes the temporary `Content` copy of a very large value.
   - The other `visit_*` methods return `Other`.
   - `deserialize_any` still parses a top-level string with validation, so the `"cut \ud83d here"` row may keep failing. If it does, contain the read instead: leave the field off `Entry` as on `main`, and on lines whose bytes contain `"isAsync"` run a small separate head parse, treating any error as "not async".
   - Correct the `ToolUseResult` doc comment to match. Its "never materialised" claim is not true of the buffering arm today.

   **Note for the Approver.** The spec's literal "untagged" mechanism causes this, so the fix departs from that wording. The requirement behind the wording, that the line must not fail, should win.

2. [nit] `src/components/turn_block/renderers/tool/mod.rs:198` — the folded row styles the duration with `muted` alone, while the unfolded row uses `span.style.patch(muted)` (`timing.rs`, `mute_line_tail`). A theme override that gives `tool_description` a background or modifier therefore styles the duration differently in the two states. Change: use `style.patch(muted)` in the folded path.

3. [nit] `src/components/turn_block/renderers/tool/mod.rs:173` — `folded_description` takes `&self` but never uses it, and line 188 clones `first` even when the clone is not needed. Change: make it a free function, and build the description span only where it is used.

4. [nit] `src/components/turn_block/renderers/tool/mod.rs:321` — no test covers what `cut_row` adds, which is the `…` taking the style of the text it replaces. `a_row_that_leaves_no_room_for_the_description_is_cut_as_a_whole` checks text only. Change: assert the `…` cell's foreground in that test with `distinct_theme()`.

5. [nit] Stale documentation. Change:
   - `src/components/turn_block/renderers/tool/mod.rs:104`: the `ToolCallSection` doc should mention the duration.
   - `src/transcript_search.rs:16` (module doc) and `:266` (`section_text` doc): say the searched description is the drawn row, with the agent type, the duration, and the tool name when the description is blank.
   - `src/model/mod.rs:509`: the `ToolCall.duration` doc should say it is empty for an asynchronous launch.

6. [nit] `src/bench_support/generate.rs:461` — the room check `large_size + 1024 <= room` counts a large output once, but the line now writes it twice, so a fixture can overshoot its target by about one large output. Today's overshoot is small (104,861,111 bytes against a 104,857,600 target). Change: use `2 * large_size + 1024`. No further `GENERATOR_VERSION` bump is needed, because 2 is new on this branch.

7. [nit] `src/bench_support/tests.rs` — no test asserts that generated lines contain `toolUseResult`, so the budgets could silently stop covering this path. Change: add a test that a generated tool-result line has it, as an object for a successful call and as a string for a failed one.

8. [nit] `src/components/transcript/search.rs:951,964` — `a_description_row_ending_in_the_duration_is_found_as_the_source_line` checks the wrapped case through a helper, `view_wrapped`, defined between two tests. Change: make the wrapped case its own test.

9. [nit] `src/harness/claude/assemble.rs:967` — `(!is_async).then(|| ...).flatten()` is harder to read than the alternative. Change: use `if is_async { None } else { (ts - call.started).to_std().ok() }`.
