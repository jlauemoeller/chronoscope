//! The references of a section, for the harness to read (SPEC cycle 2 §5.2),
//! and the composition of what it reads with the rows the model knows.

use super::content::{DetailsContent, DetailsGroup, DetailsRow};
use super::rows::{ENVIRONMENT, MESSAGE, PROMPT, TOOL_CALL, TURN};
use crate::components::turn_block::renderers::format_tokens;
use crate::components::turn_block::{SectionContent, SectionId, SectionKey, derive_sections};
use crate::duration::format_duration;
use crate::model::{
    SectionSource, SourceGroup, SourceGroupKind, SourceKey, SourceKind, SourceRow, SourceValue,
    Turn,
};
use crate::timestamp::TimestampFormatter;

/// What the details key asks for: the rows the model knows, and the
/// references of the section for the harness to read the rest from. The
/// harness is asked when the overlay opens (SPEC cycle 2 §5.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailsRequest {
    /// The rows the model knows.
    pub content: DetailsContent,
    /// The section's references; `None` when the turn keeps none.
    pub source: Option<SectionSource>,
}

impl From<DetailsContent> for DetailsRequest {
    /// A request for a section that has no references.
    fn from(content: DetailsContent) -> Self {
        Self {
            content,
            source: None,
        }
    }
}

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

/// The overlay's content: the rows the model knows, with the rows the harness
/// read, in the order of SPEC cycle 2 §5.3 (Turn, Message, Tool call, Prompt,
/// Environment).
///
/// Within a group: the harness's leading rows (a tool call's `start`), the
/// model's rows, the harness's other rows, then one muted `unavailable` row
/// when the harness could not read its source. No field appears twice: a row
/// of the model gives way to a row of the harness with the same label (a
/// prompt's `timestamp`). A group with no rows is left out.
pub fn compose(
    model: DetailsContent,
    read: &[SourceGroup],
    formatter: &TimestampFormatter,
) -> DetailsContent {
    let groups = [
        (TURN, None),
        (MESSAGE, Some(SourceGroupKind::Message)),
        (TOOL_CALL, Some(SourceGroupKind::ToolCall)),
        (PROMPT, Some(SourceGroupKind::Prompt)),
        (ENVIRONMENT, Some(SourceGroupKind::Environment)),
    ];
    let mut out = Vec::new();
    for (title, kind) in groups {
        let known = model.groups().iter().find(|g| g.title == title);
        let read = kind.and_then(|kind| read.iter().find(|g| g.kind == kind));
        let group = match (known, read) {
            (known, None) => known.cloned(),
            (known, Some(read)) => Some(merge(title, known, read, formatter)),
        };
        out.extend(group);
    }
    DetailsContent::new(out)
}

fn merge(
    title: &str,
    known: Option<&DetailsGroup>,
    read: &SourceGroup,
    formatter: &TimestampFormatter,
) -> DetailsGroup {
    let rows = |rows: &[SourceRow]| -> Vec<DetailsRow> {
        rows.iter()
            .map(|row| DetailsRow::new(row.label.clone(), format_value(&row.value, formatter)))
            .collect()
    };
    let read_labels = || read.lead.iter().chain(&read.rows).map(|r| r.label.as_str());
    let mut out = rows(&read.lead);
    if let Some(known) = known {
        out.extend(
            known
                .rows
                .iter()
                .filter(|row| !read_labels().any(|label| label == row.label))
                .cloned(),
        );
    }
    out.extend(rows(&read.rows));
    if read.unavailable {
        out.push(DetailsRow::unavailable());
    }
    DetailsGroup::new(title, out)
}

/// A value as the overlay shows it: timestamps in the configured format,
/// token counts the way tokens are shown everywhere, durations in the shared
/// format and plain counts as they are.
fn format_value(value: &SourceValue, formatter: &TimestampFormatter) -> String {
    match value {
        SourceValue::Text(text) => text.clone(),
        SourceValue::Time(time) => formatter.format(*time),
        SourceValue::Tokens(tokens) => format_tokens(*tokens),
        SourceValue::Count(count) => count.to_string(),
        SourceValue::Duration(duration) => format_duration(*duration),
    }
}
