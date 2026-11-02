STATUS: DONE

## Changes
- `src/timestamp.rs`: `is_valid_format` now also renders a sample instant and requires success, so formats that parse but cannot render (e.g. `%#z`) are rejected. `short_format` also turns `%X` into `%H:%M` and `%r` into `%I:%M %p` (`%c`/`%+` limitation documented). Documented the doubled zone with `utc = true` and a `%z`/`%Z` format.
- `src/config/mod.rs`: `Reader::take` uses an `Expected` enum (with `Display`) instead of matching on text. `Config::load` docs say to call `init_logging` first.
- Tests: `tests/config_log.rs` (new binary; warnings reach the log file), `tests/logging_failure.rs` (failure test moved to its own binary so the error must come from the filesystem), new unit tests for `%#z` in `timestamp` and `config`.

## Self-check
- Review code-review 1 [must-fix] log warnings test — `tests/config_log.rs::config_warnings_are_written_to_the_log_file`
- Review feature-review 1 [must-fix] unrenderable format — `timestamp::tests::format_that_parses_but_cannot_render_is_invalid`, `config::tests::timestamp_format_that_cannot_render_falls_back_to_default`
- Review nit Expected enum — done; nit load docs — done; nit %r/%X and %z+utc — done/documented; nit logging_append failure test — moved to `tests/logging_failure.rs`
- Not changed (nits, optional): literal paths in `Paths` tests (function is pure); splitting `from_table` and lazy default strings (cosmetic).
- All round-1 acceptance-criterion coverage unchanged.
- UI: none

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (0 failed)
