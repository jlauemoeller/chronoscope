VERDICT: PASS

## Summary
The round adds a seeded generator of Claude-format sessions, four criterion benches, a counting allocator, an ignored `budgets` test, a materialisation test with read counters, `docs/performance.md` and a `cargo bench --no-run` CI step. No hot path was tuned because none needed it. I re-ran the gate, `cargo bench --no-run` and the release `budgets` test on this machine. All four budgets pass on both 100 MiB fixtures with numbers close to the build notes (mixed: first content 2.0 ms, full parse 193 ms, worst frame 3.4 ms, peak heap 0.21x; dense: 3.1 ms, 519 ms, 1.5 ms, 0.83x). Every acceptance criterion is met. The findings below are all nits.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (all suites green; `budgets` and one older test ignored)

Also checked:
- `cargo bench --no-run` compiles `first_content`, `full_parse`, `frame_render` and `selector_index`.
- `cargo test --release --test budgets -- --ignored budgets --nocapture` passes; output as summarised above.
- No `.snap.new` files and no generated fixture are committed. The fixtures live under `target/bench-fixtures/`.

Other things I verified:
- **Load paths.** The materialisation test drives `Assembler::push_chunk`/`flush_held`/`finish` on the test thread. The real loader (`src/harness/claude/load.rs`, `Streamer::feed`) runs on a worker thread, where the thread-local counters would not see anything. Neither `load.rs` nor `assemble.rs` calls `ToolOutput::text` or `ByteSource::read`; they only read the transcript file itself. So the two paths are equivalent for this property.
- **Search.** The only other caller that materialises output at App level is `transcript_search::tool_text`. It runs only after `Msg::StartTranscriptSearch`, which needs a submitted query. A plain load never triggers it, as §9.2 allows ("materialised only when ... searched"). So the harness-level heap and materialisation measurements match what the app does on load.
- **Expanded output.** With a scratch probe outside the repo, I expanded the mixed fixture's unfolded large output past its truncation and then scrolled it. `perform(Cmd::Submit)` took 21 ms, the next draw 0.8 ms, and the worst of 400 scrolled frames inside the expanded 25 000-line output was 1.2 ms. The build notes' claim that frames after the expansion stay fast holds.

## Findings
1. [nit] `docs/performance.md` ("Measurements", the bullet "The numbers move between runs ... the margins are 10x or more either way"). This is wrong for the frame budget: 16 ms / 3.6 ms is about 4.4x. The dense full parse margin is also below 10x: 2 s / 353 ms is about 5.7x, and my rerun took 519 ms, about 3.9x. Change: drop "10x or more" and state the actual ratios, or say "4x or more".
2. [nit] `docs/performance.md` (the same bullet and the measurement table). The only range of run-to-run variation given is for the mixed session. On my run the dense full parse was 466 to 519 ms against the recorded 325 to 353 ms. Change: add a range for the dense full parse, for example "325 to 520 ms over several runs", so the recorded numbers are not read as stable.
3. [nit] Expanding the large output past its truncation (the `... N more lines` row) blocks the UI thread for 21 to 34 ms. That is over the 16 ms frame budget, and the build notes and docs report it openly. I rate it a nit, not a must-fix, for these reasons:
   - The acceptance criterion defines the frame budget as the maximum over 100 scrolled frames, and that is met.
   - The time is spent once, in `perform(Cmd::Submit)` laying out about 25 000 lines, not in rendering (the draw after it takes 0.8 ms).
   - Every scrolled frame afterwards stays at or below 1.2 ms.

   The Approver may weigh this differently against §9.2's wording "each frame rendered in under 16 ms". Suggested change: none in this task. File a follow-up to lay out an expanded output incrementally, or only the rows near the viewport, if a stricter reading is wanted.
4. [nit] `src/bench_support/generate.rs`, `Generator::output_text`. The `while text.len() < size` loop appends a whole line before it checks, so the text can be up to one line (about 100 bytes) longer than `size`. If `between(LARGE_MIN, LARGE_MAX)` draws a size within one line of `LARGE_MAX`, the large output exceeds `LARGE_MAX`. That breaks the `assert!(size <= LARGE_MAX)` in `src/bench_support/tests.rs` (`every_line_is_json_and_the_mix_is_all_there`) for some seed. It does not happen with the fixed seed, but it is a latent flake. Change: after the loop, truncate to `size` at a char boundary (for example `text.truncate(text.floor_char_boundary(size))`, or walk back with `is_char_boundary`) before `trim_end`. Alternatively, draw `large_size` from `LARGE_MIN..=LARGE_MAX - 256`.
5. [nit] `docs/performance.md` sits in a new `docs/` directory, while the repo's other documentation is in `doc/`. The task names `docs/`, so the builder followed it correctly. Change (optional, later): move it to `doc/performance.md` and update the references in `tests/budgets.rs`, `.github/workflows/ci.yml` and the module docs, if the project wants a single docs directory.
6. [nit] `src/bench_support/mod.rs`, `cached_fixture`. A cached file counts as valid if the `complete` marker exists and the file is at least `bytes` long. The cache key includes `GENERATOR_VERSION`, so this relies on someone bumping it after any change to the generator, and the code comment already says so. Two processes that use the cache at once can also race: one may run `remove_dir_all(&root)` while the other reads the file. Change (optional): document in the `cached_fixture` doc comment that concurrent first use is not supported. Even better, generate into a temporary sibling directory and `rename` it into place, so a half-written fixture is never seen.
