# Performance

product §8.2 sets four budgets for a session of about 100 MB: first content within
300 ms, a full parse under 2 s, every frame under 16 ms, and memory within
roughly 3x the file size. This page records what was measured against them and
how to measure again.

The design that makes this possible was in place before the budgets were
measured (streaming parse on a worker thread, tool outputs above 4 KiB kept as
a line count and a byte range, layout per block and lazily, only the blocks in
the viewport drawn). Measuring found nothing to fix: every budget is met with a
wide margin, so no hot path was changed for that task. The figures below were
measured again after the second round of features (tool call durations, notices,
the details overlay and the turns sidebar), and the budgets are still met with a
wide margin.

## The fixture

`chronoscope::bench_support` (`src/bench_support.rs` and `src/bench_support/`)
generates a Claude-format session of a requested size. It is deterministic: one
seed and one size always give the same bytes. The mix is human prompts (some of
several lines), assistant messages split over several lines the way Claude Code
writes them, readable and redacted thinking, `Bash`, `Read`, `Edit` and `Grep`
calls with results, `turn_duration` entries and injected attachments (opened
file, hook message) next to noise ones. Every tool result line also carries a
`toolUseResult`, as Claude Code writes it: an object that repeats the output in
the fields of the tool (`stdout`, file content, matches) for a call that
succeeded, and the error message as a string for one that failed. The assembler
reads only `isAsync` from it, but has to get past all of it, and a large result
line therefore holds its output twice. About 5% of the tool results are large
(100 KB to 2 MB, so they are stored lazily); the rest are 40 B to 3 KB.

Two 100 MiB (104 857 600 bytes) sessions are used:

| name  | large results | lines  | prompts | tool calls | large outputs | turns |
|-------|---------------|--------|---------|------------|---------------|-------|
| mixed | 5%            | 3 289  | 156     | 867        | 48            | 312   |
| dense | none          | 71 601 | 3 428   | 18 892     | 0             | 6 856 |

The mixed session is the benchmark fixture: by count 5% of the results are
large, but those 48 lines are 95% of its bytes (the JSON-escaped text, twice).
The dense one has the most lines and turns per byte, which is the worst case for
the per-line work of the parser. The budgets are checked on both.

The `toolUseResult` field changed the fixtures: they hold fewer lines, prompts
and calls than before (the mixed session had 5 975 lines and 84 large outputs,
the dense one 104 041 lines), because each result line is longer. Numbers from
before that change cannot be compared with these.

The files are written to `target/bench-fixtures/` on first use (a few seconds)
and reused after that. They are never committed.

## The machine

- AMD Ryzen AI 9 HX 370, 12 cores / 24 threads
- 54 GB RAM, Fedora Linux 44, Linux 7.2.13, btrfs on NVMe
- rustc 1.99.1, release profile (`cargo test --release`, `cargo bench`)
- page cache warm: the fixture was just written or read, so the numbers do not
  include reading the file from a cold disk

## Measurements

From `cargo test --release -- --ignored budgets --nocapture`
(worst of five loads; frames: 100 wheel steps of three rows each through the
middle of the session at 200x50, each frame being the scroll and the draw into
a `TestBackend`):

| budget                | limit     | mixed         | dense          |
|-----------------------|-----------|---------------|----------------|
| first content         | < 300 ms  | 2.3 ms        | 1.6 ms         |
| full parse            | < 2 s     | 136 ms        | 297 ms         |
| slowest frame         | < 16 ms   | 3.1 ms        | 1.0 ms         |
| peak heap             | < 3x file | 0.22x (22 MiB)| 0.63x (63 MiB) |

More numbers from the same runs:

- Loading into a `Transcript` as the application does (every update applied
  as it arrives, the sidebar's rows built on the way): loaded after 137 ms
  (mixed, 312 turns) and 309 ms (dense, 6 856 turns).
- The numbers move between runs: over four runs the mixed full parse (worst of
  five loads) took 133 to 140 ms and its slowest frame 3.1 to 4.9 ms, the
  dense full parse 297 to 444 ms; the margins are 3x or more either way (the
  tightest is the slowest mixed frame, 4.9 ms against 16 ms).
- Heap held once the load is done: 0.09x the file (mixed), 0.61x (dense). The
  mixed session is small because the lazy outputs stay on disk.
- Median frame: 0.37 to 0.55 ms (mixed), 0.55 ms (dense).
- One turn of 100 000 rounds (70 MB): full parse 665 to 677 ms.
- Unfolding a large output (the output is read and parsed again) takes up to
  about 10 ms: the slowest unfold, made before anything is expanded, took
  10.3 ms (1.9 MB), and the others 0.2 to 7 ms when they were timed one by one.
  That is one key press, not part of a scroll, and it is under a frame.
- The reference line "unfolding the slowest large output" is printed after the
  budget test has expanded one large output (next bullet), and it reads 116 to
  173 ms (1.5 MB, 21 865 lines). That output is in the same turn as the
  expanded one, and unfolding invalidates the layout of the whole block, so
  the figure is the cost of laying out the already expanded output of about
  20 000 lines again, not the cost of unfolding. The budget test on the code
  from before the second round of features printed the same 116 ms for this
  fixture.
- Expanding the unfolded large output past `truncate_output_lines` (`Enter` on
  the `... N more lines` row) lays out all of its lines: 37 to 56 ms, once
  (23 ms on the previous fixture). This, and laying out a block that holds an
  expanded output again, are the interactions that take more than a frame. They
  are not scrolled frames, so it is outside the budget,
  and the frames after it are as fast as before. Both are printed by the budget
  test as "reference" lines.

Against the code from before the second round of features (measured on the
same fixtures, one run of the budget test each):

| budget (100 MiB) | before, mixed | now, mixed | before, dense | now, dense |
|------------------|---------------|------------|---------------|------------|
| full parse       | 96 ms         | 136 ms     | 264 ms        | 297 ms     |
| slowest frame    | 2.9 ms        | 3.1 ms     | 1.0 ms        | 1.0 ms     |

The full parse is 40 ms (mixed) and 33 ms (dense) slower. The second round
adds work per line: the head of `toolUseResult`, the references to the sources,
the per-message token usage and the stop reasons. It is still 15x (mixed) and 7x
(dense) under the budget. Frames did not change.

From `cargo bench` (criterion, mean of the samples):

| benchmark                      | time        |
|--------------------------------|-------------|
| `first_content`                | 1.09 ms     |
| `full_parse/mixed`             | 128 ms      |
| `full_parse/dense`             | 267 ms      |
| `frame_render/draw`            | 0.31 ms     |
| `frame_render/scroll_and_draw` | 0.61 ms     |
| `selector_index/cold`          | 6.0 ms      |
| `selector_index/warm`          | 2.6 ms      |
| `selector_index/stats_keys`    | 2.5 ms      |
| `session_stats/first`          | 526 ms      |
| `session_stats/recompute_one_file` | 6.6 ms  |
| `session_stats/key`            | 162 us      |

`selector_index` builds the session index over 2 000 small session files in ten
projects, with a new harness (the scan cache cold) or one that has already
scanned them (warm: nothing changed, so only the files are stat'ed).
`stats_keys` asks the same warm harness for the stats key of each of the 2 000
sessions, which the coordinator thread does on every rebuild: a stat of the
main file and a failed look for its `subagents` directory, about 1.3 us per
session. `selector_index` has no budget; it is there for reference.

Heap is measured with a counting global allocator
(`bench_support::CountingAlloc`) installed in the budget test binary. The peak
is taken over the whole load into a `Transcript`, above what the process held
before the load. It includes the worker's `Assembler` and the transcript, which
share the turns through `Arc`.

## Details sources (task 008)

The details overlay reads what Claude records about a section (permission
decision, `stop_reason`, cache writes, `gitBranch`, ...) from the transcript
line when it opens. Turns keep only a reference to the line for that: one
`ByteSource` per turn and, per section, a byte range (two for a tool call, one
for its call and one for its result), in a side table on the turn. The assembler
reads none of these fields, so a line with detail fields of any type or size
assembles as before (`tests/claude_details.rs`).

What it cost, measured on this machine when the references were added, against
`main` of the time with the same two fixtures (the table above is the whole
application as it is now; this one compares the change by itself):

| budget (100 MiB)  | main, mixed | now, mixed | main, dense | now, dense |
|-------------------|-------------|------------|-------------|------------|
| first content     | 2.1 ms      | 2.6 ms     | 1.8 ms      | 1.5 ms     |
| full parse        | 140 ms      | 141 ms     | 403 ms      | 297 ms     |
| slowest frame     | 4.8 ms      | 3.2 ms     | 1.7 ms      | 0.95 ms    |
| peak heap         | 22.2 MiB    | 22.4 MiB   | 58.0 MiB    | 62.7 MiB   |
| heap after load   | 9.2 MiB     | 9.4 MiB    | 55.6 MiB    | 60.0 MiB   |

The references add about 4 MiB (8%) to the dense session, which has 6 856 turns
and 18 892 tool calls (0.63x the file at peak, against the 3x budget), and
nothing visible to the mixed one. Parse time and frames are within the noise
of the run.

One case the fixtures do not reach is a single turn with very many items
(tool results do not end a turn, so a long unattended run is one). All the
references of a turn's sections go to one table, which is only ever appended
to while assembling, never searched. The budget test checks it with a turn of
100 000 rounds of text, tool call and result (70 MB): full parse 0.66 s. That
figure is `Assembler::from_memory` and leaves out the load path. Looking each
new item up in the table first made this quadratic (tens of seconds), so
`tests/claude_details.rs` also has a test that compares the time of 3 000 and
48 000 items.

Opening the overlay re-reads the section's line, and a tool call's result line.
The budget test prints a reference line for the worst case, the tool call with
the largest result (2 033 269 bytes): reading its details took 1.1 to 1.6 ms.
The dense session's largest result (3 084 bytes) took 0.1 ms. It runs on the
UI thread, like unfolding a large output, and only the details read is known
to be far under a frame (the unfold is up to 10 ms).

## The turns sidebar (task 010)

The turns sidebar (product §3.2) keeps one slot per turn and the list of turns that
have an entry, and derives only the sections of a turn (`derive_sections`),
never a layout or a render. The budget test builds its rows during the load, as
the application does, and draws the pane after each of 100 selection moves
(four runs):

| session | turns  | load incl. rows | slowest sidebar frame (30x43) |
|---------|--------|-----------------|-------------------------------|
| mixed   | 312    | 101 to 138 ms   | 0.07 to 0.10 ms               |
| dense   | 6 856  | 292 to 415 ms   | 0.06 to 0.07 ms               |

A pane draws only the rows it shows (`lines_built` in its tests), and a turn
update derives one entry.

### A key press in the sidebar (task 012)

A key in the focused sidebar selects an entry, and the transcript goes to the
entry's block (the scroll `]` / `[` give, through one entry point) and draws.
The budget test times the whole press: the pane's `on`, the transcript's
`GO_TO_TURN_ATTR`, and a frame of each, on a 200x50 screen (a 169x43
transcript beside the 30x43 pane). It presses `j` 100 times from the first
entry, which lays out one new block each time, and `k` 100 times from the last,
and checks the slowest press against the 16 ms frame budget (the slowest of
the 100 presses, over four runs; the median press takes 0.5 to 0.8 ms):

| session | entries | slowest `j` | slowest `k` |
|---------|---------|-------------|-------------|
| mixed   | 312     | 2.4 to 2.8 ms | 1.9 to 2.6 ms |
| dense   | 6 856   | 1.5 to 2.1 ms | 1.9 to 2.0 ms |

The first press far from the top would lay out every block above it; that is a
one-off, as it is for `G` and the page keys, and the test leaves it out by
starting at the ends of the list.

## Stats of a session with many subagents

A session's stats (product §4.1) include the tokens of its subagents, which are
read from the subagent transcripts (product §2.1). The largest local session has
320 subagent files, 207 MB in total. Parsing all of them takes about 0.3 s with
the files in the OS cache, so the first computation stays background work on
the stats pool. A running session of that kind would parse them all again on
every coalesced rebuild, for hours, so each file's usage is kept in memory,
keyed by (path, size, mtime), and a recomputation reads only the files that
changed (product §8.2). `cargo bench --bench session_stats` measures both.

The fixture (`bench_support::generate_subagent_fixture`) is one Claude session
with a main transcript of 4 MiB, made like the mixed session above (5% large
results), and 320 subagent files below `<session>/subagents/` that vary
between half and one and a half times the mean size. The files are generated
until each reaches its share of 207 MB, so they come out slightly larger: 215.9
MB in all. It is deterministic, is written to `target/bench-fixtures/` on first
use and reused, and the page cache is warm. The recomputation moves one
file's modification time, which leaves the cached fixture as it is and makes
the memo treat that file as changed, as an append would. The figures were
measured on the machine above (the same one, nothing else running), with
`cargo bench --bench session_stats -- --sample-size 10`:

| benchmark                          | mean    | range             |
|------------------------------------|---------|-------------------|
| `session_stats/first`              | 526 ms  | 523 to 531 ms     |
| `session_stats/recompute_one_file` | 6.6 ms  | 6.5 to 6.6 ms     |
| `session_stats/key`                | 162 us  | 162 to 163 us     |

- `first` is a fresh harness with an empty memo: the main transcript and all
  320 files are parsed.
- `recompute_one_file` is the main transcript and the one changed file, about
  80 times faster than the first computation. The other 319 files come from the
  memo.
- `key` is `stats_key` for that session: it lists the subagent files and stats
  them, and reads none.

The stats have no budget of their own. A rebuild runs the key for every session
on the coordinator thread (`selector_index/stats_keys` above), and the
computation itself runs on the stats pool.

## Running them

Benchmarks (criterion; each prints its time and keeps a report under
`target/criterion/`):

```
cargo bench --bench first_content
cargo bench --bench full_parse
cargo bench --bench frame_render
cargo bench --bench selector_index
cargo bench --bench session_stats
```

Pass criterion options after `--`, for example
`cargo bench --bench full_parse -- --sample-size 10`. Name the benches with
`--bench`: a plain `--benches` also runs the library's own test harness, which
does not understand criterion's options.

The budget check is an ignored test, because the numbers depend on the machine
and CI runners vary. Run it in release mode; `--nocapture` shows the numbers:

```
cargo test --release -- --ignored budgets --nocapture
```

It fails when any of the four budgets is missed on either session, when a
sidebar key press or frame takes 16 ms or more, or when the one endless turn
(70 MB, 100 000 rounds) takes 2 s or more to parse, and lists which. CI runs
`cargo bench --no-run` so that the benchmarks keep compiling, but not the
benchmarks or the budget test themselves.

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
