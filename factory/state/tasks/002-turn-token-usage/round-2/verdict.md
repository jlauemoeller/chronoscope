VERDICT: REWORK

## Assessment

**Code review: FAIL**
- Finding 1 (must-fix), invalid UTF-8 inside `message.usage` fails the whole Claude line: to be fixed. See below.
- Finding 2 (nit), stale module doc in `wire.rs`: not blocking on its own. It is a one-sentence fix, so the rework note includes it.
- Finding 3 (nit), the separator rule at text widths below 10: ignored as behaviour, because the UI review confirms these widths are unreachable (`MIN_WIDTH` is 60 columns). The rework note only corrects the comment that overstates the rule.
- Finding 4 (nit), a duplicate key inside `usage` makes the whole usage `None`: ignored. The line stays parsed and its content is kept, only the token part is lost, and a normal JSON writer does not produce duplicate keys. The rework note asks for one sentence in the helper's doc comment.
- Finding 5 (nit), `{}` shows `0 tokens`: ignored. It follows the planner's "a missing number counts as zero", and `main` does the same.

**Feature review: FAIL**
- Finding 1 (must-fix): the same regression as code finding 1, to be fixed.
- Findings 2, 3 and 4 (nits): the same as code findings 3, 2 and 5, handled as above.
- All acceptance criteria are ticked, so this is the only thing blocking the work.

**UI review: PASS**, no findings. Nothing to do.

**Why the must-fix is not ignored.** The input needs a corrupt file, and the line then goes through the ordinary "skipped and counted" path, so the impact is small. It still breaks the binding planner note "A wire field added by this task must not turn a line or row that parses today into an unparsed one". Round 1 was reworked for an equally unrealistic input (`1e400`) under the same note. A small fix exists that costs valid lines nothing, and rework rounds remain.

**The regression is wider than both reviews say.** Both name only the assembler's `parse_entry`. `Entry` is also parsed in `extract_result_text` (`src/harness/claude/wire.rs`) and in `prompt_of_user` and `prompt_of_queued` (`src/harness/claude/scan.rs`), and all of them inherit the failure. `extract_result_text` is the one that matters: `is_result` also matches `*_tool_result` blocks on assistant lines, which carry `usage`. A fix in the assembler alone would keep such a line, but materialising its large result would then return nothing. The fix therefore has to be shared.

**Independent check.** I ran the real code from scratch copies of `main` and this branch, and from a copy of this branch with the fix in the rework note applied. All inputs are bytes. "Parsed, 7" means `unparsed_count` 0, the text item kept and 7 output tokens; "unparsed" means `unparsed_count` 1 and no text item.

| Invalid bytes in | `main` | Branch | With the fix |
|---|---|---|---|
| `usage`, unknown string | parsed, 7 | **unparsed** | parsed, 7 |
| `usage`, unknown nested array | parsed, 7 | **unparsed** | parsed, 7 |
| `usage`, new field `cache_read_input_tokens` | parsed, 7 | **unparsed** | parsed, 7 |
| `usage.output_tokens_details`, unknown string | parsed, 7 | **unparsed** | parsed, 7 |
| `usage`, a key | unparsed | unparsed | parsed, 7 |
| text block | unparsed | unparsed | unparsed |
| `cwd` (known top-level string) | unparsed | unparsed | unparsed |
| `usage` and text block together | unparsed | unparsed | unparsed |
| unknown top-level field | parsed, 7 | parsed, 7 | parsed, 7 |
| `extract_result_text`, bad bytes in `usage` | `Some("hello")` | **`None`** | `Some("hello")` |
| `extract_result_text`, bad bytes in the result | `None` | `None` | `None` |
| Session scan `first_prompt`, bad bytes in `usage` of a user line | `Some("real")` | **`None`** | `Some("real")` |

On the fixed copy, clippy and the full test suite with `INSTA_UPDATE=no` pass (0 failed). I did not run the release budgets on it; the retry runs only after a line has already failed to parse.

## Conflicts

The two FAIL reviews want the same outcome and disagree on the method.

- The code review suggests retrying any line with invalid UTF-8 through `String::from_utf8_lossy`. It keeps the readable figures, but as a side effect lines with invalid UTF-8 in `content` would also parse.
- The feature review forbids that retry and suggests a helper that matches serde_json's error text. That helper loses the figures (`tokens == None`), and the review itself says a fix that keeps them is better.

Both suggestions are dropped, and neither review's actual demand is:

- The unguarded retry is dropped because it changes §9.5 behaviour outside this task: malformed lines are skipped and counted. The code review reported this as a side effect, not as a demand, so dropping it does not go against the higher priority.
- The error-text helper is dropped under priority 2 (correct, idiomatic code). It depends on the wording of a dependency's error message, and it gives up figures that can be kept.

A retry guarded by a parse that skips `usage` meets both demands: the figures are kept, and every line with bad bytes outside `usage` keeps its `main` outcome. This is why the issue is not an A/B choice for the human. No two solutions remain that differ in a way that matters, and the only inputs affected are corrupt lines.

One row departs from `main` on purpose: a bad key inside `usage` is unparsed on `main` and parses with the fix. The demand covers bad bytes anywhere inside `usage`, and parsing more of a corrupt line loses nothing.

## Rework budget

Earlier REWORK verdicts: 0 of 2. Round 1 has no `verdict.md`, and none appears in git history.

## Rework note

The footer, the model, both adapters, footer search and the snapshots are correct. Do not change them. One regression remains, on Claude only (OpenCode's `Row.data` is already a `String`).

**Problem.** `lenient_object` in `src/harness/shared.rs` reads `message.usage` as `Box<RawValue>`, which must be valid UTF-8. Invalid UTF-8 anywhere inside `usage` therefore fails `serde_json::from_slice::<Entry>`, and the whole line is lost. `main` parses these lines, because it skips those values with `IgnoredAny`. This breaks the binding planner note "A wire field added by this task must not turn a line or row that parses today into an unparsed one".

`from_slice::<Entry>` is called at four sites, and all of them must be fixed:
- `Assembler::parse_entry` in `src/harness/claude/assemble.rs`
- `extract_result_text` in `src/harness/claude/wire.rs`, which re-reads a kept line to materialise a large tool result
- `prompt_of_user` in `src/harness/claude/scan.rs`
- `prompt_of_queued` in `src/harness/claude/scan.rs`

**Change.** Keep `lenient` and `lenient_object` as they are, because the `1e400` fix depends on them. Add one shared function with a retry that runs only after the normal parse has failed. This shape was checked on a scratch copy against every case listed under Tests:

1. In `src/harness/claude/wire.rs`, make `Entry` generic with a default, `pub struct Entry<M = Message>`, with `pub message: Option<M>`. Existing uses of `Entry` and `impl Entry` compile unchanged.
2. In the same file, add `MessageWithoutUsage` with only `id: Option<String>`, `model: Option<String>` and `content: Option<Content>`. Its fields are never read, so it needs `#[expect(dead_code, reason = "...")]` to pass clippy.
3. In the same file, add:

   ```rust
   pub(crate) fn read_entry(line: &[u8]) -> Result<Entry, serde_json::Error> {
       serde_json::from_slice(line).or_else(|err| entry_with_invalid_utf8_in_usage(line).ok_or(err))
   }

   fn entry_with_invalid_utf8_in_usage(line: &[u8]) -> Option<Entry> {
       if std::str::from_utf8(line).is_ok() {
           return None;
       }
       serde_json::from_slice::<Entry<MessageWithoutUsage>>(line).ok()?;
       serde_json::from_str(&String::from_utf8_lossy(line)).ok()
   }
   ```

   Give `entry_with_invalid_utf8_in_usage` a doc comment that says why the first parse skips `usage`: it keeps a line with invalid UTF-8 anywhere else unparsed, as on `main`.
4. Call `read_entry` at all four sites listed above, in place of `from_slice::<Entry>`. A line that still fails is handled as it is now, with the original error.

**Dropped suggestions.** Do not use an unguarded `from_utf8_lossy` retry: it would make lines with invalid UTF-8 in `content` or `cwd` parse, which changes §9.5 behaviour outside this task. Do not match on serde_json's error text (`invalid unicode code point`): it depends on a dependency's wording and loses the readable figures. The round-2 reviews suggested these as ways to fix the problem, and this note replaces both.

**Tests.** Add to `tests/claude_assembly.rs`, feeding bytes through `asm.push_line` directly, because the `push` helper takes `&str`. Use a normal assistant line with one text block.

Each of these `usage` values must give `unparsed_count() == 0`, the text item kept, and `output == 7`:
- `{"output_tokens":7,"service_tier":"\xff"}`
- `{"output_tokens":7,"x":{"y":["a\xc3"]}}`
- `{"output_tokens":7,"cache_read_input_tokens":"\xff"}`
- `{"output_tokens":7,"output_tokens_details":{"x":"\xc3"}}`
- `{"output_tokens":7,"\xff":1}`

These lines, all with a clean `usage` of `{"output_tokens":7}` unless stated, must keep the `main` outcome:
- `\xff` in the text block's `text`: `unparsed_count() == 1`, no text item.
- Top-level `"cwd":"\xff"`: `unparsed_count() == 1`, no text item.
- Unknown top-level field `"zzz":"\xff"`: `unparsed_count() == 0`, text item kept, `output == 7`.
- `\xff` in the text block's `text` and `"service_tier":"\xff"` in `usage`: `unparsed_count() == 1`, no text item.

Add two crate-internal tests, because these functions are not public:
- In `src/harness/claude/wire.rs`, `extract_result_text` on an assistant line with an `advisor_tool_result` block (`tool_use_id` `c1`, content `hello`):
  - with `"service_tier":"\xff"` in `usage`, it returns `Some("hello")`;
  - with a clean `usage` and `\xff` in the block's content, it returns `None`.
- In the tests in `src/harness/claude/scan.rs`, `scan_reader` on a human `user` line with `"content":"real"` and `"usage":{"service_tier":"\xff"}` in `message` gives `first_prompt == Some("real")`.

**Doc fixes (small, same round).**
- `src/harness/claude/wire.rs` module doc: add that `usage` is read leniently, so a value of the wrong type there counts as missing rather than failing the line.
- `src/harness/shared.rs`, doc comment on `lenient`: state the two limits. A `RawValue` must be valid UTF-8, so invalid UTF-8 still fails the parse, and `read_entry` in `wire.rs` handles that case. A duplicate key inside the value makes the whole value count as missing.
- `src/components/turn_block/renderers/stats.rs`, comment in `StatsSection::lines`: reword "A row never starts with the separator" to say this holds when the part and its separator fit on a row.

**Leave as is.** `{}` showing `0 tokens` (planner note: a missing number counts as zero). The duplicate-key behaviour itself. The footer at text widths below 10.

**Done when.**
- All cases above pass, and the existing `1e400` and wrong-type tests still pass unchanged.
- No `from_slice::<Entry>` call remains outside `read_entry` in `src/harness/claude`.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `INSTA_UPDATE=no cargo test` pass.
- `INSTA_UPDATE=no cargo test --release --test budgets -- --ignored budgets --nocapture` stays within budget.
- No snapshot changes in this round.
