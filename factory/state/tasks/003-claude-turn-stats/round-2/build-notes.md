STATUS: DONE

## Changes
- `tests/claude_assembly.rs`: added `a_late_message_that_adds_no_item_still_updates_the_count_and_sends_updated`, `a_compaction_summary_without_a_boundary_ends_the_association_with_the_prompt` and `messages_without_an_id_or_uuid_are_counted_by_line_offset`.
- `src/harness/claude/assemble.rs`: `on_assistant` parses the entry's timestamp only when it starts a new turn (nit 4).

## Self-check
- Review (feature 1, must-fix): no test protected the dirty marking for a count change — new test pushes a late message with `content: []` and expects one `Updated` with count 2. Mutation check: removing `changed = true` in the stats refresh makes it fail.
- Review (feature 2, must-fix): compaction without a boundary — new test; removing `self.prompt_ts = None` in `on_compact_summary` makes it fail.
- Review (code 1, nit): the line-offset fallback is now exercised, with no id and no uuid, fed through `push_line` with distinct ranges.
- Review (code 2, nit): same as feature 2.
- Review (code 3, nit): fixture left as is; the constructed tests discriminate.
- Review (code 4, nit): done.
- Review (code 5, nit): no change required.
- Review (ui): no findings.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (0 failed)
- budgets (`cargo test --release --test budgets -- --ignored budgets`): pass
