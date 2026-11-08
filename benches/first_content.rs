//! Time from `load()` to the first batch of turns of a 100 MiB session
//! (budget: 300 ms, product §8.2).

use criterion::{Criterion, criterion_group, criterion_main};

use chronoscope::bench_support::{
    FIXTURE_BYTES, LARGE_PERCENT, SEED, cached_fixture, first_content, fixture_cache_dir,
};

fn bench(c: &mut Criterion) {
    let fixture = cached_fixture(&fixture_cache_dir(), FIXTURE_BYTES, SEED, LARGE_PERCENT)
        .expect("the fixture");
    let harness = fixture.harness();
    let id = fixture.id();
    c.bench_function("first_content", |b| {
        b.iter_custom(|iters| (0..iters).map(|_| first_content(&harness, &id)).sum());
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(20);
    targets = bench
}
criterion_main!(benches);
