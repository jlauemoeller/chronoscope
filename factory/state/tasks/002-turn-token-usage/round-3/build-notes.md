STATUS: DONE

## Changes
- `src/harness/claude/wire.rs`: `Entry` is now `Entry<M = Message>`; new `MessageWithoutUsage`; new `read_entry` (normal parse, then a retry only if it failed) and `entry_with_invalid_utf8_in_usage` (retry only when the line has invalid UTF-8 and parses with `usage` skipped, then re-read with U+FFFD). Module doc updated for the lenient `usage`.
- `read_entry` replaces `from_slice::<Entry>` at all four sites: `Assembler::parse_entry` (assemble.rs), `extract_result_text` (wire.rs), `prompt_of_user` and `prompt_of_queued` (scan.rs). No other `from_slice::<Entry>` remains in `src/harness/claude`.
- `src/harness/shared.rs`: doc on `lenient` states the two limits (invalid UTF-8 fails the parse and is handled by `read_entry`; a duplicate key makes the value count as missing).
- `src/components/turn_block/renderers/stats.rs`: comment reworded (the rule holds when a part and its separator fit on a row).
- `lenient` / `lenient_object` and all footer code unchanged. No snapshot changes.

## Self-check
- Review (must-fix, all reviews): invalid UTF-8 inside `usage` — `invalid_utf8_inside_usage_keeps_the_line_parsed` (tests/claude_assembly.rs; the five listed `usage` values, each: unparsed 0, text kept, output 7), `a_result_is_read_from_a_line_with_invalid_utf8_in_usage` (wire.rs, `Some("hello")`), `a_prompt_is_found_on_a_line_with_invalid_utf8_in_usage` (scan.rs, `Some("real")`). Checked that the wire and scan tests fail without the retry.
- Main outcome kept elsewhere — `invalid_utf8_outside_usage_keeps_its_outcome` (bad text: 1 unparsed, no text; bad `cwd`: same; unknown top-level field: parsed, output 7; bad text plus bad usage: 1 unparsed), `a_result_is_not_read_from_a_line_with_invalid_utf8_in_the_result` (`None`).
- Existing `1e400` and wrong-type tests pass unchanged.
- Doc nits (wire.rs module doc, `lenient` doc, stats comment) done. Left as is per the note: `{}` showing `0 tokens`, duplicate-key behaviour, widths below 10.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- INSTA_UPDATE=no cargo test: pass (0 failed)
- INSTA_UPDATE=no cargo test --release --test budgets -- --ignored budgets --nocapture: pass
