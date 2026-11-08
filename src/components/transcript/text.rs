//! The text of the selected section for the clipboard and for files, and the
//! session as a markdown document (product §5.4).

use super::Transcript;
use crate::components::turn_block::export::{ExportSettings, export_markdown};
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

impl Transcript {
    /// The word for the selected section in the name of a saved file
    /// (`shell`, `user`, ...; see [`plain::file_kind`]). With the `… N more
    /// lines` row selected it is the tool call above it. `None` when
    /// nothing is selected.
    pub fn file_kind(&self) -> Option<&'static str> {
        let content = &self.content;
        let id = content.selected.as_ref()?;
        let turn = content.turns.get(*content.index.get(&id.turn)?)?;
        plain::file_kind(turn, id)
    }

    /// The session as markdown: as currently folded (`full` false; hidden
    /// system sections are left out) or in full. The header comes from the
    /// metadata given with [`export_meta`](Self::export_meta).
    pub fn export_document(&self, full: bool) -> String {
        let content = &self.content;
        let settings = ExportSettings {
            theme: &content.theme,
            formatter: &content.formatter,
            folds: &content.folds,
            truncate: content.truncate_output_lines,
            open_key: &content.open_subagent_key,
            show_system: content.show_system,
            block_width: self.viewport.map(|_| self.cache.width()),
        };
        export_markdown(self.export_meta.as_ref(), &content.turns, &settings, full)
    }
}
