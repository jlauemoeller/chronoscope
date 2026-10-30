//! Jumps and page scrolling: turn blocks, pages, start and end.

use std::ops::Range;

use tuirealm::command::CmdResult;

use super::Transcript;
use super::nav::Step;
use crate::components::turn_block::{SectionId, SectionKind, derive_sections};

/// How far a page key scrolls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Amount {
    /// Half the viewport height, rounded down.
    Half,
    /// The viewport height.
    Full,
}

impl Transcript {
    /// Whether `id` is a section of system / injected content.
    fn is_system(&self, id: &SectionId) -> bool {
        let Some(&at) = self.content.index.get(&id.turn) else {
            return false;
        };
        derive_sections(&self.content.turns[at], true)
            .iter()
            .any(|s| s.id == *id && s.kind() == SectionKind::System)
    }

    /// The section a jump to block `index` selects: its first section that is
    /// not system content. A block with none is not a stop.
    fn turn_stop(&self, index: usize) -> Option<SectionId> {
        self.sections_of(index)
            .into_iter()
            .find(|id| !self.is_system(id))
    }

    /// Selects the first section of the next or previous User, Assistant or
    /// Compaction block, and scrolls so that the block's top border is the
    /// top row of the viewport, as far as the end of the transcript allows.
    /// System content never forms a block of its own.
    pub(super) fn jump_turn(&mut self, step: Step) -> CmdResult {
        let before = self.offset;
        let Some(cur) = self.cursor() else {
            return CmdResult::NoChange;
        };
        let turns = self.content.turns.len();
        let found = match step {
            Step::Next => (cur.block + 1..turns).find_map(|b| self.turn_stop(b).map(|id| (b, id))),
            Step::Previous => (0..cur.block)
                .rev()
                .find_map(|b| self.turn_stop(b).map(|id| (b, id))),
        };
        let Some((block, id)) = found else {
            return CmdResult::NoChange;
        };
        self.content.selected = Some(id);
        if self.viewport.is_some() {
            self.offset = self.measured(|cache, m| cache.top(block, m));
        }
        self.refresh();
        self.outcome(before)
    }

    /// The selectable sections overlapping the viewport with their rows in
    /// the transcript.
    fn visible_sections(&self) -> Vec<(SectionId, Range<usize>)> {
        let Some(window) = &self.window else {
            return Vec::new();
        };
        window
            .blocks
            .iter()
            .flat_map(|placed| {
                placed
                    .layout
                    .section_rows()
                    .iter()
                    .map(|(id, rows)| (id.clone(), placed.top + rows.start..placed.top + rows.end))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// After scrolling: if the selected section is not fully visible any
    /// more, selects the topmost fully visible one; if none is fully visible,
    /// the one at the top row. Returns whether the selection changed.
    fn select_in_view(&mut self) -> bool {
        let Some(size) = self.viewport else {
            return false;
        };
        let (top, bottom) = (self.offset, self.offset + usize::from(size.height));
        let spans = self.visible_sections();
        let fully = |rows: &Range<usize>| rows.start >= top && rows.end <= bottom;
        if let Some(selected) = &self.content.selected
            && spans.iter().any(|(id, rows)| id == selected && fully(rows))
        {
            return false;
        }
        let target = spans
            .iter()
            .find(|(_, rows)| fully(rows))
            .or_else(|| spans.iter().find(|(_, rows)| rows.end > top))
            .map(|(id, _)| id.clone());
        match target {
            Some(id) if self.content.selected.as_ref() != Some(&id) => {
                self.content.selected = Some(id);
                true
            }
            _ => false,
        }
    }

    /// Scrolls half a page or a page, clamped to the transcript. When that
    /// moves the view, a selection that left the viewport moves to the
    /// topmost fully visible section.
    pub(super) fn scroll_pages(&mut self, step: Step, amount: Amount) -> CmdResult {
        let Some(size) = self.viewport else {
            return CmdResult::NoChange;
        };
        let height = usize::from(size.height);
        let rows = match amount {
            Amount::Half => height / 2,
            Amount::Full => height,
        };
        let before = self.offset;
        self.offset = match step {
            Step::Next => before.saturating_add(rows),
            Step::Previous => before.saturating_sub(rows),
        };
        self.refresh_scrolled();
        if self.offset == before {
            return CmdResult::NoChange;
        }
        if self.select_in_view() {
            self.refresh();
        }
        self.outcome(before)
    }

    /// Selects the first section and scrolls to the start.
    pub(super) fn jump_to_start(&mut self) -> CmdResult {
        let before = self.offset;
        let first =
            (0..self.content.turns.len()).find_map(|i| self.sections_of(i).into_iter().next());
        self.jump_to(first, 0, before)
    }

    /// Selects the last section and scrolls to the end.
    pub(super) fn jump_to_end(&mut self) -> CmdResult {
        let before = self.offset;
        let last = (0..self.content.turns.len())
            .rev()
            .find_map(|i| self.sections_of(i).pop());
        let end = match self.viewport {
            Some(size) => {
                let height = usize::from(size.height);
                self.measured(|cache, m| cache.total(m))
                    .saturating_sub(height)
            }
            None => 0,
        };
        self.jump_to(last, end, before)
    }

    fn jump_to(&mut self, section: Option<SectionId>, offset: usize, before: usize) -> CmdResult {
        let Some(section) = section else {
            return CmdResult::NoChange;
        };
        let moved = self.content.selected.as_ref() != Some(&section);
        self.content.selected = Some(section);
        if self.viewport.is_some() {
            self.offset = offset;
        }
        self.refresh();
        if !moved && self.offset == before {
            CmdResult::NoChange
        } else {
            self.outcome(before)
        }
    }
}
