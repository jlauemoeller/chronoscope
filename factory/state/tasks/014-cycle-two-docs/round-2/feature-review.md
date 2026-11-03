VERDICT: PASS

## Summary
Both round-1 must-fixes are resolved: `doc/SPEC.md` §3.1, §9.2 and §9.5 now state what the parsers do, and the small round-1 items are done. The `docs/performance.md` explanation of the 116 to 173 ms unfold also holds up under my own timing. The gates pass and nothing changed under `src/`, `tests/` or the snapshots. Four small documentation inaccuracies remain as nits.

## Acceptance criteria
This is a documentation task, so each criterion below was checked by tracing the text against the code. The README ones are also covered by `tests/release_docs.rs`.

- [x] `doc/SPEC.md` §3.1 and §3.2 list the fields now read — checked against `src/harness/claude/wire.rs`, `claude/details.rs`, `opencode/assemble.rs` and `opencode/details.rs`. The bullets around the lists are now true: wrong types in §3.1 are scoped to the overlay's re-read, and `toolUseResult` to the assembler and the overlay.
- [x] §5.1 describes the model (`TokenUsage`, `Item::Notice`, source references) and rule 6 as built — unchanged since round 1 except the message reference, which now matches `src/model/sources.rs` (`a_message_item_gets_the_latest_stopped_line_else_the_latest`).
- [x] §5.2 has the footer with `cached`, tool call rows with durations, and a Notice row — as verified in round 1; not changed in round 2.
- [x] §4.3 and §6.5 describe the details overlay and its key — as verified in round 1; §1.4 now also lists `DetailsOverlay`, matching `key_clause` in `src/components/global_events.rs`.
- [x] §4.1 and §5.2 give the shared duration format — as verified in round 1 against `src/duration.rs`; not changed in round 2.
- [x] §1.3, §4.2, §6.3 and §6.9 describe the sidebar, its component, keys and mouse, and §1.3 lists `TurnSidebar` and `DetailsOverlay` — as verified in round 1; not changed in round 2.
- [x] §8.5 has the `[keys.turns]` context — as verified in round 1; not changed in round 2.
- [x] The README's browser key table has `details`, `focus_turns`, `next_pane` and `prev_pane`, and there is a `[keys.turns]` table — `readme_keybindings_match_the_default_keymap` passes.
- [x] `docs/performance.md` has the figures of a run made for this task, including the new fixture table — four runs of the budget test here reproduce the fixture table (3 289 / 156 / 867 / 48 / 312 and 71 601 / 3 428 / 18 892 / 0 / 6 856). The other figures fall in or close to the page's ranges: full parse 102.5 to 140.5 ms in one run, expansion 37.5 to 38.8 ms, unfold reference 116.4 to 120.9 ms, details read 1.2 ms, endless turn 681.3 ms against the page's 665 to 677 ms. One size label is wrong (finding 1).
- [x] `tests/release_docs.rs` passes, including both named tests — 17 passed.
- [x] No file under `src/` and none under `factory/state/` changes, apart from this task's own round files — `git diff --name-only main...HEAD` shows `README.md`, `doc/SPEC.md`, `docs/performance.md` and files under `factory/state/tasks/014-cycle-two-docs/`.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass — rerun on an export of HEAD with `INSTA_UPDATE=no`: 3181 passed, 0 failed, 2 ignored.
- [x] `cargo test --release -- --ignored budgets --nocapture` stays within budget — rerun with a timing probe added to the export's `tests/budgets.rs`: passes, four times.
- [x] No snapshot changes — no `*.snap` file in the diff.

## Round-1 findings
- Feature 1 / code 1 (§9.5 robustness): fixed. The leniency list matches `wire.rs`, `shared::lenient*` and OpenCode `Tokens`, `finish` and `error`. A probe of `message.id: 5` gives `unparsed_count() == 1` and no turn.
- Feature 2 / code 3 (`toolUseResult` as a value): fixed. Both places are scoped, and §9.2 states the exception, which matches `find` and `find_by_result` in `src/harness/claude/subagent.rs`. See finding 2 for a separate inaccuracy in the same sentence.
- Code 2 / feature 4 (unfold cause): fixed in substance. In four runs the slowest unfold was about 10 ms before the expansion and 115 to 125 ms after it, on the same output (1 573 956 bytes, 21 865 lines). The expanded output is in the same turn and has 19 798 lines, as the page says. Every other output stays under 8 ms after the expansion.
- The remaining round-1 nits requested in the build notes are done, apart from the wording in finding 4.

## Findings
1. [nit] `docs/performance.md` line 90 says the slowest unfold before any expansion "took 10.3 ms (1.9 MB), and the others 0.2 to 7 ms". In four runs of the same measurement, the 10.1 to 10.6 ms output was always the 1 573 956-byte one in turn `...06d3`, the same output as the next bullet. The 1.9 MB output (1 959 957 bytes) took 7.2 to 7.6 ms, and the second slowest took 7.2 to 7.7 ms. The conclusion (about 10 ms, under a frame) stands. Change: write "10.3 ms (1.5 MB, the same output as in the next bullet), and the others 0.2 to 8 ms".
2. [nit] `doc/SPEC.md` §9.2 (line 810) says `toolUseResult` is read "by the assembler and the details overlay through a typed head and never loaded as a whole value". That holds for the overlay's `pick_object`, but the assembler's untagged head buffers the whole value while it tries its arms, as the comment on `ToolUseResult` in `src/harness/claude/wire.rs` says. A probe of `Assembler::from_memory` on one 2.25 to 2.4 MB `toolUseResult` peaked at 2.15 MiB without escapes and 7.94 MiB with them, against 0.00 MiB with no such field. This does not block: cycle 2 §3 asks for exactly this head and no `Value`, the buffer is transient, and the peak heap stays far under the budget. Change: replace the clause with "through a typed head and never kept as a `serde_json::Value` (the assembler's untagged head buffers it once while matching; the details overlay skips it unbuffered)".
3. [nit] `doc/SPEC.md` §9.5 (line 843) says a known field of another JSON type "fails its line, which is then skipped and counted". That is not exact for `message.content`. A probe with `content: 5`, or with a text block of `text: 5`, gives `unparsed_count() == 1` but still creates the turn with its model and timestamp and no items (`src/harness/claude/wire.rs` lines 5-6: "the whole content"). Change: after "fails its line (OpenCode: its row)", add "or, inside Claude's `message.content`, the whole content, which is also counted".
4. [nit] `docs/performance.md` lines 103-106 read "They are not scrolled frames, so it is outside the budget, and the frames after it are as fast as before", which mixes plural and singular, and line 104 is short among wrapped prose. Change: write "They are not scrolled frames, so they are outside the budget, and the frames after them are as fast as before", and rewrap.
