VERDICT: PASS

## Summary
The round-1 must-fix is closed: a row with no usable error text and no notice no longer shows `error  error`, and the row still matches the notice where there is one. The two round-2 reader fixes also render correctly, and the two required snapshots are unchanged and still match the `UI` section. Three nits remain, two of them carried from round 1.

## States checked
Unless a line says otherwise, renders went through `App` in the scratch crate at 120x40 in the `default` theme with `utc = true`.

Required snapshots:
- Assistant text section @ 80x24, `default` — snapshot `the_details_of_an_opencode_text_section_80x24_text` — ok (the content scrolls under `↓ more`)
- Assistant text section @ 120x40, `default` — snapshot `the_details_of_an_opencode_text_section_120x40_text` — ok (Turn ends in `cost`, `provider`, `agent`; then Message and Environment, in the order of §5.3)

`error` row (round-1 finding 1):
- `finish: "stop"` with `error` of `{}`, `{"type":5,"message":true}`, `""` or blank parts — rendered in scratch crate — ok (no `error` row, no `unavailable`)
- `finish: "error"` with `error` of `{}` or `{"type":5,"message":true}`, text and notice sections — rendered in scratch crate — ok (`error  error`, the text of the notice for the same row)
- `finish: "error"` with no `error` field, text and notice sections — rendered in scratch crate — ok (no `error` row, since the field is missing)
- `finish: "tool-calls"` with only `error.message`, and no `finish` with `error` as a string — rendered in scratch crate — ok (the row shows the message)
- `finish: " error "` with `error: {}`, text and notice sections — rendered in scratch crate — issue, finding 1

Repeated tool part:
- A tool part sent again by a later row — rendered in scratch crate — ok (`start`, `duration` and the snapshot all come from the later row; Turn's `provider` and `agent` come from the first row)
- Text sections of the first and the later row — rendered in scratch crate — ok (each shows its own `row id`, Message and Environment rows)
- The later row deleted — rendered in scratch crate — ok (Tool call and Environment each have one `unavailable`; Turn still reads; overlay open)
- The first row deleted — rendered in scratch crate — ok (Turn has one `unavailable`; Tool call and Environment still read; overlay open)

Parts that are not objects (`5`, `null`, a string, an array and `true` among the row's parts):
- Tool part with an id, without an id (found by position), and with a numeric id — rendered in scratch crate — ok (`start`, `duration`, `status`, each with its own values, no `unavailable`)
- Text section of the same row — rendered in scratch crate — ok
- `content` changed to a number after the session loaded — rendered in scratch crate — ok (one `unavailable` in Tool call, the same as a part gone from the row, which round 1 accepted)

Sizes:
- Repeated and non-object tool calls @ 80x24 and 60x20, scrolled to the end — rendered in scratch crate — ok (`↑ more` in the frame; Environment is reachable; frame intact)
- `finish: "stop"` and `finish: "error"` with `error: {}` @ 80x24 and 60x20 — rendered in scratch crate — ok
- Below the minimum, 40x14, 30x10, 20x8 — rendered in scratch crate — ok (`Terminal too small`, no panic)

Re-rendered from round 1:
- Section kinds: system, synthetic, prompt with and without `time.created`, thinking, notices, tool calls completed and pending, model note, footer — rendered in scratch crate — ok
- Row with no detail fields, and fields of an unexpected type — rendered in scratch crate — ok (left out, no `unavailable`)
- Row deleted (text, tool call, prompt, system), `data` not parseable, database gone — rendered in scratch crate — ok (rendered rows and styles identical to round 1; the `unavailable` row stays muted)
- Text section with styles — rendered in scratch crate — ok (identical to round 1)
- Long `provider`, `agent`, `error` and `snapshot.start` @ 80x24, 60x20, 120x40 — rendered in scratch crate — ok (wrapped in the value column, frame intact)
- **Security**: ANSI escape in `error.message` — rendered in scratch crate — ok (no ESC byte reaches the buffer)

Gates:
- `INSTA_UPDATE=no cargo test`, `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` — pass. `git diff 299d0ef..HEAD -- '*.snap'` is empty, and the task's whole diff adds exactly the two snapshots above. The release budgets were not re-run here.

## Findings
1. [nit] Details overlay, Message and Environment groups, any size, a text value with surrounding whitespace — `Rows::text` (`src/harness/opencode/details.rs`) drops only an empty string. `finish: " error "` starts one column right of the other values, and a value of only spaces would draw a row with a blank value. The notice and the `error` row both trim. §5.3 doesn't pin this, and OpenCode isn't known to write such values. Change: in `Rows::text`, trim the value, and leave the row out when nothing is left. See `ui/message-untrimmed-finish-120x40.txt`.
2. [nit] Details overlay, OpenCode text section, 80x24 — carried from round 1. The required snapshot ends at `time.completed`, so `finish`, `rawFinish` and Environment are pinned at 120x40 only. Change: optionally add an 80x24 snapshot scrolled to the end.
3. [nit] Details overlay, OpenCode text section, 80x24 and 120x40 — carried from round 1. The App snapshots are text only, so muted labels and bold group titles are not pinned at this level. This follows the task 008 App snapshots, and the styles render correctly in the scratch crate. Change: optionally add one styled App snapshot in the `default` theme.
