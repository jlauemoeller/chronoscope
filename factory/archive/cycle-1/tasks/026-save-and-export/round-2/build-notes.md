STATUS: DONE

## Changes
- `src/app/save.rs`: `resolved()` now follows a symlink that points to something that does not exist yet (absolute or relative target, chains, a link to a missing directory, hop limit 40), so a dangling link into harness data is refused before any overwrite question. Doc comments note that the check is a snapshot taken when the path is given. `set_save_dir` / `set_home_dir` are `#[doc(hidden)]`. A success toast cuts a path that is too long at its start (`tail_fitting`, now exported from `components`), so the file name stays.
- `src/components/turn_block/export.rs`: folded summaries and the bold description in the folded export escape `&` and `<`; an unknown working directory is written `(unknown)`. Folded golden re-accepted (`> Read the &lt;dump> &amp; friends`).
- `doc/save-and-export.md`: dangling links, the snapshot limit, the toast cut, and the known UI-thread cost of `E`.

## Self-check
- Review: [must-fix] dangling symlink bypass — `browser_tests::save::a_dangling_symlink_into_harness_data_is_refused_without_asking` (absolute and relative links; toast `Refusing to write into harness data`, no prompt, no file), `app::save::tests::a_dangling_symlink_is_followed_to_where_it_points`, `a_loop_of_symlinks_ends`.
- Review: [nit] folded summaries not escaped — `export::tests::folded_summaries_escape_what_a_renderer_would_take_for_markup` + golden.
- Review: [nit] check/write race — documented (code comment, doc).
- Review: [nit] `E` builds on the UI thread — noted as a known cost in `doc/save-and-export.md`; not changed (out of scope).
- Review: [nit] trailing space in the header — `export::tests::an_unknown_working_directory_is_written_as_unknown`.
- Review: [nit] `full` unused by the App — left as is (shape fixed by the planner).
- Review: [nit] pub test setters — `#[doc(hidden)]`.
- Review: [UI nit] long path in success toast — `app::save::tests::a_success_toast_keeps_the_end_of_a_long_path`.
- Review: [UI nit] path prompt without a `…` marker when scrolled — not changed (existing shared `Prompt` behaviour; optional).

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (1740 passed, 0 failed)
