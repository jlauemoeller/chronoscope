VERDICT: PASS

## Summary
The round-1 must-fix is fixed. An unfolded OpenCode Read now drops the `Read file …` / `Read directory …` framing line and moves OpenCode's `N: ` numbers into the gutter, so it renders like Claude's Read body. A new app-level test and three renderer tests pin this. The three required snapshots have not changed since round 1 and still match the `UI` section. I checked the fix end to end with the real `OpenCodeHarness` against read results shaped like the 2,298 real `read` parts in the local `opencode.db`, which I inspected read-only. `INSTA_UPDATE=no cargo test` passes with 0 failures in every binary. Only nits remain.

## States checked
- Selector, OpenCode selected, projects and sessions listed @ 120x40. Snapshots `selector_with_opencode_selected_120x40_text` / `_styled`, unchanged since round 1: ok.
- Selector, Claude selected, OpenCode unavailable (muted tab) @ 80x24. Snapshot `an_unavailable_opencode_is_muted_and_cannot_be_selected_80x24_text`, plus `an_unavailable_opencode_tab_is_muted_foreground_80x24_styled`, both unchanged: ok.
- Browser, OpenCode session with the shell tool unfolded and the `@ high` variant @ 120x40. Snapshot `an_opencode_session_in_the_browser_with_a_shell_tool_unfolded_120x40_text`, unchanged: ok.
- Browser, `ses_main` with the Read `call-read-1` unfolded @ 120x60. Covered by the new test `an_unfolded_opencode_read_shows_the_file_in_a_gutter_without_the_framing`, which checks that `Content:` shows `   1  fn main() {}`, with no `Read file` line and no `1:`: ok. This was the round-1 must-fix.
- Real OpenCode read formats, checked read-only against `~/.local/share/opencode/opencode.db`:
  - 2,218 file reads carry the `Read file <path>, lines a-b` header, and each is followed by `N: text` lines. I did not filter these on `state.status`.
  - 80 directory reads start with `Read directory <path>, entries a-b`.
  - 368 file reads end with `[Output truncated. Continue reading with offset: N]`.
  - The 14 results without a header are all `error` status.
- Browser, scratch session `ses_read` with four OpenCode reads unfolded @ 120x70 and @ 60x90 (text; the gutter and text colours are the shared Claude renderer's and were not re-checked here). Rendered in the scratch crate. Results:
  - Offset read 97–102 with the truncation trailer: ok. The path shows `(lines 97–102)`. The gutter is 3 digits wide and right-aligned. The leading indentation of the content is kept. The empty line `100:` gets a number and no text. Wrapped lines continue under the text column. The `[Output truncated…]` trailer gets an empty gutter, which is how the renderer already treats Claude's unnumbered trailers.
  - Directory read: ok. The header is dropped and the entries are numbered 1–3.
  - Failed read: ok. The error is shown without a gutter.
  - Header-only read: issue, see nit 1. See `ui/browser-opencode-reads-unfolded-120x70.txt`.
- Claude Read bodies after the shared `describe` / `shared` refactor: the existing Claude turn_block and app snapshots pass unchanged: ok.
- Test gate: `INSTA_UPDATE=no cargo test` — ok. No snapshot was created or updated.

## Findings
1. [nit] Browser, OpenCode Read whose result is only the framing line (e.g. `Read file /home/user/alpha/empty.rs, lines 1-0\n`), any size (seen at 120x70). After `numbered()` drops the header, no lines are left, so `Content:` is followed by a blank block. A Claude read with empty output shows `(no output)` in muted text. The real database has no such result: every successful file read has at least one numbered line. The empty-file format is therefore unverified, which is why this is a nit. Change: in `ReadBody::body` (`src/components/turn_block/renderers/tool/read.rs`), when `numbered(&text, offset)` returns no lines, return `muted_block("Content:", "(no output)", ctx)` as the empty-text branch already does. See `ui/browser-opencode-reads-unfolded-120x70.txt` (the `Read empty.rs` section).
2. [nit] Carried over from round 1 (nit 2), unchanged. Browser, Assistant title on narrow terminals: `acme/coder-1 @ high` becomes `acme/coder-1 @ hi…` at 60 columns. I saw this again in the scratch crate at 60x90. A long model id pushes `@ <variant>` out entirely. Possible change, which would be a cross-harness change to the turn-block title: truncate the model id first and keep `@ <variant>` whole. See `../round-1/ui/browser-narrow-title.txt`.
3. [nit] Carried over from round 1 (nit 3), unchanged. Browser, OpenCode session without rows: the empty state reads "Session file is empty". This is the fixed SPEC §1.3/§4.2 text, so it should change only with a spec change, for example to a harness-neutral "Session is empty". See `../round-1/ui/browser-empty-opencode-session-100x30.txt`.
4. [nit] Carried over from round 1 (nit 4), unchanged. An OpenCode `shell` call without a `description` input folds to just `▶ shell`. This follows the shared `<tool name>` rule. Showing the first line of `command` instead would need a cross-harness spec change.
