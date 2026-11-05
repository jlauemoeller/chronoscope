//! The stats of a session with 320 subagent files of about 207 MB in total
//! (the largest local session, SPEC cycle 3 §3.5): the first computation,
//! which parses the main transcript and every subagent file, and the
//! recomputation after one subagent file changed, which parses the main
//! transcript and that file only. For reference: it has no budget of its own;
//! `doc/performance.md` records the figures.
//!
//! The recomputation changes a file by moving its modification time, which
//! leaves the cached fixture as it is and makes the memo treat the file as
//! changed, as an append would.

use std::fs::File;
use std::time::{Duration, SystemTime};

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};

use chronoscope::bench_support::{
    SEED, SUBAGENT_BYTES, SUBAGENT_FILES, SUBAGENT_MAIN_BYTES, cached_subagent_fixture,
    fixture_cache_dir,
};
use chronoscope::harness::Harness;
use chronoscope::harness::claude::ClaudeHarness;
use chronoscope::model::SessionSummary;

/// A harness over the fixture that has built its index, and the session.
fn harness_and_session(data_dir: &std::path::Path) -> (ClaudeHarness, SessionSummary) {
    let harness = ClaudeHarness::with_data_dir(data_dir.to_path_buf(), Default::default());
    harness.refresh().expect("the index builds");
    let project = harness.projects().expect("projects").remove(0);
    let session = harness.sessions(&project.id).expect("sessions").remove(0);
    (harness, session)
}

fn bench(c: &mut Criterion) {
    let fixture = cached_subagent_fixture(
        &fixture_cache_dir(),
        SUBAGENT_MAIN_BYTES,
        SUBAGENT_FILES,
        SUBAGENT_BYTES,
        SEED,
    )
    .expect("the fixture");
    eprintln!(
        "session_stats: {} subagent files, {:.1} MB",
        fixture.files.len(),
        fixture.bytes as f64 / 1e6
    );
    let data_dir = fixture.fixture.data_dir.clone();
    let mut group = c.benchmark_group("session_stats");

    group.bench_function("first", |b| {
        b.iter_batched(
            || harness_and_session(&data_dir),
            |(harness, session)| harness.stats(&session).expect("the stats"),
            BatchSize::PerIteration,
        );
    });

    let (harness, session) = harness_and_session(&data_dir);
    let first = harness.stats(&session).expect("the stats");
    let changed = fixture.files[SUBAGENT_FILES / 2].clone();
    let mut bump = 0_u64;
    group.bench_function("recompute_one_file", |b| {
        b.iter_batched(
            || {
                bump += 1;
                File::options()
                    .write(true)
                    .open(&changed)
                    .and_then(|f| f.set_modified(SystemTime::now() + Duration::from_secs(bump)))
                    .expect("the modification time moves");
            },
            |()| harness.stats(&session).expect("the stats"),
            BatchSize::PerIteration,
        );
    });
    assert_eq!(harness.stats(&session).expect("the stats"), first);

    group.bench_function("key", |b| b.iter(|| harness.stats_key(&session)));
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = bench
}
criterion_main!(benches);
