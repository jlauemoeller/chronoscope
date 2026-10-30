VERDICT: FAIL

## Summary
Round 4 makes all three changes from the rework note in `src/components/transcript/search.rs`, and the code is correct:
- markers are stripped only in markdown sections, with fence tracking;
- `place` has a `written`/`drawn` count check, and the clamp is kept only for Edit/Write;
- the anchored window is bounded.

The round-3 must-fix is fixed. The round-3 nit on the clamp comment is addressed. All 17 requested tests exist, and the round-2/round-3 tests and snapshots are unchanged: the commit only appends to `tests/search.rs` and touches no `.snap`.

One must-fix remains, and it is in the tests only. The rework note requires every new test to assert that a drawn hit is highlighted where it is drawn. Most of the new tests do not. The build notes' self-check says they do.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (1570 tests passed across lib and integration targets, 0 failed, 1 ignored; working tree clean; no `.snap.new` files)

## Findings
1. [must-fix] `src/components/transcript/tests/search.rs`: the round-4 tests from line ~1222 to the end (`near`, `walk`, and tests 8, 9, 15, 16 and 17) skip the required highlight assertion.

   **Problem.** The rework note's test section is binding. It says: "Each asserts the hit is highlighted where it is drawn. `assert_hit_shown` does this. Matches that are not drawn (URL, fence info) get only the on-screen assertion."
   - Only two of the 17 new tests call `assert_hit_shown`: `a_unique_hash_line_within_the_limit_is_found_without_expanding` and `an_undrawn_url_before_a_code_span_stays_on_the_paragraph_and_the_far_line_is_not_reached`.
   - `near()` checks only the `i/n` position and that `line N-1` or `line N+1` is on screen. It never checks the hit row itself. A jump that lands about 9 rows too far still shows a neighbour and passes, with the hit off screen. Example: test 1 landing near line 109 shows `line 101` at the top.
   - `walk()` and tests 8, 9, 15 and 16 check only `screen.contains(..)`.

   So these drawn positions have no highlight check:
   - tests 1–6 and 17 (all via `near`);
   - tests 8 and 9;
   - test 10 at `2/3` and `3/3`, test 11 at `2/2`, test 12 at `4/5` and `5/5`, and test 13 at `3/4` and `4/4`;
   - test 15 at `3/4` and test 16 at `1/2` and `2/2`.

   **Change** (tests only; `search.rs` code needs no change):
   - **`near(t, position, line)`.** After the neighbour check, also assert `highlighted(t).iter().any(|m| m == hit)`. Add a `hit: &str` parameter and pass the query (`"done"` or `"usb"`) from tests 1–6 and 17.
   - **`walk(text, expected)`.** Give each step a flag for whether that match is drawn, for example `&[(&str, bool)]`. At drawn steps, assert the query is among `highlighted(t)` (or call `assert_hit_shown(t, pos, row, "usb")`). Skip the check only at the undrawn positions:
     - test 10 at `1/3`;
     - test 11 at `1/2`;
     - test 12 at its URL position;
     - test 13 at `1/4` and `2/4`.
   - **Tests 8, 9, 15 and 16.** Replace the bare `contains` checks with `assert_hit_shown`. Use these arguments:
     - test 8: `"done"`, with the row text `x19` or `x21`, or `done`;
     - test 9: `"# usb setup"`, `"usb"`;
     - test 15: `"or serde_json"`, `"serde"`;
     - test 16: `"the usb one"` / `"the usb two"`, `"usb"`.

     Keep the existing negative assertions (`!contains("the far serde line")`, `!contains("the usb two")`, and so on).
2. [nit] `src/components/transcript/search.rs:893`, in `SectionView::anchored`, `(row >= first).then_some(row)`. The note says the window ends at the first later line drawn *after* the window start, and that a line drawn "at or above" the window start is skipped. The code keeps a line drawn exactly at `first`. That only happens when `found`'s own line draws no row (a fence info string), and the one-row window it then gives is the right neighbourhood, so the behaviour is equal or better. Change: either keep `>=` and update the comment above it ("a later line drawn above the window does not end it"; say that a line drawn at its first row ends it after that row), or use `row > first` to match the note.
3. [nit] `src/components/transcript/search.rs:800`, in `place`, `for (i, _) in self.source.squashed.iter().enumerate()`. The element is discarded. Change: `for i in 0..self.source.squashed.len()`.
4. [nit] `src/components/transcript/search.rs`, `fence_of` and `Source::new`.
   - A fence is recognised only at the start of a line, so fenced blocks inside list items (indented by 4 or more) or block quotes (`> ````) are not tracked.
   - A backtick "fence" whose info string contains a backtick (for example a line such as ```` ```a``` b ````) is taken as an opener, although CommonMark says it is not one.

   Both fall back to the count check and the anchored window, so they do not cause a wrong row by themselves. Change (optional): reject backtick openers whose rest contains `` ` ``, and add a doc line on `fence_of` noting that nested fences are not tracked.
5. [nit] `src/components/transcript/search.rs`, `place` / `drawings` / `anchored`. Each `place` call now walks all source lines (`written`) and scans the whole `joined` string, because `drawings` no longer exits early. `anchored` adds a lookahead of up to `ANCHOR_LOOKBACK` more `place` calls. The round-3 worst case (about O(400 × section size) on the UI thread per jump on a large expanded output) therefore grew by a constant factor. `is_drawing` also allocates a `squash` of the row for every candidate. The optional precomputation in the rework note (each line's placement once per `SectionView`) would make these lookups. This is not required, and it is in line with the round-3 verdict.
6. [nit] `factory/state/tasks/024-transcript-search/round-4/build-notes.md`, the "Self-check" heading. It claims each test checks the "highlight where the text is drawn", which is not the case (see finding 1). Correct it when finding 1 is fixed.

Checked and fine:
- `&self.rows[first..until.max(first + 1)]` and `window[0]` cannot panic: `first < rows.len()` is guarded and `until <= rows.len()`.
- The markdown flag matches the renderers that call `render_markdown` (user, text, thinking, compaction, subagent).
- Test 17's wrap width of 69 at 80 columns agrees with the shell result wrap of 89 at 100 columns in the existing `shell_unfolded_with_a_long_command_at_100_text` snapshot. Round-3 `place` would take the wrapped `done` row there, so the test does guard the `drawn > written` branch.
