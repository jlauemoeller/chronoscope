# Cycle 2 documentation

## Goal
Fold this cycle's changes into the base specification and bring the README and the performance page up to date, so the documents describe the application as built.

## Spec excerpts
`SPEC.md` below is the cycle 2 specification. `spec/SPEC.md` is the base spec, which describes the application as built. Where the two conflict, `SPEC.md` wins.

From SPEC.md §8:

- Fold the changes into `doc/SPEC.md`:
  - §3.1, §3.2: fields now read.
  - §5.1: the model, and rule 6 of Claude turn assembly.
  - §5.2: the footer row, tool call rows and a Notice row.
  - §4.3, §6.5: the details overlay and its key.
  - §4.1, §5.2: the shared duration format.
  - §1.3, §4.2, §6.3, §6.9: the turns sidebar, its component, keys and mouse behaviour.
  - §8.5: the `[keys.turns]` context.
- Add `details`, `focus_turns`, `next_pane` and `prev_pane` to the README's browser key table, and add a `[keys.turns]` table.
- Update `docs/performance.md` if the measured figures change.

From SPEC.md §7:

Every task must pass:

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Tasks that change parsing, the model or rendering must also stay within budget on:

```
cargo test --release -- --ignored budgets --nocapture
```

Snapshot changes are limited to the rows a task intends to change. A snapshot changed by accident is a failure, not something to accept. Two changes are expected to touch many snapshots, and those are still reviewed one by one: the shared duration format (§2.4) changes every snapshot showing a footer or selector-row duration of a minute or longer, and the sidebar changes every browser screen snapshot at 90 columns or wider.

Planner notes (decisions not in the spec; binding for this task):

- `doc/SPEC.md` is the base spec as it stands in the repository. Edit that file, not the copies under `factory/state/`. The cycle's own documents `doc/cycle-2-SPEC.md` and `doc/cycle-2-ui-sketch.md` stay as they are.
- The base spec is written as a description of the application as built, in the present tense. Fold the changes into its prose and tables, and add no "cycle 2" notes.
- Earlier tasks added the README's `details`, `focus_turns`, `next_pane` and `prev_pane` rows and the `[keys.turns]` table, because `tests/release_docs.rs::readme_keybindings_match_the_default_keymap` requires them. Here, verify them and their descriptions rather than adding them again. Mention the `[keys.turns]` context wherever the README lists contexts, and describe the sidebar in the README's usage or mouse text where those sections describe the browser.
- `readme_config_example_is_the_one_in_the_spec` compares the README's config example with the one in `doc/SPEC.md`. If `[keys.turns]` goes into the example, change both or neither.
- Run the budgets and `cargo bench` as `docs/performance.md` describes, and update its figures if they changed. Task 004 changed the generator (`toolUseResult`), so the fixture table and the measurements are expected to change. Also add the sidebar key press measured by task 012 and the details read time printed by task 008.
- Check the base spec's §9.2 budgets still read correctly after these numbers, and do not change the budgets themselves.

## Acceptance criteria
- [ ] `doc/SPEC.md` §3.1 and §3.2 list the fields now read.
- [ ] `doc/SPEC.md` §5.1 describes the model (`TokenUsage`, `Item::Notice`, source references) and states rule 6 of Claude turn assembly as built: wall-clock duration, per-turn message count.
- [ ] `doc/SPEC.md` §5.2 has the footer row in its new form (`cached`), tool call rows with durations, and a Notice row.
- [ ] `doc/SPEC.md` §4.3 and §6.5 describe the details overlay and its key.
- [ ] `doc/SPEC.md` §4.1 and §5.2 give the shared duration format.
- [ ] `doc/SPEC.md` §1.3, §4.2, §6.3 and §6.9 describe the turns sidebar, its component, keys and mouse behaviour, and §1.3's component table lists `TurnSidebar` and the details overlay.
- [ ] `doc/SPEC.md` §8.5 has the `[keys.turns]` context.
- [ ] The README's browser key table has `details`, `focus_turns`, `next_pane` and `prev_pane`, and it has a `[keys.turns]` table.
- [ ] `docs/performance.md` has the figures of a run made for this task, if they changed, including the generator's new fixture table.
- [ ] `tests/release_docs.rs` passes, including `readme_keybindings_match_the_default_keymap` and `readme_config_example_is_the_one_in_the_spec`.
- [ ] No source file under `src/` and no file under `factory/state/` changes, apart from test updates needed if a README test's fixed text moved.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.
- [ ] `cargo test --release -- --ignored budgets --nocapture` stays within budget.
- [ ] No snapshot changes.

## UI
UI: none

## Depends on
- 004-tool-call-durations
- 006-claude-stop-notices
- 009-opencode-detail-sources
- 013-turn-sidebar-mouse
