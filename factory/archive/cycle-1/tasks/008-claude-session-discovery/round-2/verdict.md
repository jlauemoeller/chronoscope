VERDICT: REWORK

## Assessment
**Code review: FAIL.** Both round-1 must-fixes (sort key pinned to `started_at`, title from the first line) are fixed and have regression tests. One finding remains.
- Finding 1 [must-fix], Windows `ProjectId` separators: **to be fixed.** I checked it against the code.
  - In `summarize()` (`src/harness/claude/discovery.rs`), `Path::new(cwd).components().collect::<PathBuf>()` pushes `Component::RootDir` as `MAIN_SEPARATOR_STR`, and `push` joins with `\` on Windows. The fixture cwd `/home/user/alpha` therefore becomes the id `\home\user\alpha`.
  - Every `h.sessions(&ProjectId::from("/home/user/..."))` lookup and every `p.id.as_str()` comparison in `tests/claude_discovery.rs` fails on the `windows-latest` job of `.github/workflows/ci.yml`. The task's own acceptance criterion requires `cargo test` to pass, and CI runs it on Windows.
  - This is a regression from round 2, introduced while addressing an optional round-1 nit (code-review round-1 finding 7). It also changes the id away from the real cwd string, against the planner note "`ProjectId` is the real cwd".

**Feature review: PASS.** Every acceptance criterion is checked against a test.
- Finding 1 [nit], the `refresh()` doc should also say that the first `projects()`/`sessions()` call builds under the lock: **ignorable on its own.** It is included in the rework note as an optional extra because a rework round happens anyway.

**UI review: PASS.** `UI: none`, and the diff touches no UI code or snapshots. No findings.

**Earlier nits carried from round 1:**
- Round-1 code-review nit 4 (`/` in `Path::join` literals in tests): **ignored.** The code reviewer agreed it is consistency only and valid on every platform.
- Round-1 nit 5 (lock held during the build): documented, and deferred to task 010.

**Decision order:**
1. Not everything is minor: the Windows CI break blocks the merge.
2. There is no A/B choice for a human. The alternatives (revert the normalisation, or trim only trailing separators on the raw string) differ only for the `/a/b/` vs `/a/b` edge case. That is not a significantly different behaviour, and the trailing-separator test from this round settles it in favour of trimming.
3. The code review is FAIL, a single concrete note fixes it, and rework rounds remain: **REWORK**.

## Conflicts
None on the demands themselves. The feature review accepted the round-2 normalisation as satisfying "one project per real cwd", but it did not consider Windows. The code-review fix keeps that criterion and the new trailing-separator test intact, so the two reviews do not ask for incompatible things.

## Rework budget
Earlier REWORK verdicts: 0 of 2. There is no `verdict.md` in `round-1/` and none in git history, so this REWORK is the first.

## Rework note
Fix the `ProjectId` built in `summarize()` in `src/harness/claude/discovery.rs`. At the moment the cwd is normalised with `Path::new(cwd).components().collect::<PathBuf>()`, and the id comes from that `PathBuf`'s string. On Windows this rewrites `/` to `\`, so a cwd of `/home/user/alpha` gets the id `\home\user\alpha`. Most of `tests/claude_discovery.rs` then fails on the `windows-latest` CI job.

What to change:
1. **The `cwd` field.** Keep the component-normalised `PathBuf` for the `cwd` field of `SessionSummary`/`Project`. Alias matching and `display_name` use it, and they are correct as they are.
2. **The `ProjectId`.** Build the `ProjectId` from the **raw cwd string** Claude wrote. Only trim trailing `/` and `\` characters, so `/a/b/` and `/a/b` stay one project, and never change any other separator.
   - Keep a root as it is: a lone `/` stays `/`, and a Windows drive root such as `C:\` stays `C:\`. Do not turn it into `C:`, which means something else on Windows.
   - Put this in a small helper, e.g. `fn project_id(cwd: &str) -> ProjectId`, so that it can be unit-tested directly.
3. **Unit test.** Add a unit test for the helper in `discovery.rs`. It must give the same results on every OS:
   - `C:\Users\u\p\` → `C:\Users\u\p`
   - `/a/b/` → `/a/b`
   - `/a/b` → `/a/b`
   - `/` → `/`
   - `C:\` → `C:\`
4. **Optional.** In the doc on `refresh()`, add one sentence saying that the first `projects()`/`sessions()` call also builds the index under the same lock, so the "not on the UI thread" caveat applies to it too.

Do not:
- revert to the raw, untrimmed cwd string. `cwds_that_differ_only_by_a_trailing_separator_are_one_project` must keep passing.
- build the expected ids in the tests through `PathBuf`, or change the existing tests to make them pass. That would make `ProjectId` depend on the platform.

Done when:
- the new helper test passes;
- every existing test passes unchanged, including `cwds_that_differ_only_by_a_trailing_separator_are_one_project` and `sessions_carry_their_summary_fields`;
- no code path builds a `ProjectId` from a `PathBuf`'s string;
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.
