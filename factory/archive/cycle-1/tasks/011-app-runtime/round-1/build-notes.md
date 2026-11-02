STATUS: DONE

## Changes
- `Cargo.toml`: added `ctrlc` (feature `termination`) and, as a unix-only dev-dependency, `expectrl` 0.9.
- `src/lib.rs`: new modules `app`, `components`, `terminal`; `run()` now starts the application after `--help`/`--version` handling.
- `src/terminal.rs`: `TerminalSetup::from_mouse_config` (pure mouse-capture decision), `TerminalGuard` (raw mode, alternate screen, mouse capture only if configured; restores on `Drop`), idempotent `restore_terminal()` (disable mouse, leave alternate screen, show cursor, raw mode off), `install_panic_hook_with(restore, previous)` (injectable) and `install_panic_hook()`, `install_signal_handler()` (ctrlc: SIGINT/SIGTERM/SIGHUP set a flag).
- `src/app/mod.rs`: `App` (owns the tui-realm `Application`, config, theme registry, active theme, keymap, startup warnings, size, quit and redraw state), `Id`, `Msg` (`Quit`, `Resized`, `Tick`), `UserEvent` (empty for now, `Eq + Clone`), `App::update` (dispatcher with one small handler per Msg), `App::view` (layout fn + `Application::view` per visible component), `production_listener()` (crossterm input every 20 ms, tick 250 ms). `App::new` takes the `EventListenerCfg`, so tests build an app without any port.
- `src/app/layout.rs`: pure `layout(&LayoutInput, Rect) -> ScreenLayout`. Too small (< 60 cols or < 20 rows) gives only `EmptyState` over the full screen; otherwise `SelectorFrame` maps to the overlay rect.
- `src/app/redraw.rs`: `RedrawLimiter` (+ `RedrawRequest`), pure with injected `Instant`s.
- `src/app/runtime.rs`: `run_loop` (tick -> update every Msg -> draw if the limiter allows) generic over `TerminalAdapter`, and `run_app` (startup sequence of the planner notes, guard dropped after the app so the listener stops first).
- `src/app/startup.rs`: `AppInit::load` loads config, builds registry, resolves theme, builds keymap and collects all warnings (falls back to defaults with a warning if no home dir).
- `src/components/`: `GlobalEvents` (quit via keymap `global`/`quit` + literal `Ctrl-c`, resize -> `Resized`, tick -> `Tick`; `GlobalEvents::subscriptions` and `quit_key_events` build the `Sub`s from the active keymap, all through one `subscription_clause()`; plain character keys are also subscribed with `Shift`), `SelectorFrame` (draws into the overlay rect given as `view` area; `frame.area()` is the screen; `base` prop; theme via `THEME_ATTR`), `EmptyState` (message prop, `TOO_SMALL`).
- Tests: unit tests in each module, `src/app/tests.rs` (update tests, loop tests with a scripted Poll port + `TestTerminalAdapter`, screen snapshots), `tests/pty.rs`, `tests/panic_hook.rs`.

Decisions to note
- Resize vs. the 50 ms rule: the spec says both "never two draws less than 50 ms apart" and "draw immediately after a resize". The limiter applies the 50 ms interval to ordinary (dirty) requests and lets a resize request bypass it; this is the one documented exception (doc comment + tests for both).
- `TerminalGuard::enter` does not call `clear_screen`: crossterm's clear queries the cursor position (`ESC[6n`), which fails on a terminal that never answers (the pty in the test); the alternate screen is already blank and the first frame paints everything.
- `Ctrl-Shift-C` is not treated as `Ctrl-c` (the keymap normalises it to `C`).
- `App::update` returns `()`; a `Tick` does not mark the state dirty yet (nothing visible depends on it).

## Self-check
- AC: pty start, `q` / `Ctrl-q` / `Ctrl-w` / `Ctrl-c` exit 0 with `ESC[?1049l` and `ESC[?25h` — tests/pty.rs `q_quits_and_restores_the_terminal`, `ctrl_q_quits_and_restores_the_terminal`, `ctrl_w_quits_and_restores_the_terminal`, `ctrl_c_quits_and_restores_the_terminal` (also SIGTERM/SIGHUP/SIGINT, other keys do not quit, remapped quit key)
- AC: `GlobalEvents::on` quit keys, Ctrl-c, resize; `quit = "x"` — `components::global_events::tests::every_default_quit_key_quits`, `a_window_resize_becomes_resized`, `a_remapped_quit_key_replaces_the_default`, `default_quit_subscriptions_cover_the_default_keys_and_ctrl_c`, `subscriptions_follow_the_remapped_quit_key`; end to end through real subscriptions: `app::tests::every_quit_key_ends_the_loop`, pty `a_remapped_quit_key_quits_and_q_does_not`
- AC: `update(Quit)` sets quit, `update(Resized)` marks dirty — `app::tests::quit_message_sets_the_quit_state`, `resized_message_marks_the_state_dirty_and_records_the_size`
- AC: `RedrawLimiter` — `app::redraw::tests::*` (`a_dirty_request_waits_for_the_interval`, `ordinary_draws_are_never_less_than_the_interval_apart`, `nothing_is_drawn_when_clean`, `a_resize_is_drawn_immediately`, ...)
- AC: `layout()` sizes — `app::layout::tests::narrow_terminal_shows_only_the_empty_state` (59x24), `short_terminal_shows_only_the_empty_state` (80x19), `overlay_at_80x24`, `overlay_at_120x40_is_full_width`, `overlay_at_200x50_is_160_wide_and_centred`
- AC: panic hook order — tests/panic_hook.rs `restore_runs_before_the_previous_hook_on_every_panic`
- AC: mouse capture only when `mouse = true` — `terminal::tests::mouse_capture_follows_the_mouse_config`; pty `mouse_capture_is_on_by_default_and_released_on_exit`, `mouse_capture_is_not_enabled_when_the_config_turns_it_off`
- AC: component + screen snapshots — see UI below; `selector_frame::tests::*`, `empty_state::tests::*`
- AC: fmt / clippy / test — see Test gate
- UI: startup 80x24 text — `app::tests::startup_frame_80x24_text`
- UI: startup 120x40 text — `app::tests::startup_frame_120x40_text`
- UI: startup 200x50 text (frame spans columns 20-179) — `app::tests::startup_frame_200x50_text`, `startup_frame_200x50_spans_columns_20_to_179`
- UI: startup 80x24 styled (ring, surface, background) — `app::tests::startup_frame_80x24_styled`
- UI: transparent theme 80x24 styled — `app::tests::startup_frame_with_a_transparent_theme_80x24_styled` (also asserts no cell has surface or background bg)
- UI: too small 59x24 and 80x19 text — `app::tests::too_small_59x24_text`, `too_small_80x19_text`
- UI: component snapshots — `selector_frame::tests::frame_over_the_base_screen`, `frame_over_a_session_leaves_the_outside_alone`, `transparent_frame_paints_no_background_or_surface`; `empty_state::tests::too_small_message_text`, `too_small_message_styled`, `transparent_theme_paints_no_background`

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (all test binaries ok; 272 lib tests, 11 pty, 1 panic-hook, 0 failed; run three times, stable)
