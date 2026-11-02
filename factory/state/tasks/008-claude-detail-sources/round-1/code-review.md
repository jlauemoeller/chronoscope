VERDICT: FAIL

## Summary
The reader, references, harness method and composition are well built, and their behaviour matches the spec and the planner notes. Field locations also match local Claude transcripts. Two things block: the reference table makes assembly quadratic in the items of a turn, which breaks the §9.2 full-parse budget on a long single turn, and three of the new reference links have no test that would fail if they were removed.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass
- cargo test --release -- --ignored budgets --nocapture (also required by §7): pass on the two shipped fixtures, but those fixtures do not exercise finding 1. Mixed: full parse 110.6 ms, peak heap 0.22x, worst frame 3.36 ms. Dense: full parse 317.2 ms, peak heap 0.63x, worst frame 0.98 ms.

## Findings
1. [must-fix] `src/model/sources.rs:175` (`TurnSources::link`, reached through `link_message_item` at line 137 and `link_line` at line 143) — every new text, thinking, notice, model-change or injected item scans the whole table with `rposition`, because a new id is by definition not found. The `!b.index.contains_key(&id)` guards at `src/harness/claude/assemble.rs:841`, `905`, `941` and `985` only skip items that already exist, so they do not prevent this. `add_call` avoids the scan for tool calls only, and tool calls still add to the table the other items scan. Assembly is therefore quadratic in a turn's items, unlike `main`.

   Measured with `Assembler::from_memory` in release, on one Assistant turn of one-line text messages:

   | Text items in one turn | File size | `main` | this branch |
   |---|---|---|---|
   | 20 000 | 8.3 MB | 103 ms | 452 ms |
   | 50 000 | 20.8 MB | 277 ms | 2 852 ms |
   | 100 000 | 41.7 MB | 546 ms | 13 102 ms |

   The 50 000 case exceeds the §9.2 budget of 2 s for a full parse at about a fifth of the 100 MB target. Tool results do not end a turn, so a long unattended run is one turn. The budget test misses this because its fixtures spread their items over many turns (6 856 turns in the dense one). The build notes ("Assembly stays linear in a turn's items") and the subject of commit `871b919` ("linear assembly") claim the opposite.

   Change:
   - Remove the per-item lookup during assembly. Either make `link_message_item` and `link_line` push without looking (their callers already know whether the item is new; `get` uses `rfind`, so a repeated entry after a notice is removed and added again stays harmless), or keep a `HashMap<ItemId, usize>` beside `items` and use it in `link` and `get`. `HeldItem::link` and the `link_line` calls at `assemble.rs:609` and `635` must get the same treatment.
   - Add a regression check that would catch this, for example a single-turn scenario in `tests/budgets.rs` or a test on `TurnSources` that does not scan.
   - Correct the claim in `build-notes.md`.

   In a scratch copy, making only `link_message_item` push without looking gave 129 ms at 50 000 items and 277 ms at 100 000, so the scan is the cause.

2. [must-fix] `src/harness/claude/assemble.rs:986`, `842` and `609`; tests belong in `tests/claude_details.rs` — three required reference links have no effective test. In a scratch copy I removed all three together: the link for a notice, for the `model → …` note and for an injected run inside an Assistant turn. `cargo test` still passed in full. With those links gone, the sections lose their Message and/or Environment rows without any test failing. §5.3 requires the Message group for notices and Environment for every section with a source, and the planner notes name the notice and the `model → …` note under "Which line". The code itself works today: a scratch probe showed correct rows for all three.

   Change: add adapter tests that read each section through `section_source` or `TurnSources::get` and `section_details`, and that fail if the link is dropped:
   - A notice from a two-line message whose second line has `stop_reason: "max_tokens"`, asserting the Message rows come from that line (`stop_reason=max_tokens`, and the second line's `timestamp`) and that Environment comes from it too.
   - A Claude API error notice (`isApiErrorMessage: true`), asserting it has Message and Environment rows.
   - A `model → …` note, asserting `SourceKind::Other` and Environment from its message's line.
   - An injected run flushed into an Assistant turn (attachment between two assistant messages), asserting Environment from the attachment's line. The existing test only covers the User turn path.

3. [nit] `src/components/details_overlay/content.rs:119` (`label_width`) — `unavailable` is stored as a label, so it counts towards the label column. When every other label is shorter than 11 cells (for example a Prompt group whose source is gone), the values of the other rows move right. The row itself matches the task's UI section: one muted word at the row column with no value. The widening comes from task 007's accepted component and its `unavailable_80x24_default` snapshot, and fixing it here would conflict with this task's limit on snapshot changes, so I do not block on it. Change, if wanted in a later task: leave rows with an empty value and `muted` set out of `label_width`, and re-accept that one snapshot.

4. [nit] `src/harness/claude/assemble.rs:964` — when a `tool_use` id that was already seen in an earlier turn appears in a later turn, `known.source` belongs to the earlier turn's table, so the later turn's call gets no entry. A probe showed `section_source` returns `None` for it, and the overlay falls back to the model's rows. Results already route to the first turn through `self.calls`, and that part is older than this task. Change: call `add_call` unless `known.turn == b.idx`.

5. [nit] `src/model/sources.rs` — unused or over-wide API. `TurnSources::link_call` (line 151) and `is_empty` (line 228) are used only by tests. The test `a_call_has_two_lines_and_links_are_not_replaced` exercises `link_call`, not the `add_call` the assembler actually uses. `SourceValue::Duration` has no producer yet. The `bool` returned by `link_line`, `link_message_item`, `set_own`, `set_result` and `HeldItem::link` is ignored at every non-test call site. Change: remove `link_call` and `is_empty`, point the test at `add_call`, add `SourceValue::Duration` when OpenCode needs it, and drop the unused return values.

6. [nit] `src/harness/claude/assemble.rs:820` — `contains_key` followed by `get` and `insert` on `b.messages` does up to three lookups for each line. Change: use one `get` and derive `first_sight` from whether it returned a value, or use the entry API.

7. [nit] `src/harness/claude/details.rs:332` and `src/harness/mod.rs:108` — the `let _ = value;` lines exist only to silence unused parameters. Change: name the parameters `_value` and `_source`. Also in `details.rs:84`, `cache_miss_reason.clone()` can be `as_deref()`, since the value is only read.

8. [nit] `src/harness/claude/details.rs:196` (`Rows::text`) — an empty string gives a row with a label and a blank value, although the assembler treats an empty `stop_reason` as no reason and `rows.rs` filters an empty `effort`. No field was empty in the local transcripts I sampled, so this is only about consistency. Change: treat an empty string as missing.
