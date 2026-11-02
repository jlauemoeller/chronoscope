STATUS: DONE

## Changes
- `src/harness/opencode/details_tests.rs`: four test additions from the approver note: a wrong JSON type leaves out that field alone (row/tool `time`, `snapshot.*`, `model`, tool part `id`); an `error` missing or of the wrong type is no row though the notice says `error`; a deleted row makes only the groups read from it unavailable (turn row vs. section row); the WAL test now asserts the groups were read (none `unavailable`).
- `src/harness/opencode/details.rs`: reworded one comment in `message_group`. No production behaviour change, no `.snap` change.

## Self-check
- Review: approver items 1-4 — tests above, added as specified.
- Review: round-3 findings — superseded by the approver note; no production change was requested.
- Mutation check run: `read_groups(None, source)` fails `reading_details_in_wal_mode_leaves_the_database_file_untouched` (among 36 failures).

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (2905 passed, 0 failed; INSTA_UPDATE=no)
- cargo test --release -- --ignored budgets --nocapture: pass
