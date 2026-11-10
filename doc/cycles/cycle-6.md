# Chronoscope — Cycle 6 Specification: Module Files

## 0. How to read this document

**Scope.** Plan and build only the requirements in this document. The application already exists and is complete: do not scaffold it, and do not re-plan anything else. This cycle changes where module files sit, not what the code does (§0, Invariants).

**Base spec.** [product.md](../product.md) describes the application as built. It is context, not work to plan. Where the two documents conflict, this one wins. Other documents linked from it are context too.

**References.** `§n` means a section of this document, and `product §n` a section of product.md. `architecture.md`, `coding.md` and `testing.md` are the project guidelines in `doc/`.

**Guidelines.** This cycle adds one rule to coding.md, Files and modules. The rule, and the coding.md wording that depends on the move, were written into coding.md on main before the cycle started (§4). The rule binds every task and every review from the first task on. Quote it, and the rules below, into each task file, because reviewers look for guidelines in the task:

| Guideline | Rule |
|---|---|
| coding.md, Files and modules (new) | A module with submodules is a file named after it, `foo.rs`, with its submodules in `foo/`. No `mod.rs` files anywhere in the package. Clippy's `mod_module_files` lint, denied in `Cargo.toml`, enforces it |
| testing.md, Where tests live | A file's own tests are in its `mod tests`, inline or in `foo/tests.rs`; tests across files in `<module>/tests/` |
| testing.md, Snapshots | Snapshots stay where and under the name insta gives them; no test changes a snapshot's path or name; moving a test module moves its snapshot files too |

**Invariants.** These hold for every task:

- **No code change.** Every `.rs` file that moves is renamed and not edited: `git diff -M100% --name-status main -- '*.rs'` lists only `R100` lines. No other `.rs` file changes. The parents' `mod foo;` lines need no edit, because Rust finds `foo.rs` as readily as `foo/mod.rs`.
- **No snapshot body changes.** The snapshots of §2.2 move. Only their `source:` header line changes. Their body below the insta header stays byte-for-byte the same. No other snapshot file is touched.
- **No test is lost or added.** The number of tests that `cargo test` runs is the same as on main.

**Task ids.** Numbering restarts at `001`, and the factory recognises a merged task by its commit subject. Do not reuse a task id from an earlier cycle (see `git log --format=%s main`).

**Order.** §2 and §3 are one task. Do not split the move: the lint of §3 fails until every file has moved, and a half-moved tree has two styles. §4 is the final task.

## 1. Background

The project uses the old module layout, where a module with submodules is `foo/mod.rs`. Rust 2018 added the layout in which that module is `foo.rs` beside a `foo/` directory, and that is the form the Rust reference and the Rust book now show first. With it, an editor tab or a search result names the module (`theme_selector.rs`) and not one of 21 files called `mod.rs`.

The tree mixes both layouts today. These modules already use the new one, and are the model for the rest:

- `src/components/browser_header.rs` with `src/components/browser_header/`
- `src/components/turn_rows.rs` with `src/components/turn_rows/`
- `src/components/turn_block/export.rs` with `src/components/turn_block/export/`
- `src/harness/opencode/details.rs` with `src/harness/opencode/details/`
- `src/app/tests.rs` with `src/app/tests/`

`src/launch/` and `src/transcript_search/` are already in the new layout too.

The other 21 modules with submodules use `mod.rs`. `cargo clippy --all-targets -- -W clippy::mod_module_files` on main, at `df55a8e`, reports exactly these 21 files (§2.1). A dry run of §2 and §3 on `df55a8e` passed the test gate and `cargo insta test --unreferenced=reject`, with the same 3709 tests as main. None of them has a `#[path]` attribute, `include_str!`, `include_bytes!` or a custom insta snapshot path, so nothing in them depends on where the file sits.

## 2. Every module is a file named after it

### 2.1 The moves

Move each file with `git mv`, so that history follows the rename:

| Today | After |
|---|---|
| `src/app/mod.rs` | `src/app.rs` |
| `src/bench_support/mod.rs` | `src/bench_support.rs` |
| `src/components/mod.rs` | `src/components.rs` |
| `src/components/details_overlay/mod.rs` | `src/components/details_overlay.rs` |
| `src/components/help_overlay/mod.rs` | `src/components/help_overlay.rs` |
| `src/components/theme_selector/mod.rs` | `src/components/theme_selector.rs` |
| `src/components/transcript/mod.rs` | `src/components/transcript.rs` |
| `src/components/turn_block/mod.rs` | `src/components/turn_block.rs` |
| `src/components/turn_block/renderers/mod.rs` | `src/components/turn_block/renderers.rs` |
| `src/components/turn_block/renderers/tool/mod.rs` | `src/components/turn_block/renderers/tool.rs` |
| `src/components/turn_sidebar/mod.rs` | `src/components/turn_sidebar.rs` |
| `src/config/mod.rs` | `src/config.rs` |
| `src/harness/mod.rs` | `src/harness.rs` |
| `src/harness/claude/mod.rs` | `src/harness/claude.rs` |
| `src/harness/opencode/mod.rs` | `src/harness/opencode.rs` |
| `src/index/mod.rs` | `src/index.rs` |
| `src/keymap/mod.rs` | `src/keymap.rs` |
| `src/markdown/mod.rs` | `src/markdown.rs` |
| `src/model/mod.rs` | `src/model.rs` |
| `src/section_details/mod.rs` | `src/section_details.rs` |
| `src/theme/mod.rs` | `src/theme.rs` |

- Every submodule stays where it is. For example, `src/components/turn_sidebar/line.rs`, `fixtures.rs`, `tests.rs` and `tests/` do not move. Neither do the `tests.rs` files that are the `mod tests` of a moved module (`src/app/tests.rs`, `src/config/tests.rs`, `src/keymap/tests.rs`, `src/markdown/tests.rs`, `src/bench_support/tests.rs`, `src/components/transcript/tests.rs`, `src/components/turn_block/tests.rs`, `src/components/turn_sidebar/tests.rs`, `src/harness/opencode/tests.rs`).
- No `#[path]` attributes. The module tree follows the files.
- No module path changes. `crate::components::theme_selector::ThemeSelector` is the same path before and after.

### 2.2 Snapshots

Insta puts a test's snapshots in `snapshots/` next to the file that holds the test, and names each one after the module path, which does not change. A module whose `mod tests` is inline in its `mod.rs` therefore takes its snapshots one directory up when the file moves, under the same file names. That applies to two modules:

| Today | After |
|---|---|
| `src/components/help_overlay/snapshots/*.snap` (1 file) | `src/components/snapshots/` |
| `src/components/theme_selector/snapshots/*.snap` (6 files) | `src/components/snapshots/` |

- Move them with `git mv`, and remove the empty `snapshots/` directories.
- In each moved file, the `source:` header line names the new file (`source: src/components/theme_selector.rs`). Nothing else in the file changes.
- No name collides: every file name in `src/components/snapshots/` starts with its own module path.
- Every other snapshot directory stays where it is, because its test file does not move. That covers `src/app/snapshots/` (tests in `src/app/tests.rs`), `src/markdown/snapshots/` (`src/markdown/tests.rs`), `src/components/snapshots/` (the single-file components), and every `snapshots/` under a `tests/` or `export/` directory.

Acceptance:

- `find src tests benches -name mod.rs` finds nothing.
- `git diff -M100% --name-status main -- '*.rs'` lists exactly the 21 renames of §2.1, all `R100`, and nothing else.
- `git diff -M --name-status main -- '*.snap'` lists exactly the 7 renames of §2.2, and nothing else.
- For each of the 7 snapshots, the file on main and the file on the branch are identical below the insta header. Show this with a script, or with a check given in the build notes.
- `grep -rn '^source: .*mod\.rs' src tests` finds nothing.
- The number of `.snap` files under `src/` and `tests/` is the same as on main.
- `cargo insta test --unreferenced=reject` passes. No `.snap.new` or `.pending-snap` file exists.
- The number of tests run is the same as on main.

## 3. The lint keeps it that way

`Cargo.toml` gets a lints table that denies Clippy's `mod_module_files` lint:

```toml
[lints.clippy]
mod_module_files = "deny"
```

- The `[lints]` table applies to every target of the package, so `cargo clippy --all-targets` checks `src/`, `tests/` and `benches/`. A shared helper for integration tests therefore cannot be a `tests/<name>/mod.rs`. Such helpers stay in `src/test_support.rs`, as today.
- Do not enable `self_named_module_files`. It is the opposite lint, and it forbids the layout this cycle adopts.
- `rust-version` is 1.88, and the `[lints]` table needs 1.74, so the minimum version does not change.

Acceptance:

- `cargo clippy --all-targets -- -D warnings` passes on the branch.
- With any one module moved back to `foo/mod.rs` in a scratch copy, it fails and names that file. Give the output in the build notes.

## 4. Documentation (final task)

The guidelines are updated to the code as built. The text below is what the task writes. A reviewer checks it word for word, apart from line wrapping.

**coding.md is already done.** The cycle's changes to coding.md were made on main before the cycle started: the new rule that opens Files and modules, the `TurnSidebar` example (`src/components/turn_sidebar.rs`, with its submodules in `src/components/turn_sidebar/`), the emphasis rule's scope ("outside the `theme` module (`src/theme.rs` and `src/theme/`)"), and the `section_details` module in Shared code. The task does not rewrite them. It checks that they match the code as built, and fixes only what does not.

**architecture.md, Application structure.** In the handler-modules table, the Global row's Modules cell is `` `mod.rs`, `runtime.rs` (`Start`) ``. It becomes:

> `src/app.rs`, `runtime.rs` (`Start`)

**testing.md, Where tests live.** ``` `src/app/tests.rs` is the `mod tests` of `src/app/mod.rs` ``` becomes:

> `src/app/tests.rs` is the `mod tests` of `src/app.rs`

**testing.md, Snapshots.** After the rule "Snapshots stay where `insta` puts them by default …", add:

> - A module whose tests are inline in its own file has its snapshots in the `snapshots/` directory beside that file, not in its submodule directory. The snapshots of `src/components/theme_selector.rs` are in `src/components/snapshots/`.

**Other documents.**

- product.md and the README name no `mod.rs` path, and do not change.
- Any other directory-scoped wording that the task finds, where the module's root file now sits outside the directory named, is fixed the same way. List each one in the build notes.

Acceptance: `grep -rn 'mod\.rs' doc/architecture.md doc/coding.md doc/testing.md doc/product.md README.md` finds only the coding.md rule that opens Files and modules.

## 5. Not in this cycle

These were found and judged not worth changing now. Do not plan them, and reviewers do not raise them:

- The `mod.rs` strings in `src/bench_support/generate.rs:125-127`. They are synthetic session content, file names that the generated transcript mentions, not references to this tree. Changing them would change the benchmark data.
- The `src/uvc/mod.rs` string in `src/components/turn_block/tests/snapshots.rs:476` and the body of its snapshot. It is fixture data.
- The `mod.rs` paths in the earlier cycle specs (`doc/cycle-*-SPEC.md`) and in `factory/archive/`. They record what was true when they were written.
- The length of the moved files, for example `src/components/theme_selector.rs` at 1533 lines. The move does not change it.
- Moving the inline tests of `help_overlay` or `theme_selector` into a `tests.rs` of their own. That would rename their snapshots and is not needed for this cycle.

## 6. Acceptance for the whole cycle

Every task must pass the whole test gate of the stack profile:

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
INSTA_UPDATE=no cargo test --no-fail-fast
```

Nothing here touches rendering or the event path, so the budgets are not run.

- `cargo insta test --unreferenced=reject` passes after every task.
- `git status --porcelain --untracked-files=all` is empty after a gate run.
- The number of tests run is the same as on main (§0, Invariants).
- `find src tests benches -name mod.rs` finds nothing after the code task.
