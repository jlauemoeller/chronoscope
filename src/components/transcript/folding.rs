//! Folding keys, and the keeping of the selected section's screen row when a
//! change moves the content around it.

use tuirealm::command::CmdResult;

use super::Transcript;
use super::nav::{Reveal, reveal};
use crate::components::turn_block::{SectionId, derive_sections};

/// What a fold command does to the selected section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FoldOp {
    Toggle,
    Fold,
    Unfold,
}

/// Where the selected section started on screen before a change.
pub(super) struct Anchor {
    id: SectionId,
    /// The screen row of the section's first row; negative above the
    /// viewport.
    screen_row: isize,
    visible: bool,
}

impl Transcript {
    /// Remembers where the selected section's first row is on screen.
    pub(super) fn anchor_selected(&mut self) -> Option<Anchor> {
        let height = usize::from(self.viewport?.height);
        let cur = self.cursor()?;
        let start = self.section_rows_abs(&cur).start;
        let screen_row = isize::try_from(start).ok()? - isize::try_from(self.offset).ok()?;
        Some(Anchor {
            id: cur.id().clone(),
            screen_row,
            visible: usize::try_from(screen_row).is_ok_and(|row| row < height),
        })
    }

    /// Scrolls so that the selected section's first row is at the screen row
    /// it had in `anchor`, when it is still the selected section and that
    /// row was on screen; otherwise only makes sure it is in view. Clamped,
    /// so the row is kept "when possible". The window is not rebuilt.
    pub(super) fn restore_anchor(&mut self, anchor: Option<Anchor>) {
        let Some(anchor) = anchor else {
            return;
        };
        if self.content.selected.as_ref() != Some(&anchor.id) {
            return;
        }
        let (Some(size), Some(cur)) = (self.viewport, self.cursor()) else {
            return;
        };
        if anchor.visible {
            let start = self.section_rows_abs(&cur).start;
            let start = isize::try_from(start).unwrap_or(isize::MAX);
            self.offset = usize::try_from(start - anchor.screen_row).unwrap_or(0);
        } else {
            let height = usize::from(size.height);
            let span = self.span(&cur, height);
            self.offset = reveal(self.offset, height, &span, Reveal::InPlace);
        }
    }

    /// Folds, unfolds or toggles the selected section, or, with `Toggle` on
    /// the `… N more lines` row, expands the output it stands for. Sections
    /// that cannot fold, and ones already in the wanted state, ignore it.
    pub(super) fn fold_command(&mut self, op: FoldOp) -> CmdResult {
        let before = self.offset;
        let Some(cur) = self.cursor() else {
            return CmdResult::NoChange;
        };
        let id = cur.id().clone();
        if let Some(parent) = id.more_parent() {
            return if op == FoldOp::Toggle {
                self.output_expanded_by_user(&parent);
                self.expand_output(parent, cur.block)
            } else {
                CmdResult::NoChange
            };
        }
        let Some(folded) = self.is_folded(&id) else {
            return CmdResult::NoChange;
        };
        // A section the search unfolded: the user's command makes it theirs.
        if self.fold_command_on_temp(&id, op) {
            return CmdResult::Visual;
        }
        let want = match op {
            FoldOp::Toggle => !folded,
            FoldOp::Fold => true,
            FoldOp::Unfold => false,
        };
        if want == folded {
            return CmdResult::NoChange;
        }
        let anchor = self.anchor_selected();
        self.content.folds.set_folded(id, want);
        self.cache.invalidate(cur.block);
        self.restore_anchor(anchor);
        self.refresh();
        self.outcome(before)
    }

    /// Folds or unfolds every foldable section of the session, system
    /// sections included even while hidden.
    pub(super) fn set_all_folded(&mut self, folded: bool) -> CmdResult {
        self.forget_temp_unfold(folded);
        let before = self.offset;
        let mut changed = Vec::new();
        let mut blocks = Vec::new();
        for (block, turn) in self.content.turns.iter().enumerate() {
            let before = changed.len();
            for section in derive_sections(turn, true) {
                if section.kind().foldable() && self.content.folds.is_folded(&section) != folded {
                    changed.push(section.id);
                }
            }
            if changed.len() > before {
                blocks.push(block);
            }
        }
        if changed.is_empty() {
            return CmdResult::NoChange;
        }
        // Measure with the old folds before they change.
        let anchor = self.anchor_selected();
        let sections = self.sections_if(|_| true);
        for id in changed {
            self.content.folds.set_folded(id, folded);
        }
        for block in blocks {
            self.cache.invalidate(block);
        }
        self.restore_selection(sections);
        self.restore_anchor(anchor);
        self.refresh();
        self.outcome(before)
    }

    /// Shows or hides system sections. A selected system section that
    /// disappears moves the selection to the nearest following visible
    /// section, or else the previous one. When hiding, the selected section
    /// keeps its screen row. When showing, the view stays where it is (so the
    /// top of the transcript stays on screen) unless the selected section
    /// would be pushed off screen.
    pub(super) fn toggle_system(&mut self) -> CmdResult {
        let before = self.offset;
        let hiding = self.content.show_system;
        let anchor = hiding.then(|| self.anchor_selected()).flatten();
        self.set_show_system(!hiding);
        if hiding {
            self.restore_anchor(anchor);
        } else if let (Some(size), Some(cur)) = (self.viewport, self.cursor()) {
            let height = usize::from(size.height);
            let span = self.span(&cur, height);
            self.offset = reveal(self.offset, height, &span, Reveal::InPlace);
        }
        self.refresh();
        self.outcome(before)
    }
}
