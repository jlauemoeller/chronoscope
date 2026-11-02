STATUS: DONE

## Changes
- `tests/claude_discovery.rs`: the sort test now pins `started_at` as the sort key (`sessions_are_sorted_newest_first_by_start_time_not_last_activity`); new `the_prompt_title_is_the_first_line_only` and a `prompt_harness` helper.
- `src/harness/claude/scan.rs`: `ScanHead::attachment` is now loose JSON, so a non-object `attachment` no longer loses the line (new unit test).
- `src/harness/claude/discovery.rs`: the cwd is normalised through `Path::components` before it becomes the `ProjectId`/`cwd`, so `/a/b` and `/a/b/` are one project (new test `cwds_that_differ_only_by_a_trailing_separator_are_one_project`). Doc on `refresh()` notes that the lock is held for the whole build.
- `Cargo.toml`: `anyhow = "1"`.

## Self-check
- Review code-review 1 [must-fix] sort key — `sessions_are_sorted_newest_first_by_start_time_not_last_activity` (mtime of the older session set later; asserts order and that last activity is the reverse).
- Review code-review 2 [must-fix] first line — `the_prompt_title_is_the_first_line_only` ("Short first line\nsecond line" -> title is line one, `first_prompt` has both; leading blank lines skipped).
- Review 3 (nit, anyhow version) — changed to `"1"`.
- Review 4 (nit, `/` in join literals) — left as is: valid on every platform and the same as existing tests.
- Review 5 and feature 1 (nit, lock held during build) — documented on `refresh()`; to be revisited with the index worker (task 010).
- Review 6 (nit, non-object attachment) — fixed, with a unit test.
- Review 7 (nit, trailing separator) — fixed, with a test.
- UI: none

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (324 passed, 0 failed)
