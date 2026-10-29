# Large-session benchmarks and performance budgets

## Goal
Verify, and where needed reach, the §9.2 scale targets on a synthetic 100 MB Claude session:
- first content within 300 ms;
- full parse under 2 s;
- every frame rendered in under 16 ms;
- memory within roughly 3× the file size.

Add a deterministic generator for the synthetic session, `criterion` benchmarks for each budget, and a memory measurement. Tune the hot paths until the budgets are met in a release build on this machine, and record the measured numbers. The design decisions (streaming parse, lazy tool output, lazy per-block layout, virtualised rendering) are already in place from earlier tasks. This task measures them and fixes what falls short.

## Spec excerpts
From SPEC.md §9.2:

Target: sessions up to **~100 MB** stay smooth.
- Parsing is streaming (line by line for JSONL, paged queries for SQLite) and runs off the UI thread. The UI shows a loading state and becomes interactive as soon as the first screen of content is parsed.
- Large tool outputs: the model stores line counts and byte ranges up front. The full text is materialised only when the section is unfolded, searched, copied or exported.
- Layout is computed lazily per block for the current width. Only blocks overlapping the viewport are rendered.
- Session list stats are computed in the background and cached in `$XDG_CACHE_HOME/chronoscope/`, keyed by (path, size, mtime) or by (db, session id, `time_updated`).
- Session discovery reads each Claude transcript at most once per index build. Each file's scan result (cwd, start time, first prompt, last `ai-title`) is kept in memory, keyed by (path, size, mtime). The initial build and live rebuilds rescan only new or changed files, scanning several files in parallel. Results for files that are gone are dropped.
- Budgets (to be measured with benchmarks on a synthetic 100 MB fixture):
  - first content within 300 ms;
  - full parse under 2 s;
  - each frame rendered in under 16 ms;
  - memory use within roughly 3× the file size.

From SPEC.md §9.6:

- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

From SPEC.md §1.3:

The `Transcript` is **one** mounted component, because it is virtualised and can hold thousands of sections (§9.2). It does not mount one tui-realm component per section. It is still built from small parts rather than one large render function: a `TurnBlock` widget (rounded block, titles, selection marker), one renderer per section kind (§5.2), one body renderer per `ToolKind` (§5.3), the markdown renderer (§5.5) and the layout cache. Each is its own type with its own tests. Opening a subagent (§5.4) mounts a new `Transcript`. The parent's stays mounted but is not drawn, so `Backspace` returns to it unchanged.

From SPEC.md §1.2:

- Each piece of state has exactly one owner. The `App` owns domain state. Each component owns its presentation state (cursor, scroll offset, fold states, layout cache, prompt text). The `Transcript` also owns the open session's turns (§5.1), so a 100 MB session is held once (§9.2); the `App` forwards each `ModelUpdate` to it rather than keeping a copy. A `Msg` carries everything the `App` needs to act on it. For example, on `y` the transcript component computes the text as displayed and sends it in `CopyText`, so the `App` never has to read fold states.

From SPEC.md §1.1:

- The UI is a tui-realm `Application` (§2) driven by a single-threaded loop: the loop calls `Application::tick`, passes every returned `Msg` to `App::update`, and redraws only when something changed (at most every 50 ms, §7.1).
- Parsing, watching, stats computation and search indexing run on worker threads. Their results come back over channels, which sync Ports (`Poll`) read and turn into `Event::User` events. No async runtime is required; tui-realm's async ports (tokio) are not used.

Planner notes (decisions not in the spec; binding for this task):
- **Generator.** Put it in library code under `test_support` or a `bench_support` module, so benches and tests share it. It is deterministic (seeded) and writes a Claude-format JSONL of a requested size (100 MB for benches, a few MB for tests) into a temp/target dir. Never commit the generated file.
  - Use a realistic mix: human prompts, multi-line split assistant messages, text and readable/redacted thinking, Bash/Read/Edit/Grep tool calls with results, about 5% of them large (100 KB–2 MB) outputs, `turn_duration` entries, and some injected attachments.
- **Benchmarks** (`benches/`, criterion, `harness = false`):
  - `first_content`: the time from `load()` to the first `Appended` batch.
  - `full_parse`: the time from `load()` to `Loaded`.
  - `frame_render`: a `Transcript` view of the loaded session at 200x50, with the viewport in the middle and some sections unfolded (including a large output), rendered into a `TestBackend`.
  - `selector_index`: an index build over 2 000 small session files (scan-cache cold and warm), for reference.
- **Memory.** Measure peak heap after a full load, for example with a counting global allocator in a dedicated bench or ignored test binary, or with peak RSS from `/proc/self/status` on Linux. Compare it with the file size.
- **Budget check.** An `#[ignore]`d integration test (`cargo test --release -- --ignored budgets`) asserts the four budgets on the 100 MB fixture. It is ignored by default because CI machines vary. Run it in release mode on this machine and paste the output into the build notes.
- **Tuning.** If a budget fails, fix the cause. Likely suspects:
  - per-line allocations in the assembler, where `serde_json` borrowed or zero-copy parsing helps;
  - batch sizes;
  - rendering visible blocks only;
  - repeated markdown/syntect work: cache rendered lines per block for the current width/fold state, invalidated exactly like the layout cache;
  - materialising large outputs while folded.

  Keep all earlier tests passing.
- **Documentation.** Add `docs/performance.md` with the machine description, the measured numbers per budget, and how to run the benches and the budget test.
- The CI workflow keeps running `cargo test` only. Add a `cargo bench --no-run` step so benches stay compiling.

## Acceptance criteria
- [ ] A seeded generator produces a Claude-format session of a requested size with the planner-notes mix. A test generates 5 MB and parses it with the assembler without unparsed entries.
- [ ] `cargo bench --no-run` compiles benches for `first_content`, `full_parse`, `frame_render` and `selector_index`, and CI runs `cargo bench --no-run`.
- [ ] The ignored `budgets` test asserts first content < 300 ms, full parse < 2 s, frame render < 16 ms (max over 100 frames while scrolling) and peak heap < 3× the file size on the 100 MB fixture. It passes in release mode on this machine, with its output in the build notes.
- [ ] Folded large outputs are never materialised during load or rendering (asserted in a test with an instrumented output source).
- [ ] `docs/performance.md` records the measurements and how to reproduce them.
- [ ] Any tuning changes keep all existing tests and snapshots passing.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
UI: none

## Depends on
- 024-transcript-search
- 029-opencode-live-watch
