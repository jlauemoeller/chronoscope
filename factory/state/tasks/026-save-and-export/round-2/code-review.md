VERDICT: PASS

## Summary
The round-1 must-fix is fixed. `resolved()` / `resolved_in()` in `src/app/save.rs` now follows a symlink found in the part of the path that does not exist yet. That covers absolute and relative targets, chains, and links to directories that do not exist, with the rest of the path carried into the recursive call. A dangling link into harness data is now refused before any overwrite question. The new browser test would have failed on the round-1 code. The round-1 nits were handled (escaping of folded summaries, `(unknown)` cwd, `#[doc(hidden)]` setters, start-cut toast path, the race documented) or left with a stated reason. The gate is green, and the only remaining findings are nits.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (0 failures; no `.snap.new` files; working tree clean after the run)

## Findings
1. [nit] `src/app/save.rs`, `WriteKind::done`: the path is cut to fit the fixed room `MAX_TOAST_WIDTH - TOAST_CHROME - prefix` (56 columns of text). `toast_rect` in `src/app/layout.rs` caps the box at `min(60, width - 4)`, so the box is narrower at terminal widths 60–63, and 60 is `MIN_WIDTH`, a supported size. There the toast's own `truncate_with_ellipsis` cuts the end of a long path, and the file name that this round's change meant to keep is lost again (e.g. `….../brio-85a86657-shell.t…`). Change: cut against the toast room for the current terminal width, which means passing the width in or computing it where the toast is built. Alternatively, give `Toast` an option to cut its text from the start, so the single cut happens at render time with the real box width.
2. [nit] **Security** (defensive hardening) `src/app/save.rs`, `resolved_in`: when `hops == 0` it returns the unfollowed link path, so the harness-data check fails open on a symlink chain longer than `MAX_LINK_HOPS`. On Linux and macOS the kernel's own limit (40 / 32) makes the later write fail with `ELOOP`, so nothing reaches harness data there today. Change: when the limit runs out, make `is_harness_data` treat the path as refused (e.g. have `resolved_in` return `Option<PathBuf>`, `None` on exhaustion), and assert that in `a_loop_of_symlinks_ends`. That test currently only checks that the walk ends.
3. [nit] Accepted as left in round 2, for the record (no change required): `Msg::Export { full: _, .. }` is unused by the App, because the planner fixed the shape; the `Prompt` draws no `…` marker when its input is scrolled, which is existing shared behaviour; `E` builds the full document on the UI thread before the path is confirmed, which is now documented in `doc/save-and-export.md` as a known cost.
