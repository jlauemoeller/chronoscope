STATUS: DONE

## Changes
- `src/bench_support/` (new, `#[doc(hidden)] pub mod`, shared by benches and tests):
  - `generate.rs`: seeded generator (SplitMix64, no new dependency) of a Claude-format session of a requested size: human prompts (some multi-line), assistant messages split over several lines, readable and redacted thinking, Bash/Read/Edit/Grep calls with results, about 5% large results (100 KB to 2 MB), `turn_duration` entries, injected and noise attachments. `generate_session_with` takes the large share (0 gives the "dense" session of small entries only).
  - `mod.rs`: fixtures (`generate_fixture`, `cached_fixture` into `target/bench-fixtures`), `run_load` / `first_content` (time from `load()` to `Loaded` / the first `Appended`, worker stopped and awaited so memory is released), `load_transcript` (applies updates to a `Transcript` as the App does), `FrameScenario` (200x50 `TestBackend`, viewport in the middle, 12 small tool calls and one large output unfolded, wheel-scroll + draw), `write_small_sessions` for the selector benchmark.
  - `alloc.rs`: `CountingAlloc`, a counting global allocator (live and peak bytes).
  - `tests.rs`: generator and helper tests (see Self-check).
- `src/model/output.rs`, `src/model/mod.rs`: a `cfg(test)` thread-local counter of `ByteSource::read` calls (`source_reads`) next to the existing `text_calls`, so the "never materialised" test instruments both ways of reading a lazy output.
- `benches/first_content.rs`, `full_parse.rs`, `frame_render.rs`, `selector_index.rs` (criterion, `harness = false`, registered in `Cargo.toml`).
- `tests/budgets.rs`: the `#[ignore]`d `budgets` test (counting allocator installed in this binary). It checks the four budgets on two 100 MiB sessions (the mixed benchmark fixture and a dense one with no large outputs), worst of 5 loads, and prints the numbers.
- `docs/performance.md`: machine, fixtures, measurements, how to run. (The task names `docs/`; the repo's other docs are in `doc/`. I followed the task.)
- `.github/workflows/ci.yml`: added `cargo bench --no-run`.
- Tuning: none needed. Every budget is met with a wide margin on both fixtures, so no hot path was changed and all existing tests and snapshots are untouched.

## Self-check
- AC: seeded generator with the planner-notes mix; a 5 MB session parses with the assembler without unparsed entries — `bench_support::tests::every_line_is_json_and_the_mix_is_all_there`, `the_assembler_parses_a_generated_session_without_unparsed_entries` (5 MiB, `unparsed_count() == 0`, counts of prompts/tool calls/lazy outputs match the generator), `the_same_seed_gives_the_same_bytes_and_another_seed_other_bytes`, `the_file_has_the_requested_size`, `the_loader_and_the_assembler_agree_on_a_generated_session`, `the_dense_mix_has_no_large_outputs`, `a_cached_fixture_is_reused_and_describes_the_same_session`.
- AC: `cargo bench --no-run` compiles `first_content`, `full_parse`, `frame_render`, `selector_index`; CI runs it — verified locally (`cargo bench --no-run`), CI step added; `small_sessions_are_indexed_into_ten_projects` covers the selector fixture.
- AC: ignored `budgets` test asserts first content < 300 ms, full parse < 2 s, frame < 16 ms (max over 100 scrolled frames) and peak heap < 3x the file on the 100 MiB fixture — `tests/budgets.rs::budgets`; output below. `bench_support::alloc::tests::live_and_peak_follow_allocations_and_reallocations` covers the allocator.
- AC: folded large outputs never materialised during load or rendering — `bench_support::tests::folded_large_outputs_are_not_materialised_by_the_load_or_by_rendering` (counts `ToolOutput::text` and `ByteSource::read` calls across the load and drawing the whole folded session) and its control `the_frame_scenario_unfolds_a_large_output_and_that_does_materialise_it` (unfolding does raise both counters, so the instrument works; also checks the scenario scrolls and rewinds).
- AC: `docs/performance.md` records measurements and reproduction commands.
- AC: existing tests and snapshots keep passing — full `cargo test`.
- UI: none (no UI change, no snapshots).

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (2410 passed, 0 failed, 2 ignored: `budgets` and a pre-existing ignored test)

## Budget test output (this machine, release)
Machine: AMD Ryzen AI 9 HX 370 (24 threads), 54 GB RAM, Fedora 44, Linux 7.2.13, rustc 1.99.1, warm page cache.

`cargo test --release --test budgets -- --ignored budgets --nocapture`:

```
running 1 test
== mixed session (5% large outputs)
fixture: 104863096 bytes, 5975 lines, 272 prompts, 1581 tool calls, 84 large outputs
first content, 5 runs: [1.6, 1.2, 0.8, 1.1, 1.2] ms, worst 1.6 ms (budget 300 ms)
full parse, 5 runs: [122.3, 117.0, 111.1, 117.4, 114.4] ms, worst 122.3 ms (budget 2000 ms)
memory: peak heap 20.9 MiB = 0.21x the file, 11.3 MiB = 0.11x held after the load (budget 3x)
load into a transcript: first content 1.2 ms, loaded 124.2 ms, 544 turns
frames, 100 scrolled at 200x50: worst 2.31 ms, median 0.40 ms (budget 16 ms)
reference: unfolding the slowest large output (1965753 bytes) took 7.8 ms
== dense session (small entries only)
fixture: 104859318 bytes, 104041 lines, 5000 prompts, 27400 tool calls, 0 large outputs
first content, 5 runs: [1.5, 1.4, 1.7, 1.4, 1.4] ms, worst 1.7 ms (budget 300 ms)
full parse, 5 runs: [341.5, 341.9, 347.5, 327.3, 334.7] ms, worst 347.5 ms (budget 2000 ms)
memory: peak heap 83.1 MiB = 0.83x the file, 80.5 MiB = 0.80x held after the load (budget 3x)
load into a transcript: first content 1.8 ms, loaded 330.6 ms, 10000 turns
(no large output in this session: none unfolded)
frames, 100 scrolled at 200x50: worst 1.11 ms, median 0.54 ms (budget 16 ms)
test budgets ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.83s
```

Criterion (`cargo bench --bench ...`): first_content 0.71 ms, full_parse mixed 154 ms / dense 307 ms, frame_render draw 0.33 ms / scroll_and_draw 0.48 ms, selector_index cold 5.9 ms / warm 2.5 ms.
