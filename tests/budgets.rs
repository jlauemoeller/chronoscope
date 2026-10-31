//! The §9.2 performance budgets on a synthetic 100 MB Claude session.
//!
//! Ignored by default because the numbers depend on the machine. Run it in
//! release mode (see `docs/performance.md`):
//!
//! ```text
//! cargo test --release -- --ignored budgets --nocapture
//! ```
//!
//! The test checks two sessions of 100 MiB, one after the other (timings
//! must not compete with each other, so it is one test): the benchmark
//! fixture, with the planner's mix (about 5% of the tool results large), and
//! a dense one of many small entries and no large output, which is the worst
//! case for the per-line work.

use std::time::{Duration, Instant};

use chronoscope::bench_support::{
    CountingAlloc, FIXTURE_BYTES, Fixture, FrameScenario, LARGE_PERCENT, SEED, cached_fixture,
    fixture_cache_dir, load_transcript, run_load,
};
use chronoscope::components::Transcript;
use chronoscope::components::turn_block::{SectionId, SectionKey};
use chronoscope::model::{Item, Turn};
use ratatui::layout::Size;
use tuirealm::command::Cmd;
use tuirealm::component::Component;

#[global_allocator]
static ALLOC: CountingAlloc = CountingAlloc::new();

const FIRST_CONTENT: Duration = Duration::from_millis(300);
const FULL_PARSE: Duration = Duration::from_secs(2);
const FRAME: Duration = Duration::from_millis(16);
const MEMORY_FACTOR: u64 = 3;
const RUNS: usize = 5;
const FRAMES: usize = 100;
const SIZE: Size = Size::new(200, 50);

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn all_ms(durations: &[Duration]) -> String {
    let each: Vec<String> = durations.iter().map(|d| format!("{:.1}", ms(*d))).collect();
    each.join(", ")
}

/// The worst time to unfold a large output (and lay its block out again).
/// Reported, not asserted: it is one keypress, not a frame of a scroll.
fn worst_unfold(transcript: &mut Transcript) -> Option<(usize, Duration)> {
    let mut worst: Option<(usize, Duration)> = None;
    for index in 0..transcript.turn_count() {
        let turn = transcript.turn(index)?.clone();
        let Turn::Assistant(assistant) = turn.as_ref() else {
            continue;
        };
        for item in &assistant.items {
            let Item::ToolCall(call) = item else { continue };
            let Some(result) = call.result.as_ref().filter(|r| r.output.is_lazy()) else {
                continue;
            };
            let id = SectionId {
                turn: assistant.id.clone(),
                key: SectionKey::Item(call.id.clone()),
            };
            // Only outputs that are folded now are unfolded for the
            // measurement, and folded again after it.
            if transcript.is_folded(&id) != Some(true) {
                continue;
            }
            transcript.select(&id);
            let start = Instant::now();
            transcript.set_folded(&id, false);
            let took = start.elapsed();
            transcript.set_folded(&id, true);
            if worst.is_none_or(|(_, w)| took > w) {
                worst = Some((result.output.byte_len(), took));
            }
        }
    }
    worst
}

/// The time `Enter` takes on the `... N more lines` row of the scenario's
/// unfolded large output, which lays out all of its lines. Reported, not
/// asserted, like [`worst_unfold`].
fn expand_large_output(scenario: &mut FrameScenario) -> Option<Duration> {
    let more = scenario.large_section.as_ref()?.more_row()?;
    if !scenario.transcript.select(&more) {
        return None;
    }
    let start = Instant::now();
    scenario.transcript.perform(Cmd::Submit);
    Some(start.elapsed())
}

/// Measures `fixture` against the four budgets and returns the failures.
fn check(label: &str, fixture: &Fixture) -> Vec<String> {
    let session = &fixture.session;
    println!("== {label}");
    println!(
        "fixture: {} bytes, {} lines, {} prompts, {} tool calls, {} large outputs",
        session.bytes, session.lines, session.prompts, session.tool_calls, session.large_outputs
    );
    let mut failures = Vec::new();
    assert!(session.bytes >= FIXTURE_BYTES);

    // First content and the full parse: the loader alone, as the channel
    // delivers it.
    let harness = fixture.harness();
    let mut first = Vec::new();
    let mut loaded = Vec::new();
    for _ in 0..RUNS {
        let timings = run_load(&harness, &fixture.id(), |_| {});
        assert_eq!(timings.warnings, 0, "the session has unparsed entries");
        first.push(timings.first_content);
        loaded.push(timings.loaded);
    }
    let worst_first = *first.iter().max().unwrap();
    let worst_loaded = *loaded.iter().max().unwrap();
    println!(
        "first content, {RUNS} runs: [{}] ms, worst {:.1} ms (budget {:.0} ms)",
        all_ms(&first),
        ms(worst_first),
        ms(FIRST_CONTENT)
    );
    println!(
        "full parse, {RUNS} runs: [{}] ms, worst {:.1} ms (budget {:.0} ms)",
        all_ms(&loaded),
        ms(worst_loaded),
        ms(FULL_PARSE)
    );
    if worst_first >= FIRST_CONTENT {
        failures.push(format!("{label}: first content took {worst_first:?}"));
    }
    if worst_loaded >= FULL_PARSE {
        failures.push(format!("{label}: the full parse took {worst_loaded:?}"));
    }

    // Memory: the heap a full load into the transcript takes, above what the
    // process held before it, at its peak and when it is done.
    let before = ALLOC.stats().live;
    ALLOC.reset_peak();
    let (transcript, applied) = load_transcript(fixture, SIZE);
    let stats = ALLOC.stats();
    assert!(applied.turns >= session.prompts, "turns are missing");
    let peak = stats.peak.saturating_sub(before) as u64;
    let held = stats.live.saturating_sub(before) as u64;
    let mib = |bytes: u64| bytes as f64 / (1024.0 * 1024.0);
    let ratio = |bytes: u64| bytes as f64 / session.bytes as f64;
    println!(
        "memory: peak heap {:.1} MiB = {:.2}x the file, {:.1} MiB = {:.2}x held after the load \
         (budget {MEMORY_FACTOR}x)",
        mib(peak),
        ratio(peak),
        mib(held),
        ratio(held)
    );
    println!(
        "load into a transcript: first content {:.1} ms, loaded {:.1} ms, {} turns",
        ms(applied.first_content),
        ms(applied.loaded),
        applied.turns
    );
    if peak >= MEMORY_FACTOR * session.bytes {
        failures.push(format!(
            "{label}: peak heap {peak} B is not under {MEMORY_FACTOR}x the file ({} B)",
            session.bytes
        ));
    }

    // Frames: scroll down through the middle of the session, through a large
    // output that is unfolded.
    let mut scenario = FrameScenario::new(transcript, SIZE);
    if scenario.large_section.is_none() {
        println!("(no large output in this session: none unfolded)");
    }
    let frames: Vec<Duration> = (0..FRAMES).map(|_| scenario.scroll_and_draw()).collect();
    let worst_frame = *frames.iter().max().unwrap();
    let mut sorted = frames.clone();
    sorted.sort();
    println!(
        "frames, {FRAMES} scrolled at {}x{}: worst {:.2} ms, median {:.2} ms (budget {:.0} ms)",
        SIZE.width,
        SIZE.height,
        ms(worst_frame),
        ms(sorted[sorted.len() / 2]),
        ms(FRAME)
    );
    if worst_frame >= FRAME {
        failures.push(format!("{label}: the slowest frame took {worst_frame:?}"));
    }
    if let Some(took) = expand_large_output(&mut scenario) {
        println!(
            "reference: expanding the large output past its truncation (every line laid out) took {:.1} ms",
            ms(took)
        );
    }
    if let Some((bytes, took)) = worst_unfold(&mut scenario.transcript) {
        println!(
            "reference: unfolding the slowest large output ({bytes} bytes) took {:.1} ms",
            ms(took)
        );
    }
    failures
}

#[test]
#[ignore = "machine dependent: cargo test --release -- --ignored budgets --nocapture"]
fn budgets() {
    let cache = fixture_cache_dir();
    let mixed = cached_fixture(&cache, FIXTURE_BYTES, SEED, LARGE_PERCENT).expect("the fixture");
    let dense = cached_fixture(&cache, FIXTURE_BYTES, SEED, 0).expect("the dense fixture");
    let mut failures = check("mixed session (5% large outputs)", &mixed);
    failures.extend(check("dense session (small entries only)", &dense));
    assert!(failures.is_empty(), "budgets missed: {failures:#?}");
}
