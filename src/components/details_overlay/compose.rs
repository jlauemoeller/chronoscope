//! What the details key asks the overlay to show ([`DetailsRequest`]), and
//! the composition of what the harness reads with the rows the model knows.
//! Both are made from the model in [`crate::section_details`].

use super::content::{DetailsContent, DetailsGroup, DetailsRow};
use crate::model::{SectionSource, SourceGroup, SourceGroupKind, SourceRow, SourceValue};
use crate::section_details::{ENVIRONMENT, MESSAGE, PROMPT, TOOL_CALL, TURN};
use crate::timestamp::TimestampFormatter;
use crate::tokens::format_tokens;

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

/// The overlay's content: the rows the model knows, with the rows the harness
/// read, in the order of SPEC cycle 2 §5.3 (Turn, Message, Tool call, Prompt,
/// Environment).
///
/// Within a group: the harness's leading rows (a tool call's `start`), the
/// model's rows, the harness's other rows (the Turn group's `provider` and
/// `agent` follow `cost`), then one muted `unavailable` row when the harness
/// could not read its source. No field appears twice: a row of the model
/// gives way to a row of the harness with the same label, or that stands in
/// for it (a prompt's `timestamp`). A group with no rows is left out.
pub fn compose(
    model: DetailsContent,
    read: &[SourceGroup],
    formatter: &TimestampFormatter,
) -> DetailsContent {
    let groups = [
        (TURN, SourceGroupKind::Turn),
        (MESSAGE, SourceGroupKind::Message),
        (TOOL_CALL, SourceGroupKind::ToolCall),
        (PROMPT, SourceGroupKind::Prompt),
        (ENVIRONMENT, SourceGroupKind::Environment),
    ];
    let mut out = Vec::new();
    for (title, kind) in groups {
        let known = model.groups().iter().find(|g| g.title == title);
        let read = read.iter().find(|g| g.kind == kind);
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
    let read_rows = || read.lead.iter().chain(&read.rows);
    let mut out = rows(&read.lead);
    if let Some(known) = known {
        out.extend(
            known
                .rows
                .iter()
                .filter(|row| {
                    !read_rows().any(|r| r.label == row.label || r.replaces == Some(&row.label))
                })
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
/// token counts the way tokens are shown everywhere and plain counts as they
/// are.
fn format_value(value: &SourceValue, formatter: &TimestampFormatter) -> String {
    match value {
        SourceValue::Text(text) => text.clone(),
        SourceValue::Time(time) => formatter.format(*time),
        SourceValue::Tokens(tokens) => format_tokens(*tokens),
        SourceValue::Count(count) => count.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::components::turn_block::fixtures::{
        assistant, compaction, formatter, item_id, key_id, text, tool, user, with_duration,
    };
    use crate::components::turn_block::{SectionId, SectionKey};
    use crate::model::{ToolStatus, Turn, TurnStats};
    use crate::section_details::section_details;

    fn row(label: &str, value: SourceValue) -> SourceRow {
        SourceRow::new(label, value)
    }

    fn text_row(label: &str, value: &str) -> SourceRow {
        row(label, SourceValue::Text(value.to_owned()))
    }

    fn group(kind: SourceGroupKind, lead: Vec<SourceRow>, rows: Vec<SourceRow>) -> SourceGroup {
        SourceGroup {
            kind,
            lead,
            rows,
            unavailable: false,
        }
    }

    /// `(label, value)` of the rows of group `title`.
    fn rows<'a>(content: &'a DetailsContent, title: &str) -> Vec<(&'a str, &'a str)> {
        content
            .groups()
            .iter()
            .find(|g| g.title == title)
            .unwrap_or_else(|| panic!("no {title} group in {content:?}"))
            .rows
            .iter()
            .map(|r| (r.label.as_str(), r.value.as_str()))
            .collect()
    }

    fn titles(content: &DetailsContent) -> Vec<&str> {
        content.groups().iter().map(|g| g.title.as_str()).collect()
    }

    fn call_turn() -> Turn {
        assistant(
            Some("high"),
            vec![
                text("t1", "Hello"),
                with_duration(
                    tool(
                        "c1",
                        "Run ls",
                        json!({}),
                        Some(("ok", false)),
                        ToolStatus::Ok,
                    ),
                    62_000,
                ),
            ],
            Some(TurnStats {
                duration: std::time::Duration::from_secs(5),
                message_count: 1,
                tokens: None,
                cost: None,
            }),
        )
    }

    fn compose_for(turn: &Turn, id: &SectionId, read: &[SourceGroup]) -> DetailsContent {
        let model = section_details(turn, id, &formatter()).unwrap();
        compose(model, read, &formatter())
    }

    #[test]
    fn nothing_read_leaves_the_models_rows_alone_and_adds_no_unavailable_row() {
        let turn = call_turn();
        let id = item_id(&turn, "c1");
        let model = section_details(&turn, &id, &formatter()).unwrap();
        assert_eq!(compose(model.clone(), &[], &formatter()), model);
        assert!(
            !format!("{model:?}").contains("unavailable"),
            "no source, no unavailable row"
        );
    }

    #[test]
    fn a_tool_call_is_start_then_the_models_duration_and_status_then_the_rest() {
        let turn = call_turn();
        let id = item_id(&turn, "c1");
        let read = [
            group(
                SourceGroupKind::ToolCall,
                vec![row(
                    "start",
                    SourceValue::Time(crate::components::turn_block::fixtures::ts(
                        "2026-10-21T08:35:20Z",
                    )),
                )],
                vec![text_row("sandboxed", "false"), text_row("isAsync", "true")],
            ),
            group(
                SourceGroupKind::Environment,
                vec![],
                vec![text_row("cwd", "/home/user/project")],
            ),
        ];
        let content = compose_for(&turn, &id, &read);
        assert_eq!(titles(&content), ["Turn", "Tool call", "Environment"]);
        assert_eq!(
            rows(&content, "Tool call"),
            [
                ("start", "2026-10-21 08:35:20Z"),
                ("duration", "1m 2s"),
                ("status", "ok"),
                ("sandboxed", "false"),
                ("isAsync", "true"),
            ]
        );
        assert_eq!(
            rows(&content, "Environment"),
            [("cwd", "/home/user/project")]
        );
    }

    #[test]
    fn the_groups_come_in_the_order_of_the_table() {
        let turn = call_turn();
        let id = item_id(&turn, "t1");
        // The harness lists them in another order.
        let read = [
            group(
                SourceGroupKind::Environment,
                vec![],
                vec![text_row("cwd", "/p")],
            ),
            group(
                SourceGroupKind::Message,
                vec![],
                vec![text_row("message.id", "m1")],
            ),
        ];
        let content = compose_for(&turn, &id, &read);
        assert_eq!(titles(&content), ["Turn", "Message", "Environment"]);
    }

    #[test]
    fn values_are_formatted_by_their_type() {
        let turn = call_turn();
        let id = item_id(&turn, "t1");
        let read = [group(
            SourceGroupKind::Message,
            vec![],
            vec![
                row(
                    "timestamp",
                    SourceValue::Time(crate::components::turn_block::fixtures::ts(
                        "2026-10-21T08:35:20Z",
                    )),
                ),
                row("1h cache writes", SourceValue::Tokens(13_212)),
                row("web searches", SourceValue::Count(13_212)),
                row("speed", SourceValue::Text("13212".into())),
            ],
        )];
        let content = compose_for(&turn, &id, &read);
        assert_eq!(
            rows(&content, "Message"),
            [
                ("timestamp", "2026-10-21 08:35:20Z"),
                ("1h cache writes", "13.2k"),
                ("web searches", "13212"),
                ("speed", "13212"),
            ]
        );
        // The configured format, not a fixed one.
        let model = section_details(&turn, &id, &formatter()).unwrap();
        let short = crate::timestamp::TimestampFormatter::new(
            "%H:%M",
            true,
            crate::timestamp::DisplayTz::Local,
        );
        let content = compose(model, &read, &short);
        assert_eq!(rows(&content, "Message")[0], ("timestamp", "08:35Z"));
    }

    #[test]
    fn the_prompts_timestamp_is_the_harness_one_and_not_shown_twice() {
        let turn = user("Hello", Vec::new());
        let id = key_id(&turn, SectionKey::Prompt);
        let read = [group(
            SourceGroupKind::Prompt,
            vec![],
            vec![
                text_row("timestamp", "from the line"),
                text_row("promptSource", "typed"),
            ],
        )];
        let content = compose_for(&turn, &id, &read);
        assert_eq!(
            rows(&content, "Prompt"),
            [("timestamp", "from the line"), ("promptSource", "typed")]
        );
        // A line without a timestamp leaves the model's.
        let read = [group(
            SourceGroupKind::Prompt,
            vec![],
            vec![text_row("promptSource", "typed")],
        )];
        let content = compose_for(&turn, &id, &read);
        assert_eq!(
            rows(&content, "Prompt"),
            [
                ("timestamp", "2026-10-21 08:35:15Z"),
                ("promptSource", "typed")
            ]
        );
    }

    #[test]
    fn a_row_that_stands_in_for_the_models_replaces_it_whatever_its_label() {
        let turn = user("Hello", Vec::new());
        let id = key_id(&turn, SectionKey::Prompt);
        let time = SourceValue::Time(crate::components::turn_block::fixtures::ts(
            "2026-10-21T08:35:20Z",
        ));
        let read = [group(
            SourceGroupKind::Prompt,
            vec![],
            vec![SourceRow::replacing("time.created", "timestamp", time)],
        )];
        let content = compose_for(&turn, &id, &read);
        assert_eq!(
            rows(&content, "Prompt"),
            [("time.created", "2026-10-21 08:35:20Z")]
        );
    }

    #[test]
    fn the_turn_groups_read_rows_follow_the_models() {
        let turn = call_turn();
        let id = item_id(&turn, "t1");
        let read = [group(
            SourceGroupKind::Turn,
            vec![],
            vec![text_row("provider", "acme"), text_row("agent", "build")],
        )];
        let content = compose_for(&turn, &id, &read);
        let turn_rows = rows(&content, "Turn");
        let last_two = &turn_rows[turn_rows.len() - 2..];
        assert_eq!(last_two, [("provider", "acme"), ("agent", "build")]);
        assert_eq!(turn_rows[0].0, "model");
        // Unreadable: the model's rows, then the muted row.
        let read = [SourceGroup::unavailable(SourceGroupKind::Turn)];
        let content = compose_for(&turn, &id, &read);
        let turn_rows = rows(&content, "Turn");
        assert_eq!(turn_rows.last(), Some(&("unavailable", "")));
        assert_eq!(turn_rows[0].0, "model");
    }

    #[test]
    fn a_compaction_shows_its_timestamp_then_what_its_source_provides() {
        let turn = compaction("Summary");
        let id = key_id(&turn, SectionKey::Summary);
        let read = [group(
            SourceGroupKind::Environment,
            vec![],
            vec![text_row("gitBranch", "main")],
        )];
        let content = compose_for(&turn, &id, &read);
        assert_eq!(titles(&content), ["Environment"]);
        assert_eq!(
            rows(&content, "Environment"),
            [("timestamp", "2026-10-21 09:00:00Z"), ("gitBranch", "main")]
        );
    }

    #[test]
    fn an_unavailable_group_ends_with_one_muted_row_after_what_the_model_knows() {
        let turn = call_turn();
        let id = item_id(&turn, "c1");
        let read = [
            SourceGroup {
                unavailable: true,
                ..group(
                    SourceGroupKind::ToolCall,
                    vec![text_row("start", "10:00")],
                    vec![],
                )
            },
            SourceGroup::unavailable(SourceGroupKind::Environment),
        ];
        let content = compose_for(&turn, &id, &read);
        assert_eq!(
            rows(&content, "Tool call"),
            [
                ("start", "10:00"),
                ("duration", "1m 2s"),
                ("status", "ok"),
                ("unavailable", ""),
            ]
        );
        let last = content.groups()[2].rows.last().unwrap();
        assert_eq!(last, &DetailsRow::unavailable());
        assert!(last.muted);
        // A group the model knows nothing of is just the one row.
        assert_eq!(rows(&content, "Environment"), [("unavailable", "")]);
    }

    #[test]
    fn a_group_the_harness_read_nothing_of_is_left_out() {
        let turn = call_turn();
        let id = item_id(&turn, "t1");
        let read = [group(SourceGroupKind::Environment, vec![], vec![])];
        let content = compose_for(&turn, &id, &read);
        assert_eq!(titles(&content), ["Turn"]);
    }
}
