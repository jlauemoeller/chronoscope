STATUS: DONE

## Changes (round 1, unchanged unless listed under Round 2)
- `src/app/index_port.rs` (new): `IndexUpdate` (Arc-wrapped `IndexEvent`, equality by sequence number) and `IndexPort` (sync `Poll` over the task-010 receiver; never blocks, a disconnected worker is not an error). `UserEvent::Index(IndexUpdate)` in `src/app/mod.rs`.
- `src/components/global_events.rs`: subscribes to the index user events by discriminant (not `Any`, which would duplicate keys and ticks) and turns them into `Msg::Index`.
- `src/app/selector.rs` (new): `Pane`, `SelectorMsg` (`ProjectSelected`, `SessionSelected`, `Focus`, `SwitchHarness`), the selector's domain state (per-harness snapshots, stats map, current harness, selected project/session ids, focused pane, last pushed active ids) and the handlers: snapshot, stats, selection kept by id (else same index, clamped), focus (moves focus with `Application::active`, selects the newest session when Sessions is focused with nothing selected), harness switching, activity refresh on `Tick`, and pushing rows/selection/cwd/tabs/visible-row counts into the components through `attr`.
- `src/app/mod.rs`: new `Id`s, `Msg::Index`/`Msg::Selector`, mounts `HarnessTabs`, `ProjectList`, `SessionList`, `WorkingDir`, focus on `ProjectList` at startup, injectable clock (`set_clock`), `TimestampFormatter` from the config, the `IndexHandle` owned by the app (`set_index`/`shutdown`), `production_listener(index)` adds the `IndexPort`.
- `src/app/startup.rs`, `src/harness/mod.rs`: `AppInit.harnesses` (registered harnesses, via the new `registered_harnesses(config)`).
- `src/app/runtime.rs`: starts the `SessionIndex` over the enabled harnesses, hands its receiver to the port, calls `shutdown()` on exit so the stats cache is saved.
- `src/app/layout.rs`: `selector_areas(overlay)` (pure) with the planner-note offsets; `layout()` adds `HarnessTabs`, `ProjectList`, `SessionList`, `WorkingDir` between the frame and the help line.
- `src/components/`: `harness_tabs.rs`, `project_list.rs`, `session_list.rs`, `working_dir.rs`, and `list_pane.rs` (shared: `ListCursor` with scroll offset, key resolution through the keymap, pane border/title, row layout and styles). Data goes in through `Attribute::Custom` payloads and every one can be queried back.
- Tests: `src/app/fixtures.rs` and `src/app/selector_tests.rs` (update tests, loop test through the real `IndexPort`, screen snapshots, cell-level style checks) plus per-component tests/snapshots.
- Existing screen snapshots in `src/app/snapshots` were re-accepted: the only difference is the new tabs/panes/Working Directory box on the previously empty overlay.

Decisions where the spec was silent:
- Selected tab: `primary` plus the `━` row, as in the planner note; no `UNDERLINED` modifier (it would double the underline).
- Stats are dropped when fewer than 10 cells would remain for the title after a 2-cell gap between title and stats.
- Pane title with no project: `Sessions`. A project that vanishes in a live update counts as a move to another project, so the session selection is cleared (and, if Sessions has focus, the newest session of the successor is selected).
- Pressing the number of the harness already shown does nothing. Before the first snapshot arrives all tabs are muted; the first available registered harness by number is shown when snapshots arrive and none is shown yet.
- Scrolling: `App` pushes the number of visible rows (pane height - 3) on resize; the list updates its stored offset when moving or when rows are set, and `view` only reads it (it applies a pure correction so the selection is always drawn).
- A list with no selection resets its scroll offset to 0 (so a scrolled Sessions list does not carry its offset over to the next project); tests: `list_pane::tests::a_list_without_a_selection_shows_its_first_rows`, `a_scrolled_sessions_list_does_not_carry_its_offset_to_the_next_project`.
- The cell-level style checks run against a copy of the default theme with distinct colours for foreground, muted, primary, secondary foreground, ring and border (`distinct_theme`), so they cannot pass by swapping two roles.
- Enter in Projects moves focus to Sessions; Enter in Sessions does nothing yet (open comes with a later task).
- `Config.utc`/`timestamp_format` drive the rows; tests set `utc = true`.
- Working Directory text width is the box width minus 6 (starts at column 3, 3-cell right margin).

## Round 2
- [must-fix] Moves in the Sessions pane never requested a redraw: `App::session_selected` now pushes the selection through `set_attr` (which requests the redraw); `project_selected` also pushes the Projects selection, so the App stays the source of truth. Test: `moving_in_either_list_asks_for_a_redraw`.
- Index subscription has its own `worker_events_clause()` (always), documented as never filtered by focus.
- `keep_selection` clones the `Arc` of the projects, not the list.
- `registered_harnesses` and `build_harnesses_with_home` derive from one `enabled_harnesses` table.
- `index_event_kind` doc says the discriminant ignores the payload.
- Stats of sessions absent from a harness's new snapshot are dropped (`stats_of_vanished_sessions_are_dropped_on_a_snapshot`).
- A shown harness that becomes unavailable gives way to the first available one, or to nothing (`the_shown_harness_becoming_unavailable_gives_way_to_an_available_one`), so an unavailable tab is never selected.
- Test notes the arrival-order assumption of `the_first_available_harness_by_number_is_shown`.
- `GAP.width()` instead of `GAP.len()`.
- Duplicate state-4 snapshots: the band one was dropped (covered by `sessions_focused_120x40_styled` and the cell check); the unselected-row one now renders Sessions focused with s2 selected, so the primary `●` on s1 sits above a band (`the_active_marker_in_an_unselected_row_above_a_band_120x40_styled`).
- Unfocused selection in Sessions: cell check with the distinct-role theme (`the_unfocused_selection_in_sessions_is_bold_secondary_without_a_band`).
- Not changed: the `UNDERLINED` modifier on the selected tab (deliberate, planner note).

## Self-check
- All round-1 AC/UI mappings still hold (see the test names in the round-1 notes in git history); the state-4 snapshot names changed as listed above.
- Review: code-review 1 / feature-review 1 (redraw) — fixed and tested as above; code-review 2-7, feature-review 2-4, ui-review 1-2 — addressed as above; code-review 8 — deliberate, unchanged.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (700 passed, 0 failed)
