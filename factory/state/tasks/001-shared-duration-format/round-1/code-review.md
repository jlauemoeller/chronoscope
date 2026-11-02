VERDICT: PASS

## Summary
There is now one `format_duration`, in `src/duration.rs`; it implements the §2.4 table correctly, both `format_stats` functions call it, and `row_parts`, `SessionStats` and the stats cache are untouched. All 66 snapshot diffs were classified line by line, and each rendered change is a duration, a title that re-fit, or the span ranges of those same rows. The budget run stays within budget, and only nits remain.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass

## Findings
1. [nit] `src/components/session_list.rs:1112` (`stats_arrive_one_row_at_a_time`) — `text.contains("3 turns · 1m")` still passes on the old format, because it is a prefix of the new text for `stats(3, 90)`. Change: assert `"3 turns · 1m 30s"`.
2. [nit] `src/components/session_list.rs:560` (`seconds_under_an_hour_can_push_the_stats_out`) — the comment says "the same row with the old stats text", but the row uses `stats(12, 59)`, which renders `12 turns · 59s`. That is a sound stand-in, since it has the same 14-cell width as `12 turns · 59m`, and the old text can no longer be produced. The comment overstates it, though, and so does the comment at line 611 on `stats_that_no_longer_fit_snapshot`, whose second row shows `59s`. Change: reword both to say this is a 14-wide stats text, the width of the old `12 turns · 59m`.
3. [nit] `src/components/transcript/snapshots/chronoscope__components__transcript__snapshots__dim_unselected_false_at_120x40_styled.snap:3` — the `expression:` header changed from `"buffer_to_styled(&draw(&mut t, 120, 40))"` to `styled` when the snapshot was re-accepted. The new value matches the test's `insta::assert_snapshot!(styled)` and nothing rendered is affected, but it is the only metadata change among the updated snapshots and the build notes don't mention it. Change: mention the re-accepted header in the build notes.
4. [nit] `src/duration.rs:1` — the module doc cites `SPEC §2.4`. Other modules use `SPEC §n` for the base spec (`doc/SPEC.md`), where §2 is "Tech stack" and this format will be folded into §4.1 and §5.2 (cycle spec §8). Change: cite `doc/cycle-2-SPEC.md §2.4` now, or the base spec sections when the documentation task folds them in.
