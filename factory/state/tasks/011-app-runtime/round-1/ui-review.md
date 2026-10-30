VERDICT: PASS

## Summary
The startup selector frame and the "Terminal too small" state match the task's `UI` section at every listed size: overlay rect, rounded `ring` border, `surface` interior, `background` outside, no title or content, and a centred `muted_foreground` message. Every required snapshot exists and matches the spec. Scratch renders at boundary, odd, very wide and tiny sizes, across all 7 built-in themes (truecolor and 256-colour, opaque and transparent) and through live resize sequences, found no defects. `INSTA_UPDATE=no cargo test` (incl. 11 pty and 1 panic-hook tests), `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` all pass.

## States checked
- Startup frame @ 80x24, text: snapshot `app__tests__startup_frame_80x24_text`. ok (border rows 1 and 22, rows 0 and 23 blank).
- Startup frame @ 80x24, styled: snapshot `app__tests__startup_frame_80x24_styled`. ok (rows 0/23 bg `background` Rgb(10,10,10); border fg `ring` Rgb(207,207,207) on `surface` Rgb(18,18,18); interior bg `surface`).
- Startup frame @ 120x40, text: snapshot `app__tests__startup_frame_120x40_text`. ok (full width, `Rect{0,1,120,38}`).
- Startup frame @ 200x50, text: snapshot `app__tests__startup_frame_200x50_text` + `startup_frame_200x50_spans_columns_20_to_179`. ok (frame spans cols 20–179, side columns blank).
- Startup frame, transparent default theme @ 80x24, styled: snapshot `app__tests__startup_frame_with_a_transparent_theme_80x24_styled`. ok (every bg `Reset`, border still `ring`).
- Too small @ 59x24 and 80x19, text: snapshots `app__tests__too_small_59x24_text`, `too_small_80x19_text`. ok (message on row 12 / row 9, centred, nothing else drawn).
- Component snapshots `SelectorFrame` (base, over a session, transparent) and `EmptyState` (text, styled, transparent). ok.
- Boundary sizes 60x20 (frame `Rect{0,1,60,18}`), 59x20, 60x19, 59x19 (too small), 61x21: rendered in scratch crate. ok. See `ui/startup-60x20.txt`.
- Wide and odd widths 160x30, 161x30 (x=0), 201x24 (x=20, spare column right), 300x60 (x=70): rendered in scratch crate. ok. Every cell checked: outside bg `background`, inside `surface`, border fg `ring`.
- All 7 built-in themes × {truecolor, 256-colour} × {opaque, transparent} @ 80x24: rendered in scratch crate, cell-by-cell role check. ok (0 violations). See `ui/scratch-checks.txt`.
- Too small @ 18x3, 17x5, 10x1, 1x1, 30x10, and transparent @ 59x24: rendered in scratch crate. ok (whole area `background`, text `muted_foreground`; narrower than the message it clips on the right without panicking; transparent paints no bg). See `ui/too-small-17x5-styled.txt`.
- Live resize sequence through one `Terminal` (80x24 → 50x15 → 80x24 → 200x50 → 80x24 → 59x24): rendered in scratch crate. ok (each frame identical to a fresh render, with no stale cells left from the too-small or wide states).
- Degenerate `SelectorFrame` overlays (1x1, 2x2, 3x3) and an `EmptyState` message longer than its area: rendered in scratch crate. ok (no panic, clipped).

## Findings
1. [nit] Screen snapshots per built-in theme. SPEC §9.6 (quoted in the task) asks for `App::view` screen snapshots "for each built-in theme at a fixed size". Here, `every_builtin_theme_draws_the_startup_frame` only asserts that `╭` is present. Task `013-selector-panes` explicitly owns "screen snapshots of the selector for every built-in theme" (its Goal, plus the AC "The per-theme screen snapshots render `App::view` for all seven built-in themes"), so this is not a must-fix here. I checked all 7 themes cell by cell in the scratch crate and they render correctly (`ui/scratch-checks.txt`). Change (optional): in `every_builtin_theme_draws_the_startup_frame`, assert per cell that the border fg is `ring`, the interior bg is `surface` and the outside bg is `background`, so the test catches colour regressions until 013 adds the snapshots.
2. [nit] `SelectorFrame`, opaque themes, all sizes: interior cells keep fg `Reset`, because only bg is set to `surface`. This is invisible now because the interior is empty, but a base fg would give later child components a consistent default. Change (optional): in `SelectorFrame::border`'s opaque branch, use `block.style(Style::new().fg(self.theme.role(Role::Foreground)).bg(self.theme.role(Role::Surface)))`.
