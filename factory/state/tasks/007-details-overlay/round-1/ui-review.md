VERDICT: FAIL

## Summary
The overlay matches the `UI` section in every listed state: frame, heading, label and value styles, value column, wrapping, the ` ↑ more ` / ` ↓ more ` indicators and all 42 component snapshots. Two behaviours are wrong: a `details` key remapped to a chord opens the overlay but does not close it, and the overlay keeps the size of the screen it was opened on after a resize. The fmt, clippy and test gates pass with `INSTA_UPDATE=no`; the release budgets were not re-run by this review.

## States checked
- User prompt, assistant text, thinking, tool call, `model → …` note, stats footer, compaction, system, notice @ 80x24 and 120x40, `default` and `terminal` — snapshots `<kind>_<size>_<theme>` (36) — ok, except the cosmetic wrap in finding 3 (`notice`)
- Content taller than the screen @ 80x24, top and end, both themes — snapshots `tall_80x24_{top,end}_{default,terminal}` — ok
- Long value wrapped to the value column @ 80x24 — snapshot `wrapped_value_80x24_default` — ok
- Unavailable group, one muted row @ 80x24 — snapshot `unavailable_80x24_default` — ok
- Overlay over the browser @ 80x24 and 120x40 — snapshots `the_details_over_the_browser_{80x24,120x40}_text` — ok
- Help overlay with the new `i  Show section details` row @ 120x40 top and end, 200x60, remapped copy key — the 5 changed snapshots — ok: only the new row and the shift it causes
- Tall content @ 60x20 (minimum size), top and end, both themes — rendered in scratch crate — ok
- Tool call content @ 60x20 and 100x30 — rendered in scratch crate — ok
- Tall content @ 61x21, 60x21, 200x60 and 300x100 — rendered in scratch crate — ok
- Transparent theme @ 80x24, `Clear` with no `surface` fill — rendered with styles in scratch crate — ok
- Wide characters @ 60x20 — rendered in scratch crate — ok
- Muted row with a wrapped value @ 60x20 — rendered with styles in scratch crate — ok
- Newlines, tabs and carriage returns in a value @ 60x20 — rendered in scratch crate — issue, finding 4
- Open, then close with `i` and with `Esc` @ 80x24 — real binary in tmux — ok
- `details = ["g i"]`, open and close @ 80x24 — real binary in tmux and scratch crate — issue, finding 1
- Resize while open: 80x24 → 60x20 → 50x15 → 120x40 — scratch crate — issue, finding 2
- Resize while open: 120x40 → 80x40 — scratch crate — issue, finding 2
- Resize while open: 80x24 → 120x40 — real binary in tmux — ok with this fixture's short values
- `i` @ 50x15 (too small), then grown to 120x40 — real binary in tmux — issue, findings 2 and 5

## Findings
1. [must-fix] `DetailsOverlay`, open, any size, `details` bound to a chord — the key opens the overlay but does not close it, against SPEC §5.1 ("The `details` key also closes it") and the acceptance criterion that a remapped key opens and closes it. Chords are valid in `[keys.browser]` (spec/SPEC.md §8.5). With `details = ["g i"]`, the transcript's `KeyResolver` gives `Pending`, then `Action(Details)`. `DetailsOverlay::on` uses `Keymap::resolve`, which ignores chords, so both keys answer `None` and only `Esc` closes it. Change: give `DetailsOverlay` a `KeyResolver::new(Context::Browser)`. On `FrameKey::Unbound`, feed the key to it, answer `Msg::CloseDetails` on `Resolved::Action(Action::Details)` and nothing otherwise. Reset the resolver when the overlay context handles a key. Add component and app tests that a chord-bound key opens and closes the overlay. See `ui/details-chord-key-does-not-close-80x24.txt`.

2. [must-fix] `DetailsOverlay`, open across a resize — the wanted size is fixed at opening and capped by that screen's width (`open_details`, `details_size`), so the overlay no longer follows the task's binding wanted-size rule: widest row plus 8, at least 40, as high as the content, with only `help_overlay_rect` capping it. The build notes call this a deliberate choice, but it draws the wrong layout:
   - Opened at 80x24 and grown to 120x40, the overlay stays 76 wide with `cwd` wrapped; it should be 102 wide with `cwd` on one row. See `ui/details-resize-grow-80x24-to-120x40.txt`.
   - Opened at 120x40 and shrunk to 80x40, ` ↓ more ` appears although the screen has spare rows, because the height predates the wrap; it should be 29 rows with no indicator. See `ui/details-resize-shrink-120x40-to-80x40.txt`.
   - Opened at 50x15 and grown to 120x40, the overlay stays 46 wide and `start` wraps with 74 columns spare; it should be 47 wide with no wrap. See `ui/details-resize-from-too-small-to-120x40.txt`.

   With only model rows, the first two need long values, so they mainly appear once tasks 008 and 009 add `cwd`; the third reproduces today in the real binary. Change: recompute the wanted size as `desired_size(screen_width - SCREEN_MARGIN)` on every `Msg::Resized`, for example through a query attribute on the overlay so the `App` keeps no copy of the content. Do this before `refresh_routing()` in `push_viewports`, or mouse routing keeps the old rect. Add `App::update` tests for growing and shrinking that check the rect, the wrap, the indicator and that a press in the new rect is routed to the overlay.

3. [nit] `DetailsOverlay`, wrapped value, for example `notice` @ 120x40 — when the break falls just before a space, the continuation starts with that space, so its text sits one column right of the value column (`  can serve right now…` under `APIError…`). Change: when wrapping, drop one space at the start of a continuation row, and adjust `a_wrapped_value_never_loses_a_character` to match. See `ui/details-wrap-leading-space-120x40.txt`.

4. [nit] `DetailsContent::layout` / `char_wrap`, values with control characters @ 60x20 — not reachable with this task's model rows, but tasks 008 and 009 will pass raw harness strings such as `error`. A tab has no width and is not drawn, so `a\tb\tc` shows as `abc`, and a trailing newline adds an empty value row that reads as a group gap. Change: replace tabs with a space and trim trailing newlines when a row is laid out.

5. [nit] Browser @ below 60x20 — pressing `i` on the "Terminal too small" screen mounts the overlay unseen and gives it the focus, so further keys such as `q` appear to do nothing until `Esc` or a resize. `?` has the same behaviour today, because `open_help` has no such guard. Change: return early from `open_details` when `too_small` holds for the current size. See `ui/details-resize-from-too-small-to-120x40.txt`.
