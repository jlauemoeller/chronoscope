//! The mouse in the transcript (product §5.9): the wheel scrolls, a click
//! selects the section under the pointer, a click on a section's summary line
//! toggles its fold and a click on a `… N more lines` row expands the output.
//!
//! The `App` routes the event here with the position relative to the
//! transcript's area (see [`RoutedMouse`]). The row is mapped to a block and a
//! row of that block through the prepared window (the layout cache's blocks,
//! laid out for the viewport), and from there to a section or selection stop.

use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::Component;
use tuirealm::event::MouseEventKind;

use super::Transcript;
use super::folding::FoldOp;
use super::keys::{WHEEL_DOWN, WHEEL_UP};
use crate::app::{Msg, RoutedMouse};
use crate::components::turn_block::SectionId;
use crate::components::turn_block::renderers::CONTENT_COLUMN;

/// What a click landed on.
struct Hit {
    /// The section or `… N more lines` row.
    id: SectionId,
    /// Whether it is the first row of the section: its summary line.
    first_row: bool,
}

impl Transcript {
    /// A wheel step or a left press. Answers what keys answer: the new count
    /// when it changed, else [`Msg::TranscriptChanged`] if the view changed.
    pub(super) fn on_mouse(&mut self, mouse: &RoutedMouse) -> Option<Msg> {
        let new_before = self.new_sections;
        let result = match mouse.kind {
            MouseEventKind::ScrollUp => self.perform(Cmd::Custom(WHEEL_UP)),
            MouseEventKind::ScrollDown => self.perform(Cmd::Custom(WHEEL_DOWN)),
            _ if mouse.is_left_press() => self.clicked(mouse),
            _ => return None,
        };
        if self.new_sections != new_before {
            return Some(Msg::NewSections(self.new_sections));
        }
        let changed = !matches!(result, CmdResult::NoChange | CmdResult::Invalid(_));
        changed.then_some(Msg::TranscriptChanged)
    }

    /// A left press at (`column`, `row`) of the transcript's area.
    fn clicked(&mut self, mouse: &RoutedMouse) -> CmdResult {
        let result = match self.hit(mouse) {
            Some(hit) => self.click_on(hit),
            None => CmdResult::NoChange,
        };
        // As after every `Cmd`: moving the selection ends a search's
        // temporary unfolding, and the view may have left the end.
        self.settle_temp_unfold();
        self.update_following();
        result
    }

    /// The section under the pointer. `None` on the borders, the padding, the
    /// blank rows and below the last block. The padding counts for the two
    /// cells a subagent's title row draws its rule and hint into.
    fn hit(&self, mouse: &RoutedMouse) -> Option<Hit> {
        let window = self.window.as_ref().filter(|w| w.size == mouse.area)?;
        let row = window.offset + usize::from(mouse.row);
        let placed = window
            .blocks
            .iter()
            .find(|placed| (placed.top..placed.top + placed.layout.height()).contains(&row))?;
        let within = row - placed.top;
        let content = CONTENT_COLUMN..placed.layout.content_end(within);
        if !content.contains(&mouse.column) {
            return None;
        }
        placed
            .layout
            .section_rows()
            .iter()
            .find(|(_, rows)| rows.contains(&within))
            .map(|(id, rows)| Hit {
                id: id.clone(),
                first_row: within == rows.start,
            })
    }

    /// Selects the section and, as `Enter` would, toggles its fold when the
    /// summary line was hit, or expands the output when it is the truncation
    /// row. The view does not scroll: the section stays under the pointer.
    fn click_on(&mut self, hit: Hit) -> CmdResult {
        let selected_before = self.content.selected.clone();
        self.content.selected = Some(hit.id.clone());
        // The section the search unfolded refolds when the click leaves it.
        self.settle_temp_unfold();
        let toggles =
            hit.id.more_parent().is_some() || (hit.first_row && self.is_folded(&hit.id).is_some());
        if toggles {
            let result = self.fold_command(FoldOp::Toggle);
            if result != CmdResult::NoChange {
                return result;
            }
        }
        self.refresh();
        if self.content.selected == selected_before {
            CmdResult::NoChange
        } else {
            CmdResult::Visual
        }
    }
}
