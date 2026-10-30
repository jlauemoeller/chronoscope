STATUS: DONE

## Changes
- `src/components/turn_block/export.rs` (new, pure): `export_markdown(meta, turns, settings, full)` builds the markdown document (header, `## <Role> — <ts>` headings, folded export with block-quote summaries and fenced bodies for unfolded tool calls, full export with `<details>` blocks, long-enough fences, HTML-escaped summaries, lazy outputs read through `ToolOutput::text()`). Golden tests in `export/tests.rs` + snapshots.
- `src/components/turn_block/renderers/tool/mod.rs`: `ToolCallSection::plain_parts` (description, labelled blocks, hint, more row) with `plain_lines` rebuilt on it; `description_text` is public. `section.rs`: `Section::file_kind`; `plain.rs`: `file_kind`.
- `src/components/transcript`: `copy.rs` renamed `text.rs` (adds `file_kind` and `export_document`); `Transcript::export_meta` builder; `on()` answers `Msg::SaveSection { text, kind }` (`w`/`Ctrl-s` displayed, `W` full) and `Msg::Export { full, document }` (`e`/`Ctrl-e` folded, `E` full) through the keymap actions `Save`, `SaveFull`, `Export`, `ExportFull`.
- `src/app/save.rs` (new): the App's flow. Path prompt (`Save to: ` / `Export to: `, default pre-filled, cursor at end) -> on submit: refuse paths inside any harness's `watch_paths()` (resolved through `..`/symlinks, before any overwrite question) -> `Overwrite <path>? (y/n)` when the file exists (a directory is not asked about; the write fails) -> `std::fs::write`, no directories created. Toasts `Saved to` / `Exported to` / `Save failed:` / `Export failed:` / `Refusing to write into harness data`. Relative paths resolve against the start directory (`save_dir`, set from `working_dir`; test setter `set_save_dir`), `~` via `expand_tilde` with `home_dir` (test setter `set_home_dir`). `sanitize_for_file_name` is the `[A-Za-z0-9._-]` rule.
- `src/app/mod.rs`, `browser.rs`: new `Msg` variants; update arms for the file prompt come before the search arms (PromptChanged/Submit/Cancel/Nav); `BrowserState::write`; `open_project_name` factored out of `mount_browser`; the transcript gets its `ExportMeta` at mount.
- `src/components/prompt.rs`: `Prompt::confirm(before, subject, after, theme)`: the question in `help_text`, the subject (path) in `foreground`, a too-long subject cut at its start with `…`.
- `src/timestamp.rs`: `TimestampFormatter::format_with` (display timezone/utc, no `Z`) for `%Y%m%d-%H%M`.
- `doc/save-and-export.md`: behaviour notes.

Decisions (not in the planner notes):
- `ModelChange` sections get the file kind `assistant`; `ToolKind::Other` gets `tool`.
- Fixture project name: the default names in the AC (`brio-...`) come from a project whose display name is `brio` (the cwd basename when there is no alias). The tests use an index where the project is named `brio` (`index_named("brio")`); the sanitiser keeps case (`My Project/x` -> `My-Project-x`).
- Folded export: a folded section is its summary as `> ` quote lines; an unfolded tool call's description is bold and each label is bold above a fence (`sh` for a Shell command); a truncated output keeps `_… N more lines_` after its fence. In the full export, an error result is preceded by `_The tool reported an error._`, a pending call's output is `(pending)`.
- The toast shows the path as typed (e.g. `./brio-85a86657-shell.txt`), not the resolved absolute one.
- An empty path cancels silently.

## Self-check
- AC: keys `w`/`Ctrl-s`/`W`/`e`/`Ctrl-e`/`E` open the prompt with the right default name — `browser_tests::save::w_ctrl_s_and_capital_w_ask_for_a_path_with_the_default_save_name`, `e_ctrl_e_and_capital_e_ask_for_a_path_with_the_default_export_name`, `the_save_name_has_the_kind_of_the_selected_section`; transcript side `transcript::tests::save::*`; content `capital_w_saves_the_full_content_of_the_section`, `e_exports_the_session_as_currently_folded`, `capital_e_exports_the_full_session`.
- AC: non-existing path writes at once, existing asks, `y` overwrites, `n`/`Esc` leave the file — `a_path_that_does_not_exist_is_written_at_once`, `a_path_that_exists_asks_before_overwriting_and_writes_nothing_yet`, `y_overwrites`, `n_leaves_the_file_untouched`, `esc_leaves_the_file_untouched`, `any_other_key_than_y_is_a_no`, `an_existing_export_asks_too`.
- AC: folded export golden — `export::tests::the_folded_export_is_a_golden_file` (+ `the_folded_export_has_summaries_...`, `hidden_system_content_is_omitted_from_the_folded_export`, `a_truncated_output_of_the_folded_export_keeps_its_more_lines_row`).
- AC: full export golden, `<details>`, untruncated incl. lazy, system included — `export::tests::the_full_export_is_a_golden_file`, `the_full_export_has_a_details_block_per_tool_call_with_fenced_input_and_output`, `the_full_export_has_untruncated_outputs_including_lazy_ones`, `the_full_export_includes_system_content_whatever_the_toggle`, `browser_tests::save::the_full_export_has_the_system_content_the_toggle_hides`.
- AC: fence longer than a backtick run — `export::tests::a_fence_is_longer_than_the_longest_run_of_backticks_in_its_content`, `a_fence_of_the_export_holds_the_backticks_of_the_output`.
- AC: toasts and refusal — `a_missing_parent_directory_is_a_failure_and_none_is_created`, `an_export_that_cannot_be_written_says_export_failed`, `writing_into_the_data_of_a_harness_is_refused` (+ export, relative, `..`, symlink, existing file, sibling directory not refused).
- AC: sanitising — `app::save::tests::a_name_is_sanitised_character_by_character` and `the_default_name_sanitises_the_project_and_uses_the_first_eight_characters_of_the_id`.
- UI: save path prompt @ 80x24 (text + styled) — `browser_tests::save::save_prompt_80x24_text`, `save_prompt_80x24_styled`
- UI: export path prompt @ 120x40 (text) — `export_prompt_120x40_text`
- UI: overwrite confirmation @ 80x24 (text + styled) — `overwrite_confirmation_80x24_text`, `overwrite_confirmation_80x24_styled`
- UI: `Exported to …` toast @ 80x24 (text) — `exported_toast_80x24_text` (also `saved_toast_80x24_text`)

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (1732 passed, 0 failed)
