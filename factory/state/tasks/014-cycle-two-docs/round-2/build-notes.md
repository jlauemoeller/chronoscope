STATUS: DONE

## Changes
Documentation only again; no file under `src/` or `tests/` changed (I swapped two reference calls in `tests/budgets.rs` temporarily to measure, then reverted it).

- `doc/SPEC.md`: §9.5 now says what is built (every field optional; a wrong type fails the line or row while assembling, except the lenient fields; the details re-read is tolerant of every field it reads); §3.1 scoped to the overlay's re-read; §3.1 and §9.2 limit the "never a JSON value" claim to the assembler and the details overlay, with the subagent fallback (§5.4) named as the exception; §1.4 lists `DetailsOverlay` and its `Esc` / `i`; §9.2 "2 MB result (a line of about 4 MB)"; §5.1 message reference is the latest line with a non-null `stop_reason`, else the latest; §1.1 "would inherit" and "an exception, like materialising a large output".
- `docs/performance.md`: the unfold explanation is corrected (a plain unfold is up to about 10 ms, under a frame; the 116 to 173 ms line is re-laying out a block that holds an expanded output of about 20 000 lines, and it is printed after the expansion). Verified: with `worst_unfold` run first the slowest unfold was 10.3 ms (1.9 MB); after the expansion 118.6 ms. "same margin" -> "wide margin"; the endless turn added to what fails the budget test; the clause "like unfolding a large output" restored, with only the details read called far under a frame; long lines rewrapped.
- `README.md`: rewrapped the contexts paragraph.

## Self-check
- Review: code 1 / feature 1 (robustness claim) — §9.5 and §3.1 rewritten to match `wire.rs` and the lenient fields.
- Review: code 2 / feature 4 (unfold cause) — corrected, measured as above; test left as is.
- Review: code 3 / feature 2 (toolUseResult never a Value) — scoped, exception stated.
- Review: code 4 / feature 3 (DetailsOverlay in SubClause) — §1.4.
- Review: code 5 (4 MB line), 6 (margin), 7 (endless turn), 8 (wrapping), 9 (message reference) — done.
- Review: feature 6, 8 (Codex/Pi wording, UI-thread exception) — done.
- Review: feature 5 (follow-up) — not in scope. The orchestrator may want a task for the lazy-output unfold that re-lays out an expanded block (116 ms), see `docs/performance.md`.
- UI: none

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (3181 passed, 0 failed)
