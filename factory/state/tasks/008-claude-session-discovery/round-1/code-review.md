VERDICT: FAIL

## Summary
The stage-1 `Harness` trait, `HarnessId` (1–4, display names), `Project`/`ProjectId`/`SessionSummary`/`SessionId` and `ClaudeHarness` are implemented correctly. The work includes a (path, size, mtime) scan cache, parallel `std::thread::scope` scanning and one shared "human prompt" rule (`Entry::human_prompt_text`) used by both the assembler and the scanner. The cache and parallelism tests are strong. However, two required behaviours have no test that would catch them going wrong: sessions sorted by `started_at` (not by last activity), and titles built from the prompt's *first line*. The test gate passes and no `.snap.new` files are present.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass

## Findings
1. [must-fix] `tests/claude_discovery.rs` `sessions_are_sorted_newest_first_by_start_time`: this test does not pin the sort key that the planner note requires ("Sessions are sorted by `started_at`, newest first"). In every multi-session fixture project, start time and last activity give the same order:
   - `dddd0001` starts at 07:00 and is last active at 07:01;
   - `dddd0002` starts at 06:00 and is last active at 06:00:30;
   - `bbbb0002` is newer than `bbbb0001` by both keys.

   `fixture()` also sets each mtime to the last timestamp. So if `build_index` sorted by `last_activity` or by mtime, no test would fail. Change: in that test, before building, call `set_mtime(&data.join("-home-user-prompt").join("dddd0002.jsonl"), "2026-03-10T00:00:00Z")`. Then assert:
   - the order is still `["dddd0001", "dddd0002"]`;
   - `session(&sessions, "dddd0002").last_activity > session(&sessions, "dddd0001").last_activity`, which proves the sort ignores last activity.
2. [must-fix] `tests/claude_discovery.rs` `without_a_title_the_first_prompt_gives_a_truncated_first_line`: this test does not cover the "first line" part of the title rule (planner note: "the first human prompt's first line truncated to 80 characters with `…`"). The only fallback-title fixture is `dddd0001`, whose *first line* is already longer than 80 characters. So truncating the whole prompt gives the same title, and a version that ignored line breaks would pass. A version that put a newline into a short title would pass too, because `bbbb0001` is a single line. Change: add a test that writes a temp-dir transcript, as `aliases_from_the_config_are_matched_on_the_expanded_path` does. Give it one human prompt with content `"Short first line\nsecond line"` and no `ai-title`. Assert that the session's title is exactly `"Short first line"` and that `first_prompt` still holds both lines. Optionally, a prompt that starts with a blank line (`"\n  \nReal start\nmore"` → title `"Real start"`) would also pin the builder's "first non-blank line" decision.
3. [nit] `Cargo.toml:38`: `anyhow = "1.0.104"` pins a patch-level minimum, but every other dependency gives only the major/minor version (`"0.4"`, `"1"`). Change: `anyhow = "1"` (unless 1.0.104 is really needed).
4. [nit] `tests/claude_discovery.rs` (`fixture()` relative paths such as `"-home-user-alpha/aaaa0001.jsonl"` and `"tests/fixtures/claude/discovery"`, and `data.join("-home-user-.../...")` in later tests): these joins contain `/` in their string literals. This does not breach SPEC §9.1:
   - the literals go into `Path::join` and the results are `PathBuf`s;
   - `/` is a valid separator on every supported platform;
   - no test compares such a path with one returned by `read_dir`;
   - existing tests do the same.

   It is a matter of consistency only. Change, if wanted: use chained `.join(..)` calls, as `sessions_carry_their_summary_fields` already does.
5. [nit] `src/harness/claude/discovery.rs` `refresh()`/`index()` → `build()`: the `State` mutex stays held for the whole directory walk and parallel scan, so a `projects()`/`sessions()` call from another thread blocks for the length of a rebuild. This is fine for this task. When the index worker arrives (task 010), consider copying the cache entries out under the lock, scanning without it, then locking again to merge and publish the new `Arc<Index>`.
6. [nit] `src/harness/claude/scan.rs` `ScanHead::attachment: Option<AttachmentHead>`: a non-object `attachment` makes the whole line fail, so that line's `timestamp`/`cwd` is lost (the doc comment says so). Change, if you want it more robust: read `attachment` as `Option<Value>` and check `attachment.get("type")`, the same way `timestamp`/`cwd`/`aiTitle` are handled.
7. [nit] `src/harness/claude/discovery.rs` `summarize()`: `ProjectId` is the raw `cwd` string, while alias matching (`BTreeMap<PathBuf, _>::get`) compares path components. So `/a/b` and `/a/b/` would become two projects that both match the same alias. Claude writes canonical cwds, so this is only an edge case. Change, if wanted: build the id from the normalised `PathBuf` (e.g. `PathBuf::from(cwd).components().collect::<PathBuf>()`).
