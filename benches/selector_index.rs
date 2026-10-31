//! Building the session index over 2 000 small session files, with the scan
//! cache cold (a new harness) and warm (nothing changed since the last
//! build). For reference: it has no budget of its own.

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};

use chronoscope::bench_support::write_small_sessions;
use chronoscope::harness::claude::ClaudeHarness;

const SESSIONS: usize = 2_000;

fn bench(c: &mut Criterion) {
    let tmp = tempfile::tempdir().expect("a temp dir");
    let data_dir = tmp.path().join("projects");
    write_small_sessions(&data_dir, SESSIONS, 7).expect("the session files");
    let mut group = c.benchmark_group("selector_index");
    group.bench_function("cold", |b| {
        b.iter_batched(
            || ClaudeHarness::with_data_dir(data_dir.clone(), Default::default()),
            |harness| harness.refresh().expect("the index builds"),
            BatchSize::PerIteration,
        );
    });
    let warm = ClaudeHarness::with_data_dir(data_dir.clone(), Default::default());
    warm.refresh().expect("the index builds");
    group.bench_function("warm", |b| {
        b.iter(|| warm.refresh().expect("the index builds"));
    });
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
