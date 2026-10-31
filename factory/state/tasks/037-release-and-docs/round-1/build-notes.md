STATUS: DONE

## Changes
- `.github/workflows/release.yml` (new): hand-written release workflow (`dist` is not installed here, so no cargo-dist workflow could be generated). Triggers on tags `v*`. A `build` matrix job covers `x86_64-unknown-linux-gnu` (ubuntu-24.04), `aarch64-unknown-linux-gnu` (ubuntu-24.04-arm), `aarch64-apple-darwin` and `x86_64-apple-darwin` (both on macos-latest with the target added) and `x86_64-pc-windows-msvc` (windows-latest). Each job checks that the tag equals `v<Cargo.toml version>`, runs `cargo build --release --locked --target <t>`, packages the binary with `README.md` and `THIRD-PARTY-NOTICES.md` (plus `LICENSE*` if such a file ever exists) as `.tar.gz`, or `.zip` on Windows, and uploads it as a workflow artifact. A `release` job (`contents: write`) downloads all artifacts, creates the GitHub release for the tag if missing (`--generate-notes`, prerelease for tags containing `-`), and uploads the archives with `gh release upload --clobber`. No tag was pushed.
- `README.md` (new): what it is, install (releases, `cargo install --path .`), usage and CLI (§6.8), full default keybinding tables per context (action name, keys in config syntax, description; `?` shows all), configuration (per-platform location, the §8.5 example verbatim, key table, warning behaviour, restart), themes (built-ins, user themes with an example in the task-002 TOML format, `NO_COLOR`/`COLORTERM`), mouse and Shift selection, terminal multiplexers (the three tmux settings verbatim with one-line explanations, OSC52 clipboard behaviour, herdr unverified), data sources (read-only), open items (§11, with the Claude compaction item updated), acknowledgements (ratcn, MIT, `THIRD-PARTY-NOTICES.md`).
- `tests/release_docs.rs` (new): 16 tests checking README and workflow (see Self-check).
- CI (`ci.yml`) is unchanged.

## Self-check
- AC: release.yml exists, triggers on `v*`, builds/uploads all five targets — `release_workflow_builds_and_uploads_all_five_targets_on_version_tags`. YAML validation: `python3 -c 'import yaml; yaml.safe_load(open(".github/workflows/release.yml"))'` (PyYAML 6.0.3) parses it; the parsed trigger is `{'push': {'tags': ['v*']}}` and the matrix has the five targets. `actionlint` is not installed, so the workflow was not linted beyond the YAML parse; it has not been run on GitHub.
- AC: README has every section, three tmux settings verbatim, herdr unverified — `readme_has_every_planned_section`, `readme_documents_the_tmux_settings_verbatim`, `readme_says_herdr_osc52_is_unverified`, plus `readme_command_line_matches_the_spec`, `readme_lists_the_builtin_themes_and_the_colour_rules`, `readme_user_theme_example_parses`, `readme_covers_mouse_and_native_selection`.
- AC: config example matches §8.5 and loads with zero warnings — `readme_config_example_loads_without_warnings` (Config and Keymap::from_config both without warnings), `readme_config_example_is_the_one_in_the_spec` (compares the README block with the SPEC.md block), `readme_config_example_sets_every_config_key`.
- AC: keybinding overview matches the default keymap, no Ctrl-c copy, no modifier+Enter — `readme_keybindings_match_the_default_keymap` (every default binding is documented with exactly its keys, and nothing extra), `readme_has_no_ctrl_c_copy_and_no_modifier_enter`. I checked by hand that these tests fail when the README is wrong (an extra `ctrl-c` on `copy` and an invalid config value both failed several tests).
- AC: open items listed — `readme_lists_the_open_items`.
- AC: ratcn acknowledgement and THIRD-PARTY-NOTICES.md reference — `readme_credits_ratcn_and_points_to_the_notices`.
- UI: none.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (2426 passed, 0 failed)

## Notes
- The repository has no licence file, so none was invented. Archives contain README.md and THIRD-PARTY-NOTICES.md only; the README says the project's own licence is still to be chosen. A decision on it is needed before the first public release (the workflow picks up a `LICENSE*` file automatically once it exists).
- Open item wording: Claude compaction samples exist locally (the fixture `tests/fixtures/claude/compaction.jsonl` is modelled on Claude Code 2.1.304-2.1.311 sessions), OpenCode compaction stays unverified.
- The macOS x86_64 build cross-compiles on the Apple silicon `macos-latest` runner (rusqlite is bundled, so a C compiler for the target is all it needs).
