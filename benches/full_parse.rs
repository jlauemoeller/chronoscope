//! Time from `load()` to `Loaded` for a 100 MiB session (budget: 2 s,
//! product §8.2).

use criterion::{Criterion, criterion_group, criterion_main};

use chronoscope::bench_support::{
    FIXTURE_BYTES, LARGE_PERCENT, SEED, cached_fixture, fixture_cache_dir, run_load,
};

fn bench(c: &mut Criterion) {
    let cache = fixture_cache_dir();
    let mut group = c.benchmark_group("full_parse");
    for (name, large_percent) in [("mixed", LARGE_PERCENT), ("dense", 0)] {
        let fixture =
            cached_fixture(&cache, FIXTURE_BYTES, SEED, large_percent).expect("the fixture");
        let harness = fixture.harness();
        let id = fixture.id();
        group.bench_function(name, |b| {
            b.iter_custom(|iters| {
                (0..iters)
                    .map(|_| run_load(&harness, &id, |_| {}).loaded)
                    .sum()
            });
        });
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = bench
}
criterion_main!(benches);
