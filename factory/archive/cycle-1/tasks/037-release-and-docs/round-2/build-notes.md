STATUS: DONE

## Changes
- `README.md`: Rust requirement is now "1.88 or newer"; `cd <the clone>` instead of a hard-coded directory; the releases link is no longer a relative link (it works inside the packaged archive); the cache and log locations name the Windows paths (`%LOCALAPPDATA%\chronoscope\`); the compaction open item no longer names the developer's Claude Code versions.
- `Cargo.toml`: `rust-version` raised from 1.85 to 1.88 (let chains need 1.88; locked dependencies also need 1.88). Added the dev-dependency `serde_yaml_ng` for the workflow test; `Cargo.lock` updated accordingly (only `serde_yaml_ng` and `unsafe-libyaml` added).
- `src/harness/claude/assemble.rs`, `src/components/turn_block/renderers/tool/mod.rs`: with the new `rust-version` clippy enables two more lints (`collapsible_if` with let chains, `manual_is_multiple_of`); fixed them with no behaviour change.
- `.github/workflows/release.yml`: Linux builds moved to `ubuntu-22.04` / `ubuntu-22.04-arm` (glibc 2.35 instead of 2.39); `Swatinem/rust-cache` removed from the release workflow (cache-poisoning path into release artefacts); explanatory comment.
- `tests/release_docs.rs`: the workflow test now parses the YAML (`serde_yaml_ng`) and asserts `on.push.tags == ["v*"]` with no branch filter and only the push trigger, the exact five matrix targets, build and upload steps, and the `release` job (needs `build`, `contents: write`, `gh release upload`). New test `readme_rust_version_matches_the_manifest`.

## Self-check
- Review code-review 1 [must-fix] Rust 1.85 claim — README and `Cargo.toml` now say 1.88; `readme_rust_version_matches_the_manifest`.
- Review code-review 2 [nit] action pinning / cache — `rust-cache` dropped. Actions are still referenced by tag: I could not look up and verify commit SHAs offline, and invented SHAs would be worse. Left as a follow-up.
- Review code-review 3 [nit] glibc — builds on Ubuntu 22.04 runners.
- Review code-review 4 / feature-review 1 [nit] `cd chronoscope2` — now `cd <the clone>`.
- Review code-review 5 / feature-review 2 [nit] relative releases link — replaced by plain text, since the repository URL is not known yet.
- Review code-review 6 [nit] cache location — Windows paths named.
- Review code-review 7 [nit] compaction wording — reworded.
- Review code-review 8 / feature-review 4 [nit] substring workflow test — now parses the YAML.
- Review feature-review 5 [nit] licence — still to be decided by a human; unchanged, noted in the README.
- All round-1 ACs still hold (same tests, all passing).
- UI: none.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (2427 passed, 0 failed)
