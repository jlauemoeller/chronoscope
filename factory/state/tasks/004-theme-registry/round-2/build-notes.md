STATUS: DONE

## Changes
- `src/theme/resolve.rs`: `resolve_by_id` now matches on `ThemeEntry::Valid` / `Invalid` / `None` directly (no invented error). New explicit `builtin::terminal_theme()` is used in the NO_COLOR paths; `adapt` only applies the transparency override and terminal transforms. Uses `builtin::DEFAULT_ID` instead of `config::DEFAULT_THEME`. Doc comment says the built-in Terminal theme is used deliberately under NO_COLOR.
- `src/theme/registry.rs`: only the exact `.toml` extension is read (no duplicate ids); user themes sort case-insensitively with the raw id as tie-break; read failures are `ThemeError::Io`.
- `src/theme/parse.rs`: new `ThemeError::Io(String)` variant.
- `src/theme/builtin.rs`: new `terminal_theme()`.
- Both source-scanning env tests normalise `\r\n` to `\n` first.

## Self-check
- Review: code-review 1 [must-fix] CRLF in env test — both scan tests call `.replace("\r\n", "\n")` on the `include_str!` source.
- Review: code-review 2 / feature 6 dead fallback error — direct match in `resolve_by_id`.
- Review: code-review 3 / feature 1 NO_COLOR placeholder — `builtin::terminal_theme()` used explicitly.
- Review: feature 2 undocumented built-in Terminal use — documented on `resolve_by_id`/`resolve`.
- Review: code-review 4 / feature 4 `.toml`/`.TOML` duplicates — exact extension; test `only_lowercase_toml_extension_is_read`.
- Review: code-review 5 filesystem-order test — `with_user_themes_sorts_its_input_by_id_ignoring_case` feeds reversed input.
- Review: code-review 6 duplicate constant — `builtin::DEFAULT_ID` used; test `default_theme_id_is_shared_with_the_config`.
- Review: code-review 7 / feature 3 I/O error as Syntax — `ThemeError::Io`; test `file_that_cannot_be_read_is_an_io_error_entry`.
- Review: code-review 8 / feature 7 collation — case-insensitive sort (same test).
- Review: feature 5, 8 — no change required.
- All round-1 acceptance-criterion tests remain.
- UI: none

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (all passed, 0 failed)
