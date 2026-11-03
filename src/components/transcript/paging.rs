//! Jumps and page scrolling: turn blocks, pages, start and end.

use std::ops::Range;

use tuirealm::command::CmdResult;

use super::Transcript;
use super::nav::{Reveal, Step, reveal};
use crate::components::turn_block::{SectionId, SectionKind, derive_sections};
use crate::model::TurnId;

/// How far a page key scrolls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Amount {
    /// Half the viewport height, rounded down (at least one row).
    Half,
    /// The viewport height.
    Full,
    /// This many rows (a step of the mouse wheel), at least one.
    Rows(usize),
}

impl Transcript {
    /// The section a jump to block `index` selects: its first section that is
    /// not system content. A block with none is not a stop.
    fn turn_stop(&self, index: usize) -> Option<SectionId> {
        let system: Vec<SectionId> = derive_sections(&self.content.turns[index], true)
            .into_iter()
            .filter(|s| s.kind() == SectionKind::System)
            .map(|s| s.id)
            .collect();
        self.sections_of(index)
            .into_iter()
            .find(|id| !system.contains(id))
    }

    /// Selects the first section of the next or previous User, Assistant or
    /// Compaction block and scrolls to the block (see
    /// [`scroll_to_block`](Self::scroll_to_block)). System content never
    /// forms a block of its own.
    pub(super) fn jump_turn(&mut self, step: Step) -> CmdResult {
        let Some(cur) = self.cursor() else {
            return CmdResult::NoChange;
        };
        let turns = self.content.turns.len();
        let found = match step {
            Step::Next => (cur.block + 1..turns).find(|&b| self.turn_stop(b).is_some()),
            Step::Previous => (0..cur.block).rev().find(|&b| self.turn_stop(b).is_some()),
        };
        match found {
            Some(block) => self.scroll_to_block(block),
            None => CmdResult::NoChange,
        }
    }

    /// Scrolls to the block `index` and selects its first section: the
    /// block's top border becomes the top row of the viewport, as far as the
    /// end of the transcript allows. The selected section is scrolled into
    /// view only when it would otherwise be off screen (an unfolded system
    /// section in front of the prompt can push it below the viewport). This
    /// is the one way to go to a block: `]` / `[` and the turns sidebar both
    /// end here.
    ///
    /// A block whose text renders to no rows has no section to select: the
    /// view still scrolls to it and the selection stays where it is.
    pub(super) fn scroll_to_block(&mut self, index: usize) -> CmdResult {
        if index >= self.content.turns.len() {
            return CmdResult::NoChange;
        }
        let before = self.offset;
        let stop = self.turn_stop(index);
        let selected = stop.is_some();
        if let Some(id) = stop {
            self.content.selected = Some(id);
        }
        if let Some(size) = self.viewport {
            self.offset = self.measured(|cache, m| cache.top(index, m));
            // Only a section that was just selected is revealed: without one
            // the cursor is still on the old selection, somewhere else.
            if selected && let Some(cur) = self.cursor() {
                let height = usize::from(size.height);
                let span = self.span(&cur, height);
                self.offset = reveal(self.offset, height, &span, Reveal::InPlace);
            }
        }
        self.refresh();
        self.outcome(before)
    }

    /// Goes to the block of the turn `id` (the turns sidebar selected it),
    /// as `]` and `[` do: the follow rule and the temporary unfolding of a
    /// search apply as after any key. A turn the transcript does not hold
    /// changes nothing.
    pub(super) fn go_to_turn(&mut self, id: &TurnId) -> CmdResult {
        let Some(&block) = self.content.index.get(id) else {
            return CmdResult::NoChange;
        };
        let result = self.scroll_to_block(block);
        // What `perform` does after every `Cmd`.
        self.settle_temp_unfold();
        self.update_following();
        result
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
            Amount::Half => (height / 2).max(1),
            Amount::Full => height,
            Amount::Rows(rows) => rows.max(1),
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
        let first = self.first_section_from(0);
        self.jump_to(first, 0, before)
    }

    /// Selects the last section and scrolls to the end. A selected section
    /// that fits the viewport but would be off screen (a viewport of one or
    /// two rows) is scrolled into view.
    pub(super) fn jump_to_end(&mut self) -> CmdResult {
        let before = self.offset;
        let last = self.last_section();
        let end = match self.viewport {
            Some(size) => {
                let height = usize::from(size.height);
                self.measured(|cache, m| cache.total(m))
                    .saturating_sub(height)
            }
            None => 0,
        };
        let result = self.jump_to(last, end, before);
        if let Some(size) = self.viewport
            && let Some(cur) = self.cursor()
        {
            let height = usize::from(size.height);
            let rows = self.section_rows_abs(&cur);
            let visible = rows.start >= self.offset && rows.end <= self.offset + height;
            if rows.len() <= height && !visible {
                let span = self.span(&cur, height);
                self.offset = reveal(self.offset, height, &span, Reveal::InPlace);
                self.refresh();
                return self.outcome(before);
            }
        }
        result
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
