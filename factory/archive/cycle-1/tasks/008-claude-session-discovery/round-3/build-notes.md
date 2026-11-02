STATUS: DONE

## Changes
- `src/harness/claude/discovery.rs`: `ProjectId` is now built by a new helper `project_id(&str)` from the raw cwd string Claude wrote, with only trailing `/` and `\` trimmed (a lone `/` and a drive root such as `C:\` are kept). No separator is rewritten, so ids are the same on every OS. The `cwd` field of `SessionSummary`/`Project` stays the component-normalised `PathBuf` (used for alias matching and the display name). No code path builds a `ProjectId` from a `PathBuf`'s string any more.
- Doc on `refresh()` says the first `projects()`/`sessions()` call also builds under the same lock.

## Self-check
- Approver: ProjectId from the raw string, trailing separators trimmed, roots kept — `project_ids_trim_trailing_separators_only` (`C:\Users\u\p\`, `/a/b/`, `/a/b`, `/`, `C:\`, plus `/a/b//` and a mixed-separator path), which gives the same results on every OS.
- Approver: existing tests unchanged and passing, including `cwds_that_differ_only_by_a_trailing_separator_are_one_project` and `sessions_carry_their_summary_fields`.
- Approver (optional): refresh() doc — done. Also covers round-2 feature-review nit 1.
- UI: none

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (325 passed, 0 failed)
