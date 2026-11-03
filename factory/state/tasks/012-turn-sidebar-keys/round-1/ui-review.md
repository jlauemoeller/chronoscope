VERDICT: PASS

## Summary
All 12 focused-sidebar screens and both changed help overlay snapshots match the task's `UI` section, and the states I rendered myself behaved as the spec says, with no must-fix findings. There are four nits; nit 1 is a judgement call the Approver may want to read, because the acceptance criterion's wording is broader than what its test covers. I ran `INSTA_UPDATE=no cargo test` (3138 passed, 0 failed) and `cargo fmt --check` (clean); I did not run clippy or the budgets.

## States checked
Snapshots (each checked for the ring border, `>` in `primary` at column 3, one `REVERSED` band over columns 4 to 26, the transcript's top row, and the hints):
- Focused sidebar on User @ 120x40, 90x24, `default` and `terminal` — `the_focused_sidebar_on_a_user_{120x40,90x24}_{default,terminal}_styled` — ok
- Focused sidebar on Assistant, duration inside the band @ 120x40, 90x24, `default` and `terminal` — `the_focused_sidebar_on_an_assistant_{120x40,90x24}_{default,terminal}_styled` — ok
- Focused sidebar on Compaction, `>` replacing the first `═` @ 90x24, `default` and `terminal` — `the_focused_sidebar_on_a_compaction_90x24_{default,terminal}_styled` — ok
- Focused sidebar on Compaction @ 120x40, `default` and `terminal` — `the_focused_sidebar_on_a_compaction_120x40_{default,terminal}_styled` — ok; the block's top is not at the top row because the transcript's end clamps it, as the spec allows
- Help overlay, end of the list @ 120x40 — `help__the_browser_help_120x40_end_text` — ok; `Turns sidebar` comes before `Application`, each row once
- Help overlay @ 200x60 — `help__the_browser_help_200x60_text` — ok; the list no longer fits, so `↓ more` appears
- Snapshot changes overall — 12 new files and those 2 help snapshots, nothing else — ok

Rendered in the scratch crate (the real `App`, keys sent through its ports, `TestBackend`):
- Focus with `1` and `Tab`, nothing scrolled @ 90x24 — ok
- Long list (64 entries), `j` to the last entry, `k` to the first, one more press at each end: no wrap @ 90x24, 120x40 — ok; see nit 4 for `Start` and `End`
- Return with `Tab`: transcript unmoved, unfocused row style, browser hints back @ 90x24 — ok
- `Enter`: focus on the block, browser hints back @ 90x24 — ok
- Short and boundary sizes, sidebar focused @ 90x20, 90x21, 91x22, 200x20, 200x60 — ok
- Deep selection, then shrinking from 120x60 to 90x20: selected row stays visible — ok
- Hidden @ 89x24: focus moves to the transcript, hints change, and it stays there back at 90x24 — ok
- Too small @ 59x24 and 120x19: `Terminal too small`, focus on the transcript afterwards — ok
- `EmptySession`: `1` does nothing, and a sidebar focused during loading hands focus over when the session turns out empty @ 90x24 — ok
- Loading with an empty list, sidebar focused: `j` and `Enter` do nothing @ 90x24 — ok
- Turns arriving or a `Reset` while focused: no selected row, first `j` or `k` selects the first entry, `Enter` does nothing until then @ 90x24 — ok
- Entry with no rows: scrolls to the block, transcript selection unchanged @ 90x24 — ok
- Help, theme selector and session selector opened from the sidebar: focus returns on close, sidebar hints stay behind the overlay @ 90x24 — ok
- Opening another session from a selector opened from the sidebar: focus and hints go to the transcript @ 90x24 — ok
- Resizing with an overlay open: wide to wide keeps the sidebar; hidden or too small returns to the transcript (help @ 80x24, theme selector @ 50x24, session selector left to the Builder's tests for the hidden case) — ok
- Committing a theme from the sidebar (`terminal`): pane and hints restyled @ 90x24 — ok
- Marker, band, ring and border colours in all 7 built-in themes @ 90x24 — ok
- `↓ 1 new` and search `1/6` next to the sidebar hints @ 90x24 — ok
- Moving off the bottom stops following; a new turn is counted, transcript unmoved @ 90x24 — ok
- Sidebar focused while following, turn appended @ 90x24 — see nit 1
- Compaction as first and last entry; widest duration `99h 59m`; Assistant without stats @ 90x24 — ok
- Remapped keys (`f`, `o` / `enter`, `ctrl-n`): title, hints, help group @ 90x24 — ok
- `focus_turns` unbound: title is `Turns` @ 90x24 — ok
- Long key labels in the title and unbound hint actions @ 90x24 — see nits 2 and 3
- Session selector: help has no `Turns sidebar` group; `Tab` still moves panes and `1` leaves focus alone @ 120x40 — ok

Not rendered: a subagent transcript with the sidebar focused. It is covered only by the Builder's `in_a_subagent_the_sidebar_scrolls_the_transcript_on_top_and_the_hints_keep_the_depth`, which I did not re-derive.

## Findings
1. [nit] Browser, sidebar focused, transcript following at the bottom, turn appended, 90x24 — the transcript scrolls and its selection moves to the new turn, while the sidebar row stays on the old entry; on `Tab` the row jumps to the new turn. The task's criterion says an appended turn adds a row "without moving the selection or the transcript", but its test, `a_turn_that_arrives_while_the_sidebar_has_focus_adds_a_row_and_moves_nothing`, only covers a transcript that is not following. I rate this a nit because SPEC §6.4 says the usual follow rule applies, §7.2 says content at the bottom scrolls into view, and §6.7 only requires the selection not to move, so following looks intended. Change: add a test that pins this case, following while the sidebar has focus: the transcript follows and the sidebar row stays. If the criterion's literal wording is what was meant, this becomes a behaviour change instead. See `ui/follow-append-sidebar-focused-90x24.txt`.
2. [nit] Browser, title with a long remapped key, 90x24 — a label too long for the pane is cut by the border with no ellipsis and `Turns` is lost: `╭─ [Ctrl-Shift-F12 Ctrl-Shift╮`. A shorter chord also loses the dash before the corner: `╭─ [z Ctrl-Shift-F12] Turns  ╮`. Change: in `TurnSidebar::view`, shorten the key label with `…` to fit `title_width`, so the title stays `[<key>] Turns`. See `ui/title-long-chord-key-90x24.txt`.
3. [nit] Help line, sidebar focused, `open` and `next_pane` unbound, 90x24 — the hint row is blank, even though `Shift-TAB`, `Right` and `l` still leave the sidebar. With only `next_pane` unbound, the hint for leaving disappears the same way. Change: in `turns_hints`, take the key for "to switch to transcript" from `next_pane`, then `prev_pane`, then `focus_transcript`. See `ui/hints-next-pane-and-open-unbound-90x24.txt`.
4. [nit] Sidebar, long list, first or last entry selected by keys, 90x24 — `End` stays below the view on the last entry, and `Start` stays above it when coming back to the first, so no key can reveal them. This follows §6.1 ("only as far as needed to keep the selected row visible") and the list cursor predates this task, so no change is required. Optional change: when the first or last entry is selected, also reveal `Start` or `End`. See `ui/long-list-start-end-out-of-view-90x24.txt`.
