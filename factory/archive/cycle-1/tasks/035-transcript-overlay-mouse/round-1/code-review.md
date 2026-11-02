VERDICT: PASS

## Summary
The transcript's wheel and clicks, the help overlay's wheel, the theme selector's mouse, and the outside-click rules for overlays are implemented as the task and planner notes require. Wheel steps go through the page-key path (`scroll_pages` with `Amount::Rows(3)`), so follow mode and the selection-follows rule apply. Clicks are hit-tested through the prepared window's block layouts, and fold toggles go through `fold_command(Toggle)`, so the search's temporary unfold behaves exactly as with `Enter`. Every acceptance criterion has a component or update test that would fail if the behaviour were wrong, the routing respects the §1.2/§1.4 ownership and focus rules, and the gate passes. The remaining findings are minor.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (lib: 2131 passed, 0 failed; all other test binaries pass; no `.snap.new` files, working tree clean afterwards)

## Findings
1. [nit] `src/app/mouse.rs:270` (`theme_clicked`) / `src/app/mouse.rs:129` (`MouseState::press`): the theme double-click state outlives the overlay. If you click a theme, close the selector with `Esc`, reopen it with `t` and click the same theme within 400 ms, the second click counts as a double-click and keeps the theme. An outside click clears `last_press`, but closing and opening with the keyboard don't. Change: set `self.mouse.last_press = None` in `open_theme_selector` (`src/app/theme_selector.rs:51`), or when the selector closes.
2. [nit] `src/app/mouse_tests.rs:775` (`nothing_under_the_help_overlay_or_the_theme_selector_gets_the_mouse`): the rewrite dropped the wheel checks over the covered panes. Nothing now asserts that a wheel step inside the help overlay or theme selector, over a startup pane, leaves the project/session selection alone. Change: in the `for (column, row)` loop, also send `MouseEventKind::ScrollDown` at the same covered point. The existing `(project, session)` assertion after the loop then covers the wheel too.
3. [nit] `src/components/transcript/keys.rs:29` and `src/components/help_overlay/mod.rs:50`: two `pub const WHEEL_ROWS: usize = 3` define the same "rows per wheel step" decision. Change: define it once (e.g. `crate::components::WHEEL_ROWS`) and use it in both components, so they cannot drift apart.
4. [nit] `src/components/transcript/mod.rs` (doc comment of `AppComponent::on`, just above line 671): it still describes only keys. Change: add one sentence saying that a routed `UserEvent::Mouse` is handled by `on_mouse` and answers `Msg::NewSections` or `Msg::TranscriptChanged`, and that the raw `Event::Mouse` is ignored.
