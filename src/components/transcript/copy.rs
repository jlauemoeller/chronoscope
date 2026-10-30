//! The text of the selected section for the clipboard (SPEC §6.4).

use super::Transcript;
use crate::components::turn_block::plain;

impl Transcript {
    /// The selected section as displayed: the folded summary when it is
    /// folded, everything shown when it is not, as plain text (see
    /// [`plain`]). With the `… N more lines` row selected it is the tool
    /// call above it. `None` when nothing is selected.
    pub fn displayed_text(&self) -> Option<String> {
        let content = &self.content;
        let id = content.selected.as_ref()?;
        let turn = content.turns.get(*content.index.get(&id.turn)?)?;
        plain::displayed_text(
            turn,
            id,
            &content.folds,
            &content.theme,
            content.truncate_output_lines,
            &content.open_subagent_key,
            self.viewport.map(|_| self.cache.width()),
        )
    }

    /// The selected section in full, whatever its fold state and truncation
    /// (see [`plain::full_text`]). With the `… N more lines` row selected it
    /// is the tool call above it. `None` when nothing is selected.
    pub fn full_text(&self) -> Option<String> {
        let content = &self.content;
        let id = content.selected.as_ref()?;
        let turn = content.turns.get(*content.index.get(&id.turn)?)?;
        plain::full_text(turn, id, &content.theme, &content.open_subagent_key)
    }
}
