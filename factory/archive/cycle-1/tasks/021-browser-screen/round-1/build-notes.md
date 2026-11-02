STATUS: DONE

## Changes
- `src/app/session_port.rs` (new): `SessionEvent`, `SessionUpdate` (Arc payload, Eq by sequence number), `SessionPort` (Poll over one long-lived channel), `forward_to(generation, sink)` (a small forwarding thread that tags a load's updates with its generation), `session_event_kind()`. `UserEvent::Session { generation, update }` and `Msg::Session { generation, update }` carry them; `production_listener` takes the session receiver.
- `src/app/browser.rs` (new): browser handlers. `open_session` (looks up the summary in the index, starts `harness.load` through the forwarder, replaces the `WatchHandle`, unmounts selector/prompt/old browser, mounts `BrowserHeader`/`Transcript`/`BrowserHelpLine`/`EmptySession`, focuses `Transcript`), `session_update` (drops stale generations; `Progress` -> `Loading… N%`, `Loaded` clears it and asks the Transcript for its turn count, `ParseWarnings` -> header attr, `Appended`/`Updated`/`Reset` forwarded via `MODEL_UPDATE_ATTR`, never stored), `open_selector` / `close_selector` (selector mounted as an overlay with `SelectorFrame::base(false)`, open session selected, Sessions focused; focus returns to the Transcript through the focus stack).
- `src/app/mod.rs`: new `Id`s (`BrowserHeader`, `Transcript`, `BrowserHelpLine`, `EmptySession`), `Msg::OpenSelector`/`Msg::Session`, `UserEvent::Session`, App fields (harnesses, session sink, `BrowserState`), `update` dispatch, `set_harnesses`/`set_session_sink` setters (not in `AppInit`, which is `Debug + Clone`).
- `src/app/selector.rs`: `mount_selector` (shared by startup and Ctrl-o), `SelectorState::{find_session, find_project, show_session}`, `set_attr` skips components that are not mounted (the selector is unmounted while a session is open), `push_viewports` also announces the transcript's viewport.
- `src/app/layout.rs`: `Screen::{Browser, BrowserSelector}`, `browser_areas` (header rows 0-4, transcript rows 5..h-3, help line last two rows), `LayoutInput::session_empty`; overlay drawn after the browser.
- `src/app/hints.rs`: `browser_hints` from the keymap.
- `src/app/runtime.rs`: harnesses built once and shared by the index and the App; session channel wired into the listener.
- `src/components/browser_header.rs` (new): `BrowserHeader` (`PARSE_WARNINGS_ATTR`, note singular/plural, cwd cut from the left keeping 2 cells before the note).
- `src/components/help_line.rs`: `STATUS_ATTR` and an `inset` (fill covers the full width, text from column 1 to w-2).
- `src/components/transcript/mod.rs`: `TURN_COUNT_ATTR` query (the App asks the component whether the session is empty instead of keeping a count); fixtures made `pub(crate)` for the browser tests.
- `src/components/global_events.rs`: subscribes to the `open_selector` keys (same prompt/overlay clause as the other keys) and the session user events; `action_key_events` generalises `quit_key_events`.
- `src/components/empty_state.rs`: `EMPTY_SESSION`.
- Error path: `open_session` stops the old `WatchHandle` before calling `harness.load`, and mounts the new browser only after the load succeeded. If `load` fails, a toast is shown and the previous screen stays (without live updates). The order is asserted in `opening_another_session_stops_the_first_load_and_starts_a_fresh_transcript`.

## Self-check
- AC: `OpenSession` mounts header/transcript/help line, unmounts selector, focuses Transcript, starts a load — `opening_a_session_mounts_the_browser_unmounts_the_selector_and_starts_a_load`, plus `a_session_the_index_does_not_list_is_ignored`, `a_load_that_fails_leaves_the_selector_and_shows_a_toast`.
- AC: Port -> `UserEvent::Session` -> `GlobalEvents` -> `Msg` -> `App::update` -> Transcript `attr`, App holds no copy — `updates_travel_from_the_port_through_global_events_to_the_transcript`, `the_app_keeps_no_copy_of_the_turns_it_forwards` (Arc strong counts), `updated_and_reset_reach_the_transcript_too`, `session_port::tests::*`, `global_events::tests::a_session_user_event_becomes_a_session_message`.
- AC: stale generations ignored — `updates_of_an_earlier_load_are_ignored_after_another_session_is_opened`, `opening_another_session_stops_the_first_load_and_starts_a_fresh_transcript`.
- AC: `Progress(0.34)` -> `Loading… 34%`, `Loaded` removes it — `progress_shows_loading_in_the_help_line_and_loaded_removes_it`, `the_percentage_is_rounded`, snapshot `loading_browser_80x24_text`.
- AC: `ParseWarnings(3)` / `(1)` notes — `browser_header::tests::three_unparsed_entries_are_noted_on_the_cwd_row`, `one_unparsed_entry_is_singular`, `the_parse_warnings_are_set_through_an_attribute`, app level `parse_warnings_reach_the_header`.
- AC: empty session shows `Session file is empty` after `Loaded` — `a_session_without_turns_is_empty_once_loaded`, `turns_that_arrive_later_replace_the_empty_state`, snapshot `empty_session_80x24_text`.
- AC: Ctrl-o / Esc / Enter on another session / `q` in the search prompt — `ctrl_o_opens_the_selector_over_the_browser_on_the_open_session`, `esc_closes_the_selector_and_returns_the_focus_to_the_transcript`, `enter_on_another_session_opens_it`, `enter_on_the_open_session_just_closes_the_selector`, `q_typed_in_the_selectors_search_prompt_does_not_quit`, `opening_a_session_from_a_search_result_unmounts_the_prompt_too`, `ctrl_o_does_nothing_on_the_startup_selector`, `ctrl_o_with_the_selector_already_open_changes_nothing` (all driven through `App::tick` and the `GlobalEvents` subscriptions).
- AC: browser help line text — `the_browser_help_line_reads_as_the_spec_says`, `hints::tests::default_browser_hints_read_as_the_help_line_of_the_spec`, snapshots `loaded_browser_120x40_text`.
- AC: screen snapshots for all states and the seven themes — see UI below.
- UI: loaded 120x40 text / styled, 80x24 text — `loaded_browser_120x40_text`, `loaded_browser_120x40_styled`, `loaded_browser_80x24_text`.
- UI: loading 80x24 text — `loading_browser_80x24_text`.
- UI: empty session 80x24 text — `empty_session_80x24_text`.
- UI: parse warnings 120x40 text — `parse_warnings_120x40_text` (and the header's own styled snapshot `header_with_a_note_styled_snapshot`).
- UI: selector over the browser 120x40 and 200x50 text — `selector_over_the_browser_120x40_text`, `selector_over_the_browser_200x50_text` (the latter also asserts the browser in columns 0-19 and 180-199).
- UI: per-theme 120x40 styled (default, terminal, catppuccin, gruvbox, nord, tokyo-night, solarized) — `loaded_browser_theme_*`.
- Also: layout (`layout::tests::the_browser_*`, `the_selector_over_the_browser_*`), every cell painted with the background (`every_cell_of_the_browser_is_painted_with_the_background`), transparent theme, viewport follows the terminal size, help-line `inset`/`STATUS_ATTR` component tests.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (1341 passed, 0 failed)
