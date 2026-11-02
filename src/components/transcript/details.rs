//! The details of the selected section (SPEC §5): the content of the details
//! overlay, made when the key is pressed.

use super::Transcript;
use crate::app::Msg;
use crate::components::details_overlay::section_details;

impl Transcript {
    /// [`Msg::OpenDetails`] with the details of the selected section, a
    /// snapshot of the model at this moment. With the `… N more lines` row
    /// selected it is the tool call above it. `None` when nothing is
    /// selected, or the selected section is gone.
    pub(super) fn details_msg(&self) -> Option<Msg> {
        let content = &self.content;
        let id = content.selected.as_ref()?;
        let turn = content.turns.get(*content.index.get(&id.turn)?)?;
        section_details(turn, id, &content.formatter).map(Msg::OpenDetails)
    }
}
