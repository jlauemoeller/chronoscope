STATUS: DONE

## Changes
- `src/components/turn_block/renderers/tool/read.rs`: the Read body now understands OpenCode's read result. `parse_numbered` accepts the `N: text` prefix (and `N:` for an empty line), and a leading `Read file <path>, lines a-b` / `Read directory <path>, entries a-b` framing line is left out of the content. The numbers go into the gutter, as for Claude.
- `src/harness/shared.rs` (new): `UNTITLED`, `MAX_TITLE_CHARS`, `truncate_title` and `SubagentInfo`, moved out of the Claude adapter; `src/harness/describe.rs` (new): the tool-description rules, moved out of `claude/tools.rs`. Both adapters import from there; the Claude paths (`claude::UNTITLED`, `claude::truncate_title`, `claude::SubagentInfo`, `claude::tools::describe`) still work through re-exports.
- `src/harness/opencode/db.rs`: the `immutable=1` retry now runs only for `SQLITE_CANTOPEN` / `SQLITE_READONLY` failures; any other open error (busy, I/O, corrupt, not a database) is returned as it is. A relative path is made absolute before it goes into the URI.
- `src/harness/opencode/load.rs`, `subagent.rs`: one `text_at` helper reads `data` (text, blob, number, NULL) for the loader and the subagent lookups; the nested `match` is gone.
- `discovery.rs`: `with_page_size` is `#[cfg(test)] pub(crate)`; doc note that projects without sessions are not listed, as with Claude. `subagent.rs` doc and the "Two rows" test comment fixed.
- Tests added: OpenCode Read renderer tests (three), `an_unfolded_opencode_read_shows_the_file_in_a_gutter_without_the_framing` (app level), a WAL-mode full-load test (writer connection open, change living only in the `-wal`, main file bytes and side files unchanged), `needs_immutable` and relative-path URI tests; the root-skip of the immutable test now prints a message.

## Self-check
- Review (ui-review 1, must-fix: Read numbering) — fixed in `read.rs`; pinned by `opencode_numbering_and_header_become_a_plain_gutter`, `opencode_numbering_keeps_the_numbers_of_a_later_range`, `an_opencode_directory_listing_loses_its_header_line` and the app test `an_unfolded_opencode_read_shows_the_file_in_a_gutter_without_the_framing`.
- Review (code-review 2, 3, 4, 5, 6, 7, 8, 9, 10, 11; feature-review 1, 3, 4) — addressed as listed under Changes.
- Review (code-review 1, inline tool output) — left as a known limitation (follow-up task: SQLite output source); reviewers marked it a nit.
- Review (feature-review 2: `watch_paths` with a missing `-wal`) — left as the task specifies (db and `-wal`); for task 029 or a follow-up.
- Review (ui-review 2: narrow title truncation, 3: "Session file is empty", 4: shell without description) — left: shared title/empty-state/description rules that need a cross-harness or spec change.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (1896 passed, 0 failed)
