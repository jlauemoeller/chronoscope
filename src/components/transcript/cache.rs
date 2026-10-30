//! The layout cache: the height of every block for the current width, with
//! prefix sums for the row to block lookup.
//!
//! Heights are computed lazily. A block is measured the first time something
//! needs to know where it starts or ends, and the prefix sums grow one block
//! at a time, so a lookup near the top of a long transcript never measures
//! the blocks below it. The lookup itself is a binary search over the prefix
//! sums, not a scan.

/// Computes the height of a block at the cache's width.
pub trait Measure {
    /// The number of rows block `index` takes.
    fn measure(&mut self, index: usize) -> usize;
}

impl<F: FnMut(usize) -> usize> Measure for F {
    fn measure(&mut self, index: usize) -> usize {
        self(index)
    }
}

/// Where a row of the transcript is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Located {
    /// The block the row falls in.
    pub block: usize,
    /// The transcript row at which the block starts.
    pub top: usize,
}

/// Block heights for one width, with prefix sums.
#[derive(Debug, Clone)]
pub struct LayoutCache {
    width: u16,
    /// `None` where the height is not known (new or invalidated blocks).
    heights: Vec<Option<usize>>,
    /// `prefix[k]` is the number of rows of the first `k` blocks. It is valid
    /// up to the first block whose height was unknown when it was extended,
    /// so its length is at most `heights.len() + 1`.
    prefix: Vec<usize>,
    /// How many heights have been computed so far.
    computed: usize,
}

impl Default for LayoutCache {
    fn default() -> Self {
        Self {
            width: 0,
            heights: Vec::new(),
            prefix: vec![0],
            computed: 0,
        }
    }
}

impl LayoutCache {
    /// An empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// The width the heights are for.
    pub fn width(&self) -> u16 {
        self.width
    }

    /// The number of blocks.
    pub fn len(&self) -> usize {
        self.heights.len()
    }

    /// Whether there are no blocks.
    pub fn is_empty(&self) -> bool {
        self.heights.is_empty()
    }

    /// How many block heights have been computed since the cache was made.
    /// A test uses it to see which operations recompute what.
    pub fn computed(&self) -> usize {
        self.computed
    }

    /// The height of block `index` if it is known.
    pub fn known_height(&self, index: usize) -> Option<usize> {
        self.heights.get(index).copied().flatten()
    }

    /// Adds a block at the end, with unknown height.
    pub fn push(&mut self) {
        self.heights.push(None);
    }

    /// Drops every block.
    pub fn clear(&mut self) {
        self.heights.clear();
        self.prefix.truncate(1);
    }

    /// Sets the width. A different width invalidates every height; returns
    /// whether it did.
    pub fn set_width(&mut self, width: u16) -> bool {
        if width == self.width {
            return false;
        }
        self.width = width;
        self.invalidate_all();
        true
    }

    /// Forgets the height of block `index` (its content changed).
    pub fn invalidate(&mut self, index: usize) {
        if let Some(slot) = self.heights.get_mut(index) {
            *slot = None;
            self.prefix.truncate(self.prefix.len().min(index + 1));
        }
    }

    /// Forgets every height.
    pub fn invalidate_all(&mut self) {
        self.heights.iter_mut().for_each(|h| *h = None);
        self.prefix.truncate(1);
    }

    /// Stores a height the caller measured itself while drawing.
    pub fn record(&mut self, index: usize, height: usize) {
        let Some(slot) = self.heights.get_mut(index) else {
            return;
        };
        if *slot == Some(height) {
            return;
        }
        if slot.is_none() {
            self.computed += 1;
        }
        *slot = Some(height);
        self.prefix.truncate(self.prefix.len().min(index + 1));
    }

    /// Grows the prefix sums block by block until `enough(blocks, rows)` is
    /// true for the blocks and rows covered so far, or all blocks are covered.
    /// Returns the rows covered.
    fn extend(
        &mut self,
        measure: &mut impl Measure,
        mut enough: impl FnMut(usize, usize) -> bool,
    ) -> usize {
        while self.prefix.len() <= self.heights.len() {
            let index = self.prefix.len() - 1;
            if enough(index, self.prefix[index]) {
                break;
            }
            let height = match self.heights[index] {
                Some(height) => height,
                None => {
                    let height = measure.measure(index);
                    self.heights[index] = Some(height);
                    self.computed += 1;
                    height
                }
            };
            self.prefix.push(self.prefix[index] + height);
        }
        self.prefix[self.prefix.len() - 1]
    }

    /// The row at which block `index` starts, or the total height when
    /// `index` is past the last block.
    pub fn top(&mut self, index: usize, measure: &mut impl Measure) -> usize {
        self.extend(measure, |blocks, _| blocks >= index);
        self.prefix[index.min(self.prefix.len() - 1)]
    }

    /// The height of block `index`, measuring it if needed (0 past the end).
    pub fn height(&mut self, index: usize, measure: &mut impl Measure) -> usize {
        self.extend(measure, |blocks, _| blocks > index);
        self.known_height(index).unwrap_or(0)
    }

    /// The block that contains `row`, found by binary search over the prefix
    /// sums. Blocks are measured only as far as needed to reach `row`.
    pub fn locate(&mut self, row: usize, measure: &mut impl Measure) -> Option<Located> {
        let reached = self.extend(measure, |_, rows| rows > row);
        if row >= reached {
            return None;
        }
        let covered = self.prefix.partition_point(|&start| start <= row);
        let block = covered - 1;
        Some(Located {
            block,
            top: self.prefix[block],
        })
    }

    /// Whether the blocks hold at least `rows` rows.
    pub fn covers(&mut self, rows: usize, measure: &mut impl Measure) -> bool {
        self.extend(measure, |_, reached| reached >= rows) >= rows
    }

    /// The total height of all blocks (measures every block).
    pub fn total(&mut self, measure: &mut impl Measure) -> usize {
        self.extend(measure, |_, _| false)
    }

    /// The greatest offset not above `offset` that leaves the viewport filled:
    /// the last row cannot scroll above the viewport bottom.
    pub fn clamp_offset(
        &mut self,
        offset: usize,
        viewport_height: usize,
        measure: &mut impl Measure,
    ) -> usize {
        if self.covers(offset + viewport_height, measure) {
            offset
        } else {
            self.total(measure).saturating_sub(viewport_height)
        }
    }
}
