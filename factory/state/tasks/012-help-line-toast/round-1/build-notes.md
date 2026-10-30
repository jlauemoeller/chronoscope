STATUS: DONE

## Changes
- `src/components/help_line.rs` (new): `HelpLine` component and `Hint` type. Props: hints, status items, theme, `fill` role. Key labels in `help_key`; hint text, ` | ` separators and status in `help_text`. Whole hints are dropped from the end when they do not fit next to the right-aligned status items (two-space separated, never dropped for hints, a 2-column gap is kept). Text is on the last row of the area; the area is first painted with `fill` unless the theme is transparent.
- `src/components/toast.rs` (new): `Toast` component (3-row rounded box in the `toast` style, `Clear`ed first, text padded by one space, truncated with `…` via `truncate_with_ellipsis`). `on(Tick)` returns `Msg::ToastExpired` at or after `created_at + duration`; the clock is injectable (`with_clock`). With a transparent theme the toast's `bg` is dropped (nothing painted with `surface`).
- `src/app/hints.rs` (new): `selector_hints(&Keymap)` (built with `Keymap::label(Selector, ..)`, unbound actions skipped) and `startup_toast_text(&[ConfigWarning])` (`Config: <w>` / `Config: <first> (+N more, see log)`; line breaks in a warning become spaces).
- `src/app/layout.rs`: `LayoutInput::toast_text_width`; the layout now returns `HelpLine` (x = overlay.x+5, y = overlay.y+overlay.height-3, width = overlay.width-10, height 1) after the frame, and `Toast` last (right column at w-2, bottom border on row h-2, width = min(text+4, min(60, w-4))). No toast on the too-small screen.
- `src/app/mod.rs`: `Id::HelpLine`, `Id::Toast`, `Msg::ToastExpired`; `HelpLine` is mounted at startup; `show_toast` / `show_toast_for` remount the toast (a new toast replaces the current one) subscribed to `Tick`; the startup toast is mounted once in `App::new`; `ToastExpired` unmounts it and requests a redraw.
- Existing startup snapshots (`startup_frame_*`) were updated: they now include the help line (they serve as the "selector with help line" text snapshots at 80x24 / 120x40 / 200x50).

## Self-check
- AC: HelpLine renders selector hints exactly, key/text styles — `components::help_line::tests::all_hints_are_drawn_when_they_fit`, `full_line_text_and_styles` (styled snapshot), `keys_use_help_key_and_text_uses_help_text`; `app::hints::tests::default_selector_hints_match_the_sketch`; `app::tests::help_line_is_on_row_20_at_80x24_and_36_at_120x40_with_default_keys`
- AC: `[keys.selector] search = "f"` — `app::hints::tests::a_remapped_search_key_shows_in_the_first_hint`, `app::tests::a_remapped_search_key_shows_in_the_drawn_help_line`
- AC: narrow widths drop trailing hints whole, status stays right-aligned — `narrow_width_40_with_status_keeps_one_hint`, `width_60_with_status_keeps_two_hints`, `hints_are_dropped_whole_never_cut`, `status_items_are_right_aligned_and_separated_by_two_spaces`, `status_items_are_never_dropped_for_hints`
- AC: Toast expiry on Tick — `components::toast::tests::no_expiry_message_before_the_duration_has_elapsed`, `the_expiry_message_comes_at_and_after_the_duration`, `a_zero_duration_expires_on_the_first_tick`, `other_events_never_expire_the_toast`
- AC: App startup toast text, expiry, unmount, no reappearance — `app::tests::one_startup_warning_mounts_a_toast_with_its_text`, `three_startup_warnings_end_the_toast_with_two_more`, `the_expiry_message_unmounts_the_toast`, `the_startup_toast_does_not_reappear_on_later_ticks`, `a_toast_expires_on_a_tick_event_through_the_application`, `a_new_toast_replaces_the_current_one`; `app::hints::tests::*`
- AC: layout rects — `app::layout::tests::help_line_at_80x24_is_on_row_20_from_column_5_to_74`, `help_line_at_120x40_is_on_row_36_from_column_5_to_114`, `help_line_is_two_rows_above_the_bottom_border`, `toast_at_80x24_sits_in_the_bottom_right_corner`, `toast_at_120x40_sits_in_the_bottom_right_corner`, `toast_edges_are_one_column_and_one_row_inside_the_screen`, `long_toast_text_is_capped_at_60_or_width_minus_4`, `toast_is_drawn_last`
- UI: selector with help line @ 80x24 text — `app::tests::startup_frame_80x24_text` (trailing hints dropped)
- UI: selector with help line @ 120x40 text — `app::tests::startup_frame_120x40_text`; styled — `app::tests::selector_help_line_120x40_styled`
- UI: help line component @ widths 40 and 60 with `3/17` — `help_line::tests::narrow_width_40_with_status_keeps_one_hint`, `width_60_with_status_keeps_two_hints`
- UI: selector with startup toast @ 80x24 / 120x40 text + one styled — `app::tests::selector_with_toast_80x24_text`, `selector_with_toast_120x40_text`, `selector_with_toast_120x40_styled`
- UI: toast longer than max, truncated @ 80x24 — `components::toast::tests::long_text_is_truncated_with_an_ellipsis_at_80x24`; box styling — `short_text_is_padded_by_one_space_in_a_rounded_box` (styled)

## Notes for reviewers
- As the planner notes specify, the toast's top row is `h - 4`, so at 120x40 it overlaps the help line row (36) and at 80x24 it also covers the overlay's bottom border (row 22). The toast is drawn last and clears what is under it; the snapshots show this (the `ESC to Cancel` hint is partly covered at 120x40 when a toast is shown).
- `Toast` expiry race: if a toast is replaced in the same batch in which the old one expired, the old `ToastExpired` removes the new one. Not guarded; a stale message is rare and harmless.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (546 passed, 0 failed)
