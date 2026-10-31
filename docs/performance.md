# Performance

SPEC §9.2 sets four budgets for a session of about 100 MB: first content within
300 ms, a full parse under 2 s, every frame under 16 ms, and memory within
roughly 3x the file size. This page records what was measured against them and
how to measure again.

The design that makes this possible was in place before the budgets were
measured (streaming parse on a worker thread, tool outputs above 4 KiB kept as
a line count and a byte range, layout per block and lazily, only the blocks in
the viewport drawn). Measuring found nothing to fix: every budget is met with a
wide margin, so no hot path was changed for this task.

## The fixture

`chronoscope::bench_support` (`src/bench_support/`) generates a Claude-format
session of a requested size. It is deterministic: one seed and one size always
give the same bytes. The mix is human prompts (some of several lines),
assistant messages split over several lines the way Claude Code writes them,
readable and redacted thinking, `Bash`, `Read`, `Edit` and `Grep` calls with
results, `turn_duration` entries and injected attachments (opened file, hook
message) next to noise ones. About 5% of the tool results are large (100 KB to
2 MB, so they are stored lazily); the rest are 40 B to 3 KB.

Two 100 MiB (104 857 600 bytes) sessions are used:

| name  | large results | lines   | prompts | tool calls | large outputs | turns  |
|-------|---------------|---------|---------|------------|---------------|--------|
| mixed | 5%            | 5 975   | 272     | 1 581      | 84            | 544    |
| dense | none          | 104 041 | 5 000   | 27 400     | 0             | 10 000 |

The mixed session is the benchmark fixture: by count 5% of the results are
large, but those 84 outputs are 93% of its bytes (the JSON-escaped text). The dense one has the most
lines and turns per byte, which is the worst case for the per-line work of the
parser. The budgets are checked on both.

The files are written to `target/bench-fixtures/` on first use (a few seconds)
and reused after that. They are never committed.

## The machine

- AMD Ryzen AI 9 HX 370, 12 cores / 24 threads
- 54 GB RAM, Fedora Linux 44, Linux 7.2.13, btrfs on NVMe
- rustc 1.99.1, release profile (`cargo test --release`, `cargo bench`)
- page cache warm: the fixture was just written or read, so the numbers do not
  include reading the file from a cold disk

## Measurements

From `cargo test --release --test budgets -- --ignored budgets --nocapture`
(worst of five loads; frames: 100 wheel steps of three rows each through the
middle of the session at 200x50, each frame being the scroll and the draw into
a `TestBackend`):

| budget                | limit     | mixed         | dense          |
|-----------------------|-----------|---------------|----------------|
| first content         | < 300 ms  | 1.6 ms        | 1.7 ms         |
| full parse            | < 2 s     | 122 ms        | 348 ms         |
| slowest frame         | < 16 ms   | 2.3 ms        | 1.1 ms         |
| peak heap             | < 3x file | 0.21x (21 MiB)| 0.83x (83 MiB) |

More numbers from the same runs:

- Loading into a `Transcript` as the application does (every update applied
  as it arrives): loaded after 124 ms (mixed) and 331 ms (dense).
- Heap held once the load is done: 0.11x the file (mixed), 0.80x (dense). The
  mixed session is small because the lazy outputs stay on disk.
- Median frame: 0.40 ms (mixed), 0.54 ms (dense).
- Unfolding the slowest of the 84 large outputs (2 MB, which has to be read
  and parsed again) takes 7.8 ms. That is one key press, not part of a scroll,
  and it is also under the frame budget.
- Expanding a 2 MB output past `truncate_output_lines` (`Enter` on the `... N
  more lines` row) lays out all of its ~25 000 lines: about 30 ms once, then
  frames are as fast as before.

From `cargo bench` (criterion, mean of the samples):

| benchmark                      | time        |
|--------------------------------|-------------|
| `first_content`                | 0.71 ms     |
| `full_parse/mixed`             | 154 ms      |
| `full_parse/dense`             | 307 ms      |
| `frame_render/draw`            | 0.33 ms     |
| `frame_render/scroll_and_draw` | 0.48 ms     |
| `selector_index/cold`          | 5.9 ms      |
| `selector_index/warm`          | 2.5 ms      |

`selector_index` builds the session index over 2 000 small session files in ten
projects, with a new harness (the scan cache cold) or one that has already
scanned them (warm: nothing changed, so only the files are stat'ed). It has no
budget; it is there for reference.

Heap is measured with a counting global allocator
(`bench_support::CountingAlloc`) installed in the budget test binary. The peak
is taken over the whole load into a `Transcript`, above what the process held
before the load. It includes the worker's `Assembler` and the transcript, which
share the turns through `Arc`.

## Running them

Benchmarks (criterion; each prints its time and keeps a report under
`target/criterion/`):

```
cargo bench --bench first_content
cargo bench --bench full_parse
cargo bench --bench frame_render
cargo bench --bench selector_index
```

Pass criterion options after `--`, for example
`cargo bench --bench full_parse -- --sample-size 10`. Name the benches with
`--bench`: a plain `--benches` also runs the library's own test harness, which
does not understand criterion's options.

The budget check is an ignored test, because the numbers depend on the machine
and CI runners vary. Run it in release mode; `--nocapture` shows the numbers:

```
cargo test --release --test budgets -- --ignored budgets --nocapture
```

It fails when any of the four budgets is missed on either session, and lists
which. CI runs `cargo bench --no-run` so that the benchmarks keep compiling,
but not the benchmarks or the budget test themselves.

The generator and the instruments have their own tests, which run with the
normal `cargo test`: a 5 MB session parses with no unparsed entries, and a
folded large output is never read (neither by the load nor by drawing; the
test counts calls to `ToolOutput::text` and `ByteSource::read`).

## If a budget is missed

The first things to look at, from the cheapest fix up:

- per-line allocations in the assembler (`src/harness/claude/assemble.rs`,
  `wire.rs`): a borrowed parse instead of `String`s;
- batch sizes of the loader (`Tuning` in `src/harness/claude/load.rs`);
- a block drawn that is not in the viewport, or laid out more than once
  (`LayoutCache`, `Window`);
- markdown or syntax highlighting repeated per frame: cache the rendered
  lines per block, invalidated like the layout cache;
- a large output materialised while folded.
