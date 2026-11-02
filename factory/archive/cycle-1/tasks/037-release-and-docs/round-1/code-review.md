VERDICT: FAIL

## Summary
The release workflow, the README and the README tests are well built. The workflow covers all five targets with least-privilege permissions, `--locked`, a tag/version check and `--verify-tag`. The README tests compare the keybinding tables with `Keymap::default()` exactly, load the config example with zero warnings and compare it with the SPEC block. Facts I spot-checked against the code also hold: chord contexts, `NO_COLOR` non-empty, the OpenCode candidate paths, the log and cache paths and `--help`/`--version`. One documented install prerequisite is wrong: the README says a Rust 1.85 toolchain is enough, but the crate does not build with anything older than 1.88.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass

YAML check, re-run independently: `yaml.safe_load` parses `.github/workflows/release.yml`. The trigger is `{'push': {'tags': ['v*']}}` and the matrix has exactly the five §9.3 targets. `actionlint` is not installed here.

## Findings
1. [must-fix] `README.md:38` — "You need a Rust toolchain (1.85 or newer)" is false, so `cargo install --path .` fails for users on 1.85–1.87. Two things raise the minimum:
   - The crate uses let chains, which need Rust 1.88. One example is `src/harness/opencode/location.rs:31-32` (`if cfg!(windows) && let Some(local) = ...`); there are about 22 such sites in `src/`.
   - Locked normal dependencies declare `rust-version = 1.88`: `ratatui-widgets` 0.3.2, `ratatui-crossterm` 0.1.2 and `time` 0.3.55, according to `cargo metadata --locked`. `uuid` 1.29 needs 1.89 but is not a normal dependency (`cargo tree -e normal --target all -i uuid` prints nothing).

   Change:
   - In `README.md` "From source", state "Rust 1.88 or newer".
   - Raise `rust-version` in `Cargo.toml` from `"1.85"` to `"1.88"` so the manifest and the README agree. The README claim is new in this diff; it copied the stale manifest value.
   - Optionally, add a test in `tests/release_docs.rs` that asserts the README's version matches `Cargo.toml`'s `rust-version`.
2. [nit] **Security** `.github/workflows/release.yml:41-47` — third-party actions (`dtolnay/rust-toolchain@stable`, `Swatinem/rust-cache@v2`) are referenced by mutable tags in the job that produces the published binaries. `rust-cache` also restores caches written by other refs into a tag build, which is a known cache-poisoning path into release artefacts. Consider:
   - pinning the actions by commit SHA;
   - dropping `Swatinem/rust-cache` from the release workflow. Release builds are rare, so the cache saves little.
3. [nit] `.github/workflows/release.yml:28` — the Linux binaries are built on `ubuntu-24.04` and `ubuntu-24.04-arm`, so they need glibc 2.39 and will not start on Debian 12, Ubuntu 22.04 or RHEL 9. Consider one of:
   - building on `ubuntu-22.04` and `ubuntu-22.04-arm`;
   - building `*-unknown-linux-musl` targets. rusqlite is bundled and arboard's X11 backend is pure Rust, so musl should be feasible.
4. [nit] `README.md:41-42` — `cd chronoscope2` assumes the clone directory's name. Use the real repository name, or write `cd <repository directory>`.
5. [nit] `README.md:21` — `[GitHub releases](../../releases)` works on github.com but is a dead link in the README.md packaged into the release archives. Consider an absolute URL once the repository URL is fixed.
6. [nit] `README.md:375` — the cache location is given only as `$XDG_CACHE_HOME/chronoscope/`. On Windows it is `%LOCALAPPDATA%\chronoscope\` (etcetera's Windows strategy), and the log falls back there as well. Consider naming the per-platform locations, as the Configuration section does.
7. [nit] `README.md:385-389` — the compaction open item names the Claude Code versions found on the developer's machine ("2.1.304 to 2.1.311"). That is odd wording for a public README. Consider "the fixture is modelled on real compacted sessions; still to be checked against newer Claude Code versions".
8. [nit] `tests/release_docs.rs:373-395` — `release_workflow_builds_and_uploads_all_five_targets_on_version_tags` only matches substrings. It would still pass if a target sat outside the matrix, or if the `v*` pattern were under a branch filter. Consider parsing the YAML, for example with the existing `serde_json`/`toml` stack or a small dev-dependency such as `serde_yaml_ng`, and asserting on `on.push.tags` and `jobs.build.strategy.matrix.include[*].target`.
