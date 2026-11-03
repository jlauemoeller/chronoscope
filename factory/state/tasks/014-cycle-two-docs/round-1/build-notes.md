STATUS: DONE

## Changes
Documentation only. No file under `src/` or `tests/` and no snapshot changed.

- `doc/SPEC.md` (the base spec, written in the present tense, no "cycle 2" notes):
  - §1.1: `section_details` on the `Harness` trait; the details read as the one UI-thread read.
  - §1.3: `TurnSidebar`, `SidebarGap` and `DetailsOverlay` in the component table; the sidebar's rows follow the transcript's `ModelUpdate`s and swap with a subagent. §1.4: focus between transcript and sidebar, details overlay among the overlays, worker results to the sidebar.
  - §3.1, §3.2: fields now read (usage, `stop_reason`, `isApiErrorMessage`, `turn_duration` timestamp, `toolUseResult.isAsync` via a typed head, `messageCount` not read) and the detail fields re-read on demand; OpenCode `time`, `tokens`, `finish`, `rawFinish`, `error`.
  - §4.1: the shared duration format (table, truncation, no padding), the selector dropping stats below 10 cells of title.
  - §4.2: the turns sidebar (layout, rows, selection, durations, live updates, performance).
  - §4.3: the details overlay (frame, keys, groups table, where rows come from, `unavailable`).
  - §5.1: model pseudo-code with `sources`, `Item::Notice`, `ModelChange`, `TurnStats`, `TokenUsage`; bullets for tokens, sources, tool durations and the notice table; rule 6 as built (wall-clock duration, fallbacks, per-turn message count without `<synthetic>`, tokens); OpenCode stats and notices.
  - §5.2: section table (Notice row, tool durations, footer parts), footer form with `cached`, tool call rows with the tool duration table, Notice rows.
  - §6.3: `1` / `Tab` in the browser, the sidebar's keys table and rules. §6.4: save file-kind words incl. `error` / `notice`. §6.5: `i`. §6.9: sidebar mouse.
  - §7.2, §8.1, §8.2, §8.5 (contexts, `[keys.turns]` in the example, chords in `turns`), §9.2 (detail fields not parsed, sidebar rows, budgets measured), §9.5 (fields optional).
- `README.md`: verified the `details`, `focus_turns`, `next_pane`, `prev_pane` rows and the `[keys.turns]` table (already present, keys match the keymap). Added a browser paragraph in Usage (sidebar, details, footer, notices), the list of contexts, notes on `1` / `Tab` and on the sidebar's keys, the details overlay in the overlay context text, sidebar mouse text, `turns` among the contexts that take chords, and `[keys.turns]` to the config example (also in `doc/SPEC.md` §8.5, identical).
- `docs/performance.md`: figures from runs made for this task (budget test four times, `cargo bench`), the new fixture table and generator text (`toolUseResult`), the before/after comparison on the same fixtures, and the sidebar, key press and details figures refreshed.

## Findings the orchestrator may want
- The fixture changed (toolUseResult repeats each output, so the mixed session now has 48 large outputs, 312 turns; dense 6 856 turns). "Unfolding the slowest large output" now takes 116 to 173 ms (it was 8 to 12 ms). I ran the pre-cycle-2 code (worktree at `6bd3595`, deleted since) against the new fixture and got 116 ms, and 8 ms against the old fixture: it is the fixture, not a regression of cycle 2. It is a one-off key press outside the per-frame budget, but it is over a frame; the doc says so. I did not investigate why this fixture is slower to unfold.
- Full parse is about 40 ms (mixed, 96 -> 136 ms) and 33 ms (dense, 264 -> 297 ms) slower than before cycle 2 on the same fixtures (one baseline run each). Budgets unchanged and met by 7x or more.

## Self-check
- AC: §3.1/§3.2 list fields now read — `doc/SPEC.md` §3.1, §3.2 (read by hand against `wire.rs`, `opencode/assemble.rs`, `details.rs`)
- AC: §5.1 model and rule 6 — `doc/SPEC.md` §5.1, checked against `src/model/mod.rs`, `claude/assemble.rs` (`on_turn_duration`, `message_notice`)
- AC: §5.2 footer, tool call rows with durations, Notice row — `doc/SPEC.md` §5.2, checked against `renderers/stats.rs`, `tool/timing.rs`, `text.rs`
- AC: §4.3 / §6.5 details overlay and key — `doc/SPEC.md` §4.3 and §6.5
- AC: §4.1 / §5.2 shared duration format — `doc/SPEC.md` §4.1 (table), §5.2
- AC: §1.3 / §4.2 / §6.3 / §6.9 sidebar, component table, keys, mouse — `doc/SPEC.md`
- AC: §8.5 `[keys.turns]` — `doc/SPEC.md` §8.5
- AC: README key table and `[keys.turns]` — `tests/release_docs.rs::readme_keybindings_match_the_default_keymap`
- AC: README example equals the spec's — `tests/release_docs.rs::readme_config_example_is_the_one_in_the_spec`, `readme_config_example_loads_without_warnings`
- AC: performance figures from a run made for this task, new fixture table — `docs/performance.md`
- AC: no `src/` or `factory/state/` change (other than these notes), no snapshot change — `git status` shows only README.md, doc/SPEC.md, docs/performance.md and these notes
- UI: none

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (3181 passed, 0 failed)
- cargo test --release -- --ignored budgets --nocapture: pass (4 runs; all budgets met)
