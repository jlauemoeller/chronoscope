VERDICT: PASS

## Summary
The round-2 must-fix is fixed: `find_part` now reads only `type: "tool"` parts and the last part with an id wins, so a tool call's `start` and `duration` come from the same part, and mutation checks on a private export of `HEAD` fail exactly the tests the rework note names. The last commit touches only `details.rs`, `details_tests.rs` and the build notes, the branch adds only the two App-level `.snap` files with no `.snap.new`, and one new nit remains. The budgets also pass on my run (`cargo test --release -- --ignored budgets --nocapture`: `budgets` ok, one endless turn full parse 737.2 ms).

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass

## Findings
1. [nit] `src/harness/opencode/details_tests.rs:522-533` (`the_error_row_says_what_the_notice_of_the_same_row_says`) and `src/harness/opencode/details.rs:162-169` — with `finish: "error"`, the `error` row depends on the shape of `error`, and tests pin only part of this. In a throwaway test, `error` missing, `7`, `null` or `[1]` gives the notice `error` and no `error` row. `{}` and `""` give the notice `error` and an `error` row saying `error`. Both results follow the task (missing or wrongly typed fields are left out, and a row that exists has the notice's text), but no test covers the first group, so a later change could alter it unnoticed. The comment at `details.rs:162-163` also reads as if every `error` notice has a row. Change: add a test that `finish: "error"` with `error` missing or `7` has a notice and no `error` row, and reword the comment to say that an `error` value that is missing or of the wrong type is left out even when the notice says `error`.
