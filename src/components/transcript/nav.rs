//! Scrolling, moving the selection and folding: the basic `Cmd`s.

use std::ops::Range;

use tuirealm::command::CmdResult;

use super::Transcript;
use crate::components::turn_block::{SectionId, derive_sections};

/// Which way the selection moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Next,
    Previous,
}

/// The selected section within its block.
struct Cursor {
    block: usize,
    /// The selectable sections of the block with their rows (counted from
    /// the block's top border).
    rows: Vec<(SectionId, Range<usize>)>,
    pos: usize,
}

impl Cursor {
    fn id(&self) -> &SectionId {
        &self.rows[self.pos].0
    }
}

/// How to scroll a section taller than the viewport into view.
#[derive(Debug, Clone, Copy)]
enum Reveal {
    /// Arrived moving down: show its top.
    Top,
    /// Arrived moving up: show its bottom.
    Bottom,
    /// Not moving: leave it where it is if its top is visible.
    InPlace,
}

/// The offset that brings `span` into the viewport with the least scrolling.
fn reveal(offset: usize, height: usize, span: &Range<usize>, tall: Reveal) -> usize {
    if span.len() > height {
        return match tall {
            Reveal::Top => span.start,
            Reveal::Bottom => span.end - height,
            Reveal::InPlace if (offset..offset + height).contains(&span.start) => offset,
            Reveal::InPlace => span.start,
        };
    }
    if span.start < offset {
        span.start
    } else if span.end > offset + height {
        span.end - height
    } else {
        offset
    }
}

impl Transcript {
    fn outcome(&self, before: usize) -> CmdResult {
        if self.offset == before {
            CmdResult::Visual
        } else {
            CmdResult::Changed(tuirealm::component::Component::state(self))
        }
    }

    /// Scrolls one row down, not past the point where the last row is at the
    /// bottom of the viewport.
    pub(super) fn scroll_down(&mut self) -> CmdResult {
        self.scroll_to(self.offset.saturating_add(1))
    }

    /// Scrolls one row up, not above the first row.
    pub(super) fn scroll_up(&mut self) -> CmdResult {
        self.scroll_to(self.offset.saturating_sub(1))
    }

    /// Scrolls so that `offset` is the top row, clamped so that the last row
    /// cannot scroll above the bottom of the viewport. Does nothing before a
    /// viewport is announced.
    pub fn scroll_to(&mut self, offset: usize) -> CmdResult {
        if self.viewport.is_none() {
            return CmdResult::NoChange;
        }
        let before = self.offset;
        self.offset = offset;
        self.refresh();
        if self.offset == before {
            CmdResult::NoChange
        } else {
            self.outcome(before)
        }
    }

    fn cursor(&self) -> Option<Cursor> {
        let selected = self.content.selected.as_ref()?;
        let &block = self.content.index.get(&selected.turn)?;
        self.cursor_in(block, |rows| rows.iter().position(|(id, _)| id == selected))
    }

    fn cursor_in(
        &self,
        block: usize,
        pick: impl FnOnce(&[(SectionId, Range<usize>)]) -> Option<usize>,
    ) -> Option<Cursor> {
        let turn = &self.content.turns[block];
        let rows = self.content.section_rows(turn, self.nav_width());
        let pos = pick(&rows)?;
        Some(Cursor { block, rows, pos })
    }

    /// The section after or before the cursor, across blocks.
    fn neighbour(&self, cur: &Cursor, step: Step) -> Option<Cursor> {
        let turns = self.content.turns.len();
        match step {
            Step::Next if cur.pos + 1 < cur.rows.len() => Some(self.at(cur, cur.pos + 1)),
            Step::Previous if cur.pos > 0 => Some(self.at(cur, cur.pos - 1)),
            Step::Next => (cur.block + 1..turns)
                .find_map(|b| self.cursor_in(b, |rows| (!rows.is_empty()).then_some(0))),
            Step::Previous => (0..cur.block)
                .rev()
                .find_map(|b| self.cursor_in(b, |rows| rows.len().checked_sub(1))),
        }
    }

    fn at(&self, cur: &Cursor, pos: usize) -> Cursor {
        Cursor {
            block: cur.block,
            rows: cur.rows.clone(),
            pos,
        }
    }

    /// The transcript rows of the cursor's section.
    fn section_rows_abs(&mut self, cur: &Cursor) -> Range<usize> {
        let top = self.measured(|cache, m| cache.top(cur.block, m));
        let rows = &cur.rows[cur.pos].1;
        top + rows.start..top + rows.end
    }

    /// The rows to bring into view for the cursor's section: the section,
    /// plus the block's title row if it is the first and its bottom border
    /// if it is the last, as long as that still fits in the viewport.
    fn span(&mut self, cur: &Cursor, height: usize) -> Range<usize> {
        let section = self.section_rows_abs(cur);
        let (top, block_height) =
            self.measured(|cache, m| (cache.top(cur.block, m), cache.height(cur.block, m)));
        let start = if cur.pos == 0 { top } else { section.start };
        let end = if cur.pos + 1 == cur.rows.len() {
            top + block_height
        } else {
            section.end
        };
        if end - start <= height {
            start..end
        } else {
            section
        }
    }

    /// Selects the next or previous section and scrolls it into view. A
    /// selected section taller than the viewport is first scrolled through a
    /// page at a time (SPEC §6.3).
    pub(super) fn move_selection(&mut self, step: Step) -> CmdResult {
        let before = self.offset;
        let Some(cur) = self.cursor() else {
            self.ensure_selection();
            self.refresh();
            return CmdResult::NoChange;
        };
        let height = self.viewport.map(|v| usize::from(v.height));
        if let Some(offset) = height.and_then(|h| self.page_through(&cur, step, h)) {
            return self.scroll_to(offset);
        }
        let Some(next) = self.neighbour(&cur, step) else {
            return CmdResult::NoChange;
        };
        self.content.selected = Some(next.id().clone());
        if let Some(height) = height {
            let tall = if step == Step::Next {
                Reveal::Top
            } else {
                Reveal::Bottom
            };
            let span = self.span(&next, height);
            self.offset = reveal(self.offset, height, &span, tall);
        }
        self.refresh();
        self.outcome(before)
    }

    /// The offset one page further through a selected section that is taller
    /// than the viewport and extends past its edge in the direction of
    /// `step`; `None` when the selection should move instead.
    fn page_through(&mut self, cur: &Cursor, step: Step, height: usize) -> Option<usize> {
        let section = self.section_rows_abs(cur);
        if section.len() <= height {
            return None;
        }
        let offset = self.offset;
        match step {
            Step::Next if section.end > offset + height => {
                Some((offset + height).min(section.end - height))
            }
            Step::Previous if section.start < offset => {
                Some(offset.saturating_sub(height).max(section.start))
            }
            _ => None,
        }
    }

    /// Selects `id` if it is a selectable section, and scrolls it into view.
    /// Returns whether it was selected.
    pub fn select(&mut self, id: &SectionId) -> bool {
        let Some(&block) = self.content.index.get(&id.turn) else {
            return false;
        };
        let Some(cur) = self.cursor_in(block, |rows| rows.iter().position(|(i, _)| i == id)) else {
            return false;
        };
        self.content.selected = Some(id.clone());
        if let Some(size) = self.viewport {
            let height = usize::from(size.height);
            let span = self.span(&cur, height);
            self.offset = reveal(self.offset, height, &span, Reveal::Top);
        }
        self.refresh();
        true
    }

    /// Folds or unfolds a section, measuring only its block again. Sections
    /// that cannot fold ignore it.
    pub fn set_folded(&mut self, id: &SectionId, folded: bool) {
        let Some(&block) = self.content.index.get(&id.turn) else {
            return;
        };
        if self.is_folded(id).is_none() {
            return;
        }
        self.content.folds.set_folded(id.clone(), folded);
        self.cache.invalidate(block);
        self.refresh();
    }

    /// Whether the section is folded; `None` if it cannot be folded or does
    /// not exist.
    pub fn is_folded(&self, id: &SectionId) -> Option<bool> {
        let &at = self.content.index.get(&id.turn)?;
        derive_sections(&self.content.turns[at], self.content.show_system)
            .iter()
            .find(|section| section.id == *id && section.kind().foldable())
            .map(|section| self.content.folds.is_folded(section))
    }

    /// Folds or unfolds the selected section. Only that block is measured
    /// again.
    pub(super) fn toggle_fold(&mut self) -> CmdResult {
        let before = self.offset;
        let Some(cur) = self.cursor() else {
            return CmdResult::NoChange;
        };
        let id = cur.id().clone();
        let Some(folded) = self.is_folded(&id) else {
            return CmdResult::NoChange;
        };
        self.content.folds.set_folded(id, !folded);
        self.cache.invalidate(cur.block);
        if let (Some(size), Some(cur)) = (self.viewport, self.cursor()) {
            let height = usize::from(size.height);
            let span = self.span(&cur, height);
            self.offset = reveal(self.offset, height, &span, Reveal::InPlace);
        }
        self.refresh();
        self.outcome(before)
    }
}
