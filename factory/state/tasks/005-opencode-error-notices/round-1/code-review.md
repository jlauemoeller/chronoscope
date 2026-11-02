VERDICT: PASS

## Summary
`Item::Notice` and its section kind are implemented as specified across rendering, selection, search, copy, save and export, and the OpenCode rows of §4.2 produce the pinned texts, including tolerant `finish` and `error` parsing and removal when a row returns to a normal ending. The tests would catch wrong behaviour, the 24 new snapshots match the task's `UI` section for colour, bold and marker, and no existing snapshot changed. The budget run also passes (`cargo test --release -- --ignored budgets --nocapture`: full parse worst 281.3 ms of 2000, frames worst 3.27 ms of 16).

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass

## Findings
1. [nit] `src/harness/opencode/assemble.rs`, `struct RowError` — the derived `Default` is never used: `lenient_error` builds the struct explicitly, and `#[serde(default)]` on `AssistantData::error` uses the default of `Option`. Change: derive `Deserialize` only.
2. [nit] `src/harness/opencode/assemble.rs`, `ending_notice` — `message.map_or(kind.clone(), |m| format!("{kind}: {m}"))` always clones `kind`, even when the clone is thrown away, and the `part` closure takes `Option<&String>`. Change: use `match message { Some(m) => format!("{kind}: {m}"), None => kind }`, and have `part` take `Option<&str>`, with callers passing `as_deref()`.
3. [nit] `src/harness/opencode/assemble.rs`, doc comment of `Assembler::replace_row` — it says items keep their ids and positions, but a row returning to a normal ending now removes its notice and shifts later items. A notice first gained after later rows exist also lands at the end of the turn, after those rows' items, which `notice_tests.rs:278-279` pins. `follow.rs` applies updated rows before new ones, so this needs an update to an older row after a later row has arrived. Change: state both cases in the doc comment.
4. [nit] `src/components/turn_block/export.rs`, `section_markdown`, the per-line emphasis — the notice text goes between `_` delimiters unescaped, so some provider messages break the emphasis in the exported markdown. By CommonMark's rules, a `_` after a letter or digit and before punctuation can close emphasis: a message containing `[a-zA-Z0-9_-]+` ends the emphasis at that `_` and leaves the trailing `_` literal. A line with leading whitespace, such as pretty-printed JSON, gives `_  "error": "x"_`, whose first `_` cannot open emphasis. No text is lost from the file. The planner's `_text_` rule stays in place. Change: backslash-escape `_` and `*` inside each line, put the line's leading whitespace outside the emphasis, and extend `a_notice_of_several_paragraphs_is_emphasised_per_line` with both cases.
5. [nit] `src/components/transcript/tests/notice.rs:220` — the comment "Both notices have a colon; the notice is searched like any text." does not describe the assertion below it, which checks that `aborted` has one match. Change: reword it to say that only the warning contains `aborted`, or remove it.
