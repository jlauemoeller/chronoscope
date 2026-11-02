VERDICT: PASS

## Summary
Round 4 adds tests and rewords one comment, so nothing the overlay draws changed: no `.snap`, `src/components`, `src/app` or `src/model` file differs from round 3, and the only change in `details.rs` is that comment. The two required snapshots still match the `UI` section and a fresh render of each, and every state and size rendered in the scratch crate is right. Round-3 UI nit 1 is closed: the WAL test now asserts that no group is `unavailable`. Round-2 UI nits 2 and 3 stay declined by the round-2 verdict and are not raised again. There are no findings.

## States checked
Unless a line says otherwise, renders went through `App` on the OpenCode fixture in the scratch crate at 120x40 in the `default` theme with `utc = true`. Row changes made after the session loaded are marked "after load", because the assembler itself would not accept some of those rows.

Required snapshots:
- Assistant text section @ 80x24, `default` — snapshot `the_details_of_an_opencode_text_section_80x24_text` — ok (identical to the scratch render apart from trailing blanks; content continues under `↓ more`)
- Assistant text section @ 120x40, `default` — snapshot `the_details_of_an_opencode_text_section_120x40_text` — ok (identical to the scratch render apart from trailing blanks; Turn ends in `cost`, `provider`, `agent`, then Message and Environment, in the order of §5.3)

Sizes of the text section:
- 80x24 and 60x20, scrolled to the end — rendered in scratch crate — ok (the bottom frame loses `↓ more` once the last row is reached; every group is reachable)
- 200x60 — rendered in scratch crate — ok (same width as 120x40, all rows shown)
- 40x14 and 30x10 — rendered in scratch crate — ok (`Terminal too small`, the overlay does not open, no panic)
- Overlay open, resized through 120x40, 80x24, 60x20, 40x14, 20x8 — rendered in scratch crate — ok (`Terminal too small` at the two smallest, no panic)

Section kinds, each opened and closed with `Esc` in one session:
- Synthetic and system sections — rendered in scratch crate — ok (Environment with `timestamp`)
- User prompt — rendered in scratch crate — ok (Prompt with `time.created` only; no `timestamp` row beside it)
- Thinking section — rendered in scratch crate — ok (Turn, Message, Environment)
- Tool calls in the first and second rows of the turn — rendered in scratch crate — ok (`start`, `duration`, `status` in that order; Turn's `provider` and `agent` come from the first row)
- Turn footer — rendered in scratch crate — ok (Turn only)

The states the new tests pin:
- Wrong type, after load: `time.created`, `time.streamed`, `time.completed`, `snapshot.start`, `snapshot.end` — rendered in scratch crate — ok (that row alone is left out; no `unavailable`)
- `model` not an object, after load — rendered in scratch crate — ok (no `provider`, `agent` kept, no `unavailable`)
- Tool part `time` not an object, after load — rendered in scratch crate — ok (`duration`, `status`; no `start`, no `unavailable`)
- Tool part `id` of another type, named by position — rendered in scratch crate — ok (`start`, `duration`, `status`)
- `finish: "error"` with `error` missing, `7`, `null` or `[1]` — rendered in scratch crate — ok (no `error` row, no `unavailable`)
- `finish: "error"` with `error` of `{}`, a string, `{type, message}` or an aborted type — rendered in scratch crate — ok (`error  error`, `error  boom`, `error  api: overloaded`, `error  aborted: by user`)
- First row of the turn deleted, tool call in the second row, after load @ 120x40 and 80x24 — rendered in scratch crate — ok (`unavailable` in Turn only; the Tool call group keeps its `start`)
- Section's own row deleted, in a later row of the turn, after load — rendered in scratch crate — ok (Turn keeps `provider` and `agent`; `unavailable` in Tool call and Environment)
- Database in WAL mode — rendered in scratch crate — ok (real values, no `unavailable`, same rows as the rollback-journal render)

Other states:
- Row deleted @ 120x40 and 80x24, after load — rendered in scratch crate — ok (one `unavailable` row each in Turn, Message and Environment; overlay open; `Esc` closes it)
- `data` not parseable and `data` a JSON array, after load — rendered in scratch crate — ok (same three `unavailable` rows)
- Database file gone @ 120x40 and 80x24, also scrolled to the end, after load — rendered in scratch crate — ok (same three `unavailable` rows, frame intact)
- `unavailable` at cell level — rendered in scratch crate — ok (`Rgb(161, 161, 161)`, not bold, the same as the labels; group title bold; values `Rgb(250, 250, 250)`)
- Row with no detail fields — rendered in scratch crate — ok (Message with `row id` only; no Environment group)
- Notice sections: error, aborted with no message, `finish: "length"` — rendered in scratch crate — ok (`error` row is `api: overloaded` and `aborted`; `length` has `finish  length` and no `error` row)
- Blanks around values, and blank `agent`, `rawFinish` and `snapshot.end` — rendered in scratch crate — ok (values trimmed and in line; blank fields have no row)
- One tool id on a `reasoning` part and two `tool` parts — rendered in scratch crate — ok (`start` and `duration  5s` come from the last tool part)
- Non-object parts before a tool part with no id — rendered in scratch crate — ok (found by position)
- Tool part position overwritten by a `reasoning` part, after load — rendered in scratch crate — ok (`duration`, `status`, one `unavailable`; overlay open)
- Long `agent`, `error` and `snapshot.start` @ 120x40, 80x24 and 60x20, also scrolled to the end — rendered in scratch crate — ok (char-wrapped in the value column, as task 007 pins; frame intact)

Gates, run on `HEAD`:
- `INSTA_UPDATE=no cargo test` — pass (2905 passed, 0 failed, 2 ignored)
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` — pass
- `INSTA_UPDATE=no cargo test --release -- --ignored budgets --nocapture` — pass (worst first content 2.6 ms, full parse 354.6 ms, one endless turn 722.9 ms, frame 5.47 ms, peak heap 0.63x the file)
- Snapshots — `git diff main...HEAD` adds exactly the two snapshots above, none changed since they were added, and there is no `.snap.new` file.

## Findings
None.
