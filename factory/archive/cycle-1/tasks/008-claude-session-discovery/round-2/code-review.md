VERDICT: FAIL

## Summary
Both round-1 must-fixes are fixed with tests that would catch a regression:
- the session sort is pinned to `started_at`: the older session is touched last, and the test asserts that last activity runs in the reverse order;
- the title is pinned to the first non-blank line, and `first_prompt` keeps the full text.

The nits on the `attachment` field, the `anyhow` version and the lock doc are handled too. However, the new cwd normalisation in `summarize()` builds `ProjectId` from `Path::components().collect::<PathBuf>()`. That rewrites separators on Windows, so most discovery tests would fail on the `windows-latest` CI job. The gate is green on Linux.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (Linux; 324 passed. Expected to fail on `windows-latest`, see finding 1)

## Findings
1. [must-fix] `src/harness/claude/discovery.rs` `summarize()`, line `let cwd: PathBuf = Path::new(cwd).components().collect();` followed by `ProjectId::from(cwd.to_string_lossy().as_ref())`. This breaks on Windows, and round 2 introduced it.

   **Why it breaks:**
   - `FromIterator for PathBuf` calls `push` on each component.
   - `Component::RootDir.as_os_str()` is `MAIN_SEPARATOR_STR` (std `path.rs`), so on Windows the fixture cwd `/home/user/alpha` becomes the id `\home\user\alpha`.
   - SPEC §9.3 runs `cargo test` on a `ubuntu-latest`/`macos-latest`/`windows-latest` matrix (`.github/workflows/ci.yml`), and §9.1 tier 3 commits to "CI build + test suite".

   **Tests that would fail on Windows** (in `tests/claude_discovery.rs`):
   - every `h.sessions(&ProjectId::from("/home/user/..."))` lookup, because the sessions map is keyed by the id string, so these return empty lists;
   - every call to the `project()` helper, which compares `p.id.as_str()`;
   - `cwds_that_differ_only_by_a_trailing_separator_are_one_project`, which asserts `"/home/user/p"`;
   - `sessions_carry_their_summary_fields`, which asserts `s.project.as_str()`.

   The `cwd.as_deref() == Some(Path::new(..))` assertions compare components, so they still pass. Before this change the id was the raw cwd string, which is the same on every OS.

   It also goes against the planner note that `ProjectId` *is the real cwd*: its string should stay what Claude wrote.

   **Change:** in `summarize()`:
   - keep the component-normalised `PathBuf` for the `cwd` field, which alias matching and `display_name` use;
   - build `ProjectId` from the raw `cwd` string with only trailing `/` and `\` trimmed, and keep a lone root (e.g. `/`) as it is. For example: `let trimmed = cwd.trim_end_matches(['/', '\\']); let id = if trimmed.is_empty() { cwd.as_str() } else { trimmed };`.

   Whatever normalisation you choose, the id string must not change its separators. Do not fix this by building the expected ids through `PathBuf` in the tests, because that would make `ProjectId` depend on the platform. Existing tests, including the trailing-separator test, must pass unchanged.

   Optionally, add a scan/summarize unit test with a Windows-style cwd (e.g. `C:\\Users\\u\\p\\`). It should assert that the id is `C:\Users\u\p` on every OS.
