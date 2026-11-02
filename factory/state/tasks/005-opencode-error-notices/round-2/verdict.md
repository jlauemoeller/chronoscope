VERDICT: APPROVE

## Assessment
All three reviews pass, and no finding in them is above a nit. The round-1 must-fix (no test covered the index rebuild in `remove`) is resolved: the code and feature reviewers each deleted the rebuild in a scratch copy and saw the new test fail.

My own checks on this branch, all with `INSTA_UPDATE=no`:
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass.
- `cargo test` exits 0: 27 suites, 2613 passed, 0 failed, 2 ignored. No `.snap.new` files were left and the worktree is clean.
- `src/` and `tests/` have not changed since `21ab348`, so the reviews describe this code.
- Against `main`, 24 snapshots are added and none modified.

I did not re-run the budgets. The builder and all three reviewers report them within budget, and nobody disputes that. I also did not read the local database. Statements below about the 12 real rows come from the code and UI reviewers; the code reviewer notes that its immutable read excludes anything only in the WAL file.

### Code review: PASS
1. [nit] Export escaping in `emphasized_lines` is incomplete. Ignored. Backslashes, backticks, HTML-like text and deep indents can render wrongly, but no notice text goes missing from the file. The code reviewer read the 12 real rows, and the only two with a markdown-significant character (`` `verbose: true` ``) export correctly.
2. [nit] `a_hyphenated_word_is_not_split_across_rows_so_a_search_highlights_it` passes even with the old wrap. Ignored. The three `error_long_*_at_80` snapshots still catch that regression, so the behaviour is protected and only the test's name overstates it.
3. [nit] Duplicate operand in the `select` assertion at `notice.rs:326`. Ignored. It has no effect on what the test checks.
4. [nit] `part` in `ending_notice` allocates for every row with a `finish`. Ignored. The budgets are unaffected.
5. [nit] One overlong line in the `replace_row` doc comment. Ignored. It is cosmetic.

### Feature review: PASS
Every acceptance criterion is met.
1. [nit] The same weak hyphen test as code finding 2. Ignored for the same reason.
2. [nit] The `trim_end` in the `Notice` arm of `section_text` has no test. Ignored. OpenCode cannot reach it, because `ending_notice` trims every part of the text.
3. [nit] The same export escaping as code finding 1. Ignored for the same reason.
4. [nit] The same duplicate operand as code finding 3. Ignored.

### UI review: PASS
All required states and sizes match the task's `UI` section, and the rendered real rows are correct.
1. [nit] A notice with a leading newline, or with a control character alone on a line, draws a blank row with the `▒` marker, and a notice of only a control character is a selectable section with nothing to show. Ignored. OpenCode cannot produce the leading newline because `ending_notice` trims. The control-character cases are possible but cosmetic, and none of the 12 real rows reach them.
2. [nit] The same export escaping as code finding 1, plus `got <think> tag` passing through as raw HTML, which a markdown viewer hides. Ignored for the same reason. The text is still in the file, and no real row is affected. No reviewer raised it as a security issue, and I do not treat it as one: the export already writes assistant text verbatim (`section_markdown`, `SectionContent::Text`), so a notice is not a new way for HTML to reach the file.

### Also noted
The planner notes list `plain.rs` among the matches needing a `Notice` branch, and the diff leaves it unchanged. Ignored: it compiles, and `y_copies_the_text_of_the_notice`, `a_copy_of_a_wrapped_notice_has_no_added_breaks` and the UI reviewer's `y`/`Y`/`w`/`W` run show that copy and save work.

### Carry forward to the Claude notices task
These do not block this task, but they stop being unreachable there, because Claude notices take the message's own text through the same paths:
- Normalise notice text in one place (control characters to spaces, then trim both ends), and use it in `NoticeSection`, `section_text` and the blank check in `item_content`.
- Escape the rest of CommonMark's markup in `emphasized_lines`, and handle indents of four or more columns.

## Conflicts
None. The reviews suggest slightly different escape sets for the same export nit, but those are details of one fix, and the nit is ignored here.

## Rework budget
Earlier REWORK verdicts: 0 of 2. Round-1 has no `verdict.md`.
