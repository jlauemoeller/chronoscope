VERDICT: APPROVE

## Assessment
**Code review: PASS.** The gate passes (fmt, clippy `-D warnings`, `cargo test`: 2131 lib tests plus all other binaries; no `.snap.new`). Four nits remain, all ignored:
1. Theme double-click state survives `Esc` and reopening (`src/app/mouse.rs` `theme_clicked` / `MouseState::press`). **Ignored.** To trigger it, the user must click a theme, press `Esc`, press `t` and click the same row, all within 400 ms. The worst case is keeping a theme the user had just clicked twice. Clearing `last_press` in `open_theme_selector` is a one-line follow-up if wanted.
2. `nothing_under_the_help_overlay_or_the_theme_selector_gets_the_mouse` lost its wheel checks over the covered startup panes. **Ignored.** This only affects coverage. Inside an overlay, `hit_test` returns the topmost component drawn there, which is the overlay itself, so a pane under it cannot receive the event. The browser side is pinned by `the_transcript_under_the_selector_gets_no_wheel_or_click`.
3. `WHEEL_ROWS = 3` is defined twice (transcript `keys.rs`, `help_overlay/mod.rs`). **Ignored.** This is style only. Both values are correct and match the planner decision.
4. The doc comment on `Transcript`'s `AppComponent::on` still describes only keys. **Ignored.** Documentation nit. The new `mouse.rs` module docs describe the behaviour.

**Feature review: PASS.** Every acceptance criterion is checked off with a component or update test that would fail on wrong behaviour. Four nits remain:
1. With any prompt open (search, path, `y/n` resume), the transcript ignores the mouse (`takes_mouse`: `Id::Transcript | Id::Subagent(_) => !prompt`). §1.4 exempts only overlays from "acts on the element under the pointer". **Ignored, accepted as a deliberate deviation.** This is the only finding that departs from spec text. It is neither a REWORK nor an A/B escalation, for these reasons:
   - The two possible outcomes differ only in whether the wheel scrolls the transcript while a prompt is open. That is not a significant difference in functionality.
   - The browser search is not incremental: results exist only after `Enter` closes the prompt. While the prompt is open there are no results in the transcript to scroll toward. This differs from 034's Sessions-pane-during-search case.
   - Letting clicks through would move focus off the prompt via `focus_transcript` and break input mode, so clicks must stay blocked either way.
   - It follows 034's precedent of treating non-search prompts as modal for the panes under them.
   - The builder recorded the decision in the build notes, and a test pins it (`an_open_prompt_keeps_the_transcript_from_the_mouse`).
   - The human can revisit it later, for example by letting only the wheel through while the search prompt is open.
2. The update test `the_wheel_moves_the_selection_to_the_topmost_fully_visible_section` checks the selection only loosely. **Ignored.** The component test `a_selection_that_left_the_viewport_moves_to_the_topmost_fully_visible_section` pins the exact rule, so the AC is met.
3. Same as code finding 2 (lost wheel assertions). **Ignored**, same reason.
4. Same as code finding 3 (duplicate `WHEEL_ROWS`). **Ignored**, same reason.

**UI review: PASS.** `UI: none`. No `view` code or snapshots changed, and `INSTA_UPDATE=no cargo test` passes. No findings.

I read the routing (`src/app/mouse.rs`: `topmost_overlay`, `outside_overlay`, `takes_mouse`, `theme_clicked`) and the transcript hit-testing (`src/components/transcript/mouse.rs`) and they agree with the planner notes:
- An outside left press sends the `Esc` message: `CloseHelp`, `CancelTheme`, and `CloseSelector` only on `Screen::BrowserSelector` and not during the resume confirmation.
- At startup an outside click does nothing.
- The wheel outside an overlay is ignored.
- The wheel goes through `perform(Cmd::Custom(WHEEL_*))` to `scroll_pages(Amount::Rows(3))`.
- Fold toggles go through `fold_command(Toggle)`.

No security concern was raised or found.

## Conflicts
None. The code and feature reviews overlap on two nits (lost wheel assertions, duplicate `WHEEL_ROWS`) but make no opposing demands. The code review's general statement that routing respects §1.4 and the feature review's §1.4 note on prompts do not conflict: the feature reviewer also ruled that note non-blocking.

## Rework budget
Earlier REWORK verdicts: 0 of 2.
