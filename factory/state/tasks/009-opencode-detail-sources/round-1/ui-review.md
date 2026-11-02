VERDICT: FAIL

## Summary
The overlay's frame, keys and `unavailable` drawing are unchanged, and both required snapshots match the `UI` section, with the rows in the order of §5.3. Every other state and size I rendered is right, including long values, unreadable sources and small terminals. One must-fix remains: the Message group shows an invented `error  error` row for a row that has no usable error text and no notice.

## States checked
Unless a line says otherwise, renders went through `App` in the scratch crate at 120x40 in the `default` theme with `utc = true`.

- Assistant text section @ 80x24, `default` — snapshot `the_details_of_an_opencode_text_section_80x24_text` and rendered in scratch crate — ok (the content scrolls under `↓ more`)
- Assistant text section @ 120x40, `default` — snapshot `the_details_of_an_opencode_text_section_120x40_text` and rendered in scratch crate — ok
- Assistant text section @ 80x24 and 60x20, scrolled to the end — rendered in scratch crate — ok (`↑ more` in the frame; `finish`, `rawFinish` and Environment are reachable)
- Assistant text section @ 200x50 — rendered in scratch crate — ok
- Turn group, all sections of an Assistant turn — rendered in scratch crate — ok (`cost`, then `provider`, then `agent`, last in the group; values from the turn's first assistant row)
- Thinking section — rendered in scratch crate — ok
- Notice sections (`error`, `aborted`, `stopped: length`) — rendered in scratch crate — ok (`error` and `aborted` have an `error` row with the notice's text; `length` has `finish` and no `error` row)
- Tool call, completed @ 120x40 and 80x24 — rendered in scratch crate — ok (`start`, `duration`, `status`, then Environment)
- Tool call, pending — rendered in scratch crate — ok (no `duration` row)
- Tool call, `time.created` of an unexpected type — rendered in scratch crate — ok (`status` only, no `unavailable`)
- Tool call, its part gone from the row — rendered in scratch crate — ok (one muted `unavailable` in Tool call only; Turn and Environment still read)
- Prompt, with `time.created` and snapshot — rendered in scratch crate — ok (`time.created` replaces `timestamp`, shown once; Environment follows)
- Prompt, without `time.created` — rendered in scratch crate — ok (the model's `timestamp` stays)
- System and synthetic sections in a User turn, with and without a snapshot — rendered in scratch crate — ok (Environment: `timestamp`, then `snapshot.start`, `snapshot.end`)
- Synthetic section in an Assistant turn — rendered in scratch crate — ok (Turn, then Environment)
- Model note and stats footer — rendered in scratch crate — ok (the footer shows Turn only, as the build notes decide)
- Row with no detail fields — rendered in scratch crate — ok (Message has `row id` only; no Environment group; no `unavailable`)
- Empty strings and unexpected types in `agent`, `providerID`, `finish`, `rawFinish`, `snapshot`, and `error` as a number — rendered in scratch crate — ok (left out, no `unavailable`)
- Row deleted (text, tool call, prompt, system) @ 120x40; text also @ 80x24 — rendered in scratch crate — ok (one `unavailable` per affected group, after the model's rows; overlay stays open)
- `data` not parseable; database file gone — rendered in scratch crate — ok (same rows; overlay stays open)
- `unavailable` styling, `default` — rendered in scratch crate with styles — ok (`MutedForeground`, Rgb(161, 161, 161), the same as the row labels)
- Row labels, values, group titles and frame, `default` — rendered in scratch crate with styles — ok (new rows are styled like the existing ones)
- Long `provider`, `agent`, `error`, `snapshot.start` @ 60x20, 80x24, 120x40 — rendered in scratch crate — ok (wrapped in the value column, frame intact)
- Multi-line `error.message` with a tab — rendered in scratch crate — ok (a second row, in the value column)
- **Security**: ANSI escape in `error.message` — rendered in scratch crate — ok (no ESC byte reaches the buffer; the remaining text prints literally)
- Below the minimum, 40x14, 30x10, 20x8 — rendered in scratch crate — ok (`Terminal too small`, overlay not mounted, no panic)
- `error` empty or with parts of an unexpected type, `finish: "error"` — rendered in scratch crate — ok (`error`, the text of the notice for the same row)
- `error` empty or with parts of an unexpected type, `finish: "stop"` — rendered in scratch crate — issue, finding 1
- Gates: `INSTA_UPDATE=no cargo test`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` — pass. The diff adds exactly two snapshots and changes no existing one; no `.snap.new` or pending-snapshot files. The release budgets were not re-run here.

## Findings
1. [must-fix] Details overlay, Message group of an OpenCode text, thinking or notice section, every size, row with `finish` other than `error` whose `error` is `{}` or has only blank or wrongly typed parts (for example `{"type":5,"message":true}`) — the group shows `error  error`. The row has no error text and no notice, so the value is the notice's fallback and not something the source recorded. This breaks §5.3 ("Missing fields are left out") and the acceptance criterion that an unexpected JSON type in any detail field is left out; `a_value_of_an_unexpected_json_type_is_left_out_without_unavailable` only covers `"error":12`, and `the_error_row_says_what_the_notice_of_the_same_row_says` only covers `finish: "error"`. Change: in `message_group` (`src/harness/opencode/details.rs`), leave the `error` row out when the row's `finish` is not `error` and its `error` has neither a usable `type` nor a usable `message` after trimming. When `finish` is `error`, keep the `error_text` result so the row still matches the notice. Add adapter test cases for `"finish":"stop"` with `"error":{}` and with `"error":{"type":5,"message":true}`, asserting no `error` row and no `unavailable`. See `ui/message-error-placeholder-120x40.txt`.
2. [nit] Details overlay, OpenCode text section, 80x24 — the required snapshot ends at `time.completed`, so `finish`, `rawFinish` and Environment are pinned at 120x40 only. Change: optionally add an 80x24 snapshot scrolled to the end.
3. [nit] Details overlay, OpenCode text section, 80x24 and 120x40 — the App snapshots are text only, so muted labels and bold group titles are not pinned at this level. This follows the task 008 App snapshots, and the styles rendered correctly in the scratch crate. Change: optionally add one styled App snapshot in the `default` theme.
