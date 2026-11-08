//! Opening a subagent's transcript from its section (product §4.4).

use super::Transcript;
use crate::app::Msg;
use crate::components::turn_block::SectionKey;
use crate::model::{Item, ToolCall, ToolKind, Turn};

/// The label of a subagent call in the breadcrumb: the agent type of its
/// input (`subagent_type` for Claude, `agent` for OpenCode), else its
/// description, else `subagent`.
fn label_of(call: &ToolCall) -> String {
    let field = |keys: &[&str]| {
        keys.iter()
            .find_map(|key| call.input.get(*key)?.as_str())
            .map(str::trim)
            .filter(|text| !text.is_empty())
    };
    field(&["subagent_type", "agent"])
        .or_else(|| Some(call.description.trim()).filter(|text| !text.is_empty()))
        .unwrap_or("subagent")
        .to_owned()
}

impl Transcript {
    /// [`Msg::OpenSubagent`] for the selected section, when it is a subagent
    /// tool call (or the `… N more lines` row below one). `None` for any
    /// other section, and with nothing selected.
    pub(super) fn open_subagent_msg(&self) -> Option<Msg> {
        let id = self.content.selected.as_ref()?;
        let (SectionKey::Item(item) | SectionKey::More(item)) = &id.key else {
            return None;
        };
        let turn = self.content.turns.get(*self.content.index.get(&id.turn)?)?;
        let items = match turn.as_ref() {
            Turn::User(turn) => &turn.items,
            Turn::Assistant(turn) => &turn.items,
            Turn::Compaction(_) => return None,
        };
        let call = items.iter().find_map(|candidate| match candidate {
            Item::ToolCall(call) if call.id == *item && call.tool == ToolKind::Subagent => {
                Some(call)
            }
            _ => None,
        })?;
        Some(Msg::OpenSubagent {
            call_id: call.id.clone(),
            label: label_of(call),
        })
    }
}
