STATUS: DONE

## Changes
Round 2 (addresses round-1 reviews; no behaviour change to the scaffold):
- Build notes: full verbatim `cargo tree -d -e normal` output recorded below (the only must-fix).
- `src/test_support.rs`: `buffer_to_styled` now uses `writeln!` (no temporary `format!` string) and folds the continuation cells of wide glyphs into the glyph's run, so CJK/emoji stay one run; the doc comment states this and that underline colour is deliberately left out. New unit test `styled_keeps_wide_characters_in_one_run`.
- `Cargo.toml`: added `rust-version = "1.85"`; removed `license = "MIT"` (no LICENSE file exists and no licence has been chosen).
- Not changed (deferred to the CLI task, as both reviewers allowed): `run()` still lets clap exit the process for help/version/errors.

## Self-check
- AC: Cargo.toml / cargo build — `cargo build`/`cargo test` compile all declared crates.
- AC: main.rs thin, --version/--help exit 0 — `version_prints_crate_version_and_exits_zero`, `help_prints_usage_and_exits_zero` (tests/cli.rs).
- AC: no duplicate ratatui/ratatui-core/crossterm — output of `cargo tree -d -e normal` is recorded verbatim below. None of the duplicated crates (foldhash, hashbrown, memchr, miniz_oxide, regex-automata, syn, winnow) is `ratatui`, `ratatui-core` or `crossterm`; each of those appears once in the tree (ratatui v0.30.2, ratatui-core v0.1.2, crossterm v0.29.0).
- AC: .gitignore — `/target` appended, original lines kept.
- AC: test_support helpers — `render_returns_buffer_of_requested_size`, `text_has_one_line_per_row_and_keeps_trailing_spaces`, `text_does_not_duplicate_wide_characters`, `styled_lists_runs_of_identical_style`, `styled_keeps_wide_characters_in_one_run`.
- AC: smoke snapshot — `rounded_block_with_titles_text`, `rounded_block_with_titles_styled` (tests/snapshots/, no `.snap.new`).
- AC: CI workflow — `.github/workflows/ci.yml`.
- UI: none.
- Review: [must-fix] record cargo tree output — pasted verbatim below.
- Review: nit `run()` exits from library — deferred to CLI task (noted above).
- Review: nit underline colour undocumented — documented in `buffer_to_styled` doc comment.
- Review: nit `format!` push_str — replaced with `writeln!`.
- Review: nit license without LICENSE — field removed.
- Review: nit rust-version — added `rust-version = "1.85"`.
- Review: nit wide-char continuation cells noisy in styled output — merged into glyph run, with test.

### `cargo tree -d -e normal` (verbatim)
```
foldhash v0.1.5
└── hashbrown v0.15.5
    └── hashlink v0.10.0
        └── rusqlite v0.37.0
            └── chronoscope v0.1.0 (/home/jacob/Development/projects/chronoscope2)

foldhash v0.2.0
└── hashbrown v0.17.1
    ├── indexmap v2.14.2
    │   ├── plist v1.10.3
    │   │   └── syntect v5.3.0
    │   │       └── chronoscope v0.1.0 (/home/jacob/Development/projects/chronoscope2)
    │   └── toml_edit v0.23.10+spec-1.0.0
    │       └── chronoscope v0.1.0 (/home/jacob/Development/projects/chronoscope2)
    ├── kasuari v0.4.13
    │   └── ratatui-core v0.1.2
    │       ├── ratatui v0.30.2
    │       │   ├── chronoscope v0.1.0 (/home/jacob/Development/projects/chronoscope2)
    │       │   ├── tui-input v0.15.5
    │       │   │   └── chronoscope v0.1.0 (/home/jacob/Development/projects/chronoscope2)
    │       │   └── tuirealm v4.1.0
    │       │       └── chronoscope v0.1.0 (/home/jacob/Development/projects/chronoscope2)
    │       ├── ratatui-crossterm v0.1.2
    │       │   └── ratatui v0.30.2 (*)
    │       ├── ratatui-macros v0.7.2
    │       │   └── ratatui v0.30.2 (*)
    │       └── ratatui-widgets v0.3.2
    │           ├── ratatui v0.30.2 (*)
    │           └── ratatui-macros v0.7.2 (*)
    ├── lru v0.18.5
    │   └── ratatui-core v0.1.2 (*)
    ├── ratatui-core v0.1.2 (*)
    └── ratatui-widgets v0.3.2 (*)

hashbrown v0.15.5 (*)

hashbrown v0.17.1 (*)

memchr v2.8.3
├── aho-corasick v1.1.5
│   ├── regex v1.13.1
│   │   ├── lazy-regex v3.6.2
│   │   │   └── tuirealm v4.1.0 (*)
│   │   └── lazy-regex-proc_macros v3.6.2 (proc-macro)
│   │       └── lazy-regex v3.6.2 (*)
│   ├── regex-automata v0.4.18
│   │   └── fancy-regex v0.16.2
│   │       └── syntect v5.3.0 (*)
│   └── regex-automata v0.4.18
│       └── regex v1.13.1 (*)
├── nucleo-matcher v0.3.1
│   └── nucleo v0.5.0
│       └── chronoscope v0.1.0 (/home/jacob/Development/projects/chronoscope2)
├── pulldown-cmark v0.13.5
│   └── chronoscope v0.1.0 (/home/jacob/Development/projects/chronoscope2)
├── quick-xml v0.42.0
│   └── plist v1.10.3 (*)
├── regex-automata v0.4.18 (*)
└── serde_json v1.0.154
    ├── chronoscope v0.1.0 (/home/jacob/Development/projects/chronoscope2)
    └── syntect v5.3.0 (*)

memchr v2.8.3
├── regex v1.13.1 (*)
└── regex-automata v0.4.18 (*)

miniz_oxide v0.8.9
└── png v0.18.1
    └── image v0.25.10
        └── arboard v3.7.0
            └── chronoscope v0.1.0 (/home/jacob/Development/projects/chronoscope2)

miniz_oxide v0.9.1
└── flate2 v1.1.10
    ├── png v0.18.1 (*)
    └── syntect v5.3.0 (*)

regex-automata v0.4.18 (*)

regex-automata v0.4.18 (*)

syn v2.0.119
├── derive_more-impl v2.1.1 (proc-macro)
│   └── derive_more v2.1.1
│       └── crossterm v0.29.0
│           ├── chronoscope v0.1.0 (/home/jacob/Development/projects/chronoscope2)
│           ├── ratatui-crossterm v0.1.2 (*)
│           └── tuirealm v4.1.0 (*)
├── strum_macros v0.28.0 (proc-macro)
│   └── strum v0.28.0
│       ├── ratatui-core v0.1.2 (*)
│       └── ratatui-widgets v0.3.2 (*)
└── tuirealm_derive v4.1.0 (proc-macro)
    └── tuirealm v4.1.0 (*)

syn v3.0.9
├── clap_derive v4.6.7 (proc-macro)
│   └── clap v4.6.7
│       └── chronoscope v0.1.0 (/home/jacob/Development/projects/chronoscope2)
├── darling_core v0.24.1
│   ├── darling v0.24.1
│   │   └── instability v0.3.14 (proc-macro)
│   │       ├── ratatui v0.30.2 (*)
│   │       ├── ratatui-crossterm v0.1.2 (*)
│   │       └── ratatui-widgets v0.3.2 (*)
│   └── darling_macro v0.24.1 (proc-macro)
│       └── darling v0.24.1 (*)
├── darling_macro v0.24.1 (proc-macro) (*)
├── displaydoc v0.2.7 (proc-macro)
│   ├── icu_collections v2.4.0
│   │   └── icu_segmenter v2.4.0
│   │       └── textwrap v0.16.4
│   │           └── chronoscope v0.1.0 (/home/jacob/Development/projects/chronoscope2)
│   ├── icu_locale_core v2.4.0
│   │   ├── icu_locale_fallback v2.4.0
│   │   │   └── icu_segmenter v2.4.0 (*)
│   │   └── icu_provider v2.4.0
│   │       ├── icu_locale_fallback v2.4.0 (*)
│   │       └── icu_segmenter v2.4.0 (*)
│   ├── icu_provider v2.4.0 (*)
│   ├── tinystr v0.8.5
│   │   ├── icu_locale_core v2.4.0 (*)
│   │   └── icu_locale_fallback v2.4.0 (*)
│   └── zerotrie v0.2.5
│       ├── icu_provider v2.4.0 (*)
│       └── icu_segmenter v2.4.0 (*)
├── instability v0.3.14 (proc-macro) (*)
├── lazy-regex-proc_macros v3.6.2 (proc-macro) (*)
├── serde_derive v1.0.229 (proc-macro)
│   ├── serde v1.0.229
│   │   ├── bincode v1.3.3
│   │   │   └── syntect v5.3.0 (*)
│   │   ├── chronoscope v0.1.0 (/home/jacob/Development/projects/chronoscope2)
│   │   ├── plist v1.10.3 (*)
│   │   └── syntect v5.3.0 (*)
│   └── syntect v5.3.0 (*)
├── synstructure v0.14.0
│   ├── yoke-derive v0.8.4 (proc-macro)
│   │   └── yoke v0.8.4
│   │       ├── icu_collections v2.4.0 (*)
│   │       ├── icu_provider v2.4.0 (*)
│   │       ├── zerotrie v0.2.5 (*)
│   │       └── zerovec v0.11.9
│   │           ├── icu_collections v2.4.0 (*)
│   │           ├── icu_locale_core v2.4.0 (*)
│   │           ├── icu_locale_fallback v2.4.0 (*)
│   │           ├── icu_provider v2.4.0 (*)
│   │           ├── icu_segmenter v2.4.0 (*)
│   │           ├── potential_utf v0.1.6
│   │           │   ├── icu_collections v2.4.0 (*)
│   │           │   ├── icu_locale_fallback v2.4.0 (*)
│   │           │   └── icu_segmenter v2.4.0 (*)
│   │           └── tinystr v0.8.5 (*)
│   └── zerofrom-derive v0.1.8 (proc-macro)
│       └── zerofrom v0.1.8
│           ├── icu_collections v2.4.0 (*)
│           ├── icu_provider v2.4.0 (*)
│           ├── yoke v0.8.4 (*)
│           ├── zerotrie v0.2.5 (*)
│           └── zerovec v0.11.9 (*)
├── thiserror-impl v2.0.21 (proc-macro)
│   └── thiserror v2.0.21
│       ├── kasuari v0.4.13 (*)
│       ├── ratatui-core v0.1.2 (*)
│       ├── syntect v5.3.0 (*)
│       └── tuirealm v4.1.0 (*)
├── yoke-derive v0.8.4 (proc-macro) (*)
├── zerofrom-derive v0.1.8 (proc-macro) (*)
└── zerovec-derive v0.11.7 (proc-macro)
    └── zerovec v0.11.9 (*)

winnow v0.7.15
├── toml v0.9.12+spec-1.1.0
│   └── chronoscope v0.1.0 (/home/jacob/Development/projects/chronoscope2)
└── toml_edit v0.23.10+spec-1.0.0 (*)

winnow v1.0.4
└── toml_parser v1.1.5+spec-1.1.0
    ├── toml v0.9.12+spec-1.1.0 (*)
    └── toml_edit v0.23.10+spec-1.0.0 (*)
```

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (13 passed, 0 failed)
