//! Model updates and the selection that must survive them (SPEC §7.2).

use std::sync::Arc;

use super::Transcript;
use super::content::{Changes, Content};
use crate::components::turn_block::SectionId;
use crate::model::{ModelUpdate, Turn, TurnId};

/// The width used to find which sections exist before a viewport is known.
/// Whether a section renders to any rows does not depend on the width.
const DEFAULT_WIDTH: u16 = 80;

impl Transcript {
    /// Applies a model update: `Appended` adds turns, `Updated` replaces
    /// turns in place (their fold states and, where the section still exists,
    /// the selection stay), `Reset` clears everything. The other variants are
    /// for the App and are ignored.
    pub fn apply_update(&mut self, update: &ModelUpdate) {
        match update {
            ModelUpdate::Appended(turns) => self.change_turns(turns, |c, t| c.append(t)),
            ModelUpdate::Updated(turns) => self.change_turns(turns, |c, t| c.update(t)),
            ModelUpdate::Reset => self.reset(),
            ModelUpdate::Loaded => self.finish_loading(),
            ModelUpdate::Progress(_)
            | ModelUpdate::ParseWarnings(_)
            | ModelUpdate::LocalCommands(_)
            | ModelUpdate::Activity { .. } => {}
        }
    }

    fn reset(&mut self) {
        self.content.reset();
        self.forget_search();
        self.cache.clear();
        self.offset = 0;
        self.start_loading();
        self.refresh();
    }

    fn change_turns(
        &mut self,
        turns: &[Arc<Turn>],
        change: impl FnOnce(&mut Content, &[Arc<Turn>]) -> Changes,
    ) {
        let pinned = self.following && !self.loading;
        let counting = !self.following && !self.loading;
        // Whether the selection moves on to the new end (SPEC §7.2).
        let was_last = pinned
            && self.content.selected.is_some()
            && self.content.selected == self.last_section();
        let old_len = self.content.turns.len();
        let grown = if counting {
            self.tail_sections(turns)
        } else {
            Vec::new()
        };
        let before = self.sections_if(|id| turns.iter().any(|t| t.id() == id));
        let top_block = self.visible_blocks().start;
        let above = self.selected_above_top_row(turns, top_block);
        let changes = change(&mut self.content, turns);
        (0..changes.appended).for_each(|_| self.cache.push());
        self.invalidate_replaced(&changes.replaced, top_block);
        if let Some((id, rows)) = above {
            self.keep_screen_for(&id, rows);
        }
        // Only a batch that touched the selected turn can have removed its
        // section; an untouched selection needs no layout to check.
        if before.is_some() || self.content.selected.is_none() {
            self.restore_selection(before);
        }
        if counting {
            self.count_new(old_len, grown);
        }
        if pinned {
            if was_last {
                self.content.selected = self.last_section();
            }
            self.pin_to_bottom();
        }
        self.refresh();
        if counting {
            // A shorter turn can bring the end back into view.
            self.update_following();
        }
    }

    /// When the selected section lies entirely above the top row of the
    /// viewport, in the block that contains that row, and `turns` replace
    /// that block: the section and how many rows it takes now.
    fn selected_above_top_row(
        &mut self,
        turns: &[Arc<Turn>],
        top_block: usize,
    ) -> Option<(SectionId, usize)> {
        let selected = self.content.selected.as_ref()?;
        let at = *self.content.index.get(&selected.turn)?;
        if at != top_block || !turns.iter().any(|t| *t.id() == selected.turn) {
            return None;
        }
        let cur = self.cursor()?;
        let rows = self.section_rows_abs(&cur);
        (rows.end <= self.offset).then(|| (cur.id().clone(), rows.len()))
    }

    /// Moves the offset by how much the selected section `id`, which was
    /// `rows` rows high above the top row, grew or shrank, so that the rows
    /// on screen stay where they are.
    fn keep_screen_for(&mut self, id: &SectionId, rows: usize) {
        let Some(cur) = self.cursor().filter(|cur| cur.id() == id) else {
            return;
        };
        let now = self.section_rows_abs(&cur).len();
        self.offset = (self.offset + now).saturating_sub(rows);
    }

    /// Forgets the heights of the replaced blocks. A block above the top of
    /// the viewport that changed height moves the offset by the difference,
    /// so that what is on screen stays put.
    fn invalidate_replaced(&mut self, replaced: &[usize], top_block: usize) {
        for &index in replaced {
            let old = self.cache.known_height(index);
            self.cache.invalidate(index);
            if let (true, Some(old)) = (index < top_block, old) {
                let new = self.measured(|cache, m| cache.height(index, m));
                self.offset = (self.offset + new).saturating_sub(old);
            }
        }
    }

    /// Shows or hides system sections. Only the blocks that hold injected
    /// items change.
    pub(super) fn set_show_system(&mut self, show: bool) {
        if self.content.show_system == show {
            return;
        }
        let before = self.sections_if(|_| true);
        self.content.show_system = show;
        for block in self.content.system_blocks() {
            self.cache.invalidate(block);
        }
        self.restore_selection(before);
        self.refresh();
    }

    pub(super) fn nav_width(&self) -> u16 {
        self.viewport.map_or(DEFAULT_WIDTH, |size| size.width)
    }

    /// The selectable sections of block `index`.
    pub(super) fn sections_of(&self, index: usize) -> Vec<SectionId> {
        let turn = &self.content.turns[index];
        self.content
            .section_rows(turn, self.nav_width())
            .into_iter()
            .map(|(id, _)| id)
            .collect()
    }

    /// The sections of the selected block as they are now, when the turn of
    /// the selected section satisfies `touched`: the state to restore the
    /// selection from after a change.
    pub(super) fn sections_if(&self, touched: impl Fn(&TurnId) -> bool) -> Option<Vec<SectionId>> {
        let selected = self.content.selected.as_ref()?;
        let &at = self.content.index.get(&selected.turn)?;
        touched(&selected.turn).then(|| self.sections_of(at))
    }

    /// The first selectable section of the blocks from `start` on.
    pub(super) fn first_section_from(&self, start: usize) -> Option<SectionId> {
        (start..self.content.turns.len()).find_map(|i| self.sections_of(i).into_iter().next())
    }

    /// The last selectable section of the transcript.
    pub(super) fn last_section(&self) -> Option<SectionId> {
        (0..self.content.turns.len())
            .rev()
            .find_map(|i| self.sections_of(i).pop())
    }

    /// Selects the first section if nothing is selected.
    pub(super) fn ensure_selection(&mut self) {
        if self.content.selected.is_none() {
            self.content.selected = self.first_section_from(0);
        }
    }

    /// Keeps the selection after a change. When the selected section is gone,
    /// the nearest section that followed it (in `before`, the sections of its
    /// block before the change) and still exists is selected; else the first
    /// section of a later block; else the last section of all.
    pub(super) fn restore_selection(&mut self, before: Option<Vec<SectionId>>) {
        let Some(selected) = self.content.selected.clone() else {
            self.ensure_selection();
            return;
        };
        let Some(&at) = self.content.index.get(&selected.turn) else {
            self.content.selected = self.first_section_from(0);
            return;
        };
        let now = self.sections_of(at);
        if now.contains(&selected) {
            return;
        }
        // A truncation row that is gone (its output fits now, or the call is
        // folded) leaves the selection on its tool call.
        if let Some(parent) = selected.more_parent()
            && now.contains(&parent)
        {
            self.content.selected = Some(parent);
            self.reveal_selection();
            return;
        }
        let following = before.unwrap_or_default();
        let next = following
            .iter()
            .skip_while(|id| **id != selected)
            .skip(1)
            .find(|id| now.contains(id))
            .cloned();
        self.content.selected = next
            .or_else(|| self.first_section_from(at + 1))
            .or_else(|| self.last_section());
        // The selection moved by itself: show it, as any other selection.
        self.reveal_selection();
    }
}
