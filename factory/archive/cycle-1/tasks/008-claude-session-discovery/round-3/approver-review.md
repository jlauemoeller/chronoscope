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
