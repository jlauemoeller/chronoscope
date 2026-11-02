VERDICT: FAIL

## Summary
Every round-1 finding is addressed, the test gate and release budgets pass, only five new `.snap` files were added, and the wrapped footer still highlights search matches on the right cells. One regression blocks the work: the `RawValue` helpers added this round make a Claude line with invalid UTF-8 inside `message.usage` unparsed, where `main` parses it. This breaks the same binding planner note that round 1's `1e400` finding enforced. OpenCode is not affected.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass
- cargo test --release -- --ignored budgets --nocapture: pass (worst first content 1.8 ms, full parse 577.1 ms, frame 3.40 ms, peak memory 0.83x)

## Findings
1. [must-fix] `src/harness/shared.rs` `lenient_object` (used on `Message.usage`, `src/harness/claude/wire.rs:117`) — invalid UTF-8 anywhere inside `message.usage` now fails the whole line, including bytes in fields the app never reads. The line's content is lost. This breaks the binding planner note "A wire field added by this task must not turn a line or row that parses today into an unparsed one" and the §9.5 rule that changed harness data degrades rather than fails.

   The cause is that the Claude adapter parses lines with `serde_json::from_slice::<Entry>` (`src/harness/claude/assemble.rs:402`). `Box<RawValue>` has to be valid `str`, so its deserialization returns an error, and the `?` in `lenient_object` passes that error up to the whole `Entry`. On `main`, unknown fields inside `Usage` go through `IgnoredAny`, which skips strings without checking UTF-8.

   Such bytes are unlikely in real transcripts, because they need a corrupt file, but the regression is real. I ran the real `Assembler` from this branch and from `main`, with an otherwise identical line:

   | `message.usage` | `main` | Branch |
   |---|---|---|
   | `{"output_tokens":7,"service_tier":"\xff"}` | `unparsed_count` 0, text kept, 7 tokens | `unparsed_count` 1, no assistant turn |
   | `{"output_tokens":7,"x":{"y":["a\xc3"]}}` | `unparsed_count` 0, text kept, 7 tokens | `unparsed_count` 1, no assistant turn |

   OpenCode is not affected, because `Row.data` is already a `String`.

   Change: a line whose invalid UTF-8 appears only in `usage` fields must keep parsing, with its text item and its readable token figures. One way: in `parse_entry` (`src/harness/claude/assemble.rs:402`), when `from_slice::<Entry>` fails and `std::str::from_utf8(line)` also fails, retry once with `serde_json::from_str::<Entry>(&String::from_utf8_lossy(line))`. This runs only on the error path, so it costs valid lines nothing. On a scratch copy of the branch, this retry made both lines above parse with the text item and `output: 7`, and the full test suite still passed; I did not rerun the budgets on it. As a side effect, lines with invalid UTF-8 in `content`, which are unparsed on `main` too, would also parse with U+FFFD. Also add a sentence about this limit to the doc comment on `lenient`, which says only what the helper tolerates.

   Add a test to `tests/claude_assembly.rs`. Feed a byte line with `\xff` in an unknown string inside `usage`, by calling `asm.push_line` directly because the `push` helper takes `&str`. Assert `unparsed_count() == 0`, that the text item is kept, and that `output` is still counted.

2. [nit] `src/harness/claude/wire.rs:3-6` — the module doc says a known field with a different JSON type "still fails the line". That is no longer true for `usage` and its fields. Change: add that `usage` is read leniently, and a value of the wrong type there counts as missing.

3. [nit] `src/components/turn_block/renderers/stats.rs:60` — at text widths below 10, `textwrap` breaks the glued `word\u{a0}·` at character boundaries. Rows can then start with the separator or carry stray spaces: at width 9 `"  messages "` then `"  · 45.2k"`, at width 8 `"   · 45.2k"`, and at width 7 a row `"  ·"`. The test's comment limits its claim to width 10 and up, and blocks this narrow are already barely readable, so no change is needed unless the rule should hold at every width. Change, if wanted: state the width limit in the comment in `StatsSection::lines` too.

4. [nit] `src/harness/shared.rs` `lenient_object` — a duplicate key inside `usage` or `tokens` (`{"output_tokens":7,"cache_read_input_tokens":1,"cache_read_input_tokens":2}`) leaves the line parsed but makes the whole usage `None`, so the footer loses the token part. `main` shows 7 tokens for this input. A normal JSON writer does not produce duplicate keys, and the line's content is kept. Change: none needed; note it if the helper's doc comment is touched for finding 1.

5. [nit] `src/harness/claude/assemble.rs` `token_usage`, `src/harness/opencode/assemble.rs` `Tokens::usage` — carried over from round 1: a usage object with no readable number (`{}`) still shows `0 tokens`. The builder's reason for leaving it (planner note: a missing number counts as zero; `main` does the same for `{}`) holds. Change: none needed for this task.
