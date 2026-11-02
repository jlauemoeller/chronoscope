VERDICT: FAIL

## Summary
The save/export flow works and is well tested. The transcript builds the text and document, the App owns the path prompt, the overwrite prompt, writing and toasts, all snapshots are present, and the gate is green. One must-fix remains. The harness-data guard (§9.5) can be bypassed through a dangling symlink, which lets `y` on the overwrite prompt create a file inside harness data.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (no `.snap.new` files in the tree; working tree clean after the run)

## Findings
1. [must-fix] **Security** `src/app/save.rs`, `resolved()` / `is_harness_data()` / `path_chosen()`: a **dangling** symlink that points into harness data is not refused.
   - **How it happens:**
     - `resolved()` canonicalises the longest existing prefix. `canonicalize()` fails on a dangling link, because its target does not exist.
     - So the prefix walk stops at the link's parent and appends the link's own name unchanged. Example: `./link.txt -> ~/.claude/projects/x/new.jsonl` resolves to `<cwd>/link.txt`, and `is_harness_data` returns false.
     - `path.symlink_metadata()` succeeds on the link, so the overwrite question is asked.
     - On `y`, `std::fs::write` follows the link and creates `new.jsonl` inside the Claude data dir.
   - **Why it matters:** this breaks SPEC §9.5 ("Never writes to harness data") and the planner's "Never write inside the harness data directories: refuse…". It also contradicts `doc/save-and-export.md`, which says paths are compared "after resolving `..` and symlinks".
   - **Change:** in `resolved()` (or before calling it in `is_harness_data`), follow the link when the final component is a symlink:
     - check `path.symlink_metadata().map(|m| m.file_type().is_symlink())`;
     - read the target with `std::fs::read_link`;
     - join a relative target onto the link's parent;
     - repeat, with a small hop limit (e.g. 40), then run the existing prefix walk on the final target.
   - **Test:** add a `#[cfg(unix)]` test to `src/app/browser_tests/save.rs`. Create `dir/dangling.txt -> data/new.txt` with `data/new.txt` absent, submit `dangling.txt`, and assert:
     - the toast is `Refusing to write into harness data`;
     - no overwrite prompt is mounted;
     - `data/new.txt` does not exist.

     Add a unit case for `resolved()` in `save.rs` tests as well.
2. [nit] `src/components/turn_block/export.rs`, `section_markdown` (the `summary()` closure) and `unfolded_tool` (`format!("**{}**", ...)`): folded summary lines and bold descriptions are written without escaping.
   - The folded golden file contains `> Read the <dump> & friends`. CommonMark reads `<dump>` as an inline HTML tag, so GitHub and similar renderers drop it from the rendered summary.
   - The full export already runs `escape_html` on the `<summary>`.
   - **Why a nit and not must-fix:** the file holds the exact text, the planner asks for the summary "as plain text" matching what `y` copies, and prompts are verbatim markdown with the same property.
   - **Suggested change:** apply `escape_html` (or backslash-escape `<`, `&`) to folded summary lines and to the bold description/label in the folded export, then re-accept the golden file.
3. [nit] `src/app/save.rs` `resolved()` / `path_chosen()`: the harness-data check and the write are separate steps, so a link created between them is not caught. This is acceptable for an interactive tool; a doc comment noting the limitation would help.
4. [nit] `src/components/transcript/text.rs` `export_document` / `src/components/turn_block/export.rs` `details()`: the whole document, including every lazy output read back from disk for `E`, is built synchronously on the UI thread when the key is pressed. It is then held in `PendingWrite` while the prompt is open, even if the user cancels. On a ~100 MB session this blocks a frame for a long time and doubles memory for the duration (§9.2).
   - Not required by this task.
   - Consider a follow-up: build the document after the path is confirmed, or off the UI thread.
5. [nit] `src/components/turn_block/export.rs` `header()`: when the cwd is unknown, `- Working directory: ` ends with a trailing space. Write `- Working directory: (unknown)`, or omit the line.
6. [nit] `src/app/mod.rs` `Msg::Export { full: _, document }`: the App ignores `full`. This is fine as the planner fixed the shape. It could be logged or used in the toast if ever useful; otherwise leave it.
7. [nit] `src/app/save.rs` `set_save_dir` / `set_home_dir`: these are `pub` setters that only tests use. This matches the existing `set_harnesses` precedent, so it is mentioned only for API hygiene. `#[doc(hidden)]` or a `pub(crate)` + `cfg(test)` pattern would keep them out of the public surface.
