STATUS: DONE

## Changes
- `src/components/browser_header.rs`: the id budget moved into `BrowserHeader::fitted_id`. The whole-names attempt (names whole, id cut from its start) now runs only when a breadcrumb is open. At depth 0 the project name gives way first, down to 6 columns, and the whole id stays, as on `main`: `chronoscope2` at 60 columns renders `╭─ Project chrono… ─ <whole id> ─╮` again. Correction to the round-2 notes: the root header was changed in round 2 and is restored now; below 60 columns an id that cannot fit is still cut from the start instead of dropped, which is kept. The subagent fitting is untouched.
- `src/app/browser.rs`: one wording for the mount failure toast, `Could not open the subagent` (the load failure keeps `: <error>` appended).
- Tests: `at_depth_0_the_project_gives_way_before_the_session_id_does` (`chronoscope2` at 60, a 30-character and a 33-character name at 80: the whole `SESSION` is in row 0, the name is cut with `…`, the row is `width` cells and ends in `─╮`, and the exact row for `chronoscope2` at 60); `the_fitting_constants_match_the_strings_they_stand_for` (`SEPARATOR_WIDTH`, `TITLE_CHROME`, `COLLAPSED`).

## Self-check
- Rework note: root header restored — `at_depth_0_the_project_gives_way_before_the_session_id_does`; `a_long_project_name_is_cut_with_an_ellipsis_and_keeps_the_session_id` and all breadcrumb and `fit.rs` tests pass; no snapshot changed (no `.snap` modified, no `.snap.new`).
- Optional: constants tied to their strings by a test; one toast wording. Not done: a `forget_pending` test (needs an in-flight worker answer).
- Not changed, as instructed: the subagent id-versus-names balance, the key-label casing, the `BrowserState` `Deref`.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- INSTA_UPDATE=no cargo test: pass (0 failed)
