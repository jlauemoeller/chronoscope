//! The references of a section, for the harness to read (SPEC cycle 2 §5.2).

use crate::components::turn_block::{SectionContent, SectionId, SectionKey, derive_sections};
use crate::model::{SectionSource, SourceKey, SourceKind, Turn};

/// The references of the section `id` of `turn`, and what the section is, for
/// [`Harness::section_details`](crate::harness::Harness::section_details).
/// `None` when the turn has no section like that or keeps no reference for
/// it (a harness that gives no source details, a footer without a
/// `turn_duration` line).
pub fn section_source(turn: &Turn, id: &SectionId) -> Option<SectionSource> {
    let id = id.more_parent().unwrap_or_else(|| id.clone());
    let sections = derive_sections(turn, true);
    let section = sections.iter().find(|s| s.id == id)?;
    let (key, kind) = match (&id.key, &section.content) {
        (SectionKey::Prompt, _) => (SourceKey::Own, SourceKind::Prompt),
        (SectionKey::Summary, _) => (SourceKey::Own, SourceKind::Other),
        (SectionKey::Footer, _) => (SourceKey::Footer, SourceKind::Other),
        (SectionKey::Item(item) | SectionKey::More(item), content) => {
            let kind = match content {
                SectionContent::Tool(_) => SourceKind::ToolCall,
                SectionContent::Text(_)
                | SectionContent::Thinking(_)
                | SectionContent::Notice(..) => SourceKind::Message,
                _ => SourceKind::Other,
            };
            (SourceKey::Item(item.clone()), kind)
        }
    };
    let refs = turn.sources().get(&key)?;
    Some(SectionSource { kind, refs })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::components::turn_block::fixtures::{item_id, key_id, text_and_call, user};
    use crate::harness::claude::Assembler;

    // Turns the Claude assembler made, which keep their references.
    const LINES: [&str; 4] = [
        r#"{"type":"user","uuid":"u1","origin":{"kind":"human"},"timestamp":"2026-01-01T00:00:00Z","message":{"role":"user","content":"go"}}"#,
        r#"{"type":"assistant","uuid":"a1","timestamp":"2026-01-01T00:00:01Z","message":{"id":"m1","model":"x","content":[{"type":"text","text":"Hi"},{"type":"thinking","thinking":"hm","signature":"s"},{"type":"tool_use","id":"c1","name":"Bash","input":{"command":"ls"}}]}}"#,
        r#"{"type":"user","uuid":"r1","timestamp":"2026-01-01T00:00:02Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"c1","content":"ok"}]}}"#,
        r#"{"type":"system","subtype":"turn_duration","uuid":"d1","timestamp":"2026-01-01T00:00:03Z","durationMs":3000}"#,
    ];

    fn assembled() -> Vec<Arc<Turn>> {
        let text = LINES.join("\n");
        Assembler::from_memory(Arc::from(text.as_bytes())).into_turns()
    }

    #[test]
    fn a_sections_kind_is_what_the_harness_is_asked_for() {
        let turns = assembled();
        let kind = |turn: usize, key: SectionKey| {
            let id = SectionId {
                turn: turns[turn].id().clone(),
                key,
            };
            section_source(&turns[turn], &id).map(|s| s.kind)
        };
        let item = |id: &str| SectionKey::Item(crate::model::ItemId::from(id));
        assert_eq!(kind(0, SectionKey::Prompt), Some(SourceKind::Prompt));
        assert_eq!(kind(1, item("m1:0")), Some(SourceKind::Message));
        assert_eq!(kind(1, item("m1:1")), Some(SourceKind::Message));
        assert_eq!(kind(1, item("c1")), Some(SourceKind::ToolCall));
        assert_eq!(kind(1, SectionKey::Footer), Some(SourceKind::Other));
        // The `… N more lines` row is its tool call.
        let more = SectionKey::More(crate::model::ItemId::from("c1"));
        assert_eq!(kind(1, more), Some(SourceKind::ToolCall));
        // A section that is not there has no source.
        assert_eq!(kind(1, item("nope")), None);
        assert_eq!(kind(1, SectionKey::Prompt), None);
    }

    #[test]
    fn a_tool_call_has_both_lines_and_a_message_one() {
        let turns = assembled();
        let id = |item: &str| SectionId {
            turn: turns[1].id().clone(),
            key: SectionKey::Item(crate::model::ItemId::from(item)),
        };
        let call = section_source(&turns[1], &id("c1")).unwrap();
        assert!(call.refs.lines().unwrap().result.is_some());
        let text = section_source(&turns[1], &id("m1:0")).unwrap();
        assert!(text.refs.lines().unwrap().result.is_none());
    }

    #[test]
    fn a_turn_that_keeps_no_references_has_no_source() {
        let turn = text_and_call();
        assert_eq!(section_source(&turn, &item_id(&turn, "c1")), None);
        assert_eq!(section_source(&turn, &item_id(&turn, "t1")), None);
        let turn = user("Hello", Vec::new());
        assert_eq!(
            section_source(&turn, &key_id(&turn, SectionKey::Prompt)),
            None
        );
    }
}
