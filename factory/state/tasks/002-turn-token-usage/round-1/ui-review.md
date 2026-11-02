VERDICT: PASS

## Summary
The Assistant block footer matches the task's `UI` section: `<duration> · <n> messages · <tokens> tokens · <cached> cached · $<cost>`, one muted row indented by two spaces, with `cached` hidden at zero and cost last. All four required snapshots exist with the right content, no existing snapshot changed, and `cargo fmt --check`, clippy, `cargo test` (with `INSTA_UPDATE=no`) and the release budgets all pass. Two cosmetic nits remain.

## States checked
- Footer with `cached` @ block width 80 — snapshot `footer_with_cached_at_80` / rendered in scratch crate — ok
- Footer with `cached` @ block width 120 — snapshot `footer_with_cached_at_120` — ok
- Footer with `cached` and cost @ block width 80 — snapshot `footer_with_cached_and_cost_at_80` / rendered in scratch crate — ok
- Footer with `cached` and cost @ block width 120 — snapshot `footer_with_cached_and_cost_at_120` / rendered in scratch crate — ok
- Task's example figures (12 messages, 45.2k tokens, 359.4M cached) — rendered in scratch crate — ok, text is exactly `5m 2s · 12 messages · 45.2k tokens · 359.4M cached`
- Spec figures (2.8k, 336k, 1.2M writes, 359M reads) through the Claude assembler — rendered in scratch crate — ok, `1.5M tokens · 359.0M cached`
- OpenCode row with reasoning 683, output 120, cache read and write, cost — rendered in scratch crate — ok, `1.8k tokens · 359.4M cached · $0.42`
- `cached` hidden at zero, with and without cost — unit tests / unchanged existing snapshots — ok
- No usage (absent, `null`, wrong JSON type), both harnesses — rendered in scratch crate — ok, no token part, no `cached` part, `unparsed_count` 0
- Usage object with no readable number (`{}` or all fields of a wrong type), both harnesses — rendered in scratch crate — ok, shows `0 tokens`; main does the same for `{}`, and the planner notes say a missing number counts as zero
- Theme role — styled render at 80 and 40, default theme; footer colour compared to `muted_foreground` at 80 on all 7 built-in themes — ok, the whole row including `cached` and cost is `muted_foreground`
- Indentation — styled render at 80 — ok, footer starts at the same column as the assistant text
- Selected footer and `dim_unselected` false @ 80 — rendered in scratch crate — ok, `▒` marker and bold keep the token colour, as `emphasize` does for every section; unchanged by this task
- Word wrap at block widths 70, 66, 65, 64, 63, 62, 61, 60, 50, 40, 30, 24, 20, 12 — rendered in scratch crate — ok, no clipping or overflow, height matches the rows drawn; see finding 1
- Block widths 8 and 6 — rendered in scratch crate — ok for this task, the text area is too small to show content and the blocks are tall; not caused by this change
- Live update adding `cached` and cost to an existing footer @ 120x12, 80x12, 60x12, 40x12 — `Transcript` rendered in scratch crate — ok, at 60 the block grows by one row without leaving stale rows
- Search for `359.4M cached`, `cached`, `$0.42` @ block widths 120, 80, 60, 50, 45, 40 — rendered in scratch crate — ok, highlighted on the footer row, except that `359.4M cached` is not highlighted at 55, 30 and 20, where the wrap splits it; `widget.rs` documents that a match wrapping onto the next row is not highlighted, and this task did not change that
- Markdown export (folded and full) — rendered in scratch crate — ok, `_5m 2s · 12 messages · 45.2k tokens · 359.4M cached · $0.42_`
- Claude fixture `session.jsonl` through the assembler — rendered in scratch crate — ok, `150.4k tokens · 270.0k cached` and `100.1k tokens · 180.0k cached`
- Snapshot scope — `git diff main...HEAD` — ok, four new `.snap` files, no existing snapshot changed, no `.snap.new` files
- Test gates — `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `INSTA_UPDATE=no cargo test`, `INSTA_UPDATE=no cargo test --release --test budgets -- --ignored budgets --nocapture` — ok

## Findings
1. [nit] Assistant block footer, `cached` and cost, block widths 60 to 66 and narrower — a wrapped row can start with the separator: `· $0.42` at 64, `· 359.4M` at 20. This is the existing word wrap, but the longer footer now reaches it at wider blocks. Change: in `StatsSection`, wrap so that a separator stays at the end of the row (`… 359.4M cached ·`) and the next row starts with the part (`$0.42`). Leave `format_stats` as it is, because search matches against its text. See `ui/footer-cached-cost-64.txt` and `ui/footer-cached-cost-20.txt`.
2. [nit] Footer snapshots at 80 and 120 — all four are text-only, so they don't pin the colour of the `cached` part. The row is correct today, and `footer_is_muted_and_indented_by_two_spaces` covers the same code path with a footer without `cached`. Change: add a `_styled` variant of `footer_with_cached_and_cost_at_80` using `buffer_to_styled`.
