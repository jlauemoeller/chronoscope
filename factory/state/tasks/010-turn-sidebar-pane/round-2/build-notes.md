STATUS: DONE

## Changes
- The gap column is now its own component. `BrowserAreas.sidebar_gap` (30,5,1,h), `Id::SidebarGap` drawn between the sidebar and the transcript only when the sidebar is drawn, and a new `BackgroundFill` component (`src/components/background_fill.rs`) that paints `background` or nothing for a transparent theme. `hit_test` skips `Id::SidebarGap`. It is in `BROWSER_IDS`, `THEMED` and the exhaustive `Id` test. `TurnSidebar::view` paints only its own area and no longer imports from `app::layout`.
- Renames/cleanups: `list_rows` -> `sidebar_list_rows`; `Look` no longer re-exported; one `select` instead of `select`/`select_entry`; `SidebarScenario::draw_once` removed.
- Tests: transparent-theme coverage for the pane (`a_transparent_theme_leaves_the_pane_unpainted`) and the fill (`background_fill::tests`), and `(10, 10)`, `(30, 10)` added to `a_transparent_theme_leaves_the_browser_unpainted`; `readable_thinking_is_a_section_and_redacted_thinking_is_not`; stale lines removed from the inserted-entries test; exact row asserted for the scrolled-to-end test; the transcript/sidebar agreement test now sends live updates; doc comments fixed.
- Widest durations: `widest_durations()` lists `9999h 59m` (9 cells) first, `widest_duration_selected` selects entry 1; 8 snapshots re-generated and reviewed (the band covers `9999h 59m` ending at column 26; `100h 30m` and `26h 30m` visible at 90x24 without selection).

## Self-check
- Review: code 1 (draws only itself) — `SidebarGap` + `BackgroundFill`; layout tests `the_gap_is_its_own_area_between_the_sidebar_and_the_transcript`, updated id lists; `hit_testing_finds_the_sidebar_in_the_pane_nothing_in_the_gap_and_the_transcript_beyond` and `every_cell_of_the_browser_is_painted_with_the_background` pass unchanged. No screen snapshot changed.
- Review: code 2 (transparent guard) — `a_transparent_theme_leaves_the_pane_unpainted`, `background_fill::tests::a_transparent_theme_paints_nothing`, `a_transparent_theme_leaves_the_browser_unpainted` with pane and gap cells.
- Review: code 3 / feature 2 / UI 1, 2 (widest durations) — fixture reordered, `Some(1)` selected, snapshots regenerated.
- Review: code 6, 7, 8 — done as listed. Code 9, 10, 11, 12, 13 / feature 4, 5, 6 — done as listed.
- Review: feature 1 / UI 3 (gap outside the pane) — as code 1.
- Review: code 4 and UI 5 (`sync` resets the offset on live updates; for task 013), code 5 (focus when hidden under overlay or by EmptySession; task 012), UI 4 (reveal `Start`/`End` at the ends; for the key task) — notes for the later tasks, no change here.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (3013 passed, 0 failed)
- cargo test --release -- --ignored budgets --nocapture: pass (sidebar frame worst 0.08 ms)
