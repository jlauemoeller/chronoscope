//! A frame of the transcript of a loaded 100 MiB session at 200x50, with the
//! viewport in the middle and some sections unfolded, one of them a large
//! output (budget: 16 ms per frame, SPEC §9.2).

use criterion::{Criterion, criterion_group, criterion_main};
use ratatui::layout::Size;

use chronoscope::bench_support::{
    FIXTURE_BYTES, FrameScenario, LARGE_PERCENT, SEED, cached_fixture, fixture_cache_dir,
    load_transcript,
};

fn bench(c: &mut Criterion) {
    let size = Size::new(200, 50);
    let fixture = cached_fixture(&fixture_cache_dir(), FIXTURE_BYTES, SEED, LARGE_PERCENT)
        .expect("the fixture");
    let (transcript, _) = load_transcript(&fixture, size);
    let mut scenario = FrameScenario::new(transcript, size);
    let mut group = c.benchmark_group("frame_render");
    group.bench_function("draw", |b| {
        b.iter_custom(|iters| (0..iters).map(|_| scenario.draw_once()).sum());
    });
    group.bench_function("scroll_and_draw", |b| {
        b.iter_custom(|iters| {
            let mut total = std::time::Duration::ZERO;
            for i in 0..iters {
                // Start over every 100 frames, so the scroll never reaches
                // the end of the session.
                if i % 100 == 0 {
                    scenario.rewind();
                }
                total += scenario.scroll_and_draw();
            }
            total
        });
    });
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
