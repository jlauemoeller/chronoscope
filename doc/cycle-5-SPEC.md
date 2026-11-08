# Chronoscope — Cycle 5 Specification: Guideline Alignment

## 0. How to read this document

**Scope.** Plan and build only the requirements in this document. The application already exists and is complete: do not scaffold it, and do not re-plan anything else. This cycle changes how the code is organised, not what the application does (§0, Invariants).

**Base spec.** [product.md](product.md) describes the application as built. It is context, not work to plan. Where the two documents conflict, this one wins. Other documents linked from it are context too.

**References.** `§n` means a section of this document, and `product §n` a section of product.md. `architecture.md`, `coding.md` and `testing.md` are the project guidelines in `doc/`.

**Guidelines.** This cycle exists to bring the code into line with the guidelines. Each requirement names the rule it restores. These rules bind every task and every review. Quote the ones that apply into each task file, because reviewers look for guidelines in the task:

| Guideline | Rule |
|---|---|
| architecture.md, Application structure, Rules | Data flows down through `attr`, intent flows up as `Msg`; the `App` never reaches into a component's internals; one owner per piece of state; a `Msg` carries everything the `App` needs |
| architecture.md, Application structure | `App::update` is the only place that changes domain state, mounts, moves focus and pushes data; it is a dispatcher; `Msg` is in application terms, split into nested enums once it grows |
| architecture.md, Events, focus and keys | Mouse events go to the topmost component under the pointer, whose area comes from the pure layout function; overlays block the components under them (routing as amended in §0 item 6) |
| architecture.md, Data flow | Parsing and file or database scans run on worker threads; the UI thread reads only for the two documented exceptions |
| coding.md, Components | One component per element; a component draws only itself; no long render functions (more than one element, or past about 50 lines); rendering has no side effects; colours and modifiers come from the theme |
| coding.md, Shared code | A component never depends on another component's internals; a helper two components share gets a module of its own |
| coding.md, Files and modules | Code that several adapters share sits beside them in `src/harness/` |
| coding.md, Portability | No direct OS APIs; all paths are `PathBuf`, no hard-coded `/` separators |
| testing.md, Where tests live | Test code is test-only because of where it sits, not because of its name; a file's own tests are in its `mod tests`; tests across files in `<module>/tests/`; only `fixtures` may sit beside the code |
| testing.md, Snapshots | Snapshots stay where and under the name insta gives them; no test changes a snapshot's path or name |

**Departures from the guidelines.** Seven rules are amended rather than enforced as written. The code changes below follow the amended rule, and the final task (§13) writes the amendment into the guideline. Reviewers judge these points against the amended rule, not the old text:

1. **Emphasis modifiers** (coding.md, Components: "All colours and modifiers come from theme tokens"). Product §7.2 and §4.5 fix the bold, reverse-video, italic, underline and strikethrough modifiers, and no theme token carries them. Amended rule: colours come from theme roles and tokens. The fixed modifiers of product §7.2 and §4.5 come from one module, `src/theme/emphasis.rs` (§5). No component writes a `Modifier` itself.
2. **Input-mode keys** (architecture.md, Events, focus and keys: "Components never match literal `KeyEvent`s, except that `Prompt` takes printable characters as text"). Product §5 fixes `Enter`, `Esc`, `Backspace` and the arrow keys in input mode, and product §7.5 has no input context. Amended rule: `Prompt` also matches those fixed input-mode keys. No code change.
3. **Resize redraw** (architecture.md, Data flow: "at most every 50 ms", and product §6.1). A resize is drawn at once (`RedrawRequest::Resized` in `src/app/redraw.rs`), so the screen is never shown at a stale size. Amended rule: the 50 ms limit applies to every redraw except one that follows a resize. product §6.1 gets the same exception. No code change.
4. **Unix process and file APIs** (coding.md, Portability: "No direct OS APIs"). `exec` for resume (`src/app/resume.rs`) and the device and inode check for a replaced file (`src/harness/claude/load.rs`) have no cross-platform crate. Amended rule: direct OS APIs are allowed only in `src/platform.rs`, each with a portable fallback for other platforms (§7.4).
5. **Facts a component publishes** (architecture.md, Application structure, Rules: "Intent flows up as `Msg`"). In tui-realm a component can send a `Msg` only from `on(&Event)`. `attr` returns nothing, so a component cannot answer data the `App` pushes with a message. Amended rule: right after pushing data with `attr`, the `App` may `query` a fact the component computed from that data, for example a count, rows built from the turns, or a search request. The `App` never queries presentation state that a key or a click moved, such as a cursor or a selection. That comes up as a `Msg` (§9.1).
6. **Mouse routing** (architecture.md, Events, focus and keys, Mouse: "for example through mouse subscriptions whose row/column ranges are that component's area"). tui-realm 4.1 cannot route the mouse by position:
   - It does not know where a component was drawn (`Application::view` keeps no area).
   - A mouse clause matches on position alone, ignoring its kind and modifiers (`MouseEventClause::is_in_range`).
   - Overlapping subscriptions all get the event, with no z-order and no way to consume it.
   - `unsubscribe` removes more than the one subscription it names (`application.rs:276`).

   Amended rule:
   - One pure function over the pure layout, `hit_test`, decides which component is on top. That is the one place z-order is defined.
   - `App::update` sends that component the event in its local coordinates, as `UserEvent::Pointer`.
   - The component alone decides what the event means: selection, folding, focus, double-clicks, a press outside an overlay.
   - The `App` interprets no click and keeps no click state, apart from which component got the last left press (§8.3).
   - `Msg::Pointer`, the raw pointer event that `GlobalEvents` hands to `update`, is the one `Msg` that is not in application terms, and it exists for this routing only.
7. **Snapshots named per case** (testing.md, Snapshots: "Snapshots stay … under the name insta gives them"). A test that loops over cases (kinds, sizes, themes) and takes one snapshot per case may name each snapshot after its case, as `details_overlay` (`every_kind_of_section_at_both_sizes_in_both_themes`, `content_taller_than_the_screen_at_80x24_scrolled_to_the_top_and_to_the_end`) and `turn_sidebar` (the helper `snapshot_state` and the 12 tests that call it) do. Amended rule: a test that takes one snapshot names none; a test that loops over cases names each snapshot after its case, and nothing else names snapshots. No code change: these tests keep their names and their snapshots are only moved and renamed by §3.1.

**Invariants.** These hold for every task:

- **No behaviour change.** Every screen, key, mouse action, message, toast, file written and text copied is the same before and after. A test that changes what it asserts about behaviour is a failure, not an update. Tests may change how they reach the code: module paths, helpers, message names, and where the double-click clock is injected (§8).
- **No snapshot content changes.** A snapshot may move or be renamed (§3). Its body below the insta header stays byte-for-byte the same. Any other snapshot change is accidental, and a failure, not something to accept.
- **No test is lost.** The number of tests that `cargo test` runs does not go down, except where tests are merged and the task says so.

**Task ids.** Numbering restarts at `001`, and the factory recognises a merged task by its commit subject. Do not reuse a task id from an earlier cycle (see `git log --format=%s main`).

**Order.** §2 is the first task, because the test gate fails on main without it. §3 comes next, so every later task writes its tests in their new places. §4 to §7 do not depend on each other and may run in any order, but §4 and §5 both touch `list_pane.rs` and `turn_sidebar/line.rs`, so they do not run at the same time. §8 comes before §9 and §10, because it removes messages they would otherwise restructure. §10 is the last code task. §13 is the final task. Each of §2, §4, §5, §6 and §7 fits one task. §3, §8 and §9 may each be split into two tasks along the lines they give.

## 1. Background

The guidelines were split out of the factory prompt and the base spec in commit `bca8a7d`, after four cycles had been built. An audit of the code against them on 2026-11-08 found the deviations below. The table is context, not work: the sections it points to are the work.

| # | Finding | Evidence | Disposition |
|---|---|---|---|
| 1 | `cargo test` fails on main: a test reads `doc/SPEC.md`, which `bca8a7d` renamed to `product.md` | `tests/release_docs.rs:128` | Fix, §2 |
| 2 | Test modules named as tests and sitting beside the code (`*_tests.rs`, `snapshots.rs`, `testing.rs`, test-only `durations.rs`, `subagent_title.rs`) | `src/app/mod.rs:34-53`, `src/harness/opencode/mod.rs:11-25`, `src/components/turn_block/mod.rs:34-43` and others | Fix, §3 |
| 3 | Extra inline test modules beside `mod tests` | `opencode/assemble.rs:941,1134`, `harness/mod.rs:228,291` | Fix, §3 |
| 4 | Tests that name their snapshots | `transcript/snapshots.rs:74`, `details_overlay/snapshots.rs:295,305`, `browser_tests/launch.rs:202`; the looping tests in `details_overlay/snapshots.rs:256,274,281` and `turn_sidebar/snapshots.rs:35` | Fix, §3; amend for the looping tests, §0 item 7 |
| 5 | Components use helpers that live in another component's module | `truncate_start` (working_dir), `truncate_with_ellipsis` (toast), `tail_fitting` (prompt, a duplicate of `truncate_start`), `format_cost` (turn_block renderers), `section_details`/`section_source` (details_overlay, used by transcript), `TurnRows` (turn_sidebar, built by transcript), `CONTENT_COLUMN` (renderers, used by transcript mouse), `titled_block`/`ListCursor` (list_pane, used by seven components) | Fix, §4 |
| 6 | Modifiers written in components; ThemeSelector's `>` marker ignores the `selection_marker` token | `Modifier::` in production code across components, the sidebar and markdown (§5 lists them); `theme_selector/rows.rs:88` | Fix and amend, §5 |
| 7 | Render functions over 50 lines or drawing several elements | `ReadBody::body` 66 lines, `BrowserHeader::cwd_row` 56, `Prompt::view` 52, `EditBody::body` 50 | Fix, §6 |
| 8 | `SelectorFrame` paints the whole frame, not its own area | `selector_frame.rs:50` | Fix, §6 |
| 9 | Log output from the render path | `transcript/mod.rs:467-470` | Fix, §6 |
| 10 | `AssistantTextSection` has no test of its own | `renderers/text.rs:13` | Fix, §6 |
| 11 | ThemeSelector builds a `HelpLine` in every `view` instead of owning it | `theme_selector/mod.rs:207-211` | Fix, §6 |
| 12 | A subagent id with `/` joined into a path | `harness/claude/subagent.rs:115-118` | Fix, §7 |
| 13 | A project id split on `/` to get its last segment | `app/browser.rs:384` | Fix, §7 |
| 14 | The same title fallback and notify filter written in both adapters | `claude/discovery.rs:776-778` and `opencode/discovery.rs:562-564`; `claude/watch.rs:353-377` and `opencode/watch.rs:97-122` | Fix, §7 |
| 15 | Subagent lookup scans files and the database on the UI thread | `app/browser.rs:542` | Fix, §7 |
| 16 | `std::os::unix` in production code | `claude/load.rs:139-157`, `app/resume.rs:287-301` | Isolate and amend, §7 |
| 17 | The `App` interprets clicks: per-component policy (`takes_mouse`), double-click state, `*Clicked` handlers that decide focus, selection and opening. Hit testing over the layout stays, as routing (§0 item 6) | `app/mouse.rs:98-147, 255-338` | Fix and amend, §8 and §0 item 6 |
| 18 | `App::update` re-enters itself | `app/mouse.rs:216,242` | Fix, §8 and §9 |
| 19 | The `App` pulls state out of components with `query`, including presentation state a user action moved | `app/search.rs:190-197`, `session_panels.rs:50-69`, `browser.rs:827,882,922-928,971,997`, `transcript_search.rs:224-241,312`, `details.rs:107` | Fix and amend, §9 and §0 item 5 |
| 20 | The `App` calls component methods before mounting them | `app/details.rs:43,46`, `app/theme_selector.rs:73` | Fix, §9 |
| 21 | Public mutators that change state outside `update` | `app/runtime.rs:61-74`, `selector.rs:479,487,493,523`, `browser.rs:319`, `mod.rs:556` | Fix, §9 |
| 22 | `Msg` has 45 flat variants; prompt messages are routed by guarded arms; confirmations are compared with `"y"` | `app/mod.rs:155-300, 721-751`, `resume.rs:207`, `save.rs:184` | Fix, §10 |
| 23 | `Prompt` matches fixed editing keys | `prompt.rs:178-184` | Amend, §0 item 2 |
| 24 | Resize bypasses the 50 ms limit | `app/redraw.rs:56` | Amend, §0 item 3 |
| 25 | architecture.md describes things that have changed: the `Harness` trait, `SidebarGap`, the crate list, the handler modules, mouse and tick routing | see §13 | Docs, §13 |
| 26 | Borderline items | see §11 | Not in this cycle |

## 2. The test gate passes on main

- `tests/release_docs.rs` reads `doc/product.md` where it reads `doc/SPEC.md` today. `readme_config_example_is_the_one_in_the_spec` passes, and is renamed `readme_config_example_is_the_one_in_the_product_doc`.
- The comments that cite the old base spec as `SPEC §n` cite the section of product.md they mean, or the guideline when the section moved there. The old sections map to the new ones as follows: old §9.2 is product §8.2, old §9.3 is product §8.3, old §9.4 is product §8.4, old §9.6 is testing.md, old §7.1 is product §6.1, old §6.6 is product §5.6, old §6.8 is product §5.8, old §8.5 is product §7.5, old §11 is product §10. The comments are in:
  - `tests/release_docs.rs:1`, `tests/harness_registry.rs:1`, `tests/claude_live.rs:1` and `tests/opencode_live.rs:1`;
  - `tests/claude_local_commands.rs:458`, which means cycle 3 §2.2;
  - `tests/fixtures/README.md:3`;
  - `benches/first_content.rs:2`, `benches/frame_render.rs:3` and `benches/full_parse.rs:2`.
- References to a cycle spec (`SPEC cycle 3 §2.1`, `cycle 2 SPEC §4.2`) stay as they are, because those documents still exist.

Acceptance: the whole test gate of the stack profile passes on the branch. `grep -rnE 'SPEC §[0-9]' src tests benches` finds only references that name a cycle.

## 3. Test layout

Restores testing.md, Where tests live and Snapshots. This is a move: no test changes what it does.

### 3.1 Where each file goes

| Today | After |
|---|---|
| `src/app/browser_tests.rs`, `src/app/browser_tests/*.rs` | `src/app/tests/browser.rs`, `src/app/tests/browser/*.rs` |
| `src/app/mouse_tests.rs`, `search_tests.rs`, `selection_tests.rs`, `selector_tests.rs`, `selector_totals_tests.rs`, `session_panels_tests.rs`, `session_panels_values_tests.rs` | `src/app/tests/mouse.rs`, `search.rs`, `selection.rs`, `selector.rs`, `selector_totals.rs`, `session_panels.rs`, `session_panels_values.rs` |
| `src/app/tests.rs` | stays: it is the `tests` module of `src/app/mod.rs`, and declares the files above as its submodules |
| `src/harness/opencode/details_tests.rs` | `src/harness/opencode/details/tests.rs`, the `mod tests` of `details.rs` |
| `src/harness/opencode/notice_tests.rs`, `stats_tests.rs` | `src/harness/opencode/tests/notice.rs`, `tests/stats.rs`, submodules of `src/harness/opencode/tests.rs` |
| `src/harness/opencode/testing.rs` (`pub(crate) mod testing`) | `src/harness/opencode/fixtures.rs` (`#[cfg(test)] pub(crate) mod fixtures`) |
| `src/components/turn_block/renderers/tool/testing.rs` | `src/components/turn_block/renderers/tool/fixtures.rs` |
| `src/components/transcript/snapshots.rs`, `key_snapshots.rs` | `src/components/transcript/tests/snapshots.rs`, `tests/key_snapshots.rs` |
| `src/components/turn_block/snapshots.rs`, `notice_snapshots.rs`, `durations.rs`, `subagent_title.rs` (test-only modules) | `src/components/turn_block/tests/snapshots.rs`, `tests/notice_snapshots.rs`, `tests/durations.rs`, `tests/subagent_title.rs`, submodules of `turn_block/tests.rs` |
| `src/components/details_overlay/snapshots.rs` | `src/components/details_overlay/tests/snapshots.rs`, a submodule of the overlay's `mod tests` |
| `src/components/turn_sidebar/snapshots.rs` | `src/components/turn_sidebar/tests/snapshots.rs` |

- No `#[path]` attributes. The module tree follows the files.
- `replace_tests` and `token_tests` in `src/harness/opencode/assemble.rs` become one `#[cfg(test)] mod tests` in that file. Grouping submodules inside it is fine. The same goes for `default_details_tests` and `registered_tests` in `src/harness/mod.rs`.
- The fixtures modules that stay beside the code (`src/app/fixtures.rs`, `src/components/transcript/fixtures.rs`, `src/components/turn_block/fixtures.rs`, `src/components/turn_sidebar/fixtures.rs`) are unchanged.
- After the move, no file or module under `src/` has a name ending in `_tests` or `snapshots`, or is called `testing`, except files under a `tests/` directory.

### 3.2 Snapshots

- Moving a test module renames its snapshots, as testing.md says. Insta names a snapshot `<crate>__<module path>__<test name>.snap` and puts it in `snapshots/` next to the test file. For example, `src/app/browser_tests/snapshots/chronoscope__app__browser_tests__copy__copied_toast_120x40_text.snap` becomes `src/app/tests/browser/snapshots/chronoscope__app__tests__browser__copy__copied_toast_120x40_text.snap`.
- Accept the renamed snapshots, and delete the old files in the same commit. Only the header may differ (`source:`, `expression:`). The body below the header must be byte-for-byte the same.
- The four tests that pass a name to `assert_snapshot!` without looping over cases stop doing so (the looping tests keep their names, §0 item 7):
  - `a_long_value_wraps_to_the_value_column_at_80x24` (`details_overlay/snapshots.rs:295`), `an_unavailable_group_is_one_muted_row` (`details_overlay/snapshots.rs:305`) and `after_esc_the_screen_is_the_normal_startup_screen` (`browser_tests/launch.rs:202`) take one snapshot each. They drop the name, and the snapshot is renamed after the test.
  - `after_a_resize_from_120_to_80_columns_text_is_wrapped_again_at_80x24_text` (`transcript/snapshots.rs:74`) takes two. It becomes two tests, each with one snapshot and a name that states the state and the size (testing.md, Prefer): one for the 120-column view before the resize, and one for the 80-column view after it, which keeps its assertions.
  - The renamed snapshots have the bodies of the old ones.
- §3.1 may be split into two tasks: `src/app/` in one, everything else in the other.

Acceptance:

- `cargo insta test --unreferenced=reject` passes. No `.snap.new` or `.pending-snap` file exists.
- The number of `.snap` files under `src/` and `tests/` is the same as on main.
- A script, or a check given in the build notes, pairs every deleted `.snap` with an added one and shows that the bodies are identical.
- The number of tests run is the same as on main plus one, for the split test.

## 4. Shared helpers get modules of their own

Restores coding.md, Shared code: "A component never depends on another component's internals. A helper that two components share gets a module of its own."

- **Text fitting.** `truncate_with_ellipsis` (`src/components/toast.rs:116`), `truncate_start` (`src/components/working_dir.rs:31`) and `tail_fitting` (`src/components/prompt.rs:266`) move to one module, `src/components/text_fit.rs`. `truncate_start` and `tail_fitting` are the same algorithm, cutting from the start with `…` in front. They become one function, and every caller uses it. This includes `app/resume.rs:103` and `app/save.rs:60`. Where the two differ in any edge case (an empty string, a width of 0 or 1, wide characters), the unit tests of the merged function pin down today's behaviour for each caller. If they really differ, keep two functions in the module, each named for what it does.
- **Cost.** `format_cost` moves from `src/components/turn_block/renderers/stats.rs` to `src/cost.rs`, beside `src/tokens.rs`. coding.md, Shared code lists it with `format_duration` and `format_tokens` (§13).
- **Section details.** `section_details` and `section_source` (`src/components/details_overlay/rows.rs`, `source.rs`) are called in production only by the `Transcript` (`transcript/details.rs:6`). They move to a module outside both components, `src/section_details.rs` (or `src/section_details/` with the same split as today). `DetailsRequest` stays with the overlay as its props type. The overlay's tests that cover these functions move with them.
- **Turn rows.** `TurnRows`, `entry_for` and `TURN_ROWS_ATTR` (`turn_sidebar/rows.rs`, `turn_sidebar/mod.rs:82`) are built by the `Transcript` and shown by the `TurnSidebar`. They move to a module that both import, `src/components/turn_rows.rs`. The sidebar keeps its drawing (`line.rs`) and its presentation state.
- **Content column.** `transcript/mouse.rs` uses no `pub(crate)` renderer constant. `TurnBlock` (or `BlockLayout`) gains a public method that answers what the Transcript needs, for example the content column, or which section a row and column belong to. `CONTENT_COLUMN` stays private to the renderers.
- **List pane parts.** `list_pane.rs` says it holds "What `ProjectList` and `SessionList` share", but `titled_block` is also used by `WorkingDir`, `SessionTime`, `SessionTokens`, `OverlayFrame` and `ThemeSelector`, and `ListCursor` and `VISIBLE_ROWS_ATTR` by `ThemeSelector`. `titled_block` moves to `src/components/titled_block.rs`, and `ListCursor` with `VISIBLE_ROWS_ATTR` to `src/components/list_cursor.rs`. `list_pane.rs` keeps only what the two lists share.

Acceptance: no file under `src/components/<a>/` or `src/components/<a>.rs` imports from another component's module `src/components/<b>`. The exceptions are the shared modules named above, the component types themselves, and their props and attribute types re-exported from `src/components/mod.rs`. The build notes list the imports between component modules before and after. No snapshot changes.

## 5. Emphasis comes from the theme

Restores coding.md, Components ("All colours and modifiers come from theme tokens"), amended as in §0 item 1.

- A new module, `src/theme/emphasis.rs`, gives every fixed style of product §7.2 and §4.5 a name. The styles include:
  - the selected transcript section and theme-selector row (bold);
  - the focused list band (reverse video, not bold);
  - the selected row of an unfocused list or sidebar (bold `secondary_foreground`);
  - the selected harness tab (underlined);
  - markdown strong, emphasis, strikethrough and link text;
  - headings (bold, and underlined at level 1);
  - table headers;
  - the bold labels of the help and details overlays and the browser header.

  The module combines each with the theme's colours where product §7.2 says so. It is the only place in production code outside `src/theme/` that names `ratatui::style::Modifier`.
- Every component, renderer and the markdown renderer use it. Today's sites include:
  - `browser_header.rs:210`, `list_pane.rs:433,441`, `help_overlay/content.rs:195`, `details_overlay/content.rs:209` and `theme_selector/rows.rs:91`;
  - `turn_sidebar/line.rs:70,76` and `turn_block/emphasis.rs:25`;
  - `markdown/blocks.rs:89-92,257` and `markdown/mod.rs:60-62`;
  - all other production uses of `ratatui::style::Modifier`, for example in `turn_block/renderers/tool/subagent.rs`, `help_overlay/mod.rs`, `details_overlay/mod.rs` and `theme_selector/mod.rs`.
- The selection styles that `turn_sidebar/line.rs` (`Styles::new`) and `list_pane.rs` (`RowStyles::new`) build almost line for line come from the new module once, not from two copies.
- The ThemeSelector's `>` marker (`theme_selector/rows.rs:88`) takes `Token::SelectionMarker`, as product §7.1 says, not `Role::Primary`. The default token is `primary`, so no snapshot changes. A new test renders the theme selector with a theme that overrides `selection_marker` and checks the marker's colour.
- `src/markdown/mod.rs:11-12` claims that "every colour and modifier comes from the theme tokens". It is reworded to say what is true after this change.
- No theme token is added, and theme files are unchanged.

Acceptance:

- Outside `src/theme/`, no production code names `ratatui::style::Modifier`. Test code (`#[cfg(test)]` modules, files under `tests/`, fixtures) and `src/test_support.rs` are not counted. `KeyModifiers` is a different type and is not counted either. The build notes give the command used to check this and its output.
- Every styled snapshot is unchanged.
- The budgets test passes (§12).

## 6. Components draw only themselves, in short functions

Restores coding.md, Components ("A component draws only itself"; "No long render functions"; "Rendering has no side effects") and architecture.md, Components ("Each is its own type with its own tests").

- **`ReadBody::body`** (`renderers/tool/read.rs:144`, 66 lines) only dispatches. The numbered `Content:` block of a readable result is built by a helper of its own.
- **`EditBody::body`** (`renderers/tool/edit.rs:81`, 50 lines) moves its diff-line loop into a helper, so it is clearly under the limit.
- **`BrowserHeader::cwd_row`** (`browser_header.rs:234`, 56 lines) lays out the row only. The parse-warning note and the `● live` marker are drawn by small `Widget` types, each with a unit test.
- **`Prompt::view`** (`prompt.rs:297`, 52 lines) draws the label and delegates the text and the cursor cells to a widget, as `draw_question` already does. The widget writes to the buffer, and `view` does not.
- **`SelectorFrame`** draws only its own area: its border, its sizing and its overlay background. The full-screen background it paints today when it is the base screen (`selector_frame.rs:47-51`) is painted by a `BackgroundFill` under its own `Id` (for example `Id::ScreenBackground`). It is mounted, drawn first and unmounted with the selector screen, the way `Id::SidebarGap` is. The transparent-background case is unchanged.
- **`ThemeSelector`** owns its `HelpLine` as a child, built when its hints or theme change, and does not build one in every `view`.
- **No logging from `view`.** `Transcript::window_for` (`transcript/mod.rs:467-470`) does not log. If the diagnostic is still useful, it is logged where the viewport is set, outside rendering.
- **`AssistantTextSection`** (`renderers/text.rs:13`) gets direct unit tests of its lines, beside the other section renderers' tests.

Acceptance:

- No function on the render path in `src/components/` or `src/markdown/` is over 50 lines, counted from its signature to its closing brace. The render path is `view`, `render`, `body`, `lines` and the helpers they call. The build notes list the five longest with their lengths.
- No snapshot changes.
- The budgets test passes (§12).

## 7. Harness: paths, shared code and the UI thread

### 7.1 Paths

Restores coding.md, Portability ("no hard-coded `/` separators").

- `resolve` in `src/harness/claude/subagent.rs` joins a subagent's file from its parts (`root`, `subagents`, `agent-<x>.jsonl`), one component at a time, as `resolve_opened` already does. It does not join `format!("{id}.jsonl")` for an id that contains `/`.
- `src/app/browser.rs:384` takes a project's last path segment with `Path::file_name`, not `rsplit('/')`. A unit test covers a cwd with the platform's separator.

### 7.2 Shared adapter code

Restores coding.md, Files and modules ("Code that several adapters share sits beside them in `src/harness/`").

- The title fallback (`claude/discovery.rs:776-778`, `opencode/discovery.rs:562-564`) becomes `title_from_prompt(Option<&str>) -> String` in `src/harness/shared.rs`. It returns the first non-empty trimmed line, cut by `truncate_title`, or `UNTITLED`. Both adapters call it, and it has unit tests of its own.
- The notify event filter (`claude/watch.rs:353-377`, `opencode/watch.rs:97-122`) becomes one function in `src/harness/`. It wakes on an error, skips `EventKind::Access`, and wakes when `need_rescan()` is set or the event has no paths. Both watchers call it, and each keeps only its own path matching.

### 7.3 Subagent lookup off the UI thread

Restores architecture.md, Data flow ("Parsing, watching, stats computation and search indexing run on worker threads").

- `Harness::subagent` may list directories, read `*.meta.json` files, stream the parent transcript (`find_by_result`), or scan the parent's rows in the OpenCode database. It is no longer called from `App::update` (`src/app/browser.rs:542`). The lookup runs on a worker, for example as the first step of the load worker that opening a subagent starts today. Its result comes back over the existing session port.
- What the user sees is unchanged:
  - A subagent that is found opens as today, with the same loading state, breadcrumb and focus.
  - One that is not found shows the same `NO_SUBAGENT_TRANSCRIPT` toast and leaves the parent as it was, with its focus, selection and scroll.
  - The subagent stack is pushed only when the lookup succeeds.
- A second `a` while a lookup is pending does what a second `a` while a subagent is loading does today. The task states which case that is and tests it.
- A result for a lookup that is no longer current (the user went back or opened another session) is dropped, by the same generation check that load results use (`load_seq`).

### 7.4 One platform module

Restores coding.md, Portability, amended as in §0 item 4.

- `src/platform.rs` holds every direct use of `std::os::*` in production code:
  - `file_identity(&Metadata)`, the device and inode on Unix, with today's `created()` fallback elsewhere, used by `claude/load.rs:139-157`;
  - `exec_or_run(Command)`, `exec` on Unix, with today's `status()` fallback elsewhere, used by `app/resume.rs:287-301`.
- Each function has a doc comment that names the fallback, and has unit tests on the platform it runs on.
- `grep -rn 'std::os::' src --include=*.rs` finds production uses only in `src/platform.rs`. Test code may keep its `#[cfg(unix)]` uses.

Acceptance for §7: the adapter tests and the browser subagent tests pass unchanged, apart from waiting for the worker where a test opened a subagent synchronously before. The opening tests of `src/app/tests/browser/subagent.rs` (after §3) cover a lookup that fails, a lookup that is overtaken by going back, and `a` pressed twice.

## 8. Mouse events go to the component under the pointer

Restores the user's rule that there is no central event handler, under the amended mouse rule (§0 item 6). Routing, meaning which component is on top, stays one pure function over the layout. Everything a click means moves into the component that was clicked. Today `app/mouse.rs` does both:

- it hit-tests (`hit_test`);
- it decides which component takes the mouse when (`takes_mouse`);
- it keeps double-click state (`MouseState`, `ClickTarget`);
- it turns clicks into focus, selection and opening (`project_clicked`, `session_clicked`, `theme_clicked`, `turn_clicked`, `outside_overlay`);
- it re-enters `update` with the component's answer.

### 8.1 Routing

- **The pointer arrives as today.** `GlobalEvents` keeps its one full-screen mouse subscription and turns each left press and wheel step without modifiers into `Msg::Pointer(MouseEvent)`. This is today's `Msg::Mouse`, renamed. Raw `Event::Mouse` events that tui-realm also forwards to the focused component are still ignored by every component.
- **Who is on top is pure.** `hit_test` moves to `src/app/layout.rs`, beside `layout`. It returns where a pointer event goes, as one of:
  - the topmost mouse-taking component that contains the point, with its area;
  - a press outside the topmost overlay, addressed to that overlay;
  - nothing.

  It is a pure function of the `ScreenLayout` and the layout input, with unit tests of its own.
- **Structural blocking goes with it.** The rules that depend on the screen, not on any one component, are part of that pure function, and are tested there:
  - Nothing under an open overlay takes the pointer: help, details, theme selector, and the selector over the browser.
  - While the prompt is open, only the overlays take it, and the Sessions pane during the selector search.
  - The toast (`Id::Toast`) and `Id::SidebarGap` take nothing: a press on the toast reaches no component, as today.
- **A component's own conditions move into it.** For example, while the selector is filtered, the wheel leaves the Projects pane alone. `ProjectList` gets a prop that says the selector is filtered, and ignores the wheel itself. After this, no `App` code decides by component `Id` what a pointer event does.
- **Delivery without re-entry.** `App::update` gives the component `Event::User(UserEvent::Pointer(RoutedMouse))`, the event in the component's own coordinates, as today. It queues the returned `Msg` with `emit` (`app/mod.rs:655`) instead of calling `self.update` (`app/mouse.rs:216,242`). No other code in `src/app/` calls `update` from inside `update`.
- **The top transcript.** The layout places "the transcript". The router gives the event to the transcript on top (`top_transcript`), as today.

### 8.2 Components decide what a click means

- **Overlays close themselves.** A press outside the topmost overlay goes to that overlay as an outside press (a flag on `RoutedMouse`, or its own `UserEvent` variant). The overlay sends what its `Esc` sends:
  - `HelpOverlay`: `CloseHelp`;
  - `DetailsOverlay`: `CloseDetails`;
  - `ThemeSelector`: `CancelTheme`;
  - `SelectorFrame` over the browser: `CloseSelector`. It does nothing when it is the base screen (its existing `base` prop) or while the resume confirmation is open, which it learns from a new prop.

  `outside_overlay` goes.
- **Double-clicks belong to the components.** `ProjectList`, `SessionList`, `ThemeSelector` and `TurnSidebar` each keep their own last press, with an injectable clock like `Toast`'s. A double-click is two left presses on the same row at most `DOUBLE_CLICK` (400 ms) apart. A completed double-click is forgotten, so a third press starts a new one. A press of the component's own that is not on a row ends a double-click under way. `App::set_click_clock` keeps working for the tests: the `App` passes the clock to these components when it mounts them.
- **Clicks become intent.** A component sends one message per event, in application terms. Where a click means several things, the message names the intent and says whether the click completed a double-click, for example `SelectorMsg::ChooseSession { id, open: bool }`. Its handler does what `session_clicked` does today: focus, select and, on a double-click, open. The handlers keep no click state and do no hit testing. `ProjectClicked`, `SessionClicked`, `ThemeClicked` and `TurnClicked` are renamed to such intents. What each one does is unchanged.

### 8.3 The press-elsewhere notice

Today a left press anywhere that is not on a row ends a double-click under way: on another component, a tab, the toast, or empty space. A component cannot see presses that go elsewhere, so the router tells it.

- The router remembers the `Id` of the component that got the last left press, and nothing else: no row, no time, no target.
- On every left press, if that component is still mounted and the press goes anywhere else, it first gets `Event::User(UserEvent::PointerElsewhere)`. That includes a press on another component, a press that reaches no component, and a press outside an overlay. Then the press is routed as usual.
- A component that gets `PointerElsewhere` forgets its last press. It sends no message.
- The wheel sends no notice and does not end a double-click, as today.

### 8.4 What is removed

- `takes_mouse`, `MouseState`, `ClickTarget`, `outside_overlay`, `project_clicked`, `session_clicked`, `theme_clicked`, `turn_clicked` and the `row_pressed` bookkeeping go from `src/app/mouse.rs`.
- `src/app/mouse.rs` keeps only the routing step: calling `hit_test`, the press-elsewhere notice, delivery and `emit`. Its module doc describes §8.1 to §8.3.
- `Msg::Mouse` becomes `Msg::Pointer`, and `UserEvent::Mouse` becomes `UserEvent::Pointer`, with `UserEvent::PointerElsewhere` added. Their doc comments say they are built by the router, not by a port (§0 item 6).

### 8.5 Tests

- The existing mouse tests keep what they assert:
  - `src/app/tests/mouse.rs`, plus `tests/browser/mouse.rs`, `tests/browser/overlay_mouse.rs` and `tests/browser/sidebar_mouse.rs` under `src/app/` (paths after §3);
  - they may only rename the messages they send.
- The double-click tests keep passing, with the clock injected as today.
- `hit_test` and the structural blocking get pure unit tests:
  - the topmost component wins where areas overlap;
  - a press on the toast reaches nothing;
  - nothing under each overlay is hit;
  - a press outside the topmost overlay is addressed to that overlay;
  - the prompt cases: the Sessions pane takes the pointer during the selector search, and nothing does during the resume confirmation.
- Each component that takes the pointer gets unit tests through `on()`:
  - its clicks and wheel steps give the right `Msg` or `Cmd`;
  - an outside press closes each overlay, except the base selector and the selector during the resume confirmation.
- Each component with double-clicks gets unit tests through `on()`, with a fixed clock:
  - two presses on the same row inside 400 ms are a double-click, and after 400 ms they are not;
  - a third press starts over;
  - a press on another row, a press not on a row, and `PointerElsewhere` between the two presses each end it.
- Update tests check the notice end to end:
  - a press on a session row, then a press on a harness tab, the toast and empty space in turn, then the same row again, is not a double-click;
  - the same in the theme selector and in the turns sidebar.
- A test checks that `update` does not re-enter itself. For example, it counts reconcile passes per pointer `Msg`, or asserts that the component's answer arrives through the queue.

§8 may be split into two tasks:

1. The pure `hit_test` with structural blocking, delivery with `emit`, and overlays closing themselves;
2. Double-clicks in the components, the press-elsewhere notice, intent messages, and the removal of the `App`'s click handlers.

## 9. State flows up as messages

Restores architecture.md, Application structure (Rules): "Intent flows up as `Msg`. … the `App` never reaches into a component's internals." "A `Msg` carries everything the `App` needs to act on it." And: "`App::update` is the only place that changes domain state, mounts and unmounts components, moves focus and pushes new data into components."

### 9.1 Components report; the App does not ask

The `App` uses `query` today for three kinds of thing, and each is treated by where the change starts. A component can send a `Msg` only from `on(&Event)`. `attr` returns nothing (§0 item 5).

**Moved by a key or a click: the component sends a `Msg`.**

| Today | After |
|---|---|
| `browser.rs:922-928` reads the top transcript's `SELECTED_SECTION_ATTR` after every message, `Tick` included | The transcript's `on()` sends `SelectionChanged(SectionId)` whenever a key, a click or the wheel changes its selection. Where the selection moves because of data the `App` pushed (a live update, a search jump, a subagent swap), the `App` queries it once right after that push, as item 5 allows, and not after every message |
| `selected_row()` reads `SessionList`'s cursor (`SELECTED_ATTR`, `search.rs:190-197`) when a key or click moved it | `SessionList`'s `on()` sends the selected row's id whenever a key, a click or the wheel moves its cursor |

**Moved by the `App`: the `App` keeps the value and pushes it down.**

| Today | After |
|---|---|
| While search results are listed, the `App` moves `SessionList`'s cursor with `SEARCH_NAV_ATTR` and then asks the list where it is (`selected_row()`, used by `search_submit`, `push_filtered_results` and `panels_session`) | The `App` keeps the selected result's index as its own search state, moves it, and pushes it to the list. It never reads it back |
| `session_panels.rs:69` reads back the rows the `App` pushed (`SESSION_ROWS_ATTR`) | The `App` takes them from its own index state |
| `browser.rs:997` reads back `EmptySession`'s text | The `App` keeps the value it pushed |

**Computed from data the `App` just pushed: the query stays (§0 item 5).**

- These stay, each made right after the push it depends on:
  - `NEW_SECTIONS_ATTR` (`browser.rs:827`), `TURN_ROWS_ATTR` (`browser.rs:882`) and `TURN_COUNT_ATTR` (`browser.rs:971`), after a `ModelUpdate`;
  - `SEARCH_POSITION_ATTR` (`transcript_search.rs:312`), after search results;
  - `SEARCH_REQUEST_ATTR` (`transcript_search.rs:224-241`), after `SEARCH_QUERY_ATTR`.
- Each is wrapped in one small `App` helper per fact, beside the push, with a doc comment that cites §0 item 5.
- None of them runs on every message.

After this section, every `self.application.query(` in `src/app/` falls under item 5. The build notes list each one with the push it follows. Mount checks such as `mounted` are not counted. The doc comments that describe the old pull (for example `app/mod.rs:223-224, 235`) are updated.

### 9.2 No component calls before mounting

- `app/details.rs:43,46` calls `overlay.attr(MAX_WIDTH_ATTR, ..)` and `overlay.desired_size(..)`, and `app/theme_selector.rs:73` calls `selector.desired_size()`. These calls go. The size comes from the content type, which the `App` builds from plain data, as `app/help.rs:45` does with `HelpContent`. Mounting produces no event, so the overlay cannot report its size in a message.
- The overlay's size stays the `App`'s layout input, as today.

### 9.3 Startup goes through `update`

- `run_app` (`app/runtime.rs:61-74`) starts the application by sending messages to `App::update` (for example `Msg::Start(Startup)` with the preselection, the seeded index and the session or filter from the command line). It does not call mutators directly.
- `set_preselection`, `seed_index`, `open_summary`, `show_filtered` (`selector.rs:479,487,493,523`, `browser.rs:319`) and `show_toast` (`mod.rs:556`) are no longer `pub`. They are `pub(super)` handlers that only `update` calls. `App::new` may mount the components every screen needs and set the first focus, which is construction, not update.
- The startup tests (`launch`, `selector` and the CLI tests) assert the same screens and focus.

§9 may be split into two tasks: §9.1 in one, §9.2 and §9.3 in the other.

## 10. Messages in application terms

Restores architecture.md, Application structure: "`Msg`: what happened, in application terms … Split it into nested enums per screen or concern once it grows", and "It is a dispatcher: each `Msg` arm calls a small handler, grouped in modules by concern."

- `Msg` is split into nested enums, one per concern, beside the existing `Selector(SelectorMsg)`. The split follows the handler modules, for example:
  - `Browser`: opening, live updates, subagents, sidebar;
  - `Transcript`: search, selection reports;
  - `Overlay`: help, details;
  - `Theme`;
  - `Prompt`;
  - `Output`: copy, save, export, resume;
  - `Worker`: index, stats, search results.

  The top level keeps only what is global (`Quit`, `Tick`, `Resized`, and the like). Each top-level arm of `App::update` hands the inner message to the handler module of its concern. `App::update` is a dispatcher of a few lines per arm, with no guards.
- **The prompt says what it is for.** The `Prompt` is mounted with a purpose:
  - selector search;
  - transcript search;
  - save path;
  - overwrite confirmation;
  - resume confirmation.

  It sends its submit, cancel and navigation messages with that purpose, so the `App` does not pick an owner with guarded arms from its own state (`app/mod.rs:721-751`).
- **Confirmations are booleans.** A `y/n` prompt sends `Confirm(bool)`. `if answer == "y"` (`resume.rs:207`, `save.rs:184`) goes. Which keys answer yes and no is unchanged (product §5).
- Names stay in application terms. No variant carries a raw key, and only `Msg::Pointer` carries a raw mouse event (§0 item 6).

Acceptance:

- `App::update` and each concern's dispatcher are under 50 lines.
- No arm of `App::update` has a guard.
- Every existing test passes, with only the messages it sends renamed.

## 11. Not in this cycle

These were found and judged not worth changing now. Do not plan them, and reviewers do not raise them:

- The immutable `OnceLock` syntax sets in `src/markdown/highlight.rs`, set lazily on first use. They are not UI state.
- Moving the section model (`derive_sections`, `SectionId`, `FoldState`) out of `src/components/turn_block/`, although `turn_sidebar`, `details_overlay` and `transcript_search` use it. It is re-exported, and moving it touches most of the transcript.
- `src/model/output.rs:64` calling `crate::harness::claude::extract_result_text`, a model-to-adapter dependency against the data-flow direction.
- `src/transcript_search.rs` importing turn-block renderers. It is not a component, so coding.md's rule does not cover it.
- The near-duplicates `take_held` (both `assemble.rs`) and the alias part of `display_name` (both `discovery.rs`). Each has harness-specific differences.
- An OpenCode compaction fixture. Product §10 lists the OpenCode compaction format as unverified, and no compaction is produced. testing.md records the exception (§13).
- `OverlayFrame` and `PanelRow` having no snapshot tests of their own. They are helpers, not components, and are covered through the overlays and panels.
- `src/test_support.rs` being compiled into the library. The integration tests need it, and testing.md names it.
- The `App` keeping overlay sizes, the toast text and the prompt text for the pure layout and the help line, where §8 and §9 don't remove them.
- Long functions that draw nothing: `App::new`, `run_app`, `layout`, `search_sessions`, `section_markdown`, `reveal_match` and `anchored`.
- `ListPane`'s free functions `render_rows` and `row_line`.

## 12. Acceptance for the whole cycle

Every task must pass the whole test gate of the stack profile:

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
INSTA_UPDATE=no cargo test --no-fail-fast
```

§5, §6 and §8 touch rendering or the event path, so their tasks must also hold the budgets:

```
INSTA_UPDATE=no cargo test --release -- --ignored budgets --nocapture
```

- No snapshot body changes in any task. In §3, snapshots are renamed, and nowhere else are snapshot files added or removed, except the new tests that §5, §6 and §8 ask for.
- `cargo insta test --unreferenced=reject` passes after every task.
- `git status --porcelain --untracked-files=all` is empty after a gate run.
- The number of tests run does not go down (§0, Invariants).

## 13. Documentation (final task)

The guidelines are updated to the code as built, and to the amended rules. Each change below is a rule or a description. Rules are written so a reviewer can check them.

**architecture.md:**

- Data flow:
  - the `Harness` trait sketch adds `Send + Sync` and the methods it lacks (`refresh`, `watch_paths`, `watch_files`, `stats`, `stats_key`, `open_file`), with their defaults;
  - the subagent lookup runs on a worker (§7.3);
  - the resize exception to the 50 ms limit (§0 item 3).
- Application structure:
  - the handler modules as built (§10);
  - `Msg`'s nested enums;
  - the prompt's purpose;
  - startup through `update` (§9.3);
  - the amended data-flow rule for facts a component publishes (§0 item 5).
- Components:
  - the table lists `BackgroundFill`, mounted as `SidebarGap` and as the screen background (§6);
  - the shared modules of §4 (`text_fit`, `titled_block`, `list_cursor`, `turn_rows`) beside `PanelRow`.
- Events, focus and keys:
  - the mouse rule as amended in §0 item 6, which replaces the "for example through mouse subscriptions" text: the pure `hit_test` and its structural blocking, delivery as `UserEvent::Pointer` in local coordinates, the press-elsewhere notice, overlays closing themselves, and double-clicks and click intent in components;
  - why: tui-realm 4.1 cannot route by position (§0 item 6);
  - Application structure's `UserEvent` line: what the ports deliver, plus the router's `Pointer` and `PointerElsewhere`;
  - the data-flow diagram's mouse path;
  - `Tick` drives toast expiry and the live marker, while the redraw limit is the run loop's;
  - `GlobalEvents` also takes resize and tick;
  - the amended input-mode key rule (§0 item 2).
- Tech stack: the crates added since cycle 1 (`toml_edit`, `unicode-segmentation`, `base64`, `pulldown-cmark`, `syntect`, `similar`, `chrono`, `log`, `anyhow`, `shell-words`, `ctrlc`).

**coding.md:**

- Components: the amended modifier rule and `src/theme/emphasis.rs` (§0 item 1, §5).
- Shared code:
  - `format_cost` in `src/cost.rs`;
  - `src/components/text_fit.rs`;
  - `src/section_details.rs`;
  - the shared adapter helpers (`title_from_prompt`, the watch filter).
- Portability:
  - the `src/platform.rs` exception (§0 item 4);
  - `ctrlc` in the list of cross-platform crates.

**testing.md:**

- Where tests live: the `src/app/tests/` layout as an example beside `src/components/transcript/tests/`.
- Snapshots: when a test needs two snapshots of different states, it is two tests; a test that loops over cases names each snapshot after its case (§0 item 7).
- What is tested: the OpenCode compaction exception (§11).

**product.md:** §6.1 gets the resize exception to the 50 ms limit (§0 item 3).

- Stale `product §n` references in the doc comments of moved code are fixed where the task found them.
- The README is updated only if it describes something this cycle changed. It should not.
