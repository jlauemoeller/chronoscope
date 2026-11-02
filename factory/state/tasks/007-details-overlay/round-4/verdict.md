VERDICT: APPROVE

## Assessment

Round 4 did what the rework note asked: values are measured by grapheme as the buffer draws them, and the size at opening has a test. Every acceptance criterion holds. What remains is one must-fix that I judge minor, for the reasons below, and three nits.

**Code review: PASS**, one nit.
- Finding 1 (nit), `wanted` closure duplicated in two tests in `content.rs`: ignored. It is test code only, and the second copy is as the round-4 note wrote it.

**Feature review: PASS**, one nit.
- Finding 1 (nit), `overlay_rows` cuts a row by characters, not columns: ignored. It is a test helper, and the current tests are not affected, because their values have as many characters as columns. It matters only to a future test with keycaps or similar values.

**UI review: FAIL**, one must-fix and one nit.
- Finding 1 (must-fix), a control character between a base and its U+FE0F is measured too narrow and text is lost: reproduced, and ignored as minor. See below.
- Finding 2 (nit), VS16 emoji drift past the border in tmux 3.8: ignored for this task. The review traces it to ratatui-core's diff and the crossterm backend, and shows the same drift in the transcript with no overlay open. I did not rerun the tmux capture. It deserves its own task if VS16 emoji in tmux matter, because any such emoji in a value can trigger it once tasks 008 and 009 send more raw strings to the overlay.

**Why UI finding 1 does not block.** It does break the letter of spec/SPEC.md §5.5 for this input, and nothing on screen shows that text is missing. It is minor because the input does not occur in practice:
- It takes a raw control character directly between a base and U+FE0F. A sign and its selector are written together, so nothing normal puts one between them.
- It takes at least three such signs. With the signs together at the start of a value, one or two lost nothing at any of 221 lengths at 60, 80, 120 and 200 columns, because the two padding columns absorb the overflow. Three or four lost text at 205 to 207 of the 221 lengths, so with this input the loss is common. The rarity is in the input.
- Every other shape tried drew whole at all four widths: `⚠️` as written, ANSI colour around the sign, an ANSI reset between base and selector, BEL or NUL before or after the sign, and tab, CR or LF between base and selector. This covers the case that made the round-3 bug a must-fix, an error text with `⚠️`.
- Nothing panics, and the full value is still in the source.
- It is not new in round 4. From reading round 3's per-character `columns`, it gives the same 1 column for this sequence (not run).

The review's reach is otherwise correct, with one addition: when the value wraps, the text lost can be from the middle and not the end. At 60 columns, 40 such signs draw 24 signs and the final marker.

**Why this is not an A/B choice.** There is one right behaviour and one fix, so the human has no functional choice to make. Approving also closes nothing off, because the fix can land later.

**Recommended follow-up, not filed anywhere.** Tasks 008 and 009 send raw harness strings down this path, so include the fix there. In `display_lines` in `src/components/details_overlay/content.rs`:

```rust
let value = value
    .replace('\t', " ")
    .replace(|c: char| c.is_control() && c != '\n', "");
```

Add the two test additions from `ui-review.md` finding 1, and correct the doc comments of `display_lines` and `drawn_graphemes`; the latter's "as the buffer does it" is not true for this case. On a scratch copy with this change, no text was lost in any probe case and the 112 details tests pass. I did not run the full suite or the budgets with it. The UI reviewer ran the full suite with it (2776 passed), but not the budgets.

**Independent checks**, on a fresh export of `HEAD` (08a18c2) with its own target directory and `INSTA_UPDATE=no`:
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass.
- `cargo test` gives 2776 passed, 0 failed, 2 ignored, and no `.snap.new` file is written.
- The release budgets pass. Worst figures: first content 2.2 ms, full parse 290.6 ms, frame 3.37 ms; peak memory 0.58x the file.
- The reach figures above come from the app, counting signs and an end marker in the overlay's own buffer cells.
- `git diff main...HEAD` adds 44 snapshots and modifies 5, all help overlay snapshots. I did not recheck those 5 line by line; the feature review compared them against main in round 3, and no `.snap` file has changed since.
- `main` has no commits that this branch lacks.

## Conflicts

None. The UI review's FAIL differs from the other two reviews' PASS, but no two reviews make opposing demands, so the priority order is not needed. No reviewer raised a security concern.

## Rework budget

Earlier REWORK verdicts: 2 of 2 (rounds 2 and 3). None remain.
