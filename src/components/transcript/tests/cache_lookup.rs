//! The layout cache on its own, with synthetic block heights.

use std::cell::Cell;

use super::super::cache::{LayoutCache, Located};

/// A cache of `heights.len()` blocks and a measurer that reads `heights` and
/// counts its calls.
fn cache_of(heights: &[usize]) -> LayoutCache {
    let mut cache = LayoutCache::new();
    heights.iter().for_each(|_| cache.push());
    cache
}

#[test]
fn locate_finds_the_block_containing_a_row() {
    let heights = [3, 5, 2, 7];
    let mut cache = cache_of(&heights);
    let mut m = |i: usize| heights[i];
    let at = |block, top| Some(Located { block, top });
    assert_eq!(cache.locate(0, &mut m), at(0, 0));
    assert_eq!(cache.locate(2, &mut m), at(0, 0));
    assert_eq!(cache.locate(3, &mut m), at(1, 3));
    assert_eq!(cache.locate(7, &mut m), at(1, 3));
    assert_eq!(cache.locate(8, &mut m), at(2, 8));
    assert_eq!(cache.locate(9, &mut m), at(2, 8));
    assert_eq!(cache.locate(10, &mut m), at(3, 10));
    assert_eq!(cache.locate(16, &mut m), at(3, 10));
    assert_eq!(cache.locate(17, &mut m), None);
}

#[test]
fn locate_is_a_binary_search_over_known_prefix_sums() {
    let heights: Vec<usize> = (0..10_000).map(|i| 2 + i % 5).collect();
    let mut cache = cache_of(&heights);
    let calls = Cell::new(0);
    let mut m = |i: usize| {
        calls.set(calls.get() + 1);
        heights[i]
    };
    assert_eq!(cache.total(&mut m), heights.iter().sum::<usize>());
    assert_eq!(calls.get(), 10_000);
    // With every height known, any lookup measures nothing and agrees with a
    // scan.
    let mut start = 0;
    for (block, h) in heights.iter().enumerate().step_by(97) {
        let top: usize = heights[..block].iter().sum();
        start = start.max(top);
        for row in [top, top + h - 1] {
            let found = cache.locate(row, &mut m);
            assert_eq!(found, Some(Located { block, top }));
        }
    }
    assert_eq!(calls.get(), 10_000);
}

#[test]
fn locate_measures_only_the_blocks_it_has_to() {
    let heights = [4; 100];
    let mut cache = cache_of(&heights);
    let calls = Cell::new(0);
    let mut m = |i: usize| {
        calls.set(calls.get() + 1);
        heights[i]
    };
    cache.locate(9, &mut m);
    assert_eq!(calls.get(), 3, "rows 0..12 are blocks 0, 1 and 2");
    assert_eq!(cache.known_height(3), None);
    cache.locate(2, &mut m);
    assert_eq!(calls.get(), 3, "known heights are not measured again");
    assert_eq!(cache.computed(), 3);
}

#[test]
fn invalidating_one_block_measures_only_that_block_again() {
    let heights = [4, 6, 8];
    let mut cache = cache_of(&heights);
    let calls = Cell::new(0);
    let mut m = |i: usize| {
        calls.set(calls.get() + 1);
        heights[i]
    };
    assert_eq!(cache.total(&mut m), 18);
    cache.invalidate(1);
    assert_eq!(cache.total(&mut m), 18);
    assert_eq!(calls.get(), 4);
    assert_eq!(cache.top(2, &mut m), 10);
}

#[test]
fn set_width_invalidates_everything_only_when_it_changes() {
    let heights = [4, 6];
    let mut cache = cache_of(&heights);
    let mut m = |i: usize| heights[i];
    assert!(cache.set_width(80));
    cache.total(&mut m);
    assert!(!cache.set_width(80));
    assert_eq!(cache.known_height(0), Some(4));
    assert!(cache.set_width(60));
    assert_eq!(cache.known_height(0), None);
    assert_eq!(cache.known_height(1), None);
}

#[test]
fn covers_and_clamp_offset_stop_as_soon_as_they_know() {
    let heights = [5; 50];
    let mut cache = cache_of(&heights);
    let calls = Cell::new(0);
    let mut m = |i: usize| {
        calls.set(calls.get() + 1);
        heights[i]
    };
    assert!(cache.covers(0, &mut m));
    assert_eq!(calls.get(), 0);
    assert!(cache.covers(12, &mut m));
    assert_eq!(calls.get(), 3);
    assert_eq!(cache.clamp_offset(10, 10, &mut m), 10);
    assert_eq!(calls.get(), 4);
    assert_eq!(cache.clamp_offset(1000, 10, &mut m), 240);
    assert_eq!(cache.clamp_offset(245, 10, &mut m), 240);
    assert_eq!(cache.clamp_offset(240, 10, &mut m), 240);
    assert_eq!(cache.clamp_offset(241, 10, &mut m), 240);
}

#[test]
fn an_empty_cache_has_no_rows() {
    let mut cache = LayoutCache::new();
    let mut m = |_: usize| unreachable!();
    assert_eq!(cache.locate(0, &mut m), None);
    assert_eq!(cache.total(&mut m), 0);
    assert_eq!(cache.clamp_offset(5, 10, &mut m), 0);
    assert!(cache.is_empty());
}

#[test]
fn record_stores_a_height_measured_elsewhere() {
    let mut cache = cache_of(&[0, 0]);
    cache.record(1, 9);
    assert_eq!(cache.known_height(1), Some(9));
    assert_eq!(cache.computed(), 1);
    cache.record(1, 9);
    assert_eq!(cache.computed(), 1);
    let mut m = |_: usize| 4;
    assert_eq!(cache.total(&mut m), 13);
}
