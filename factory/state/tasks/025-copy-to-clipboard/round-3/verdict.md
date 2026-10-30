VERDICT: APPROVE

## Assessment
Round 3 does exactly what the round-2 rework note (`approver-review.md`) asked:
- `soft_joined` joins a wrapped row with the whitespace the source has at the break, and adds nothing where the screen broke inside a word. The `locate` helper returns that gap.
- The rows of a quote are joined under one `│ ` prefix. `quote_prefix` handles nested quotes too.
- The redundant filter is gone, and the doc comments are updated, including `verbatim_lines` (CRLF becomes LF).

All three reviews PASS and raise only nits. I re-ran the gate on `factory/025-copy-to-clipboard` at `7cd3c00`:
- `cargo fmt --check`: pass.
- `cargo clippy --all-targets -- -D warnings`: pass.
- `INSTA_UPDATE=no cargo test`: pass. That is 1421 lib tests (1 ignored), and every other target passes.
- The working tree is clean and there are no `.snap.new` files.

I also read `soft_joined`, `locate` and `quote_prefix` in `src/components/turn_block/plain.rs`, and they match what the reviewers describe.

**Code review: PASS**
1. [nit] The `displayed_text` doc comment is not reflowed after the edit, so one line stops early. **Ignored.** It is cosmetic, and code, feature and UI all raised the same point.
2. [nit] `soft_joined` builds a 5-tuple only to unpack it right away. **Ignored.** This is a readability preference with no change in behaviour.
3. [nit] `quote_prefix` reads a quote whose own text starts with a literal `│ ` as a nested quote. **Ignored.** It is rare, and the result falls back to the screen rows (separate lines, as drawn). It never adds or corrupts text.

**Feature review: PASS**
Every acceptance criterion is met and has a test.
1. [nit] The doc-comment reflow. **Ignored**, as code 1.
2. [nit] The mid-word, spaced and quote cases are in two new test functions, `soft_joined_joins_with_the_sources_own_whitespace` and `soft_joined_joins_the_rows_of_a_quote_under_one_bar`, rather than inside `soft_joined_matches_rows_to_the_unwrapped_lines`. **Ignored.** The coverage is the same, and the unmatched-quote assertion is kept, as the rework note required.

**UI review: PASS**
The four toast snapshots are unchanged and match the `UI` section. The reviewer also rendered many folded-prompt edge cases at 60x20, 80x24 and 120x40, and every copy matched the screen.
1. [nit] The doc-comment reflow. **Ignored**, as code 1.

**Items carried over from the round-2 verdict, still ignored on purpose:**
- the Read gutter under `Y`;
- the whole description of a folded tool call;
- the 80-cell rule;
- the synchronous lazy read.

When task 026 reuses `full_text` for save and export, keep the synchronous lazy read in mind for the §9.2 budgets.

**Not an acceptance criterion, and no blocker:** `doc/clipboard.md` documents the tmux settings (`set-clipboard on`, `allow-passthrough on`). It also says that herdr's OSC52 behaviour (SPEC §9.4) has not been verified yet. That check needs a human on a real herdr setup and should be tracked as a follow-up.

## Conflicts
None. All three reviewers raise the same reflow nit, and they agree on it.

## Rework budget
Earlier REWORK verdicts: 1 of 2 (round 2).
