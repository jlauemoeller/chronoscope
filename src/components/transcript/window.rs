//! The viewport slice: which blocks overlap the visible rows, and drawing
//! them clipped.

use std::collections::HashMap;

use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};

use super::cache::LayoutCache;
use super::content::{Content, Measurer};
use crate::components::turn_block::BlockLayout;

/// A block that overlaps the viewport.
#[derive(Debug, Clone)]
pub struct Placed {
    /// The block's position among the turns.
    pub index: usize,
    /// The transcript row at which the block starts.
    pub top: usize,
    /// The block laid out for the viewport width.
    pub layout: BlockLayout,
}

/// The blocks overlapping `offset..offset + size.height`.
#[derive(Debug, Clone)]
pub struct Window {
    pub size: Size,
    pub offset: usize,
    pub blocks: Vec<Placed>,
}

impl Window {
    /// Like [`build`](Self::build), but moves `offset` back first if it would
    /// scroll the last row above the bottom of the viewport. The blocks are
    /// laid out once when the offset is valid, which is nearly always.
    pub fn build_clamped(
        cache: &mut LayoutCache,
        measurer: &mut Measurer<'_>,
        size: Size,
        offset: usize,
    ) -> Self {
        let window = Self::build(cache, measurer, size, offset);
        let clamped = cache.clamp_offset(offset, usize::from(size.height), measurer);
        if clamped == offset {
            window
        } else {
            Self::build(cache, measurer, size, clamped)
        }
    }

    /// Finds the blocks that overlap the rows from `offset` on and lays out
    /// each of them. The cache must be for `size.width`. Blocks above the
    /// viewport are measured by the lookup only when their heights are
    /// unknown; blocks below it are never touched.
    pub fn build(
        cache: &mut LayoutCache,
        measurer: &mut Measurer<'_>,
        size: Size,
        offset: usize,
    ) -> Self {
        let mut blocks = Vec::new();
        let bottom = offset + usize::from(size.height);
        if let Some(found) = cache.locate(offset, measurer) {
            let mut top = found.top;
            let mut index = found.block;
            while top < bottom && index < cache.len() {
                let layout = measurer.layout(index);
                cache.record(index, layout.height());
                let next = top + layout.height();
                blocks.push(Placed { index, top, layout });
                top = next;
                index += 1;
            }
        }
        Self {
            size,
            offset,
            blocks,
        }
    }

    /// The layouts of the window's blocks by index, to reuse in the next one.
    pub fn into_layouts(self) -> HashMap<usize, BlockLayout> {
        self.blocks
            .into_iter()
            .map(|placed| (placed.index, placed.layout))
            .collect()
    }

    /// The indices of the blocks in the window.
    pub fn block_range(&self) -> std::ops::Range<usize> {
        match (self.blocks.first(), self.blocks.last()) {
            (Some(first), Some(last)) => first.index..last.index + 1,
            _ => 0..0,
        }
    }

    /// Where the blocks go inside `area`: for each, the index into
    /// `blocks`, the rows to skip at its top and the rectangle to draw in.
    /// Rows that fall outside the area are clipped away.
    pub fn placements(&self, area: Rect) -> Vec<(usize, usize, Rect)> {
        let mut out = Vec::new();
        for (n, placed) in self.blocks.iter().enumerate() {
            let skip = self.offset.saturating_sub(placed.top);
            let y = placed.top.saturating_sub(self.offset);
            if y >= usize::from(area.height) {
                break;
            }
            let rows = (placed.layout.height() - skip).min(usize::from(area.height) - y);
            // `y` and `rows` are at most `area.height`, so they fit in a `u16`.
            let rect = Rect::new(area.x, area.y + y as u16, area.width, rows as u16);
            out.push((n, skip, rect));
        }
        out
    }

    /// Draws the blocks into `area`.
    pub fn render(&self, content: &Content, area: Rect, buf: &mut Buffer) {
        for (n, skip, rect) in self.placements(area) {
            let placed = &self.blocks[n];
            content
                .block(placed.index)
                .skip_rows(skip)
                .render_layout(&placed.layout, rect, buf);
        }
    }
}
