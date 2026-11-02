STATUS: DONE

## Changes
- `src/harness/claude/watch.rs`: a failed reload no longer ends the worker. `reload` logs the error, sends `Loaded` and sets `needs_reload`; the next change (event, or poll tick) forces `Reset` + full re-parse whatever the size or identity. `forward` treats a watcher error, `need_rescan()` or an event without paths as a change of the session file. The 1 s poll interval is now a `POLL_EVERY` const (the `poll_every` field of `Job` and `ClaudeHarness` is gone). Module docs state the truncate-and-regrow limitation.
- `src/harness/claude/load.rs`: dropped the unused `Default`/`#[default]` on `WatchMode`; `Path` imported; `catch_up` sizes its read buffer to the bytes to read (at most 256 KiB); module docs state that a cut-off last line is never counted as unparsable.
- `src/app/browser.rs`: the `Activity` comment says a later task stores it and draws `● live`.
- `tests/claude_live.rs`: new tests for warning counts and for fixture sessions appended in pieces; removed the `Path` import hack.

## Self-check
- Review (code 1, feature 3): `ParseWarnings` on change and only then — `the_warning_count_is_sent_when_it_changes_and_only_then`; fixtures also check the final count.
- Review (feature 1, code 2): failed reload keeps watching — `needs_reload` flag in `Watch` (checked first in `on_change`, also forces a poll tick); no test, as the review allows.
- Review (feature 2, code 4): fixture live appends — `the_session_fixture_appended_in_pieces_equals_a_fresh_parse` (cut before the >4 KiB lazy `Read` result, pieces ending mid-line and in the malformed line, turns incl. byte ranges equal `Assembler::from_file`), `the_injected_fixture_appended_in_pieces_equals_a_fresh_parse` (cut between injected entries and their reply). A cut right before a prompt differs from a fresh parse by design (held entries go to the open turn at a batch end; covered in `claude_injected.rs`), so the test avoids it and says why.
- Review (code 3): documented in `load.rs` module docs. Review (feature 4): documented in `watch.rs` module docs.
- Review (code 5, 6, 7, 8, 9, 10; ui 1): done as described above.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (1362 passed, 0 failed; `claude_live` passed 10 runs in a row)
